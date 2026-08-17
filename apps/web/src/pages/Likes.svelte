<script lang="ts">
  import { onMount, getContext } from 'svelte';
  import { getSoundcloudLikes, downloadUrl } from '../lib/api';
  import type { SoundcloudLikeDto } from '../lib/api';
  import { formatTime } from '../lib/AudioPlayer.svelte';
  import { GLOBAL_PLAYER, type GlobalPlayer, type PlayerTrack } from '../lib/player';

  let tracks: SoundcloudLikeDto[] = $state([]);
  let count = $state(0);
  let loading = $state(true);
  let error: string | null = $state(null);
  let search = $state('');

  // ── Playback: one shared player for every row ───────────────────────────────
  const player = getContext<GlobalPlayer>(GLOBAL_PLAYER);

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
      source: 'soundcloud',
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
    player?.play(playerTrack(track), tracks.map(playerTrack));
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

<div class="likes-page">
  <header class="page-header">
    <div class="header-text">
      <h1>Liked</h1>
      <p class="lede">Your SoundCloud liked tracks. Play any of them, or send one to your library.</p>
    </div>
    <div class="header-actions">
      {#if !loading && !error && tracks.length > 0}
        <div class="search-field">
          <i class="lni lni-search-1 field-icon" aria-hidden="true"></i>
          <input
            type="text"
            placeholder="Filter by title or artist"
            bind:value={search}
            aria-label="Filter liked tracks"
            autocomplete="off"
            spellcheck="false"
          />
        </div>
        <span class="count">{filtered.length} of {count}</span>
      {/if}
      <button class="btn-secondary btn-sm" onclick={load} disabled={loading}>
        {#if loading}
          <span class="spinner"></span>Loading
        {:else}
          <i class="lni lni-refresh-circle-1-clockwise" aria-hidden="true"></i>Refresh
        {/if}
      </button>
    </div>
  </header>

  {#if loading}
    <ul class="track-list" aria-hidden="true">
      {#each { length: 6 } as _}
        <li class="track-row skeleton">
          <div class="cover"></div>
          <div class="track-info">
            <span class="sk sk-title"></span>
            <span class="sk sk-sub"></span>
          </div>
        </li>
      {/each}
    </ul>
  {:else if error}
    <div class="callout callout-error" role="alert">
      <i class="lni lni-xmark-circle" aria-hidden="true"></i>
      <div class="callout-body">
        <strong>Couldn't load your likes.</strong>
        <span>{error}</span>
      </div>
    </div>
    <div class="empty">
      <i class="lni lni-soundcloud" aria-hidden="true"></i>
      <p class="empty-title">SoundCloud not connected</p>
      <p class="empty-hint">Open Tools, then Providers, to connect your SoundCloud account.</p>
    </div>
  {:else if tracks.length === 0}
    <div class="empty">
      <i class="lni lni-heart" aria-hidden="true"></i>
      <p class="empty-title">No liked tracks</p>
      <p class="empty-hint">Connect SoundCloud in Tools, then like tracks there to see them here.</p>
    </div>
  {:else if filtered.length === 0}
    <div class="empty">
      <i class="lni lni-search-1" aria-hidden="true"></i>
      <p class="empty-title">No matches</p>
      <p class="empty-hint">Nothing matches "{search}". Try a different title or artist.</p>
    </div>
  {:else}
    <ul class="track-list">
      {#each filtered as track (track.id)}
        {@const playing = player?.isPlaying(track.id, 'soundcloud')}
        {@const resolving = player?.isResolving(track.id, 'soundcloud')}
        <li
          class="track-row"
          class:current={player?.isCurrent(track.id, 'soundcloud')}
          class:playing
        >
          <div class="cover">
            {#if track.artwork_url}
              <img src={track.artwork_url} alt="" />
            {:else}
              <i class="lni lni-music-note cover-ph" aria-hidden="true"></i>
            {/if}
          </div>

          <div class="track-info">
            <span class="track-title">{track.title}</span>
            <span class="track-artists">{track.artist}</span>
            {#if rowErrors[track.id]}
              <span class="row-error">{rowErrors[track.id]}</span>
            {/if}
          </div>

          <span class="duration">{formatTime(track.duration_secs)}</span>

          <div class="row-actions">
            <button
              class="icon-btn"
              onclick={() => toggle(track)}
              disabled={resolving}
              title={playing ? 'Pause' : 'Play'}
              aria-label={playing ? 'Pause' : 'Play'}
            >
              {#if resolving}
                <span class="spinner"></span>
              {:else if playing}
                <i class="lni lni-pause" aria-hidden="true"></i>
              {:else}
                <i class="lni lni-play" aria-hidden="true"></i>
              {/if}
            </button>

            {#if queued[track.id]}
              <span class="icon-btn is-done" title="Queued" aria-label="Queued">
                <i class="lni lni-check" aria-hidden="true"></i>
              </span>
            {:else}
              <button
                class="icon-btn"
                onclick={() => download(track)}
                disabled={downloading[track.id]}
                title="Download"
                aria-label="Download"
              >
                {#if downloading[track.id]}
                  <span class="spinner"></span>
                {:else}
                  <i class="lni lni-cloud-download" aria-hidden="true"></i>
                {/if}
              </button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>


<style>
  .likes-page {
    width: 100%;
    box-sizing: border-box;
    padding: 1.5rem 2rem 6rem;
    display: flex;
    flex-direction: column;
    gap: 1.75rem;
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.85rem 1.25rem;
  }
  .header-text {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-width: 0;
  }
  h1 {
    font-size: 1.25rem;
    font-weight: 700;
    margin: 0;
  }
  @media (min-width: 768px) {
    h1 {
      font-size: 1.5rem;
    }
  }
  .lede {
    margin: 0;
    color: var(--muted);
    font-size: 0.95rem;
    line-height: 1.55;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .count {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--muted-2);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  /* ── Search field ────────────────────────────────────────────────────── */
  .search-field {
    display: flex;
    align-items: center;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding-left: 10px;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }
  .search-field:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .field-icon {
    font-size: 15px;
    color: var(--muted-2);
    flex-shrink: 0;
  }
  .search-field:focus-within .field-icon {
    color: var(--accent);
  }
  .search-field input {
    min-width: 0;
    width: 15rem;
    max-width: 100%;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
    padding: 0.45rem 0.65rem;
  }
  .search-field input::placeholder {
    color: var(--muted);
  }

  /* ── Buttons ─────────────────────────────────────────────────────────── */
  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.55rem 1rem;
    color: var(--text);
    font-family: inherit;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.12s ease;
  }
  .btn-secondary:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .btn-secondary:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn-sm {
    padding: 0.4rem 0.8rem;
    font-size: 0.82rem;
  }
  .btn-secondary .lni {
    font-size: 14px;
  }

  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* ── Status callout ──────────────────────────────────────────────────── */
  .callout {
    display: flex;
    align-items: flex-start;
    gap: 0.7rem;
    padding: 0.85rem 1rem;
    border-radius: 10px;
    border: 1px solid transparent;
    font-size: 0.9rem;
  }
  .callout .lni {
    font-size: 19px;
    flex-shrink: 0;
    line-height: 1.35;
  }
  .callout-body {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .callout-body strong {
    font-weight: 600;
    color: var(--text-bright);
  }
  .callout-error {
    background: var(--error-bg);
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
    color: var(--error);
  }

  /* ── Track list ──────────────────────────────────────────────────────── */
  .track-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .track-row {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    padding: 0.55rem 0.7rem;
    border-radius: 8px;
    border: 1px solid transparent;
    transition:
      background 0.1s ease,
      border-color 0.1s ease;
  }
  .track-row:not(.skeleton):hover {
    background: var(--surface);
  }
  .track-row.current {
    background: color-mix(in srgb, var(--accent) 10%, var(--surface));
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .cover {
    flex-shrink: 0;
    width: 44px;
    height: 44px;
    border-radius: 6px;
    overflow: hidden;
    background: linear-gradient(135deg, #241f33, #15131c);
    border: 1px solid var(--border-soft);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .cover-ph {
    font-size: 18px;
    color: var(--muted-2);
  }

  .track-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .track-title {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track-row.current .track-title {
    color: var(--accent-2);
  }
  .track-artists {
    font-size: 0.78rem;
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
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-shrink: 0;
  }
  .icon-btn {
    width: 32px;
    height: 32px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    transition:
      background 0.12s ease,
      color 0.12s ease,
      border-color 0.12s ease;
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--surface-2);
    border-color: var(--accent);
    color: var(--accent-2);
  }
  .icon-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .icon-btn .lni {
    font-size: 15px;
  }
  .track-row.playing .icon-btn:first-child {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .icon-btn.is-done {
    color: var(--success);
    border-color: color-mix(in srgb, var(--success) 45%, transparent);
    background: color-mix(in srgb, var(--success) 16%, var(--panel));
    cursor: default;
  }

  /* ── Empty state ─────────────────────────────────────────────────────── */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.4rem;
    padding: 2.5rem 1rem;
    color: var(--muted);
  }
  .empty .lni {
    font-size: 30px;
    color: var(--muted-2);
    margin-bottom: 0.3rem;
  }
  .empty-title {
    margin: 0;
    font-weight: 600;
    color: var(--text);
  }
  .empty-hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }

  /* ── Loading skeleton ────────────────────────────────────────────────── */
  .skeleton .cover {
    background: var(--surface);
    border-color: transparent;
  }
  .sk {
    height: 0.7rem;
    border-radius: 4px;
    background: var(--surface);
    animation: sk-pulse 1.3s ease-in-out infinite;
  }
  .sk-title {
    width: 40%;
  }
  .sk-sub {
    width: 24%;
    height: 0.6rem;
  }
  @keyframes sk-pulse {
    50% {
      opacity: 0.45;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner,
    .sk {
      animation: none;
    }
    .track-row,
    .icon-btn,
    .search-field,
    .btn-secondary {
      transition: none;
    }
  }

  @media (max-width: 640px) {
    .likes-page {
      padding: 1.25rem 1rem 5rem;
    }
    .search-field input {
      width: 100%;
    }
    .duration {
      display: none;
    }
  }
</style>
