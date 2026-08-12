<script lang="ts">
  import { onMount } from 'svelte';
  import { getSoundcloudLikes, getSoundcloudStreamUrl, downloadUrl } from '../lib/api';
  import type { SoundcloudLikeDto } from '../lib/api';
  import AudioPlayer, {
    formatTime,
    type PlayerHandle,
    type PlayerTrack,
  } from '../lib/AudioPlayer.svelte';

  let tracks: SoundcloudLikeDto[] = $state([]);
  let count = $state(0);
  let loading = $state(true);
  let error: string | null = $state(null);
  let search = $state('');

  // ── Playback: one shared player for every row ───────────────────────────────
  let player: PlayerHandle | null = $state(null);

  // ── Per row state ───────────────────────────────────────────────────────────
  let downloading: Record<number, boolean> = $state({});
  let queued: Record<number, boolean> = $state({});
  let rowErrors: Record<number, string> = $state({});

  let query = $derived(search.trim().toLowerCase());
  let filtered = $derived(
    query === ''
      ? tracks
      : tracks.filter(
          (t) => t.title.toLowerCase().includes(query) || t.artist.toLowerCase().includes(query),
        ),
  );

  function message(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  function playerTrack(track: SoundcloudLikeDto): PlayerTrack {
    return {
      id: track.id,
      title: track.title,
      artist: track.artist,
      artwork: track.artwork_url,
      durationSecs: track.duration_secs,
      waveformUrl: track.waveform_url ?? null,
    };
  }

  async function load() {
    loading = true;
    error = null;
    try {
      const res = await getSoundcloudLikes();
      count = res.count;
      tracks = res.tracks;
    } catch (err: unknown) {
      error = message(err);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
  });

  function toggle(track: SoundcloudLikeDto) {
    delete rowErrors[track.id];
    player?.toggle(playerTrack(track));
  }

  function onPlaybackError(track: PlayerTrack, msg: string) {
    rowErrors[track.id] = msg;
  }

  async function download(track: SoundcloudLikeDto) {
    downloading[track.id] = true;
    delete rowErrors[track.id];
    try {
      await downloadUrl(track.permalink_url);
      queued[track.id] = true;
    } catch (err: unknown) {
      rowErrors[track.id] = message(err);
    } finally {
      delete downloading[track.id];
    }
  }
</script>

<div class="page-container likes-page">
  <div class="page-header">
    <h1>Likes</h1>
    <div class="header-right">
      {#if !loading && !error}
        <span class="count">{filtered.length} of {count} liked tracks</span>
      {/if}
      <button class="btn-header" onclick={load} disabled={loading}>
        {#if loading}
          <span class="spinner"></span> Loading
        {:else}
          Refresh
        {/if}
      </button>
    </div>
  </div>

  {#if loading}
    <p class="status">Loading your SoundCloud likes. This can take a few seconds.</p>
  {:else if error}
    <p class="status error">{error}</p>
    <p class="status-hint">Open Tools, then Providers, to connect SoundCloud.</p>
  {:else if tracks.length === 0}
    <p class="status">No liked tracks found on this account.</p>
  {:else}
    <div class="toolbar">
      <input class="search" type="text" placeholder="Filter by title or artist" bind:value={search} />
    </div>

    {#if filtered.length === 0}
      <p class="status">No liked track matches "{search}".</p>
    {:else}
      <ul class="rows">
        {#each filtered as track (track.id)}
          <li class="row" class:current={player?.isCurrent(track.id)}>
            <div class="thumb">
              {#if track.artwork_url}
                <img src={track.artwork_url} alt="" />
              {:else}
                <div class="cover-ph">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                    <path d="M9 18V5l12-2v13" />
                    <circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />
                  </svg>
                </div>
              {/if}
            </div>

            <div class="info">
              <span class="title">{track.title}</span>
              <span class="artist">{track.artist}</span>
              {#if rowErrors[track.id]}
                <span class="row-error">{rowErrors[track.id]}</span>
              {/if}
            </div>

            <span class="duration">{formatTime(track.duration_secs)}</span>

            <button
              class="btn-play"
              onclick={() => toggle(track)}
              disabled={player?.isResolving(track.id)}
              title={player?.isPlaying(track.id) ? 'Pause' : 'Play'}
              aria-label={player?.isPlaying(track.id) ? 'Pause' : 'Play'}
            >
              {#if player?.isResolving(track.id)}
                <span class="spinner"></span>
              {:else if player?.isPlaying(track.id)}
                <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <rect x="6" y="5" width="4" height="14" rx="1" />
                  <rect x="14" y="5" width="4" height="14" rx="1" />
                </svg>
              {:else}
                <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <path d="M8 5l11 7-11 7z" />
                </svg>
              {/if}
            </button>

            {#if queued[track.id]}
              <span class="queued">Queued</span>
            {:else}
              <button class="btn-download" onclick={() => download(track)} disabled={downloading[track.id]}>
                {#if downloading[track.id]}
                  <span class="spinner"></span> Sending
                {:else}
                  Download
                {/if}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<AudioPlayer
  bind:this={player}
  resolveSrc={(track) => getSoundcloudStreamUrl(track.id)}
  onError={onPlaybackError}
/>

<style>
  .likes-page {
    padding-bottom: 6rem;
  }

  .count {
    white-space: nowrap;
  }

  .status-hint {
    font-size: 0.8rem;
    color: var(--muted);
    margin: 0.25rem 0 0;
    text-align: center;
  }

  .spinner {
    display: inline-block;
    width: 11px;
    height: 11px;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Rows ─────────────────────────────────────────────────────────────── */
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.45rem 0.6rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  .row.current {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 9%, var(--surface));
  }

  .thumb {
    width: 40px;
    height: 40px;
    flex-shrink: 0;
    border-radius: 4px;
    overflow: hidden;
    background: var(--surface-2);
  }

  .thumb img {
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

  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
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

  .row-error {
    font-size: 0.72rem;
    color: var(--error);
  }

  .duration {
    font-size: 0.75rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
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

  .btn-play:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .btn-play:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-play svg { width: 15px; height: 15px; }

  .btn-download {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    flex-shrink: 0;
    padding: 0.28rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--text);
    font-size: 0.75rem;
    font-family: inherit;
    cursor: pointer;
    white-space: nowrap;
  }

  .btn-download:hover:not(:disabled) { background: var(--surface); }
  .btn-download:disabled { opacity: 0.6; cursor: default; }

  .queued {
    flex-shrink: 0;
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--accent);
    padding: 0.28rem 0.5rem;
    white-space: nowrap;
  }

  @media (max-width: 640px) {
    .duration { display: none; }
  }
</style>
