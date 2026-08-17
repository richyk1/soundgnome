<script lang="ts">
  import { onMount } from 'svelte';
  import { downloadUrl, getRecentTracks, getProviders } from '../lib/api';
  import type { DownloadResult, RecentTrack } from '../lib/api';

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

  function providerIcon(p: string): string {
    const s = p.toLowerCase();
    if (s.includes('spotify')) return 'lni-spotify';
    if (s.includes('soundcloud')) return 'lni-soundcloud';
    if (s.includes('youtube music')) return 'lni-youtube-music';
    if (s.includes('youtube')) return 'lni-youtube';
    return 'lni-music-note';
  }
</script>

<div class="download-page">
  <header class="page-header">
    <h1>Download</h1>
    <p class="lede">
      Paste a track, album, or playlist link. Soundgnome fetches the audio, enriches the metadata,
      and files it into your library.
    </p>
  </header>

  <form class="downloader" onsubmit={handleSubmit}>
    <div class="url-field">
      <i class="lni lni-cloud-download field-icon" aria-hidden="true"></i>
      <input
        type="url"
        placeholder="Paste a Spotify, SoundCloud, or YouTube link"
        bind:value={url}
        disabled={loading}
        autocomplete="off"
        spellcheck="false"
        aria-label="Track, album, or playlist URL"
      />
      <button type="submit" class="download-btn" disabled={loading || !url.trim()}>
        {#if loading}
          <span class="spinner"></span>Downloading
        {:else}
          <i class="lni lni-cloud-download" aria-hidden="true"></i>Download
        {/if}
      </button>
    </div>

    {#if providers.length > 0}
      <div class="sources">
        <span class="sources-label">Works with</span>
        {#each providers as platform}
          <span class="source-pill">
            <i class="lni {providerIcon(platform)}" aria-hidden="true"></i>{platform}
          </span>
        {/each}
      </div>
    {/if}
  </form>

  {#if error}
    <div class="callout callout-error" role="alert">
      <i class="lni lni-xmark-circle" aria-hidden="true"></i>
      <div class="callout-body">
        <strong>Couldn't download that link.</strong>
        <span>{error}</span>
      </div>
    </div>
  {/if}

  {#if result && !loading}
    {#if result.type === 'track'}
      <div class="callout callout-success" role="status">
        <i class="lni lni-check-circle-1" aria-hidden="true"></i>
        <div class="callout-body">
          <strong>Added {result.title}</strong>
          <span>
            {#if result.artists.length}{result.artists.join(', ')}{/if}
            {#if result.needs_validation}<span class="inline-badge">needs review</span>{/if}
          </span>
        </div>
      </div>
    {:else}
      <div class="callout callout-success" role="status">
        <i class="lni lni-check-circle-1" aria-hidden="true"></i>
        <div class="callout-body">
          <strong>Playlist syncing</strong>
          <span>
            Task #{result.task_id} is running.
            {#if onNavigateTasks}
              <button type="button" class="link-inline" onclick={onNavigateTasks}>Track its progress</button>
            {/if}
          </span>
        </div>
      </div>
    {/if}
  {/if}

  <section class="recent">
    <div class="recent-header">
      <h2>Recent downloads</h2>
      {#if onNavigateTasks}
        <button type="button" class="link-btn" onclick={onNavigateTasks}>
          Activity<i class="lni lni-arrow-right" aria-hidden="true"></i>
        </button>
      {/if}
    </div>

    {#if recentLoading}
      <ul class="track-list" aria-hidden="true">
        {#each { length: 5 } as _}
          <li class="track-row skeleton">
            <div class="cover"></div>
            <div class="track-info">
              <span class="sk sk-title"></span>
              <span class="sk sk-sub"></span>
            </div>
          </li>
        {/each}
      </ul>
    {:else if visibleRecent.length === 0}
      <div class="empty">
        <i class="lni lni-music-note" aria-hidden="true"></i>
        <p class="empty-title">No downloads yet</p>
        <p class="empty-hint">Paste a link above and it will show up here.</p>
      </div>
    {:else}
      <ul class="track-list">
        {#each visibleRecent as track (track.id)}
          <li class="track-row">
            <div class="cover">
              {#if track.cover}
                <img src={track.cover} alt="" />
              {:else}
                <i class="lni lni-music-note cover-ph" aria-hidden="true"></i>
              {/if}
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
    padding: 1.5rem 2rem 2rem;
    display: flex;
    flex-direction: column;
    gap: 1.75rem;
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-header {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
    max-width: 68ch;
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

  /* ── Primary action: the URL control ─────────────────────────────────── */
  .downloader {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .url-field {
    display: flex;
    align-items: center;
    gap: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding-left: 14px;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }
  .url-field:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .field-icon {
    font-size: 18px;
    color: var(--muted-2);
    flex-shrink: 0;
  }
  .url-field:focus-within .field-icon {
    color: var(--accent);
  }
  .url-field input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-family: inherit;
    font-size: 0.95rem;
    padding: 0.95rem 0.75rem;
  }
  .url-field input::placeholder {
    color: var(--muted);
  }
  .url-field input:disabled {
    opacity: 0.6;
  }
  .download-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    margin: 6px;
    padding: 0.6rem 1.2rem;
    border: none;
    border-radius: 8px;
    background: var(--accent);
    color: #fff;
    font-family: inherit;
    font-weight: 600;
    font-size: 0.9rem;
    white-space: nowrap;
    cursor: pointer;
    transition:
      filter 0.12s ease,
      opacity 0.12s ease;
  }
  .download-btn .lni {
    font-size: 16px;
  }
  .download-btn:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .download-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* ── Supported sources ───────────────────────────────────────────────── */
  .sources {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }
  .sources-label {
    font-size: 0.8rem;
    color: var(--muted-2);
    margin-right: 0.15rem;
  }
  .source-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.28rem 0.65rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 999px;
    font-size: 0.78rem;
    color: var(--muted);
  }
  .source-pill .lni {
    font-size: 14px;
    color: var(--text);
  }

  /* ── Status callouts ─────────────────────────────────────────────────── */
  .callout {
    display: flex;
    align-items: flex-start;
    gap: 0.7rem;
    padding: 0.85rem 1rem;
    border-radius: 10px;
    border: 1px solid transparent;
    font-size: 0.9rem;
    animation: callout-rise 0.24s cubic-bezier(0.22, 1, 0.36, 1);
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
  .callout-success {
    background: color-mix(in srgb, var(--success) 16%, var(--panel));
    border-color: color-mix(in srgb, var(--success) 45%, transparent);
    color: var(--success);
  }
  .inline-badge {
    font-size: 0.7rem;
    padding: 0.1rem 0.4rem;
    margin-left: 0.35rem;
    border-radius: 4px;
    background: var(--warning-bg);
    color: var(--warning);
    vertical-align: middle;
  }
  .link-inline {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: inherit;
    font-weight: 600;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  @keyframes callout-rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  /* ── Recent downloads ────────────────────────────────────────────────── */
  .recent {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .recent-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--muted-2);
    margin: 0;
  }
  .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    background: none;
    border: none;
    color: var(--muted);
    font-family: inherit;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: color 0.12s ease;
  }
  .link-btn:hover {
    color: var(--text-bright);
  }
  .link-btn .lni {
    font-size: 14px;
  }

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
    transition: background 0.1s ease;
  }
  .track-row:not(.skeleton):hover {
    background: var(--surface);
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
  .track-artists {
    font-size: 0.78rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .duration {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
  }

  /* ── Empty + loading states ──────────────────────────────────────────── */
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
    .callout,
    .sk {
      animation: none;
    }
  }

  @media (max-width: 640px) {
    .download-page {
      padding: 1.25rem 1rem 1.5rem;
    }
    .download-btn {
      padding: 0.55rem 0.85rem;
    }
    .duration {
      display: none;
    }
  }
</style>
