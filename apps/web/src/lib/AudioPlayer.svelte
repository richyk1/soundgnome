<script module lang="ts">
  /** Minimal description of whatever is playing, built by the calling page. */
  export interface PlayerTrack {
    id: number;
    title: string;
    artist: string;
    artwork: string | null;
    durationSecs: number | null;
    /** Optional precomputed waveform peaks url (SoundCloud), enables the scrubber. */
    waveformUrl?: string | null;
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
  import 'media-chrome';
  import Waveform from './Waveform.svelte';

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
  // Whether the current track's waveform loaded; drives the fall back to a plain range.
  let waveReady = $state(false);
  // Some sources hand out signed URLs that expire: allow exactly one silent re-resolve per track.
  let retriedCurrent = false;

  let total = $derived(
    Number.isFinite(duration) && duration > 0 ? duration : (current?.durationSecs ?? 0),
  );
  let waveUrl = $derived(current?.waveformUrl ?? null);

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

  function seekTo(secs: number) {
    if (audio) audio.currentTime = secs;
  }

  onDestroy(() => audio?.pause());
</script>

<!--
  Our own markup lives outside <media-control-bar>: media-chrome styles its
  slotted children as controls, which flattened the title block to 2px tall.
-->
<div class="player" class:visible={current}>
<media-controller audio class="mc">
  <audio
    slot="media"
    bind:this={audio}
    bind:paused
    bind:currentTime
    bind:duration
    onerror={onAudioError}
    onended={() => onEnded?.()}
  ></audio>

  {#if current}
    <media-control-bar class="bar">
      <media-play-button class="play"></media-play-button>

      <media-time-display class="time" showduration></media-time-display>

      {#if waveUrl}
        <div class="wave-slot" class:ready={waveReady}>
          <Waveform
            waveformUrl={waveUrl}
            currentTime={currentTime}
            duration={total}
            onSeek={seekTo}
            bind:available={waveReady}
          />
        </div>
      {/if}
      {#if !waveUrl || !waveReady}
        <media-time-range class="range"></media-time-range>
      {/if}

      <media-mute-button class="mute"></media-mute-button>
      <media-volume-range class="volume"></media-volume-range>
    </media-control-bar>
  {/if}
</media-controller>

{#if current}
  <div class="player-meta">
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
  </div>
{/if}
</div>

<style>
  .player {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 100;
    display: none;
    width: 100%;
    background: color-mix(in srgb, var(--surface) 82%, transparent);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    border-top: 1px solid var(--border);
    box-shadow: 0 -6px 26px rgba(0, 0, 0, 0.4);

    /* Map media-chrome to the app theme. */
    --media-primary-color: var(--text);
    --media-secondary-color: transparent;
    --media-control-background: transparent;
    --media-control-hover-background: var(--surface-2);
    --media-font-family: inherit;
    --media-font-size: 0.75rem;
    --media-font-weight: 500;
    --media-control-height: auto;
    --media-range-track-height: 4px;
    --media-range-track-border-radius: 3px;
    --media-range-track-background: color-mix(in srgb, var(--muted) 32%, transparent);
    --media-range-bar-color: var(--accent);
    --media-time-range-buffered-color: color-mix(in srgb, var(--muted) 22%, transparent);
    --media-range-thumb-background: var(--accent);
    --media-range-thumb-width: 12px;
    --media-range-thumb-height: 12px;
    --media-tooltip-display: none;
  }

  .player.visible {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 1rem;
    box-sizing: border-box;
  }

  /* Track identity sits before the controls and keeps a fixed footprint so the
     transport does not shift when a long title loads. */
  .player-meta {
    order: -1;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 16rem;
    flex: 0 0 auto;
    min-width: 0;
  }

  .mc {
    flex: 1 1 auto;
    min-width: 0;
    background: transparent;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    box-sizing: border-box;
  }

  .player-thumb {
    width: 52px;
    height: 52px;
    flex-shrink: 0;
    border-radius: 6px;
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
  .cover-ph svg { width: 42%; height: 42%; }

  .player-info {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
    width: 12rem;
    flex-shrink: 0;
  }

  .title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artist {
    font-size: 0.78rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .play {
    flex-shrink: 0;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--accent);
    --media-primary-color: #fff;
    --media-control-hover-background: transparent;
    --media-button-icon-width: 18px;
    --media-button-icon-height: 18px;
    --media-button-padding: 0;
    cursor: pointer;
  }

  .play:hover {
    filter: brightness(1.12);
  }

  .time {
    flex-shrink: 0;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    padding: 0;
    background: transparent;
  }

  .wave-slot {
    flex: 0;
    width: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
  }

  .wave-slot.ready {
    flex: 1;
    width: auto;
    min-width: 5rem;
    overflow: visible;
  }

  .range {
    flex: 1;
    min-width: 5rem;
    height: 36px;
  }

  .mute {
    flex-shrink: 0;
    --media-button-icon-width: 18px;
    --media-button-icon-height: 18px;
    cursor: pointer;
  }

  .volume {
    flex-shrink: 0;
    width: 74px;
  }

  @media (max-width: 720px) {
    .player-info { width: 8rem; }
    .mute,
    .volume { display: none; }
  }

  @media (max-width: 480px) {
    .time { display: none; }
  }
</style>
