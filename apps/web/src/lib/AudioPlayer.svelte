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
  let canStep = $derived(queue.length > 1);
  // Keep `active` in sync with whether a track is loaded.
  $effect(() => {
    active = current != null;
    upNext = qIndex >= 0 ? queue.slice(qIndex + 1) : [];
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
      el.play().catch(() => {});
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

  function playIndex(i: number) {
    if (i < 0 || i >= queue.length) return;
    qIndex = i;
    playTrack(queue[i]);
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

    if (current?.id === track.id) {
      if (paused) el.play().catch(() => {});
      else el.pause();
      return;
    }
    await playTrack(track);
  }

  /** Move within the queue. Shuffle picks a random other track; otherwise step
     linearly, wrapping only when repeat is 'all'. */
  function stepTo(delta: number) {
    const n = queue.length;
    if (!n) return;
    let i: number;
    if (shuffle && n > 1) {
      do {
        i = Math.floor(Math.random() * n);
      } while (i === qIndex);
    } else {
      i = qIndex + delta;
      if (i < 0) i = repeat === 'all' ? n - 1 : 0;
      else if (i >= n) i = repeat === 'all' ? 0 : n - 1;
    }
    playIndex(i);
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
  }
  function cycleRepeat() {
    repeat = repeat === 'off' ? 'all' : repeat === 'all' ? 'one' : 'off';
  }

  /** Auto-advance when a track finishes, honoring repeat/shuffle. */
  function onEndedInternal() {
    const n = queue.length;
    if (repeat === 'one') {
      if (audio) {
        audio.currentTime = 0;
        audio.play().catch(() => {});
      }
      return;
    }
    if (shuffle && n > 1) {
      stepTo(1);
      return;
    }
    if (qIndex + 1 < n) {
      playIndex(qIndex + 1);
      return;
    }
    if (repeat === 'all' && n > 0) {
      playIndex(0);
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
    if (audio.paused) audio.play().catch(() => {});
    else audio.pause();
  }

  /** Exposed on the handle so the shell can bind Space to play/pause. */
  export function playPause() {
    if (current) togglePlay();
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
    <div class="pl-left">
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
    .player-thumb { width: 48px; height: 48px; }
  }
</style>
