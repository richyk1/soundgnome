<script module lang="ts">
  /** Minimal description of whatever is playing, built by the calling page. */
  export interface PlayerTrack {
    id: number;
    title: string;
    artist: string;
    artwork: string | null;
    durationSecs: number | null;
  }

  /** What a parent gets back through `bind:this`. */
  export interface PlayerHandle {
    toggle(track: PlayerTrack): Promise<void>;
    isCurrent(id: number): boolean;
    isPlaying(id: number): boolean;
    isResolving(id: number): boolean;
  }

  export function formatTime(secs: number | null | undefined): string {
    if (secs == null || !Number.isFinite(secs) || secs <= 0) return '0:00';
    const total = Math.floor(secs);
    return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
  }
</script>

<script lang="ts">
  import { onDestroy } from 'svelte';

  let {
    resolveSrc,
    onEnded,
    onError,
  }: {
    /** Returns a playable URL for a track. May be async when the URL has to be resolved. */
    resolveSrc: (track: PlayerTrack) => string | Promise<string>;
    onEnded?: () => void;
    onError?: (track: PlayerTrack, message: string) => void;
  } = $props();

  let audio: HTMLAudioElement | null = $state(null);
  let current: PlayerTrack | null = $state(null);
  let resolvingId: number | null = $state(null);
  let paused = $state(true);
  let currentTime = $state(0);
  let duration = $state(0);
  // Some sources hand out signed URLs that expire: allow exactly one silent re-resolve per track.
  let retriedCurrent = false;

  let total = $derived(
    Number.isFinite(duration) && duration > 0 ? duration : (current?.durationSecs ?? 0),
  );

  function message(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  export async function toggle(track: PlayerTrack) {
    const el = audio;
    if (!el) return;

    if (current?.id === track.id) {
      if (paused) {
        el.play().catch(() => {});
      } else {
        el.pause();
      }
      return;
    }

    resolvingId = track.id;
    try {
      const src = await resolveSrc(track);
      current = track;
      retriedCurrent = false;
      currentTime = 0;
      duration = 0;
      el.src = src;
      // Media failures surface through the error event, which re-resolves once.
      el.play().catch(() => {});
    } catch (err: unknown) {
      onError?.(track, message(err));
    } finally {
      resolvingId = null;
    }
  }

  export function isCurrent(id: number): boolean {
    return current?.id === id;
  }

  export function isPlaying(id: number): boolean {
    return current?.id === id && !paused;
  }

  export function isResolving(id: number): boolean {
    return resolvingId === id;
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

  onDestroy(() => audio?.pause());
</script>

<audio
  bind:this={audio}
  bind:paused
  bind:currentTime
  bind:duration
  onerror={onAudioError}
  onended={() => onEnded?.()}
></audio>

{#if current}
  <div class="player-bar">
    <div class="player-thumb">
      {#if current.artwork}
        <img src={current.artwork} alt="" />
      {:else}
        <div class="cover-ph">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
            <path d="M9 18V5l12-2v13" />
            <circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />
          </svg>
        </div>
      {/if}
    </div>

    <div class="player-info">
      <span class="title">{current.title}</span>
      <span class="artist">{current.artist}</span>
    </div>

    <button
      class="btn-play"
      onclick={() => current && toggle(current)}
      title={paused ? 'Play' : 'Pause'}
      aria-label={paused ? 'Play' : 'Pause'}
    >
      {#if paused}
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M8 5l11 7-11 7z" />
        </svg>
      {:else}
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <rect x="6" y="5" width="4" height="14" rx="1" />
          <rect x="14" y="5" width="4" height="14" rx="1" />
        </svg>
      {/if}
    </button>

    <span class="player-time">{formatTime(currentTime)}</span>
    <input
      class="seek"
      type="range"
      min="0"
      max={total}
      step="0.5"
      bind:value={currentTime}
      aria-label="Seek"
    />
    <span class="player-time">{formatTime(total)}</span>
  </div>
{/if}

<style>
  .player-bar {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.5rem 0.9rem;
    background: var(--surface);
    border-top: 1px solid var(--border);
    box-shadow: 0 -4px 22px rgba(0, 0, 0, 0.35);
  }

  .player-thumb {
    width: 38px;
    height: 38px;
    flex-shrink: 0;
    border-radius: 4px;
    overflow: hidden;
    background: var(--surface-2);
  }

  .player-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .cover-ph {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
  }
  .cover-ph svg { width: 45%; height: 45%; }

  .player-info {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
    width: 10rem;
    flex-shrink: 0;
  }

  .title {
    font-size: 0.85rem;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artist {
    font-size: 0.75rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .player-time {
    font-size: 0.72rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .seek {
    flex: 1;
    min-width: 4rem;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .btn-play {
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }

  .btn-play:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .btn-play svg { width: 15px; height: 15px; }

  @media (max-width: 640px) {
    .player-info { width: 6rem; }
  }
</style>
