<script lang="ts">
  import { getPendingValidations, approveValidation, rejectValidation } from '../lib/api';
  import TrackCard from '../lib/TrackCard.svelte';
  import type { PendingValidationDto, PatchValidationBody } from '../lib/types';
  import { flip } from 'svelte/animate';

  const reduce =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

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
  // These throw on failure; the per-card StatefulButton catches it to show its
  // error state and inline reason, so there is no page-level alert box.
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
  <header class="page-head">
    <div class="header-text">
      <h1>Validations</h1>
      {#if !loading && !error}
        <p class="page-meta">{tracks.length} to review</p>
      {/if}
      <p class="lede">
        Tracks that need a human decision before they're filed. Review the metadata, then approve or
        reject each candidate.
      </p>
    </div>
    <button class="btn-secondary refresh-btn" onclick={load} disabled={loading}>
      {#if loading}
        <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Loading
      {:else}
        <i class="pxi pxi-refresh" aria-hidden="true"></i>Refresh
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
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>
      <div class="callout-body">
        <strong>Couldn't load validations.</strong>
        <span>{error}</span>
      </div>
    </div>
  {:else if tracks.length === 0}
    <div class="empty">
      <i class="pxi pxi-check" aria-hidden="true"></i>
      <p class="empty-title">All caught up</p>
      <p class="empty-hint">Nothing needs your review right now.</p>
    </div>
  {:else}
    <div class="tabs" role="tablist">
      <button
        role="tab"
        class="tab"
        class:active={activeTab === 'partial'}
        onclick={() => { activeTab = 'partial'; search = ''; }}
        aria-selected={activeTab === 'partial'}
      >
        Partial match
        {#if partialTracks.length > 0}
          <span class="tab-count tab-count--warning">{partialTracks.length}</span>
        {/if}
      </button>
      <button
        role="tab"
        class="tab"
        class:active={activeTab === 'no_match'}
        onclick={() => { activeTab = 'no_match'; search = ''; }}
        aria-selected={activeTab === 'no_match'}
      >
        No match
        {#if noMatchTracks.length > 0}
          <span class="tab-count">{noMatchTracks.length}</span>
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
          <span class="tab-count tab-count--error">{drmTracks.length}</span>
        {/if}
      </button>
    </div>

    <p class="tab-hint">
      {#if activeTab === 'partial'}
        Confidence was below the auto-approve threshold. Confirm the top match or pick a candidate.
      {:else if activeTab === 'no_match'}
        No match was found automatically. Edit the metadata, then approve.
      {:else}
        DRM-protected on SoundCloud. Pick the matching YouTube source to download.
      {/if}
    </p>

    {#if activeTracks.length > 0}
      <div class="filter-row">
        <div class="search-field">
          <i class="pxi pxi-search field-icon" aria-hidden="true"></i>
          <input
            type="text"
            placeholder="Filter by title, artist, album…"
            bind:value={search}
            autocomplete="off"
            spellcheck="false"
            aria-label="Filter validations"
          />
        </div>
        <p class="list-count">
          {filteredTracks.length} / {activeTracks.length} track{activeTracks.length > 1 ? 's' : ''}
        </p>
      </div>
    {/if}

    {#if activeTracks.length === 0}
      <div class="empty">
        <i class="pxi pxi-check" aria-hidden="true"></i>
        <p class="empty-title">Nothing here</p>
        <p class="empty-hint">No tracks in this category.</p>
      </div>
    {:else}
      <ul class="track-list" role="list">
        {#each filteredTracks as track (track.id)}
          <li animate:flip={{ duration: reduce ? 0 : 220 }}>
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
    padding: var(--space-page);
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  .header-text {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
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
  .page-meta {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }
  .lede {
    margin: 4px 0 0;
    font-size: 14px;
    line-height: 1.45;
    color: var(--muted);
  }
  .refresh-btn { flex-shrink: 0; }

  /* ── Tabs (global .tabs) ─────────────────────────────────────────────── */
  .tabs { margin-bottom: 0; }
  .tab-count {
    font-variant-numeric: tabular-nums;
    letter-spacing: 0;
    color: var(--muted-2);
  }
  .tab-count--warning { color: var(--warning); }
  .tab-count--error { color: var(--error); }

  .tab-hint {
    margin: 0;
    max-width: 72ch;
    font-size: 14px;
    line-height: 1.45;
    color: var(--muted);
  }

  /* ── Filter ──────────────────────────────────────────────────────────── */
  .filter-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 16px;
  }
  .search-field {
    position: relative;
    flex: 1 1 260px;
    min-width: 0;
    max-width: 420px;
  }
  .field-icon {
    position: absolute;
    top: 50%;
    left: 12px;
    margin-top: -8px;
    font-size: 16px;
    line-height: 1;
    color: var(--muted-2);
    pointer-events: none;
    transition: color var(--motion-fast) var(--ease-out);
  }
  .search-field:focus-within .field-icon { color: var(--accent); }
  .search-field input {
    min-height: 40px;
    padding-left: 36px;
  }
  .list-count {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }

  /* ── List ────────────────────────────────────────────────────────────── */
  .track-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* ── Loading skeleton ────────────────────────────────────────────────── */
  .skeleton-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .skeleton-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }
  .sk-cover {
    flex-shrink: 0;
    width: 56px;
    height: 56px;
    border-radius: var(--radius-control);
    background: var(--surface-2);
    animation: sk-pulse 1.3s steps(4) infinite;
  }
  .sk-lines {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .sk {
    display: block;
    height: 10px;
    border-radius: 2px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s steps(4) infinite;
  }
  .sk-title { width: 42%; }
  .sk-sub { width: 26%; height: 8px; }
  @keyframes sk-pulse {
    50% { opacity: 0.45; }
  }
  @media (prefers-reduced-motion: reduce) {
    .sk,
    .sk-cover { animation: none; }
  }

  @media (max-width: 640px) {
    .page-head { flex-direction: column; }
  }
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .search-field { max-width: none; flex-basis: 100%; }
    .search-field input { min-height: 44px; }
  }
</style>
