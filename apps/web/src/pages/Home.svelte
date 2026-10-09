<script lang="ts">
  import { onMount } from 'svelte';
  import { downloadUrl, getRecentTracks, getProviders } from '../lib/api';
  import type { DownloadResult, RecentTrack } from '../lib/api';
  import PixelCover from '../lib/PixelCover.svelte';
  import { trackCoverSeed } from '../lib/pixel-art';

  let { onNavigateTasks = undefined }: { onNavigateTasks?: () => void } = $props();

  let url = $state('');
  let loading = $state(false);
  let result: DownloadResult | null = $state(null);
  let error: string | null = $state(null);

  let recentTracks: RecentTrack[] = $state([]);
  let recentLoading = $state(true);
  let providers: string[] = $state([]);

  let visibleRecent = $derived(recentTracks.filter((t) => !t.needs_validation));

  async function loadRecent() {
    try {
      recentTracks = await getRecentTracks(20);
    } catch {
      // silently fail if API not up
    } finally {
      recentLoading = false;
    }
  }

  onMount(() => {
    loadRecent();
    getProviders().then((p) => {
      providers = p;
    });
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (!url.trim()) return;
    loading = true;
    result = null;
    error = null;
    try {
      result = await downloadUrl(url.trim());
      url = '';
      await loadRecent();
    } catch (err: unknown) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  function formatDuration(s: number | null): string {
    if (!s) return '';
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  }

  function coverUrl(t: RecentTrack): string | null {
    return t.cover && /^(https?:\/\/|\/)/.test(t.cover) ? t.cover : null;
  }
</script>

<div class="download-page">
  <header class="page-head">
    <h1>Download</h1>
    <p class="lede">Add a track, album, or playlist to your library.</p>
  </header>

  <form class="downloader" onsubmit={handleSubmit}>
    <div class="url-field">
      <i class="pxi pxi-download field-icon" aria-hidden="true"></i>
      <input
        type="url"
        placeholder="Paste a music link"
        bind:value={url}
        disabled={loading}
        autocomplete="off"
        spellcheck="false"
        aria-label="Track, album, or playlist URL"
      />
    </div>
    <button type="submit" class="btn-save download-btn" disabled={loading || !url.trim()}>
      {#if loading}
        <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Downloading
      {:else}
        <i class="pxi pxi-download" aria-hidden="true"></i>Download
      {/if}
    </button>

    {#if providers.length > 0}
      <div class="sources">
        <span class="sources-label">Works with</span>
        {#each providers as platform}
          <span class="source-tag">{platform}</span>
        {/each}
      </div>
    {/if}
  </form>

  {#if error}
    <div class="callout callout-error" role="alert">
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>
      <div class="callout-body">
        <strong>Couldn't download that link.</strong>
        <span>{error}</span>
      </div>
    </div>
  {/if}

  {#if result && !loading}
    {#if result.type === 'track'}
      <div class="callout callout-success" role="status">
        <i class="pxi pxi-check" aria-hidden="true"></i>
        <div class="callout-body">
          <strong>Added {result.title}</strong>
          <span>
            {#if result.artists.length}{result.artists.join(', ')}{/if}
            {#if result.needs_validation}<span class="review-tag">Needs review</span>{/if}
          </span>
        </div>
      </div>
    {:else}
      <div class="callout callout-success" role="status">
        <i class="pxi pxi-check" aria-hidden="true"></i>
        <div class="callout-body">
          <strong>Playlist syncing</strong>
          <span>
            Task <span class="mono">#{result.task_id}</span> is running.
            {#if onNavigateTasks}
              <button type="button" class="link-inline" onclick={onNavigateTasks}>Track its progress</button>
            {/if}
          </span>
        </div>
      </div>
    {/if}
  {/if}

  <section class="recent" aria-labelledby="recent-heading">
    <div class="recent-header">
      <h2 id="recent-heading">
        Recent downloads
        {#if !recentLoading && visibleRecent.length > 0}<span class="recent-count">{visibleRecent.length}</span>{/if}
      </h2>
      {#if onNavigateTasks}
        <button type="button" class="link-btn" onclick={onNavigateTasks}>
          Activity<i class="pxi pxi-arrow-right" aria-hidden="true"></i>
        </button>
      {/if}
    </div>

    {#if recentLoading}
      <ul class="track-list" aria-hidden="true">
        {#each { length: 5 } as _}
          <li class="track-row skeleton">
            <div class="cover-wrap row-cover"></div>
            <div class="track-info">
              <span class="sk sk-title"></span>
              <span class="sk sk-sub"></span>
            </div>
          </li>
        {/each}
      </ul>
    {:else if visibleRecent.length === 0}
      <div class="empty">
        <i class="pxi pxi-music" aria-hidden="true"></i>
        <p class="empty-title">No downloads yet</p>
        <p class="empty-hint">Paste a link above and it will show up here.</p>
      </div>
    {:else}
      <ul class="track-list">
        {#each visibleRecent as track (track.id)}
          <li class="track-row">
            <div class="cover-wrap row-cover">
              <PixelCover src={coverUrl(track)} seed={trackCoverSeed(track)} />
            </div>
            <div class="track-info">
              <span class="track-title">{track.title}</span>
              <span class="track-artists">
                {track.artists.map((a) => a.name).join(', ')}{track.album
                  ? ` · ${track.album.title}`
                  : ''}
              </span>
            </div>
            {#if track.duration}
              <span class="duration">{formatDuration(track.duration)}</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .download-page {
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-page);
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 68ch;
  }
  h1 {
    margin: 0;
    font-size: 28px;
    font-weight: 600;
    line-height: 1.1;
    letter-spacing: -0.035em;
  }
  @media (min-width: 768px) {
    h1 { font-size: 32px; }
  }
  .lede {
    margin: 0;
    font-size: 14px;
    line-height: 1.45;
    color: var(--muted);
  }

  /* ── Primary action: the URL control ─────────────────────────────────── */
  .downloader {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }
  @media (min-width: 861px) and (hover: hover) and (pointer: fine) {
    .downloader { grid-template-columns: minmax(0, 1fr) auto; }
    .sources { grid-column: 1 / -1; }
  }
  .url-field {
    position: relative;
    min-width: 0;
  }
  .field-icon {
    position: absolute;
    top: 50%;
    left: 14px;
    margin-top: -8px;
    font-size: 16px;
    line-height: 1;
    color: var(--muted-2);
    pointer-events: none;
    transition: color var(--motion-fast) var(--ease-out);
  }
  .url-field:focus-within .field-icon { color: var(--accent); }
  .url-field input {
    min-height: 48px;
    padding-left: 40px;
  }
  .url-field input:disabled { opacity: 0.6; }
  .download-btn { min-height: 48px; }

  /* ── Supported sources: plain mono tags, no brand marks ──────────────── */
  .sources {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .sources-label {
    margin-right: 4px;
    font-size: 13px;
    color: var(--muted-2);
  }
  .source-tag {
    display: inline-flex;
    align-items: center;
    min-height: 24px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  /* ── Result callouts (global .callout) ───────────────────────────────── */
  .review-tag {
    display: inline-flex;
    align-items: center;
    margin-left: 6px;
    padding: 1px 6px;
    border: 1px solid color-mix(in srgb, var(--warning) 45%, transparent);
    border-radius: var(--radius-chip);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--warning);
    vertical-align: middle;
  }
  .link-inline {
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-weight: 500;
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .link-inline:hover { color: var(--accent-2); }

  /* ── Recent downloads ────────────────────────────────────────────────── */
  .recent {
    display: flex;
    flex-direction: column;
  }
  .recent-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--text-bright);
  }
  .recent-count {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 400;
    letter-spacing: 0;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }
  .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 0 4px;
    border: none;
    border-radius: var(--radius-chip);
    background: none;
    color: var(--muted);
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: color var(--motion-fast) var(--ease-out);
  }
  .link-btn:hover { color: var(--text-bright); }
  .link-btn .pxi { font-size: 16px; }

  .track-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .track-row {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 56px;
    padding: 8px 4px;
    border-bottom: 1px solid var(--border-soft);
    transition: background-color var(--motion-fast) var(--ease-out);
  }
  .track-row:not(.skeleton):hover { background: var(--surface); }
  .row-cover {
    flex-shrink: 0;
    width: 40px;
    height: 40px;
    border-radius: var(--radius-chip);
  }
  .track-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .track-title {
    font-size: 14px;
    font-weight: 500;
    line-height: 1.35;
    color: var(--text-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track-artists {
    font-size: 13px;
    line-height: 1.35;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .duration {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }

  /* ── Loading skeleton ────────────────────────────────────────────────── */
  .sk {
    display: block;
    height: 10px;
    border-radius: 2px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s steps(4) infinite;
  }
  .sk-title { width: 40%; }
  .sk-sub { width: 24%; height: 8px; margin-top: 4px; }
  @keyframes sk-pulse {
    50% { opacity: 0.45; }
  }
  @media (prefers-reduced-motion: reduce) {
    .sk { animation: none; }
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .link-btn { min-height: 44px; }
    .url-field input { min-height: 52px; }
    .download-btn { min-height: 48px; }
  }
</style>
