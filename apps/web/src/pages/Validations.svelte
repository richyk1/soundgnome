<script lang="ts">
  import { getPendingValidations, approveValidation, rejectValidation } from '../lib/api';
  import TrackCard from '../lib/TrackCard.svelte';
  import type { PendingValidationDto, PatchValidationBody } from '../lib/types';

  interface Props {
    onDownloaded?: () => void;
  }
  let { onDownloaded }: Props = $props();

  type Tab = 'partial' | 'no_match' | 'drm';

  let tracks: PendingValidationDto[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);
  let search = $state('');
  let activeTab: Tab = $state('partial');

  // ── Grouped by reason ──────────────────────────────────────────────────────
  let partialTracks = $derived(
    tracks.filter((t) => t.validation_reason === 'metadata_partial_match'),
  );
  let noMatchTracks = $derived(
    tracks.filter((t) => t.validation_reason === 'metadata_no_match'),
  );
  let drmTracks = $derived(
    tracks.filter((t) => t.validation_reason === 'soundcloud_drm_protected'),
  );

  // Active tab tracks
  let activeTracks = $derived(
    activeTab === 'partial' ? partialTracks : activeTab === 'no_match' ? noMatchTracks : drmTracks,
  );

  // Filtered within active tab
  let filteredTracks = $derived(
    search.trim() === ''
      ? activeTracks
      : activeTracks.filter((t) => {
          const q = search.toLowerCase();
          return (
            t.title.toLowerCase().includes(q) ||
            t.artists.some((a) => a.name.toLowerCase().includes(q)) ||
            (t.album?.title.toLowerCase().includes(q) ?? false)
          );
        }),
  );

  // ── Load ───────────────────────────────────────────────────────────────────
  async function load() {
    loading = true;
    error = null;
    try {
      tracks = await getPendingValidations();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  // ── Handlers ──────────────────────────────────────────────────────────────
  async function handleApprove(id: number, patch: PatchValidationBody) {
    await approveValidation(id, patch);
    tracks = tracks.filter((t) => t.id !== id);
    onDownloaded?.();
  }

  async function handleReject(id: number) {
    await rejectValidation(id);
    tracks = tracks.filter((t) => t.id !== id);
    onDownloaded?.();
  }
</script>

<div class="validations-page">
  <header class="page-header">
    <div class="header-text">
      <h1>Validations</h1>
      <p class="lede">
        Tracks that need a human decision before they're filed. Review the metadata, then approve or
        reject each candidate.
      </p>
    </div>
    <button class="btn-secondary btn-sm" onclick={load} disabled={loading}>
      {#if loading}
        <span class="spinner" aria-hidden="true"></span>Loading
      {:else}
        <i class="lni lni-refresh-circle-1-clockwise" aria-hidden="true"></i>Refresh
      {/if}
    </button>
  </header>

  {#if loading}
    <ul class="skeleton-list" aria-hidden="true">
      {#each { length: 4 } as _}
        <li class="skeleton-row">
          <div class="sk-cover"></div>
          <div class="sk-lines">
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
        <strong>Couldn't load validations.</strong>
        <span>{error}</span>
      </div>
    </div>
  {:else if tracks.length === 0}
    <div class="empty">
      <i class="lni lni-check-circle-1" aria-hidden="true"></i>
      <p class="empty-title">All caught up</p>
      <p class="empty-hint">Nothing needs your review right now.</p>
    </div>
  {:else}
    <!-- Tabs -->
    <div class="tabs" role="tablist">
      <button
        role="tab"
        class="tab"
        class:active={activeTab === 'partial'}
        onclick={() => { activeTab = 'partial'; search = ''; }}
        aria-selected={activeTab === 'partial'}
      >
        Partial Match
        {#if partialTracks.length > 0}
          <span class="tab-badge tab-badge--warning">{partialTracks.length}</span>
        {/if}
      </button>
      <button
        role="tab"
        class="tab"
        class:active={activeTab === 'no_match'}
        onclick={() => { activeTab = 'no_match'; search = ''; }}
        aria-selected={activeTab === 'no_match'}
      >
        No Match
        {#if noMatchTracks.length > 0}
          <span class="tab-badge">{noMatchTracks.length}</span>
        {/if}
      </button>
      <button
        role="tab"
        class="tab"
        class:active={activeTab === 'drm'}
        onclick={() => { activeTab = 'drm'; search = ''; }}
        aria-selected={activeTab === 'drm'}
      >
        Errors
        {#if drmTracks.length > 0}
          <span class="tab-badge tab-badge--error">{drmTracks.length}</span>
        {/if}
      </button>
    </div>

    <!-- Tab description as a tinted callout -->
    {#if activeTab === 'partial'}
      <div class="callout callout-warning" role="note">
        <i class="lni lni-info-triangle" aria-hidden="true"></i>
        <div class="callout-body">
          <span>
            A metadata provider found a likely match, but confidence was not high enough for automatic
            approval. Review the candidates and confirm or correct.
          </span>
        </div>
      </div>
    {:else if activeTab === 'no_match'}
      <div class="callout callout-info" role="note">
        <i class="lni lni-info-circle" aria-hidden="true"></i>
        <div class="callout-body">
          <span>No metadata match was found automatically. Edit the metadata manually before approving.</span>
        </div>
      </div>
    {:else}
      <div class="callout callout-error" role="note">
        <i class="lni lni-xmark-circle" aria-hidden="true"></i>
        <div class="callout-body">
          <span>
            SoundCloud track is DRM-protected and could not be downloaded. Find the matching YouTube
            video and select it as the audio source.
          </span>
        </div>
      </div>
    {/if}

    <!-- Search -->
    {#if activeTracks.length > 0}
      <div class="search-field">
        <i class="lni lni-search-1 field-icon" aria-hidden="true"></i>
        <input
          type="text"
          placeholder="Filter by title, artist, album…"
          bind:value={search}
          autocomplete="off"
          spellcheck="false"
          aria-label="Filter validations"
        />
      </div>
    {/if}

    <!-- Track list -->
    {#if activeTracks.length === 0}
      <div class="empty">
        <i class="lni lni-check-circle-1" aria-hidden="true"></i>
        <p class="empty-title">Nothing here</p>
        <p class="empty-hint">No tracks in this category.</p>
      </div>
    {:else}
      <p class="count">
        {filteredTracks.length} / {activeTracks.length} track{activeTracks.length > 1 ? 's' : ''}
      </p>
      <ul class="track-list" role="list">
        {#each filteredTracks as track (track.id)}
          <li>
            <TrackCard {track} onApprove={handleApprove} onReject={handleReject} />
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .validations-page {
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
    flex-direction: row;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }
  .header-text {
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

  /* ── Buttons ─────────────────────────────────────────────────────────── */
  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 8px;
    padding: 0.55rem 1rem;
    font-family: inherit;
    font-weight: 600;
    font-size: 0.9rem;
    white-space: nowrap;
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
    font-size: 15px;
    color: var(--muted);
  }
  .spinner {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--text) 35%, transparent);
    border-top-color: var(--text);
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* ── Tabs ────────────────────────────────────────────────────────────── */
  .tabs {
    display: flex;
    gap: 0.25rem;
    border-bottom: 1px solid var(--border);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.55rem 0.9rem;
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    font-family: inherit;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    margin-bottom: -1px;
    transition:
      color 0.15s ease,
      border-color 0.15s ease;
  }
  .tab:hover {
    color: var(--text);
  }
  .tab.active {
    color: var(--text-bright);
    border-bottom-color: var(--accent);
  }
  .tab-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    background: var(--surface-2);
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 0.68rem;
    font-weight: 700;
    border-radius: 9px;
    font-variant-numeric: tabular-nums;
  }
  .tab.active .tab-badge {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent-2);
  }
  .tab-badge--warning {
    background: color-mix(in srgb, var(--warning) 20%, transparent);
    color: var(--warning);
  }
  .tab-badge--error {
    background: color-mix(in srgb, var(--error) 18%, transparent);
    color: var(--error);
  }

  /* ── Callouts ────────────────────────────────────────────────────────── */
  .callout {
    display: flex;
    align-items: flex-start;
    gap: 0.7rem;
    padding: 0.85rem 1rem;
    border-radius: 10px;
    border: 1px solid transparent;
    font-size: 0.9rem;
    line-height: 1.5;
  }
  .callout .lni {
    font-size: 19px;
    flex-shrink: 0;
    line-height: 1.4;
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
  .callout-warning {
    background: var(--warning-bg);
    border-color: color-mix(in srgb, var(--warning) 45%, transparent);
    color: var(--warning);
  }
  .callout-info {
    background: var(--surface);
    border-color: var(--border);
    color: var(--muted);
  }
  .callout-info .lni {
    color: var(--muted-2);
  }

  /* ── Search ──────────────────────────────────────────────────────────── */
  .search-field {
    display: flex;
    align-items: center;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding-left: 12px;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }
  .search-field:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .search-field .field-icon {
    font-size: 16px;
    color: var(--muted-2);
    flex-shrink: 0;
  }
  .search-field:focus-within .field-icon {
    color: var(--accent);
  }
  .search-field input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-family: inherit;
    font-size: 0.9rem;
    padding: 0.6rem 0.75rem;
  }
  .search-field input::placeholder {
    color: var(--muted);
  }

  /* ── Count ───────────────────────────────────────────────────────────── */
  .count {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
  }

  /* ── List ────────────────────────────────────────────────────────────── */
  .track-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  /* ── Empty state ─────────────────────────────────────────────────────── */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.4rem;
    padding: 3rem 1rem;
    color: var(--muted);
  }
  .empty .lni {
    font-size: 34px;
    color: var(--success);
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
  .skeleton-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .skeleton-row {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    padding: 0.85rem 1rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 10px;
  }
  .sk-cover {
    flex-shrink: 0;
    width: 48px;
    height: 48px;
    border-radius: 6px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s ease-in-out infinite;
  }
  .sk-lines {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .sk {
    height: 0.7rem;
    border-radius: 4px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s ease-in-out infinite;
  }
  .sk-title {
    width: 42%;
  }
  .sk-sub {
    width: 26%;
    height: 0.6rem;
  }
  @keyframes sk-pulse {
    50% {
      opacity: 0.45;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner,
    .sk,
    .sk-cover {
      animation: none;
    }
  }

  @media (max-width: 640px) {
    .validations-page {
      padding: 1.25rem 1rem 1.5rem;
    }
    .page-header {
      flex-direction: column;
    }
  }
</style>
