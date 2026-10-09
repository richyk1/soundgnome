<script module lang="ts">
  export function formatTime(secs: number | null | undefined): string {
    if (secs == null || !Number.isFinite(secs) || secs <= 0) return '0:00';
    const total = Math.floor(secs);
    return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
  }

  // Spotify exposes album art without auth via oEmbed. Resolved + cached per url.
  const spotifyArtCache = new Map<string, string | null>();
  async function resolveSpotifyArt(url: string): Promise<string | null> {
    if (spotifyArtCache.has(url)) return spotifyArtCache.get(url) ?? null;
    try {
      const res = await fetch(`https://open.spotify.com/oembed?url=${encodeURIComponent(url)}`);
      const art = res.ok
        ? (((await res.json()) as { thumbnail_url?: string }).thumbnail_url ?? null)
        : null;
      spotifyArtCache.set(url, art);
      return art;
    } catch {
      spotifyArtCache.set(url, null);
      return null;
    }
  }
</script>

<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from 'svelte';
  import type { PlayerTrack, TrackSource } from './player';
  import { usesNativeAudio } from './player';
  import Waveform from './Waveform.svelte';
  import EqPanel from './EqPanel.svelte';
  import PixelCover from './PixelCover.svelte';
  import ArtGlow from './ArtGlow.svelte';
  import { pop } from './motion';
  import { haptic } from './haptics';
  import { Equalizer, loadEqState, saveEqState, type EqState } from './equalizer';
  import * as scrobbler from './scrobbler';
  import { lib } from './library/store.svelte';

  let {
    resolveSrc,
    onEnded,
    onError,
    active = $bindable(false),
    upNext = $bindable([]),
  }: {
    /** Returns a playable URL for a track. May be async when the URL has to be resolved. */
    resolveSrc: (track: PlayerTrack) => string | Promise<string>;
    onEnded?: () => void;
    onError?: (track: PlayerTrack, message: string) => void;
    /** True while a track is loaded, so the shell can reserve space for the bar. */
    active?: boolean;
    /** Upcoming tracks (queue after the current one), for the sidebar queue. */
    upNext?: PlayerTrack[];
  } = $props();

  // -- Equalizer (opt-in Web Audio graph on the shared <audio> element) --------
  const eq = new Equalizer();
  // iPhone and iPad play through native audio (background playback), which
  // cannot route through the EQ or normalization graph, so those controls are
  // not offered there; saved settings stay intact for other devices.
  const nativeAudio = usesNativeAudio();
  let eqState = $state<EqState>(loadEqState());
  let eqOpen = $state(false);
  let eqBtnEl: HTMLButtonElement | undefined = $state();
  let eqStyle = $state('');

  // -- Playback normalization (ReplayGain-style, non-destructive) --------------
  const NORMALIZE_KEY = 'soundgnome:normalize:v1';
  function loadNormalize(): boolean {
    try {
      const v = localStorage.getItem(NORMALIZE_KEY);
      return v === null ? true : v === '1';
    } catch {
      return true;
    }
  }
  function saveNormalize(on: boolean): void {
    try {
      localStorage.setItem(NORMALIZE_KEY, on ? '1' : '0');
    } catch {
      /* storage unavailable */
    }
  }
  let normalizeEnabled = $state(loadNormalize());
  // Gain (dB) for the current track, from its measured loudness. 0 until fetched.
  let currentGainDb = $state(0);

  /** Move a node to <body> so it escapes the player bar's clipping/stacking. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  /** Anchor the popover just above the EQ button, right-aligned, viewport-fixed. */
  function positionEq() {
    if (!eqBtnEl) return;
    const r = eqBtnEl.getBoundingClientRect();
    const viewport = window.visualViewport;
    const right = Math.max(8, window.innerWidth - r.right);
    const bottom = window.innerHeight - Math.min(r.top, (viewport?.offsetTop ?? 0) + (viewport?.height ?? window.innerHeight)) + 12;
    eqStyle = `right:${right}px; bottom:${bottom}px;`;
  }

  $effect(() => {
    if (!eqOpen) return;
    positionEq();
    const onResize = () => positionEq();
    window.addEventListener('resize', onResize);
    window.visualViewport?.addEventListener('resize', onResize);
    window.visualViewport?.addEventListener('scroll', onResize);
    return () => {
      window.removeEventListener('resize', onResize);
      window.visualViewport?.removeEventListener('resize', onResize);
      window.visualViewport?.removeEventListener('scroll', onResize);
    };
  });

  /** (Re)build the Web Audio graph on the shared element, keeping position and
   *  play state. attach() creates the MediaElementSource on an element already
   *  playing its current resource; Chrome leaves that resource on the old direct
   *  output so it goes silent once routed through the new graph, so reload the
   *  current resource to flow it through. Cross-origin streams can't be routed. */
  function buildGraphNow() {
    if (!audio || nativeAudio || eq.isBuilt) return;
    const el = audio;
    const at = el.currentTime;
    const wasPlaying = !el.paused;
    eq.attach(el, eqState);
    void eq.resume().catch(reportProcessingError);
    eq.setNormalization(normalizeEnabled ? currentGainDb : 0);
    const abs = el.currentSrc || el.src;
    if (abs && new URL(abs, location.href).origin === location.origin) {
      pendingSeek = at > 0 ? at : null;
      resumeOnLoad = wasPlaying;
      el.load();
    }
  }

  /** Push EQ changes onto the graph (building it on first enable) and persist. */
  function handleEqUpdate(s: EqState) {
    if (nativeAudio) return;
    if (audio) {
      if (s.enabled && !eq.isBuilt) {
        buildGraphNow();
      } else {
        eq.apply(s);
        if (s.enabled) void eq.resume().catch(reportProcessingError);
      }
    }
    saveEqState(s);
  }

  /** Turn playback normalization on/off; persist and (build then) apply. */
  function setNormalize(on: boolean) {
    if (nativeAudio) return;
    normalizeEnabled = on;
    saveNormalize(on);
    if (on && audio && !eq.isBuilt) buildGraphNow();
    if (eq.isBuilt) {
      void eq.resume().catch(reportProcessingError);
      eq.setNormalization(on ? currentGainDb : 0);
    }
  }

  /** Never connect iOS's media element to Web Audio, even for restored settings. */
  function ensureEq() {
    if (!audio || nativeAudio) return;
    if ((eqState.enabled || normalizeEnabled) && !eq.isBuilt) eq.attach(audio, eqState);
    if (eq.isBuilt) {
      eq.setNormalization(normalizeEnabled ? currentGainDb : 0);
    }
  }

  // Fetch the current track's loudness and keep the normalization gain in sync.
  $effect(() => {
    const t = current;
    currentGainDb = 0;
    if (!t || t.source !== 'library' || nativeAudio) return;
    let cancelled = false;
    fetch(`/api/tracks/${t.id}/loudness`)
      .then((r) => (r.ok ? r.json() : null))
      .then((d) => {
        if (!cancelled && current?.id === t.id && current.source === t.source && d) currentGainDb = d.gain_db ?? 0;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });
  // Push the gain onto the graph whenever it or the toggle changes.
  $effect(() => {
    const gain = normalizeEnabled ? currentGainDb : 0;
    if (eq.isBuilt) eq.setNormalization(gain);
  });


  // -- Persistence: keep the queue + current track across page reloads --------
  const STORAGE_KEY = 'soundgnome:player:v1';
  interface PersistedPlayer {
    current: PlayerTrack | null;
    queue: PlayerTrack[];
    qIndex: number;
    currentTime: number;
    paused: boolean;
    shuffle: boolean;
    repeat: 'off' | 'all' | 'one';
    volume: number;
    muted: boolean;
    order: number[];
    orderPos: number;
  }
  function readPersisted(): PersistedPlayer | null {
    if (typeof localStorage === 'undefined') return null;
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return null;
      const s = JSON.parse(raw) as PersistedPlayer;
      if (!s || !Array.isArray(s.queue)) return null;
      return s;
    } catch {
      return null;
    }
  }
  const persisted = readPersisted();

  let audio: HTMLAudioElement | null = $state(null);
  let current: PlayerTrack | null = $state(persisted?.current ?? null);
  let resolvingId: number | null = $state(null);
  let resolvingSource: TrackSource | undefined = $state(undefined);
  let paused = $state(true);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(persisted?.volume ?? 1);
  // No mute control on native audio (the device's buttons do it), so never restore a muted state there.
  let muted = $state(!nativeAudio && (persisted?.muted ?? false));
  // Whether the current track's waveform loaded; drives the fall back to a plain range.
  let waveReady = $state(false);
  // Some sources hand out signed URLs that expire: allow exactly one silent re-resolve per track.
  let retriedCurrent = false;
  let playbackError = $state('');
  let requestId = 0;
  let restoringId: number | null = null;
  let wantsPlayback = false;
  let destroyed = false;

  // Queue + transport — feature parity with offtop (shuffle, prev/next, repeat).
  let queue: PlayerTrack[] = $state(persisted?.queue ?? []);
  let qIndex = $state(persisted?.qIndex ?? 0);
  let shuffle = $state(persisted?.shuffle ?? false);
  let repeat: 'off' | 'all' | 'one' = $state(persisted?.repeat ?? 'off');
  // `order` is the play order: a permutation of queue indices. In shuffle it is a
  // shuffled permutation (current track pinned first); otherwise the identity.
  // Both the transport (next/prev) and the shown queue read from it, so they agree.
  let order: number[] = $state(persisted?.order ?? []);
  let orderPos = $state(persisted?.orderPos ?? 0);
  // Init-only: rebuild unless the restored order still matches the restored queue.
  // Read from the plain snapshot (not the reactive state) to avoid a spurious
  // "captures initial value" warning.
  const restoredOrderValid =
    persisted != null &&
    Array.isArray(persisted.order) &&
    persisted.order.length === (persisted.queue?.length ?? 0) &&
    persisted.order[persisted.orderPos ?? 0] === (persisted.qIndex ?? 0);
  if (!restoredOrderValid) rebuildOrder();
  let canStep = $derived(queue.length > 1);
  // Keep `active` in sync with whether a track is loaded.
  $effect(() => {
    active = current != null;
    let upcoming = order.slice(orderPos + 1);
    if (repeat === 'all') upcoming = upcoming.concat(order.slice(0, orderPos));
    upNext = upcoming.map((i) => queue[i]).filter((t): t is PlayerTrack => t != null);
  });

  // Persist the queue + current track (with position) so a reload restores them.
  // `currentTime` is written but not a dependency (untrack), so per-tick playback
  // updates never thrash localStorage; pagehide/visibilitychange capture the
  // final position just before the page goes away.
  function writeSnapshot() {
    if (typeof localStorage === 'undefined') return;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          current,
          queue,
          qIndex,
          order,
          orderPos,
          currentTime: pendingSeek ?? currentTime,
          paused,
          shuffle,
          repeat,
          volume,
          muted,
        } satisfies PersistedPlayer)
      );
    } catch {
      /* storage unavailable/full: playback still works, just no persistence */
    }
  }
  $effect(() => {
    void current;
    void queue;
    void qIndex;
    void order;
    void orderPos;
    void paused;
    void shuffle;
    void repeat;
    void volume;
    void muted;
    untrack(writeSnapshot);
  });

  let total = $derived.by(() => {
    if (Number.isFinite(duration) && duration > 0) return duration;
    return current ? (current.durationSecs ?? 0) : 0;
  });
  let waveUrl = $derived.by(() => (current ? (current.waveformUrl ?? null) : null));
  // The resolved audio URL of the current track, so the waveform can decode it.
  let srcUrl: string | null = $state(null);
  // Album art resolved on demand (Spotify oEmbed) when the track carries none.
  let resolvedArt: string | null = $state(null);
  $effect(() => {
    const t = current;
    resolvedArt = null;
    if (!t || t.artwork || !t.spotifyUrl) return;
    let cancelled = false;
    resolveSpotifyArt(t.spotifyUrl).then((a) => {
      if (!cancelled && current?.id === t.id && current.source === t.source) resolvedArt = a;
    });
    return () => {
      cancelled = true;
    };
  });

  // Report playback to the Last.fm scrobbler (no-op unless connected + enabled).
  $effect(() => {
    if (current) scrobbler.onProgress(current, currentTime, total);
  });

  // Publish from native media events and transport calls, not animation frames
  // or reactive DOM updates (which can be suspended while the page is hidden).
  function publishMetadata(track: PlayerTrack | null, title = track?.title, artist = track?.artist, art = track?.artwork) {
    if (!('mediaSession' in navigator) || typeof MediaMetadata === 'undefined') return;
    navigator.mediaSession.metadata = track ? new MediaMetadata({
      title: title ?? '',
      artist: artist ?? '',
      artwork: art ? [{ src: new URL(art, location.href).href }] : [],
    }) : null;
  }

  function publishPlaybackState() {
    if (!('mediaSession' in navigator)) return;
    const ms = navigator.mediaSession;
    const el = audio;
    ms.playbackState = current ? (!el || el.paused || el.ended || el.error ? 'paused' : 'playing') : 'none';
    if (!ms.setPositionState) return;
    const length = el && Number.isFinite(el.duration) && el.duration > 0
      ? el.duration : (current?.durationSecs ?? 0);
    if (!current || !el || !Number.isFinite(length) || length <= 0) {
      ms.setPositionState();
      return;
    }
    ms.setPositionState({
      duration: length,
      position: Math.max(0, Math.min(Number.isFinite(el.currentTime) ? el.currentTime : 0, length)),
      playbackRate: el.playbackRate > 0 ? el.playbackRate : 1,
    });
  }

  function syncAudioState() {
    if (!audio || destroyed) return;
    const position = Number.isFinite(audio.currentTime) ? audio.currentTime : 0;
    // Native repeat does not emit ended, so each wrap starts a fresh listen.
    if (audio.loop && audio.readyState > 0 && !audio.seeking && !audio.error && position < 1 && currentTime > 1 && current) {
      void scrobbler.onPlay(current);
    }
    paused = audio.paused || audio.ended || audio.error != null;
    currentTime = position;
    duration = Number.isFinite(audio.duration) ? audio.duration : 0;
    publishPlaybackState();
  }

  $effect(() => {
    publishMetadata(current, displayTitle, displayArtist, npArt);
  });

  function message(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  function reportError(track: PlayerTrack, err: unknown) {
    wantsPlayback = false;
    resumeOnLoad = false;
    audio?.pause();
    syncAudioState();
    playbackError = message(err);
    onError?.(track, playbackError);
  }

  function reportProcessingError(err: unknown) {
    if (current) reportError(current, `Audio processing failed: ${message(err)}`);
  }

  function pausePlayback() {
    wantsPlayback = false;
    resumeOnLoad = false;
    if (restoringId !== requestId) {
      requestId++;
      resolvingId = null;
      resolvingSource = undefined;
    }
    audio?.pause();
    syncAudioState();
  }

  async function startPlayback(track: PlayerTrack, id = requestId) {
    const el = audio;
    if (!el || destroyed || !wantsPlayback) return;
    const restarting = el.ended || (el.readyState > 0 && el.currentTime === 0);
    playbackError = '';
    try {
      ensureEq();
      // Call play() in the same task as the media action/user gesture. Neither
      // loading the next source nor this call waits for a Svelte render or RAF.
      const resumed = eq.isBuilt ? eq.resume() : undefined;
      await Promise.all([el.play(), resumed]);
      if (id !== requestId || destroyed) return;
      if (restarting) void scrobbler.onPlay(track);
      syncAudioState();
    } catch (err: unknown) {
      // A newer track or an explicit pause intentionally cancels pending play.
      if (id !== requestId || destroyed) return;
      // The media error event owns resource failures and its one URL refresh.
      if (el.error) return;
      reportError(track, err instanceof DOMException && err.name === 'NotAllowedError'
        ? 'Playback was blocked by the browser. Tap Play to continue.'
        : err);
    }
  }

  function playCurrent() {
    if (!current || !audio) return;
    wantsPlayback = true;
    if (restoringId === requestId) return;
    if (!srcUrl || audio.error) void playTrack(current);
    else void startPlayback(current);
  }

  /** Resolve a track's URL and start it. Shared by toggle, transport, and retry. */
  async function playTrack(track: PlayerTrack) {
    const el = audio;
    if (!el) return;
    const id = ++requestId;
    wantsPlayback = true;
    resumeOnLoad = false;
    pendingSeek = null;
    el.pause();
    current = track;
    retriedCurrent = false;
    currentGainDb = 0;
    playbackError = '';
    srcUrl = null;
    el.removeAttribute('src');
    el.load();
    syncAudioState();
    publishMetadata(track);
    resolvingId = track.id;
    resolvingSource = track.source;
    try {
      const resolved = resolveSrc(track);
      const src = typeof resolved === 'string' ? resolved : await resolved;
      if (id !== requestId || destroyed || !wantsPlayback) return;
      srcUrl = src;
      el.src = src;
      publishMetadata(track);
      publishPlaybackState();
      void scrobbler.onPlay(track);
      await startPlayback(track, id);
    } catch (err: unknown) {
      if (id === requestId && !destroyed) reportError(track, err);
    } finally {
      if (id === requestId) {
        resolvingId = null;
        resolvingSource = undefined;
      }
    }
  }

  // -- Restore after a reload -------------------------------------------------
  // Applied once a track's metadata is ready: setting currentTime before the
  // browser has the media duration is ignored, so the seek waits for that event.
  let pendingSeek: number | null = null;
  let resumeOnLoad = false;
  function onLoadedMetadata() {
    const el = audio;
    if (!el) return;
    duration = Number.isFinite(el.duration) ? el.duration : 0;
    if (pendingSeek != null) {
      seekTo(pendingSeek);
      pendingSeek = null;
      writeSnapshot();
    }
    syncAudioState();
    if (resumeOnLoad && wantsPlayback && current) {
      resumeOnLoad = false;
      void startPlayback(current);
    }
  }

  /** Restore the source and saved seek; browser autoplay policy still applies. */
  async function restoreTrack(track: PlayerTrack, position: number, wasPaused: boolean) {
    const el = audio;
    if (!el) return;
    const id = ++requestId;
    wantsPlayback = !wasPaused;
    restoringId = id;
    pendingSeek = Number.isFinite(position) && position > 0 ? position : null;
    writeSnapshot();
    try {
      const resolved = resolveSrc(track);
      const src = typeof resolved === 'string' ? resolved : await resolved;
      if (id !== requestId || destroyed) return;
      srcUrl = src;
      resumeOnLoad = wantsPlayback;
      el.src = src;
      ensureEq();
      publishMetadata(track);
      publishPlaybackState();
    } catch (err: unknown) {
      if (id === requestId && !destroyed) reportError(track, err);
    } finally {
      if (restoringId === id) restoringId = null;
    }
  }

  onMount(() => {
    scrobbler.flushQueue();
    const installedActions: MediaSessionAction[] = [];

    if ('mediaSession' in navigator) {
      const ms = navigator.mediaSession;
      const set = (action: MediaSessionAction, handler: MediaSessionActionHandler) => {
        try {
          ms.setActionHandler(action, handler);
          installedActions.push(action);
        } catch {
          /* action unsupported on this browser */
        }
      };
      set('play', playCurrent);
      set('pause', pausePlayback);
      set('previoustrack', () => prev());
      set('nexttrack', () => next());
      set('seekbackward', (d) => seekTo((audio?.currentTime ?? 0) - (d.seekOffset ?? 10)));
      set('seekforward', (d) => seekTo((audio?.currentTime ?? 0) + (d.seekOffset ?? 10)));
      set('seekto', (d) => {
        if (d.seekTime != null) seekTo(d.seekTime);
      });
      set('stop', pausePlayback);
    }
    const save = () => { syncAudioState(); writeSnapshot(); };
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') save();
      else syncAudioState();
    };
    window.addEventListener('pagehide', save);
    document.addEventListener('visibilitychange', onVisibility);

    if (persisted?.current) {
      restoreTrack(persisted.current, persisted.currentTime ?? 0, persisted.paused ?? true);
    }

    return () => {
      window.removeEventListener('pagehide', save);
      document.removeEventListener('visibilitychange', onVisibility);
      if ('mediaSession' in navigator) {
        for (const action of installedActions) navigator.mediaSession.setActionHandler(action, null);
        navigator.mediaSession.metadata = null;
        navigator.mediaSession.playbackState = 'none';
        navigator.mediaSession.setPositionState?.();
      }
    };
  });

  /** Rebuild the play order from the current queue, shuffle flag, and qIndex.
     Only for a new context (another list, or shuffle switched on/off): taps
     within the playing list go through `jumpTo` and keep the order. */
  function rebuildOrder() {
    const n = queue.length;
    const idxs = Array.from({ length: n }, (_, i) => i);
    if (shuffle && n > 1) {
      // Fisher–Yates, then pin the current track first so toggling shuffle (or
      // adopting a new queue) never jumps away from what is playing.
      for (let i = n - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [idxs[i], idxs[j]] = [idxs[j], idxs[i]];
      }
      const p = idxs.indexOf(qIndex);
      if (p > 0) {
        idxs.splice(p, 1);
        idxs.unshift(qIndex);
      }
      orderPos = 0;
    } else {
      orderPos = Math.max(0, idxs.indexOf(qIndex));
    }
    order = idxs;
  }

  /** Make queue index `idx` the current track without reshuffling. In order
     mode that is its own position; in shuffle the picked track moves to just
     after the current one, so what already played stays behind it (Previous)
     and the rest of the shuffle keeps its sequence. */
  function jumpTo(idx: number) {
    if (!shuffle) {
      orderPos = Math.max(0, order.indexOf(idx));
    } else {
      const p = order.indexOf(idx);
      if (p !== orderPos) {
        const next = [...order];
        next.splice(p, 1);
        const at = p < orderPos ? orderPos : orderPos + 1;
        next.splice(at, 0, idx);
        order = next;
        orderPos = at;
      }
    }
    qIndex = idx;
  }

  /** Same tracks in the same order: tapping another row of the list that is playing. */
  function sameQueue(a: PlayerTrack[], b: PlayerTrack[]): boolean {
    return a.length === b.length && a.every((t, i) => t.id === b[i].id && t.source === b[i].source);
  }

  /** Play the track at position `pos` within the current play order. */
  function playAt(pos: number) {
    if (pos < 0 || pos >= order.length) return;
    orderPos = pos;
    qIndex = order[pos];
    void playTrack(queue[qIndex]);
  }

  export async function toggle(track: PlayerTrack, q?: PlayerTrack[]) {
    const el = audio;
    if (!el) return;

    // Adopt the caller's list as the queue so prev/next/shuffle have context.
    // Tapping within the list that is already playing keeps the play order.
    if (q && q.length && !sameQueue(q, queue)) {
      queue = q;
      const idx = q.findIndex((t) => t.id === track.id && t.source === track.source);
      qIndex = idx >= 0 ? idx : 0;
      rebuildOrder();
    } else {
      const idx = queue.findIndex((t) => t.id === track.id && t.source === track.source);
      if (idx >= 0) jumpTo(idx);
      else {
        queue = [track];
        qIndex = 0;
        rebuildOrder();
      }
    }

    if (current?.id === track.id && current.source === track.source) {
      togglePlay();
      return;
    }
    await playTrack(track);
  }

  function isDisliked(t: PlayerTrack | null | undefined): boolean {
    if (!t || t.source !== 'library') return false;
    return lib.tracks.find((x) => x.id === t.id)?.rating === 'disliked';
  }

  /** The order position `dir` steps away, skipping disliked tracks. Wraps only
     when repeat is 'all'. Null when nothing playable remains that way (e.g. the
     first track with repeat off, or only disliked tracks are left). */
  function stepTarget(dir: number): number | null {
    const n = order.length;
    let p = orderPos;
    for (let tries = 0; tries < n; tries++) {
      p += dir;
      if (p < 0) {
        if (repeat === 'all') p = n - 1;
        else return null;
      } else if (p >= n) {
        if (repeat === 'all') p = 0;
        else return null;
      }
      if (!isDisliked(queue[order[p]])) return p;
    }
    return null;
  }

  /** Start the track `dir` steps away; false when there is none (see stepTarget). */
  function advance(dir: number): boolean {
    const p = stepTarget(dir);
    if (p === null) return false;
    playAt(p);
    return true;
  }

  function next() {
    advance(1);
  }
  function prev() {
    // Restart the track first if we're past the intro, like every real player.
    if ((audio?.currentTime ?? 0) > 3) {
      seekTo(0);
      return;
    }
    advance(-1);
  }
  function toggleShuffle() {
    shuffle = !shuffle;
    rebuildOrder();
  }
  function cycleRepeat() {
    repeat = repeat === 'off' ? 'all' : repeat === 'all' ? 'one' : 'off';
  }

  /** Auto-advance when a track finishes, honoring repeat and skipping disliked. */
  function onEndedInternal() {
    syncAudioState();
    if (!wantsPlayback) return;
    if (repeat === 'one') {
      seekTo(0);
      playCurrent();
      return;
    }
    if (!advance(1)) {
      wantsPlayback = false;
      publishPlaybackState();
      onEnded?.();
    }
  }

  // Auto-skip disliked tracks. `advance` already skips them on normal
  // next/prev/auto-advance; this covers the live case: the store calls this
  // whenever a track is disliked (player bar, now-playing, or a row), and if it
  // is the one currently playing we move on. Imperative (not a reactive effect)
  // to avoid a feedback loop with the playback state it changes.
  const onTrackDisliked = (id: number) => {
    if (!paused && current?.source === 'library' && current.id === id) advance(1);
  };
  lib.onTrackDisliked = onTrackDisliked;

  export function isCurrent(id: number, source?: TrackSource): boolean {
    return current?.id === id && (source === undefined || current?.source === source);
  }

  export function isPlaying(id: number, source?: TrackSource): boolean {
    return isCurrent(id, source) && !paused;
  }

  export function isResolving(id: number, source?: TrackSource): boolean {
    return resolvingId === id && (source === undefined || resolvingSource === source);
  }

  async function onAudioError() {
    const el = audio;
    const track = current;
    if (!track || !el || !el.error) return;
    const id = requestId;
    const position = el.currentTime;
    const shouldResume = wantsPlayback;
    const mediaError = el.error.message || `Media error ${el.error.code}`;
    syncAudioState();

    // A deleted/missing library track 404s deterministically, so retrying is
    // pointless. Detect it and skip with an accurate message instead of the
    // generic "try again" - this happens when a cleanup removes a track a
    // client still has queued.
    if (track.source === 'library') {
      try {
        const res = await fetch(`/api/tracks/${track.id}/audio`, { method: 'HEAD' });
        if (id !== requestId || destroyed) return;
        if (res.status === 404) {
          reportError(track, 'This track is no longer available.');
          if (shouldResume && canStep) advance(1);
          return;
        }
      } catch {
        /* probe failed (offline?): fall through to the normal retry */
      }
    }
    if (id !== requestId || destroyed) return;

    if (retriedCurrent) {
      reportError(track,
        track.source === 'soundcloud'
          ? `Playback failed after refreshing the audio link: ${mediaError}`
          : `Playback failed: ${mediaError}`,
      );
      return;
    }

    retriedCurrent = true;
    try {
      const resolved = resolveSrc(track);
      const src = typeof resolved === 'string' ? resolved : await resolved;
      if (id !== requestId || destroyed) return;
      pendingSeek = Number.isFinite(position) && position > 0 ? position : null;
      srcUrl = src;
      el.src = src;
      if (shouldResume && wantsPlayback) await startPlayback(track, id);
      else syncAudioState();
    } catch (err: unknown) {
      if (id === requestId && !destroyed) reportError(track, err);
    }
  }

  function seekTo(secs: number) {
    const el = audio;
    if (!el || !Number.isFinite(secs)) return;
    const length = Number.isFinite(el.duration) ? el.duration : total;
    if (!(length > 0)) return;
    el.currentTime = Math.max(0, Math.min(secs, length));
    syncAudioState();
  }

  function togglePlay() {
    if (!audio) return;
    if (audio.paused && resolvingId == null) playCurrent();
    else pausePlayback();
  }

  /** Exposed on the handle so the shell can bind Space to play/pause. */
  export function playPause() {
    if (current) togglePlay();
  }

  // ── Mobile "Now Playing": the bar expands to a full-screen sheet ───────────
  let expanded = $state(false);
  let sheetH = $state(0);
  let dragging = $state(false);
  let dragY = $state(0);
  let npEl: HTMLDivElement | undefined = $state();
  $effect(() => {
    if (!expanded || !npEl) return;
    const previous = document.activeElement;
    npEl.querySelector<HTMLButtonElement>('.np-close')?.focus();
    return () => {
      if (previous instanceof HTMLElement) previous.focus();
    };
  });

  function onNPKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      closeNP();
    } else if (e.key === 'Tab' && npEl) {
      const controls = [...npEl.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex="0"]')];
      const first = controls[0];
      const last = controls[controls.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
    }
  }
  const reduceMotion =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  // Opening and closing morph the floating bar into the sheet, and the small
  // cover into the big one, with the View Transition API. The browser snapshots
  // both states and animates between them, while `morphing` holds the live sheet
  // still. Without the API, or with reduced motion, the sheet slides as before.
  let morphing = $state(false);
  let activeMorph: ViewTransition | undefined;
  async function setExpanded(next: boolean) {
    const settle = async () => {
      expanded = next;
      dragging = false;
      dragY = 0;
      await tick();
    };
    if (reduceMotion || typeof document.startViewTransition !== 'function') {
      await settle();
      return;
    }
    const root = document.documentElement;
    // A tap mid-morph replaces the running one; only the latest cleans up.
    activeMorph?.skipTransition();
    activeMorph = undefined;
    morphing = true;
    root.classList.add('np-morph'); // names the morphing pair; see app.css
    // A route change still settling is skipped by this transition; unname its page area now.
    root.classList.remove('page-nav');
    root.classList.toggle('np-morph-closing', !next);
    let transition: ViewTransition | undefined;
    try {
      transition = document.startViewTransition(settle);
      activeMorph = transition;
      void transition.ready.catch(() => {});
      await transition.finished.catch(() => {});
    } catch {
      // The API can exist yet refuse to snapshot; the state still has to change.
      await settle();
    } finally {
      if (activeMorph === transition) {
        activeMorph = undefined;
        morphing = false;
        root.classList.remove('np-morph', 'np-morph-closing');
      }
    }
  }
  function openNP() {
    // Only a listening affordance on phones; the desktop bar is already complete.
    if (typeof window !== 'undefined' && window.innerWidth > 860) return;
    if (current) void setExpanded(true);
  }
  function closeNP() {
    void setExpanded(false);
  }

  // Swipe-down-to-dismiss: track 1:1, project momentum on release (apple-design),
  // then either fall closed or spring back. Springs/settle handled by CSS.
  let dragStartY = 0;
  let lastY = 0;
  let lastT = 0;
  let velY = 0;
  function onSheetPointerDown(e: PointerEvent) {
    if ((e.target as HTMLElement).closest('button')) return;
    dragging = true;
    dragStartY = e.clientY;
    lastY = e.clientY;
    lastT = performance.now();
    velY = 0;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function onSheetPointerMove(e: PointerEvent) {
    if (!dragging) return;
    const dy = e.clientY - dragStartY;
    dragY = dy >= 0 ? dy : dy * 0.2; // rubber-band upward drags
    const now = performance.now();
    const dt = now - lastT;
    if (dt > 0) velY = ((e.clientY - lastY) / dt) * 1000; // px/s
    lastY = e.clientY;
    lastT = now;
  }
  function onSheetPointerUp() {
    if (!dragging) return;
    const projected = dragY + velY * 0.12;
    // Closing keeps `dragging` until the morph snapshots the sheet where the finger left it.
    if (projected > (sheetH || 500) * 0.3 || velY > 900) {
      closeNP();
      return;
    }
    dragging = false;
    dragY = 0;
  }

  // Swipe the Now Playing cover sideways to change track, like Spotify: left is
  // next, right is the previous track itself (no restart, unlike the button).
  // The cover tracks the finger, slides out, and the new one slides in from the
  // other side; short or slow swipes spring back.
  let swipeX = $state(0);
  let swipeTransition = $state('');
  let swipeStartX = 0;
  let swipeStartY = 0;
  let swipeAxis: 'x' | 'y' | null = null;
  let swipeLastX = 0;
  let swipeLastT = 0;
  let swipeVel = 0;
  let swipePointer: number | null = null;
  const SWIPE_OUT_MS = 160;
  function onArtPointerDown(e: PointerEvent) {
    if (e.pointerType === 'mouse' && e.button !== 0) return;
    swipePointer = e.pointerId;
    swipeAxis = null;
    swipeStartX = swipeLastX = e.clientX;
    swipeStartY = e.clientY;
    swipeLastT = performance.now();
    swipeVel = 0;
    swipeTransition = 'none';
  }
  function onArtPointerMove(e: PointerEvent) {
    if (e.pointerId !== swipePointer) return;
    const dx = e.clientX - swipeStartX;
    if (!swipeAxis) {
      // Decide the axis once the finger has clearly moved, so taps stay taps.
      if (Math.hypot(dx, e.clientY - swipeStartY) < 8) return;
      swipeAxis = Math.abs(dx) > Math.abs(e.clientY - swipeStartY) ? 'x' : 'y';
      if (swipeAxis === 'x') (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    }
    if (swipeAxis !== 'x') return;
    swipeX = canStep ? dx : dx * 0.2; // nothing to step to: rubber-band
    const now = performance.now();
    if (now > swipeLastT) swipeVel = ((e.clientX - swipeLastX) / (now - swipeLastT)) * 1000; // px/s
    swipeLastX = e.clientX;
    swipeLastT = now;
  }
  function onArtPointerUp(e: PointerEvent) {
    if (e.pointerId !== swipePointer) return;
    swipePointer = null;
    if (swipeAxis !== 'x') return;
    const width = (e.currentTarget as HTMLElement).clientWidth || 300;
    const projected = swipeX + swipeVel * 0.12;
    // Commit after a real distance, then on either reach (with momentum) or a quick flick.
    const far = Math.abs(swipeX) > 48 && (Math.abs(projected) > width * 0.3 || Math.abs(swipeVel) > 700);
    const direction = far ? Math.sign(projected) : 0;
    // Resolve the destination now, so a swipe toward nothing (first track with
    // repeat off, only disliked tracks left) springs back instead of faking a change.
    const step = direction === 0 || e.type === 'pointercancel' ? null : stepTarget(direction < 0 ? 1 : -1);
    // Wrapping back onto the playing track (a one-track queue on repeat) is not a change either.
    const target = step === orderPos ? null : step;
    if (target === null) {
      swipeTransition = '';
      swipeX = 0;
      return;
    }
    haptic(e.currentTarget as Element);
    if (reduceMotion) {
      playAt(target);
      swipeTransition = 'none';
      swipeX = 0;
      return;
    }
    swipeTransition = `translate ${SWIPE_OUT_MS}ms var(--ease-out)`;
    swipeX = direction * width * 1.2;
    setTimeout(() => {
      playAt(target);
      // Jump to the far side unseen, then glide the new cover into place.
      swipeTransition = 'none';
      swipeX = -direction * width * 1.2;
      requestAnimationFrame(() => requestAnimationFrame(() => {
        swipeTransition = '';
        swipeX = 0;
      }));
    }, SWIPE_OUT_MS);
  }

  // ── Like / dislike the current library track (Now Playing only) ────────────
  let currentLibTrack = $derived.by(() => {
    const c = current;
    if (!c || c.source !== 'library') return null;
    return lib.tracks.find((t) => t.id === c.id) ?? null;
  });
  // Prefer the live library track's metadata so edits (e.g. AI cleanup) show up
  // in the now-playing widget immediately; the snapshot `current` is only a
  // fallback for non-library sources.
  let displayTitle = $derived(currentLibTrack?.title ?? current?.title ?? '');
  let displayArtist = $derived(
    currentLibTrack
      ? currentLibTrack.artists.map((a) => a.name).join(', ') || (current?.artist ?? '')
      : (current?.artist ?? ''),
  );
  // Artwork: prefer the live library track's cover so it stays correct after an
  // edit and after a reload (the persisted `current` snapshot can predate the
  // cover being generated). External URLs (Spotify/SoundCloud) are used as-is;
  // our own cover route is sized per surface - small thumb in the bar, 512px for
  // the full-screen art.
  function coverAtSize(url: string, size: 'large'): string {
    return /^\/api\/tracks\/\d+\/cover$/.test(url) ? `${url}?size=${size}` : url;
  }
  let coverBase = $derived(currentLibTrack?.cover ?? current?.artwork ?? resolvedArt ?? null);
  let barArt = $derived(coverBase);
  let npArt = $derived(coverBase ? coverAtSize(coverBase, 'large') : null);
  let coverSeed = $derived(current ? (current.coverSeed ?? `track:${current.id}`) : '');
  /** Played/filled share of a native range, for its painted track fill. */
  function fillPct(value: number, max: number): number {
    return max > 0 ? Math.min(100, Math.max(0, (value / max) * 100)) : 0;
  }
  function rateCurrent(rating: 'liked' | 'disliked') {
    const t = currentLibTrack;
    if (t) lib.setRating(t, t.rating === rating ? null : rating);
  }

  onDestroy(() => {
    destroyed = true;
    requestId++;
    audio?.pause();
    lib.onTrackDisliked = null;
    void eq.destroy().catch((err: unknown) => console.error('Closing audio processing failed', err));
  });
</script>

<!-- Inline player: fills the shell's bottom dock with a bound native <audio>
  element and custom controls in a three-column CSS grid. -->
<div class="player" class:idle={!current} class:collapsed={!expanded}>
  <audio
    bind:this={audio}
    preload="metadata"
    loop={repeat === 'one'}
    onplay={syncAudioState}
    onplaying={syncAudioState}
    onpause={syncAudioState}
    ontimeupdate={syncAudioState}
    ondurationchange={syncAudioState}
    onseeked={syncAudioState}
    onratechange={syncAudioState}
    bind:volume
    bind:muted
    onerror={onAudioError}
    onended={onEndedInternal}
    onloadedmetadata={onLoadedMetadata}
  ></audio>

  {#if current}
    <div class="pl-left">
      <button
        class="pl-identity"
        type="button"
        aria-label={`Open Now Playing: ${displayTitle}`}
        aria-expanded={expanded}
        onclick={openNP}
      >
        <div class="player-thumb cover-wrap">
          <PixelCover src={barArt} seed={coverSeed} loading="eager" />
        </div>
        <div class="player-info">
          <span class="title">{displayTitle}</span>
          <span class="artist">{displayArtist}</span>
        </div>
      </button>
      {#if currentLibTrack}
        <div class="pl-rate">
          <button class="btn-rate" class:active-like={currentLibTrack.rating === 'liked'} onclick={() => rateCurrent('liked')} title="Like" aria-label="Like"><i class="pxi pxi-thumbs-up" aria-hidden="true" use:pop={currentLibTrack.rating === 'liked'}></i></button>
          <button class="btn-rate" class:active-dislike={currentLibTrack.rating === 'disliked'} onclick={() => rateCurrent('disliked')} title="Dislike" aria-label="Dislike"><i class="pxi pxi-thumbs-down" aria-hidden="true" use:pop={currentLibTrack.rating === 'disliked'}></i></button>
        </div>
      {/if}
    </div>

    <div class="pl-center">
      <div class="transport">
        <button class="tbtn shuffle" class:on={shuffle} onclick={toggleShuffle} disabled={!canStep} title="Shuffle" aria-label="Shuffle" aria-pressed={shuffle}><i class="pxi pxi-shuffle" aria-hidden="true" use:pop={shuffle}></i></button>
        <button class="tbtn previous" onclick={prev} disabled={!canStep} title="Previous" aria-label="Previous"><i class="pxi pxi-skip-back" aria-hidden="true"></i></button>
        <button class="play" onclick={togglePlay} aria-label={paused ? 'Play' : 'Pause'}><i class="pxi {paused ? 'pxi-play' : 'pxi-pause'}" aria-hidden="true" use:pop={paused}></i></button>
        <button class="tbtn" onclick={next} disabled={!canStep} title="Next" aria-label="Next"><i class="pxi pxi-skip-forward" aria-hidden="true"></i></button>
        <button class="tbtn repeat" class:on={repeat !== 'off'} onclick={cycleRepeat} title={'Repeat: ' + repeat} aria-label="Repeat"><i class="pxi pxi-reload" aria-hidden="true" use:pop={repeat}></i>{#if repeat === 'one'}<span class="rep-one" aria-hidden="true">1</span>{/if}</button>
      </div>

      <div class="progress-row">
        <span class="time">{formatTime(currentTime)}</span>
        {#if waveUrl || srcUrl}
          <div class="wave-slot" class:ready={waveReady}>
            <Waveform waveformUrl={waveUrl} srcUrl={srcUrl} currentTime={currentTime} duration={total} onSeek={seekTo} bind:available={waveReady} />
          </div>
        {/if}
        {#if !waveReady}
          <input class="range" type="range" min="0" max={total || 0} step="0.1" value={currentTime} style="--fill: {fillPct(currentTime, total)}%" oninput={(e) => seekTo(+e.currentTarget.value)} aria-label="Seek" />
        {/if}
        <span class="time dur">{formatTime(total)}</span>
      </div>
    </div>

    <div class="pl-right">
      {#if !nativeAudio}
        <div class="eq-wrap">
          <button
            class="eq-btn"
            class:on={eqState.enabled}
            bind:this={eqBtnEl}
            onclick={() => (eqOpen = !eqOpen)}
            title="Equalizer"
            aria-label="Equalizer"
            aria-expanded={eqOpen}
          >
            <i class="pxi pxi-sliders-vertical" aria-hidden="true" use:pop={eqOpen}></i>
          </button>
          {#if eqOpen && !expanded}
            <button
              class="eq-backdrop"
              aria-label="Close equalizer"
              onclick={() => (eqOpen = false)}
              use:portal
            ></button>
            <div class="eq-pop" style={eqStyle} use:portal>
              <label class="norm-toggle">
                <input
                  type="checkbox"
                  checked={normalizeEnabled}
                  onchange={(e) => setNormalize(e.currentTarget.checked)}
                />
                <span>Normalize volume</span>
              </label>
              <EqPanel bind:state={eqState} onUpdate={handleEqUpdate} />
            </div>
          {/if}
        </div>
      {/if}
      {#if !nativeAudio}
        <button class="mute" onclick={() => (muted = !muted)} aria-label={muted ? 'Unmute' : 'Mute'}>
          <i class="pxi {muted || volume === 0 ? 'pxi-volume-x' : volume < 0.5 ? 'pxi-volume-1' : 'pxi-volume-3'}" aria-hidden="true"></i>
        </button>
      {/if}
      {#if nativeAudio}
        <span class="device-volume">Use device volume</span>
      {:else}
        <input class="volume" type="range" min="0" max="1" step="0.01" bind:value={volume} style="--fill: {fillPct(volume, 1)}%" aria-label="Volume" />
      {/if}
    </div>
  {:else}
    <div class="pl-idle">
      <i class="pxi pxi-music" aria-hidden="true"></i>
      <span>Nothing playing</span>
    </div>
  {/if}
</div>

{#if current}
  <!-- Mobile Now Playing: full-screen sheet that slides up from the bar. -->
  <div
    class="np"
    bind:this={npEl}
    role="dialog"
    aria-modal={expanded}
    aria-label="Now playing"
    tabindex="-1"
    inert={!expanded}
    onkeydown={onNPKeyDown}
    class:open={expanded}
    class:dragging
    bind:clientHeight={sheetH}
    style="transform: translateY({dragging ? dragY + 'px' : expanded ? '0px' : '100%'}); opacity: {reduceMotion ? (expanded ? 1 : 0) : 1}; transition: {dragging || morphing ? 'none' : reduceMotion ? 'opacity 200ms var(--ease-out)' : 'transform var(--motion-sheet) var(--ease-drawer)'}; pointer-events: {expanded ? 'auto' : 'none'}"
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="np-head"
      onpointerdown={onSheetPointerDown}
      onpointermove={onSheetPointerMove}
      onpointerup={onSheetPointerUp}
      onpointercancel={onSheetPointerUp}
    >
      <button class="np-close" onclick={closeNP} aria-label="Close now playing"><i class="pxi pxi-chevron-down" aria-hidden="true"></i></button>
      <div class="np-grabber"></div>
    </div>

    <div class="np-stage">
      <ArtGlow src={npArt} seed={coverSeed} />
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="np-art cover-wrap"
        style:translate="{swipeX}px 0"
        style:transition={swipeTransition || null}
        onpointerdown={onArtPointerDown}
        onpointermove={onArtPointerMove}
        onpointerup={onArtPointerUp}
        onpointercancel={onArtPointerUp}
        ondragstart={(e) => e.preventDefault()}
      >
        <PixelCover src={npArt} seed={coverSeed} loading="eager" />
      </div>
    </div>

    <div class="np-meta">
      <div class="np-title">{displayTitle}</div>
      <div class="np-artist">{displayArtist}</div>
    </div>
    {#if playbackError}
      <div class="callout callout-error playback-error" role="status">
        <i class="pxi pxi-square-alert" aria-hidden="true"></i>
        <div class="callout-body"><strong>{playbackError}</strong></div>
      </div>
    {/if}

    <div class="np-scrub">
      {#if waveUrl || srcUrl}
        <div class="wave-slot" class:ready={waveReady}>
          <Waveform waveformUrl={waveUrl} srcUrl={srcUrl} currentTime={currentTime} duration={total} onSeek={seekTo} bind:available={waveReady} />
        </div>
      {/if}
      {#if !waveReady}
        <input class="range" type="range" min="0" max={total || 0} step="0.1" value={currentTime} style="--fill: {fillPct(currentTime, total)}%" oninput={(e) => seekTo(+e.currentTarget.value)} aria-label="Seek" />
      {/if}
      <div class="np-times"><span>{formatTime(currentTime)}</span><span>{formatTime(total)}</span></div>
    </div>

    <div class="np-transport">
      <button class="tbtn shuffle" class:on={shuffle} onclick={toggleShuffle} disabled={!canStep} aria-label="Shuffle"><i class="pxi pxi-shuffle" aria-hidden="true" use:pop={shuffle}></i></button>
      <button class="tbtn" onclick={prev} disabled={!canStep} aria-label="Previous"><i class="pxi pxi-skip-back" aria-hidden="true"></i></button>
      <button class="np-play" onclick={togglePlay} aria-label={paused ? 'Play' : 'Pause'}><i class="pxi {paused ? 'pxi-play' : 'pxi-pause'}" aria-hidden="true" use:pop={paused}></i></button>
      <button class="tbtn" onclick={next} disabled={!canStep} aria-label="Next"><i class="pxi pxi-skip-forward" aria-hidden="true"></i></button>
      <button class="tbtn repeat" class:on={repeat !== 'off'} onclick={cycleRepeat} aria-label="Repeat"><i class="pxi pxi-reload" aria-hidden="true" use:pop={repeat}></i>{#if repeat === 'one'}<span class="rep-one" aria-hidden="true">1</span>{/if}</button>
    </div>

    <div class="np-secondary">
      {#if currentLibTrack}
        <button class="btn-rate" class:active-like={currentLibTrack.rating === 'liked'} onclick={() => rateCurrent('liked')} aria-label="Like"><i class="pxi pxi-thumbs-up" aria-hidden="true" use:pop={currentLibTrack.rating === 'liked'}></i></button>
        <button class="btn-rate" class:active-dislike={currentLibTrack.rating === 'disliked'} onclick={() => rateCurrent('disliked')} aria-label="Dislike"><i class="pxi pxi-thumbs-down" aria-hidden="true" use:pop={currentLibTrack.rating === 'disliked'}></i></button>
      {/if}
      {#if !nativeAudio}
        <button class="eq-btn" class:on={eqState.enabled} onclick={() => (eqOpen = !eqOpen)} aria-label="Equalizer" aria-expanded={eqOpen}><i class="pxi pxi-sliders-vertical" aria-hidden="true" use:pop={eqOpen}></i></button>
      {/if}
      {#if !nativeAudio}
        <button class="mute" onclick={() => (muted = !muted)} aria-label={muted ? 'Unmute' : 'Mute'}><i class="pxi {muted || volume === 0 ? 'pxi-volume-x' : volume < 0.5 ? 'pxi-volume-1' : 'pxi-volume-3'}" aria-hidden="true"></i></button>
      {/if}
    </div>
    {#if nativeAudio}
      <p class="device-volume">Use your device's volume buttons or Control Center.</p>
    {:else}
      <input class="volume np-vol" type="range" min="0" max="1" step="0.01" bind:value={volume} style="--fill: {fillPct(volume, 1)}%" aria-label="Volume" />
    {/if}

    {#if eqOpen && expanded && !nativeAudio}
      <div class="np-eq">
        <label class="norm-toggle">
          <input
            type="checkbox"
            checked={normalizeEnabled}
            onchange={(e) => setNormalize(e.currentTarget.checked)}
          />
          <span>Normalize volume</span>
        </label>
        <EqPanel bind:state={eqState} onUpdate={handleEqUpdate} />
      </div>
    {/if}
  </div>
{/if}

<style>
  /* Fills the shell's bottom dock; App.svelte owns its ground and top hairline. */
  .player {
    width: 100%;
    height: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(320px, min(560px, 44vw)) minmax(0, 1fr);
    align-items: center;
    gap: 24px;
    padding: 0 24px;
  }
  .player.idle { display: flex; align-items: center; justify-content: center; }

  .pl-idle {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--muted-2);
    font-size: 13px;
    font-weight: 500;
  }
  .pl-idle .pxi { font-size: 16px; }

  /* ── Left: track identity ────────────────────────────────────────────── */
  .pl-left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .pl-identity {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
  }
  .player-thumb {
    width: 48px;
    flex-shrink: 0;
    border-radius: var(--radius-chip);
  }
  .player-info { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .title {
    font-size: 14px;
    font-weight: 500;
    line-height: 1.35;
    color: var(--text-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .artist {
    font-size: 13px;
    line-height: 1.35;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pl-rate {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }
  .pl-rate .btn-rate { width: 32px; height: 32px; }

  /* ── Center: transport (40px) over progress (28px) ───────────────────── */
  .pl-center {
    display: grid;
    grid-template-rows: 40px 28px;
    row-gap: 4px;
    justify-items: center;
    min-width: 0;
    width: 100%;
  }
  .transport { display: flex; align-items: center; gap: 8px; }
  .tbtn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .tbtn .pxi { font-size: 24px; }
  .tbtn:hover:not(:disabled) { color: var(--text-bright); background: var(--surface-2); }
  .tbtn:disabled { color: var(--text-disabled); cursor: default; }
  .tbtn.on, .tbtn.on:hover:not(:disabled) { color: var(--accent); }
  /* Repeat-one: a tiny mono key cap on the repeat glyph's corner. */
  .rep-one {
    position: absolute;
    top: 4px;
    right: 4px;
    padding: 0 2px;
    background: var(--accent);
    color: var(--on-accent);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    line-height: 11px;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }

  /* Play/pause: the ink key. */
  .play,
  .np-play {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    padding: 0;
    border: 0;
    background: var(--text-bright);
    color: var(--bg);
    cursor: pointer;
  }
  .play {
    width: 40px;
    height: 40px;
    border-radius: var(--radius-control);
  }
  .play .pxi, .np-play .pxi { font-size: 24px; }
  .play:hover, .np-play:hover { background: color-mix(in srgb, var(--text-bright) 86%, var(--bg)); }
  .play:active, .np-play:active { background: color-mix(in srgb, var(--text-bright) 78%, var(--bg)); }

  .progress-row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 28px;
  }
  .time {
    flex-shrink: 0;
    min-width: 40px;
    color: var(--muted-2);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .time.dur { text-align: left; }
  .wave-slot {
    flex: 1;
    min-width: 0;
    height: 28px;
    display: flex;
    align-items: center;
  }

  /* ── Right: equalizer + volume ───────────────────────────────────────── */
  .pl-right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    min-width: 0;
  }
  .eq-wrap { position: relative; display: flex; align-items: center; }
  .eq-btn,
  .mute {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .eq-btn .pxi, .mute .pxi { font-size: 16px; }
  .eq-btn:hover, .mute:hover { color: var(--text-bright); background: var(--surface-2); }
  .eq-btn.on, .eq-btn.on:hover { color: var(--accent); }
  .pl-right .volume { margin-left: 6px; }
  .device-volume { color: var(--muted); font-size: 12px; line-height: 1.5; }

  /* Portalled to <body>, so positioned via viewport-fixed inline coords that
     escape the dock's stacking. A floating layer: float ground and shadow. */
  .eq-backdrop {
    position: fixed;
    inset: 0;
    z-index: 290;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .eq-pop {
    position: fixed;
    z-index: 300;
    width: min(380px, calc(100vw - 16px));
    max-height: calc(var(--app-height, 100dvh) - 32px);
    padding: 16px;
    overflow-y: auto;
    overscroll-behavior: contain;
    border: 1px solid var(--float-border);
    border-radius: var(--radius-card);
    background: var(--float);
    color: var(--text);
    box-shadow: var(--float-shadow);
  }
  .norm-toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    margin-bottom: 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
  }
  .norm-toggle input {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--accent);
  }

  /* Native range inputs (seek + volume): a hairline track, a painted fill
     (live for playback, neutral for volume), and a small square thumb. */
  .range,
  .volume {
    --fill-color: var(--live);
    -webkit-appearance: none;
    appearance: none;
    height: 28px;
    margin: 0;
    padding: 0;
    background: transparent;
    cursor: pointer;
  }
  .volume { --fill-color: var(--text); width: 96px; flex-shrink: 0; }
  .range { flex: 1; min-width: 0; }
  .range::-webkit-slider-runnable-track,
  .volume::-webkit-slider-runnable-track {
    height: 2px;
    background: linear-gradient(to right, var(--fill-color) var(--fill, 0%), var(--border-strong) var(--fill, 0%));
  }
  .range::-moz-range-track,
  .volume::-moz-range-track {
    height: 2px;
    background: var(--border-strong);
  }
  .range::-moz-range-progress,
  .volume::-moz-range-progress {
    height: 2px;
    background: var(--fill-color);
  }
  .range::-webkit-slider-thumb,
  .volume::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 10px;
    height: 10px;
    margin-top: -4px;
    border: 0;
    border-radius: 0;
    background: var(--fill-color);
  }
  .range::-moz-range-thumb,
  .volume::-moz-range-thumb {
    width: 10px;
    height: 10px;
    border: 0;
    border-radius: 0;
    background: var(--fill-color);
  }
  .range:focus-visible,
  .volume:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* ── Phones: a 64px floating card above the tabs ──────────────────────── */
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .player {
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 8px;
      padding: 0 8px 0 12px;
      border: 1px solid var(--float-border);
      border-radius: var(--radius-card);
      background: var(--float);
      box-shadow: var(--float-shadow);
    }
    .pl-center { display: flex; width: auto; }
    .transport { gap: 0; }
    .pl-identity { gap: 12px; width: 100%; min-height: 44px; cursor: pointer; }
    .player-info { flex: 1; }
    .progress-row, .pl-right, .pl-rate { display: none; }
    .player .shuffle, .player .repeat, .player .previous { display: none; }
    .player-thumb { width: 40px; }
    .player .tbtn { width: 44px; height: 44px; }
    .player .play { width: 44px; height: 44px; background: transparent; color: var(--text-bright); }
    .player .play:hover, .player .play:active { background: var(--surface-2); }

    /* Named only while opening or closing, so page transitions leave them alone:
       the card becomes the sheet, the thumbnail becomes the big cover. */
    :global(:root.np-morph) .player.collapsed:not(.idle) { view-transition-name: np-surface; }
    :global(:root.np-morph) .player.collapsed .player-thumb { view-transition-name: np-cover; }
    :global(:root.np-morph) .np.open { view-transition-name: np-surface; }
    :global(:root.np-morph) .np.open .np-art { view-transition-name: np-cover; }
  }

  /* ── Mobile Now Playing (full-screen sheet) ────────────────────────────── */
  .np {
    display: none;
    position: fixed;
    top: var(--app-top, 0px);
    left: 0;
    right: 0;
    height: var(--app-height, 100dvh);
    z-index: 300;
    flex-direction: column;
    align-items: center;
    background: var(--bg);
    padding: calc(var(--safe-top) + 4px) calc(var(--space-page) + var(--safe-right)) calc(var(--safe-bottom) + 24px) calc(var(--space-page) + var(--safe-left));
    /* Everything fits one screen; only the opened EQ (non-iOS) may need to scroll. */
    overflow: hidden;
    overscroll-behavior: contain;
    will-change: transform;
  }
  .np:has(.np-eq) {
    overflow-x: hidden;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
  }
  .np > * { flex-shrink: 0; }
  .np-head {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    min-height: 48px;
    touch-action: none;
    cursor: grab;
  }
  /* By day the art halo rises past the sheet's top edge; fade it into the page
     colour (which the white status bar also uses) instead of cutting it off. */
  :global([data-theme='light']) .np-head::before {
    content: '';
    position: absolute;
    z-index: -1;
    top: calc(-1 * (var(--safe-top) + 4px));
    bottom: -12px;
    left: calc(-1 * (var(--space-page) + var(--safe-left)));
    right: calc(-1 * (var(--space-page) + var(--safe-right)));
    background: linear-gradient(var(--bg) 20%, transparent);
    pointer-events: none;
  }
  .np-grabber { width: 36px; height: 4px; background: var(--border-heavy); }
  .np-close {
    position: absolute;
    left: -10px;
    top: 2px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .np-close .pxi { font-size: 24px; }
  .np-close:hover { color: var(--text-bright); background: var(--surface-2); }
  /* The art sits on its own glow; everything after the stage stacks above it. */
  .np-stage {
    /* The art takes whatever height the controls leave, so the sheet never needs to scroll. */
    --np-art-size: min(100%, 360px, calc(var(--app-height, 100dvh) - 440px));
    /* The glow rises from the cover: larger than it and centered a little above. */
    --glow-size: calc(var(--np-art-size) * 1.55);
    --glow-y: 44%;
    position: relative;
    z-index: 0;
    display: flex;
    justify-content: center;
    width: 100%;
    margin-top: 16px;
  }
  .np-stage ~ * { position: relative; z-index: 1; }
  .np-art {
    position: relative;
    z-index: 1;
    width: var(--np-art-size);
    border-radius: var(--radius-card);
    /* Horizontal drags change track; vertical ones stay with the page. */
    touch-action: pan-y;
    user-select: none;
    -webkit-user-select: none;
    -webkit-touch-callout: none;
    transition: translate var(--motion-normal) var(--ease-spring);
  }
  @media (prefers-reduced-motion: reduce) {
    .np-art { transition: none; }
  }
  .np-meta { width: 100%; margin-top: 24px; text-align: center; }
  .np-title {
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 600;
    line-height: 1.25;
    letter-spacing: -0.02em;
    color: var(--text-bright);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .np-artist { margin-top: 4px; color: var(--muted); font-size: 15px; line-height: 1.4; overflow-wrap: anywhere; }
  .playback-error { width: 100%; margin-top: 16px; }
  .np-scrub { width: 100%; margin-top: 20px; }
  .np-scrub .wave-slot { width: 100%; height: 48px; }
  .np-scrub .range { display: block; width: 100%; height: 44px; }
  .np-times {
    display: flex;
    justify-content: space-between;
    margin-top: 4px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .np-transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: clamp(4px, 3vw, 16px);
    width: 100%;
    margin-top: 20px;
  }
  .np-transport .tbtn { width: 48px; height: 48px; color: var(--text); }
  .np-transport .tbtn:disabled { color: var(--text-disabled); }
  .np-transport .tbtn.on { color: var(--accent); }
  .np-play {
    width: 64px;
    height: 64px;
    border-radius: var(--radius-card);
  }
  .np-secondary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-top: 16px;
  }
  .np-secondary .pxi { font-size: 24px; }
  .np-secondary .btn-rate,
  .np-secondary .eq-btn,
  .np-secondary .mute { width: 48px; height: 48px; }
  .np-vol { width: min(100%, 320px); height: 44px; margin-top: 8px; }
  .np .device-volume { margin: 8px 0 0; text-align: center; }
  .np-eq { width: 100%; margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--border); }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .np { display: flex; }
  }

  /* High contrast: native ranges and outlined keys read in system colors. */
  @media (forced-colors: active) {
    .range, .volume { -webkit-appearance: auto; appearance: auto; }
    .play, .np-play { border: 1px solid ButtonText; }
  }
</style>
