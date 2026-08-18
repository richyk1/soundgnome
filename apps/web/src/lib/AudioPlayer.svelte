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
  import { onDestroy, onMount, untrack } from 'svelte';
  import type { PlayerTrack, TrackSource } from './player';
  import Waveform from './Waveform.svelte';
  import EqPanel from './EqPanel.svelte';
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
  let eqState = $state<EqState>(loadEqState());
  let eqOpen = $state(false);
  let eqBtnEl: HTMLButtonElement | undefined = $state();
  let eqStyle = $state('');

  /** Move a node to <body> so it escapes the player bar's clipping/stacking. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  /** Anchor the popover just above the EQ button, right-aligned, viewport-fixed. */
  function positionEq() {
    if (!eqBtnEl) return;
    const r = eqBtnEl.getBoundingClientRect();
    const right = Math.max(8, window.innerWidth - r.right);
    const bottom = window.innerHeight - r.top + 12;
    eqStyle = `right:${right}px; bottom:${bottom}px;`;
  }

  $effect(() => {
    if (!eqOpen) return;
    positionEq();
    const onResize = () => positionEq();
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  });

  /** Push EQ changes onto the graph (building it on first enable) and persist. */
  function handleEqUpdate(s: EqState) {
    if (audio) {
      if (s.enabled && !eq.isBuilt) {
        eq.attach(audio, s);
        eq.resume();
      } else {
        eq.apply(s);
        if (s.enabled) eq.resume();
      }
    }
    saveEqState(s);
  }

  /** Build the graph on the first play if EQ was left enabled, and resume the
     AudioContext (it starts suspended until a user gesture). */
  function ensureEq() {
    if (!audio) return;
    if (eqState.enabled && !eq.isBuilt) eq.attach(audio, eqState);
    if (eq.isBuilt) eq.resume();
  }


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
  let muted = $state(persisted?.muted ?? false);
  // Whether the current track's waveform loaded; drives the fall back to a plain range.
  let waveReady = $state(false);
  // Some sources hand out signed URLs that expire: allow exactly one silent re-resolve per track.
  let retriedCurrent = false;

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
          currentTime,
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
      if (!cancelled && current?.id === t.id) resolvedArt = a;
    });
    return () => {
      cancelled = true;
    };
  });

  // Report playback to the Last.fm scrobbler (no-op unless connected + enabled).
  $effect(() => {
    if (current) scrobbler.onProgress(current, currentTime, total);
  });

  // Media Session: shows the track on the lock screen / notification shade and
  // drives OS + headphone/Bluetooth transport controls and background playback.
  $effect(() => {
    if (!('mediaSession' in navigator)) return;
    const t = current;
    if (!t) {
      navigator.mediaSession.metadata = null;
      return;
    }
    const art = t.artwork ?? resolvedArt;
    navigator.mediaSession.metadata = new MediaMetadata({
      title: t.title,
      artist: t.artist,
      artwork: art ? [{ src: art, sizes: '512x512' }] : [],
    });
  });
  $effect(() => {
    if (!('mediaSession' in navigator)) return;
    navigator.mediaSession.playbackState = current ? (paused ? 'paused' : 'playing') : 'none';
  });
  $effect(() => {
    if (!('mediaSession' in navigator) || !navigator.mediaSession.setPositionState) return;
    if (!current || !(total > 0) || !Number.isFinite(currentTime)) return;
    try {
      navigator.mediaSession.setPositionState({
        duration: total,
        position: Math.min(currentTime, total),
        playbackRate: 1,
      });
    } catch {
      /* duration not settled yet — ignore */
    }
  });

  function message(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  /** Resolve a track's URL and start it. Shared by toggle, transport, and retry. */
  async function playTrack(track: PlayerTrack) {
    const el = audio;
    if (!el) return;
    resolvingId = track.id;
    resolvingSource = track.source;
    try {
      const src = await resolveSrc(track);
      current = track;
      retriedCurrent = false;
      currentTime = 0;
      duration = 0;
      srcUrl = src;
      el.src = src;
      ensureEq();
      el.play().catch(() => {});
      void scrobbler.onPlay(track);
    } catch (err: unknown) {
      onError?.(track, message(err));
    } finally {
      resolvingId = null;
      resolvingSource = undefined;
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
    if (pendingSeek != null) {
      el.currentTime = pendingSeek;
      pendingSeek = null;
    }
    if (resumeOnLoad) {
      resumeOnLoad = false;
      // Best-effort: autoplay without a gesture is blocked, so this may reject,
      // leaving the track loaded and paused at the saved position.
      el.play().catch(() => {});
    }
  }

  /** Reload the persisted track's audio (seeked, without auto-erroring). Unlike
     `playTrack`, a resolve failure here is silent: the track stays shown in the
     bar and pressing play surfaces the real error through the normal path. */
  async function restoreTrack(track: PlayerTrack, position: number, wasPaused: boolean) {
    const el = audio;
    if (!el) return;
    try {
      const src = await resolveSrc(track);
      if (current?.id !== track.id) return; // user already started something else
      srcUrl = src;
      pendingSeek = position > 0 ? position : null;
      resumeOnLoad = !wasPaused;
      el.src = src;
    } catch {
      /* track no longer resolvable (deleted/moved): leave it shown, paused */
    }
  }

  onMount(() => {
    scrobbler.flushQueue();

    if ('mediaSession' in navigator) {
      const ms = navigator.mediaSession;
      const set = (action: MediaSessionAction, handler: MediaSessionActionHandler) => {
        try {
          ms.setActionHandler(action, handler);
        } catch {
          /* action unsupported on this browser */
        }
      };
      set('play', () => void audio?.play());
      set('pause', () => audio?.pause());
      set('previoustrack', () => prev());
      set('nexttrack', () => next());
      set('seekbackward', (d) => seekTo(Math.max(0, currentTime - (d.seekOffset ?? 10))));
      set('seekforward', (d) => seekTo(currentTime + (d.seekOffset ?? 10)));
      set('seekto', (d) => {
        if (d.seekTime != null) seekTo(d.seekTime);
      });
      set('stop', () => audio?.pause());
    }
    const save = () => writeSnapshot();
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') writeSnapshot();
    };
    window.addEventListener('pagehide', save);
    document.addEventListener('visibilitychange', onVisibility);

    if (persisted?.current) {
      restoreTrack(persisted.current, persisted.currentTime ?? 0, persisted.paused ?? true);
    }

    return () => {
      window.removeEventListener('pagehide', save);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  });

  /** Rebuild the play order from the current queue, shuffle flag, and qIndex. */
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

  /** Play the track at position `pos` within the current play order. */
  function playAt(pos: number) {
    if (pos < 0 || pos >= order.length) return;
    orderPos = pos;
    qIndex = order[pos];
    playTrack(queue[qIndex]);
  }

  export async function toggle(track: PlayerTrack, q?: PlayerTrack[]) {
    const el = audio;
    if (!el) return;

    // Adopt the caller's list as the queue so prev/next/shuffle have context.
    if (q && q.length) {
      queue = q;
      const idx = q.findIndex((t) => t.id === track.id);
      qIndex = idx >= 0 ? idx : 0;
    } else {
      const idx = queue.findIndex((t) => t.id === track.id);
      if (idx >= 0) qIndex = idx;
      else {
        queue = [track];
        qIndex = 0;
      }
    }

    rebuildOrder();

    if (current?.id === track.id) {
      if (paused) el.play().catch(() => {});
      else el.pause();
      return;
    }
    await playTrack(track);
  }

  /** Step within the play order. Shuffle is baked into `order`, so this is a
     simple positional move in both modes — and it matches the shown queue. */
  function stepTo(delta: number) {
    const n = order.length;
    if (!n) return;
    let p = orderPos + delta;
    if (p < 0) p = repeat === 'all' ? n - 1 : 0;
    else if (p >= n) p = repeat === 'all' ? 0 : n - 1;
    playAt(p);
  }

  function next() {
    stepTo(1);
  }
  function prev() {
    // Restart the track first if we're past the intro, like every real player.
    if ((audio?.currentTime ?? 0) > 3) {
      if (audio) audio.currentTime = 0;
      return;
    }
    stepTo(-1);
  }
  function toggleShuffle() {
    shuffle = !shuffle;
    rebuildOrder();
  }
  function cycleRepeat() {
    repeat = repeat === 'off' ? 'all' : repeat === 'all' ? 'one' : 'off';
  }

  /** Auto-advance when a track finishes, honoring repeat and the play order. */
  function onEndedInternal() {
    const n = order.length;
    if (repeat === 'one') {
      if (audio) {
        audio.currentTime = 0;
        audio.play().catch(() => {});
      }
      return;
    }
    if (orderPos + 1 < n) {
      playAt(orderPos + 1);
      return;
    }
    if (repeat === 'all' && n > 0) {
      playAt(0);
      return;
    }
    onEnded?.();
  }

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
    if (!track || !el) return;

    if (retriedCurrent) {
      el.pause();
      onError?.(track, 'Playback failed. The audio link could not be refreshed.');
      return;
    }

    retriedCurrent = true;
    try {
      const src = await resolveSrc(track);
      if (current?.id !== track.id) return;
      el.src = src;
      el.play().catch(() => {});
    } catch (err: unknown) {
      el.pause();
      onError?.(track, message(err));
    }
  }

  function seekTo(secs: number) {
    if (audio) audio.currentTime = secs;
  }

  function togglePlay() {
    if (!audio) return;
    ensureEq();
    if (audio.paused) audio.play().catch(() => {});
    else audio.pause();
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
  const reduceMotion =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  function openNP() {
    // Only a listening affordance on phones; the desktop bar is already complete.
    if (typeof window !== 'undefined' && window.innerWidth > 860) return;
    if (current) expanded = true;
  }
  function closeNP() {
    expanded = false;
    dragY = 0;
  }

  // Swipe-down-to-dismiss: track 1:1, project momentum on release (apple-design),
  // then either fall closed or spring back. Springs/settle handled by CSS.
  let dragStartY = 0;
  let lastY = 0;
  let lastT = 0;
  let velY = 0;
  function onSheetPointerDown(e: PointerEvent) {
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
    dragging = false;
    const projected = dragY + velY * 0.12;
    if (projected > (sheetH || 500) * 0.3 || velY > 900) closeNP();
    else dragY = 0;
  }

  // ── Like / dislike the current library track (Now Playing only) ────────────
  let currentLibTrack = $derived.by(() => {
    const c = current;
    if (!c || c.source !== 'library') return null;
    return lib.tracks.find((t) => t.id === c.id) ?? null;
  });
  function rateCurrent(rating: 'liked' | 'disliked') {
    const t = currentLibTrack;
    if (t) lib.setRating(t, t.rating === rating ? null : rating);
  }

  onDestroy(() => audio?.pause());
</script>

<!-- Inline player: fills the shell's bottom bar. Built on a plain bound <audio>
  plus our own controls, so the bar is a real CSS grid we fully control.
  (media-chrome's <media-controller> slotted children into its shadow DOM and
  ignored our grid, which broke the 3-column layout.) -->
<div class="player" class:idle={!current}>
  <audio
    bind:this={audio}
    bind:paused
    bind:currentTime
    bind:duration
    bind:volume
    bind:muted
    onerror={onAudioError}
    onended={onEndedInternal}
    onloadedmetadata={onLoadedMetadata}
  ></audio>

  {#if current}
    <div
      class="pl-left"
      role="button"
      tabindex="0"
      onclick={openNP}
      onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openNP(); } }}
    >
      <div class="player-thumb">
        {#if current.artwork || resolvedArt}
          <img src={current.artwork ?? resolvedArt} alt="" />
        {:else}
          <div class="cover-ph"><i class="lni lni-music-note"></i></div>
        {/if}
      </div>
      <div class="player-info">
        <span class="title">{current.title}</span>
        <span class="artist">{current.artist}</span>
      </div>
    </div>

    <div class="pl-center">
      <div class="transport">
        <button class="tbtn shuffle" class:on={shuffle} onclick={toggleShuffle} disabled={!canStep} title="Shuffle" aria-label="Shuffle" aria-pressed={shuffle}><i class="lni lni-shuffle"></i></button>
        <button class="tbtn" onclick={prev} disabled={!canStep} title="Previous" aria-label="Previous"><i class="lni lni-backward"></i></button>
        <button class="play" onclick={togglePlay} aria-label={paused ? 'Play' : 'Pause'}><i class="lni {paused ? 'lni-play' : 'lni-pause'}"></i></button>
        <button class="tbtn" onclick={next} disabled={!canStep} title="Next" aria-label="Next"><i class="lni lni-forward"></i></button>
        <button class="tbtn repeat" class:on={repeat !== 'off'} onclick={cycleRepeat} title={'Repeat: ' + repeat} aria-label="Repeat"><i class="lni lni-repeat-1"></i>{#if repeat === 'one'}<span class="rep-one">1</span>{/if}</button>
      </div>

      <div class="progress-row">
        <span class="time">{formatTime(currentTime)}</span>
        {#if waveUrl || srcUrl}
          <div class="wave-slot" class:ready={waveReady}>
            <Waveform waveformUrl={waveUrl} srcUrl={waveUrl ? null : srcUrl} currentTime={currentTime} duration={total} onSeek={seekTo} bind:available={waveReady} />
          </div>
        {/if}
        {#if !waveReady}
          <input class="range" type="range" min="0" max={total || 0} step="0.1" value={currentTime} oninput={(e) => seekTo(+e.currentTarget.value)} aria-label="Seek" />
        {/if}
        <span class="time dur">{formatTime(total)}</span>
      </div>
    </div>

    <div class="pl-right">
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
          <i class="lni lni-sliders-triple-vertical-1"></i>
        </button>
        {#if eqOpen && !expanded}
          <button
            class="eq-backdrop"
            aria-label="Close equalizer"
            onclick={() => (eqOpen = false)}
            use:portal
          ></button>
          <div class="eq-pop" style={eqStyle} use:portal>
            <EqPanel bind:state={eqState} onUpdate={handleEqUpdate} />
          </div>
        {/if}
      </div>
      <button class="mute" onclick={() => (muted = !muted)} aria-label={muted ? 'Unmute' : 'Mute'}>
        <i class="lni {muted || volume === 0 ? 'lni-volume-off' : volume < 0.5 ? 'lni-volume-low' : 'lni-volume-high'}"></i>
      </button>
      <input class="volume" type="range" min="0" max="1" step="0.01" bind:value={volume} aria-label="Volume" />
    </div>
  {:else}
    <div class="pl-idle">
      <i class="lni lni-music-note"></i>
      <span>Nothing playing</span>
    </div>
  {/if}
</div>

{#if current}
  <!-- Mobile Now Playing: full-screen sheet that slides up from the bar. -->
  <div
    class="np"
    class:open={expanded}
    class:dragging
    bind:clientHeight={sheetH}
    style="transform: translateY({dragging ? dragY + 'px' : expanded ? '0px' : '100%'}); opacity: {reduceMotion ? (expanded ? 1 : 0) : 1}; transition: {dragging ? 'none' : reduceMotion ? 'opacity .2s ease' : 'transform .34s cubic-bezier(.32,.72,0,1)'}; pointer-events: {expanded ? 'auto' : 'none'}"
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="np-head"
      onpointerdown={onSheetPointerDown}
      onpointermove={onSheetPointerMove}
      onpointerup={onSheetPointerUp}
      onpointercancel={onSheetPointerUp}
    >
      <button class="np-close" onclick={closeNP} aria-label="Close now playing"><i class="lni lni-chevron-down"></i></button>
      <div class="np-grabber"></div>
    </div>

    <div class="np-art">
      {#if current.artwork || resolvedArt}
        <img src={current.artwork ?? resolvedArt} alt="" />
      {:else}
        <div class="cover-ph"><i class="lni lni-music-note"></i></div>
      {/if}
    </div>

    <div class="np-meta">
      <div class="np-title">{current.title}</div>
      <div class="np-artist">{current.artist}</div>
    </div>

    <div class="np-scrub">
      {#if waveUrl || srcUrl}
        <div class="wave-slot" class:ready={waveReady}>
          <Waveform waveformUrl={waveUrl} srcUrl={waveUrl ? null : srcUrl} currentTime={currentTime} duration={total} onSeek={seekTo} bind:available={waveReady} />
        </div>
      {/if}
      {#if !waveReady}
        <input class="range" type="range" min="0" max={total || 0} step="0.1" value={currentTime} oninput={(e) => seekTo(+e.currentTarget.value)} aria-label="Seek" />
      {/if}
      <div class="np-times"><span>{formatTime(currentTime)}</span><span>{formatTime(total)}</span></div>
    </div>

    <div class="np-transport">
      <button class="tbtn shuffle" class:on={shuffle} onclick={toggleShuffle} disabled={!canStep} aria-label="Shuffle"><i class="lni lni-shuffle"></i></button>
      <button class="tbtn" onclick={prev} disabled={!canStep} aria-label="Previous"><i class="lni lni-backward"></i></button>
      <button class="np-play" onclick={togglePlay} aria-label={paused ? 'Play' : 'Pause'}><i class="lni {paused ? 'lni-play' : 'lni-pause'}"></i></button>
      <button class="tbtn" onclick={next} disabled={!canStep} aria-label="Next"><i class="lni lni-forward"></i></button>
      <button class="tbtn repeat" class:on={repeat !== 'off'} onclick={cycleRepeat} aria-label="Repeat"><i class="lni lni-repeat-1"></i>{#if repeat === 'one'}<span class="rep-one">1</span>{/if}</button>
    </div>

    <div class="np-secondary">
      {#if currentLibTrack}
        <button class="btn-rate" class:active-like={currentLibTrack.rating === 'liked'} onclick={() => rateCurrent('liked')} aria-label="Like"><i class="lni lni-thumbs-up-1"></i></button>
        <button class="btn-rate" class:active-dislike={currentLibTrack.rating === 'disliked'} onclick={() => rateCurrent('disliked')} aria-label="Dislike"><i class="lni lni-thumbs-down-1"></i></button>
      {/if}
      <button class="eq-btn" class:on={eqState.enabled} onclick={() => (eqOpen = !eqOpen)} aria-label="Equalizer"><i class="lni lni-sliders-triple-vertical-1"></i></button>
      <button class="mute" onclick={() => (muted = !muted)} aria-label={muted ? 'Unmute' : 'Mute'}><i class="lni {muted || volume === 0 ? 'lni-volume-off' : volume < 0.5 ? 'lni-volume-low' : 'lni-volume-high'}"></i></button>
    </div>
    <input class="volume np-vol" type="range" min="0" max="1" step="0.01" bind:value={volume} aria-label="Volume" />

    {#if eqOpen && expanded}
      <div class="np-eq"><EqPanel bind:state={eqState} onUpdate={handleEqUpdate} /></div>
    {/if}

    {#if upNext.length > 0}
      <div class="np-queue">
        <div class="np-queue-head">Up next</div>
        {#each upNext.slice(0, 20) as q}
          <div class="np-q-row">
            <div class="np-q-art" style={q.artwork ? `background-image:url(${q.artwork})` : ''}>
              {#if !q.artwork}<i class="lni lni-music-note"></i>{/if}
            </div>
            <div class="np-q-meta">
              <div class="np-q-title">{q.title}</div>
              <div class="np-q-artist">{q.artist}</div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  /* Fills the shell's bottom player bar (App.svelte owns the bar background). */
  .player {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 24px;
    padding: 0 24px;
  }
  .player.idle { display: flex; align-items: center; justify-content: center; }

  .pl-idle {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--muted-2);
    font-size: 13px;
    font-weight: 600;
  }
  .pl-idle .lni { font-size: 18px; }

  /* ── Left: track identity ────────────────────────────────────────────── */
  .pl-left {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .player-thumb {
    width: 60px;
    height: 60px;
    border-radius: 8px;
    overflow: hidden;
    background: var(--surface-2);
    flex-shrink: 0;
  }
  .player-thumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .cover-ph {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted-2);
    background: linear-gradient(135deg, #241f33, #15131c);
  }
  .cover-ph .lni { font-size: 22px; }
  .player-info { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .title {
    font-size: 14px;
    font-weight: 700;
    color: var(--text-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .artist {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ── Center: transport + progress ────────────────────────────────────── */
  .pl-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: min(620px, 46vw);
  }
  .transport { display: flex; align-items: center; gap: 16px; }
  .tbtn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    padding: 4px;
    position: relative;
    transition: color 0.12s, transform 0.12s;
  }
  .tbtn .lni { font-size: 18px; }
  .tbtn:hover:not(:disabled) { color: var(--text-bright); transform: scale(1.08); }
  .tbtn:disabled { opacity: 0.35; cursor: default; }
  .tbtn.on { color: var(--accent); }
  .tbtn.repeat .rep-one {
    position: absolute;
    top: -2px;
    right: -1px;
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 600;
    color: var(--accent);
  }

  .play {
    width: 46px;
    height: 46px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    border: none;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    flex-shrink: 0;
    transition: transform 0.12s, filter 0.12s;
  }
  .play:hover { transform: scale(1.05); filter: brightness(1.08); }
  .play .lni { font-size: 18px; line-height: 1; }

  .progress-row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
  }
  .time {
    color: var(--muted-2);
    font-family: var(--font-mono);
    font-size: 11px;
    flex-shrink: 0;
    min-width: 34px;
  }
  .time.dur { text-align: right; }
  .wave-slot {
    flex: 1;
    min-width: 0;
    height: 30px;
    display: flex;
    align-items: center;
  }

  /* ── Right: volume ───────────────────────────────────────────────────── */
  .pl-right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }
  .mute {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    padding: 4px;
    display: flex;
    align-items: center;
  }
  .mute:hover { color: var(--text-bright); }
  .mute .lni { font-size: 17px; }

  /* ── Equalizer button + popover ──────────────────────────────────────── */
  .eq-wrap { position: relative; display: flex; align-items: center; }
  .eq-btn {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    padding: 4px;
    display: flex;
    align-items: center;
  }
  .eq-btn:hover { color: var(--text-bright); }
  .eq-btn.on { color: var(--accent); }
  .eq-btn .lni { font-size: 17px; }
  /* Portalled to <body>, so positioned via viewport-fixed inline coords. This
     escapes the player bar's `overflow: hidden` (which was clipping it). */
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
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
  }

  /* Native range inputs (seek + volume), themed to the violet accent. */
  .range, .volume {
    -webkit-appearance: none;
    appearance: none;
    height: 4px;
    border-radius: 999px;
    background: var(--surface-2);
    cursor: pointer;
    outline: none;
  }
  .range { flex: 1; min-width: 0; }
  .volume { width: 96px; flex-shrink: 0; }
  .range::-webkit-slider-thumb,
  .volume::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
  }
  .range::-moz-range-thumb,
  .volume::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border: none;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
  }

  @media (max-width: 860px) {
    .player { grid-template-columns: auto 1fr; gap: 12px; padding: 0 14px; }
    .pl-center { width: auto; }
    .progress-row, .pl-right { display: none; }
    .player .shuffle, .player .repeat { display: none; }
    .player-thumb { width: 48px; height: 48px; }
  }

  /* ── Mobile Now Playing (full-screen sheet) ────────────────────────────── */
  .np {
    display: none;
    position: fixed;
    inset: 0;
    z-index: 300;
    flex-direction: column;
    align-items: center;
    background: var(--bg);
    padding: calc(env(safe-area-inset-top) + 6px) 22px calc(env(safe-area-inset-bottom) + 20px);
    box-sizing: border-box;
    overflow-y: auto;
    will-change: transform;
  }
  .np-head {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    padding: 6px 0 2px;
    flex-shrink: 0;
    touch-action: none;
    cursor: grab;
  }
  .np-grabber { width: 40px; height: 5px; border-radius: 999px; background: var(--surface-2); }
  .np-close {
    position: absolute;
    left: -6px;
    top: 0;
    background: none;
    border: none;
    color: var(--muted);
    font-size: 24px;
    cursor: pointer;
    padding: 4px 8px;
  }
  .np-art {
    width: min(72vw, 340px);
    aspect-ratio: 1;
    border-radius: 16px;
    overflow: hidden;
    background: var(--surface-2);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
    margin-top: 5vh;
    flex-shrink: 0;
  }
  .np-art img { width: 100%; height: 100%; object-fit: cover; }
  .np-art .cover-ph { font-size: 64px; color: var(--muted-2); }
  .np-meta { width: 100%; text-align: center; margin-top: 20px; }
  .np-title {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 1.35rem;
    letter-spacing: -0.02em;
    color: var(--text-bright);
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .np-artist { color: var(--muted); font-size: 0.95rem; margin-top: 5px; }
  .np-scrub { width: 100%; margin-top: 18px; }
  .np-scrub .wave-slot { width: 100%; }
  .np-times {
    display: flex;
    justify-content: space-between;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--muted);
    margin-top: 6px;
  }
  .np-transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 20px;
    margin-top: 16px;
  }
  .np-transport .tbtn {
    position: relative;
    background: none;
    border: none;
    color: var(--text);
    font-size: 22px;
    cursor: pointer;
    padding: 6px;
  }
  .np-transport .tbtn:disabled { opacity: 0.35; cursor: default; }
  .np-transport .tbtn.on { color: var(--accent); }
  .np-play {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: var(--text-bright);
    color: var(--bg);
    border: none;
    font-size: 26px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .np-secondary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 20px;
    margin-top: 16px;
  }
  .np-secondary .btn-rate { font-size: 20px; }
  .np-secondary .eq-btn,
  .np-secondary .mute {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 19px;
    cursor: pointer;
    padding: 4px;
  }
  .np-secondary .eq-btn.on { color: var(--accent); }
  .np-vol { width: min(80%, 300px); margin-top: 10px; }
  .np-eq { width: 100%; margin-top: 14px; }
  .np-queue { width: 100%; margin-top: 22px; }
  .np-queue-head {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    margin-bottom: 10px;
  }
  .np-q-row { display: flex; align-items: center; gap: 10px; padding: 6px 0; }
  .np-q-art {
    width: 38px;
    height: 38px;
    border-radius: 6px;
    background: var(--surface-2) center/cover no-repeat;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted-2);
  }
  .np-q-meta { min-width: 0; }
  .np-q-title { font-size: 0.9rem; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .np-q-artist { font-size: 0.78rem; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  @media (max-width: 860px) {
    .np { display: flex; }
    .pl-left { cursor: pointer; }
  }
</style>
