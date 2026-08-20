<script lang="ts">
  import { getContext } from 'svelte';
  import { lib, LIBRARY_PLAYER, type LibraryPlayer, type TrackSortBy } from './store.svelte';
  import TrackTable from './TrackTable.svelte';
  import QualityBadge from './QualityBadge.svelte';

  const player = getContext<LibraryPlayer | undefined>(LIBRARY_PLAYER);

  const sortTabs: { value: TrackSortBy; label: string }[] = [
    { value: 'title', label: 'Title' },
    { value: 'artist', label: 'Artist' },
    { value: 'duration', label: 'Duration' },
  ];
</script>

{#snippet coverWrap(src: string | null | undefined, alt: string)}
  <div class="cover-wrap">
    {#if src && (src.startsWith('http://') || src.startsWith('https://'))}
      <img {src} {alt} class="cover-img" loading="lazy" />
    {:else}
      <div class="cover-ph">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
          <path d="M9 18V5l12-2v13"/>
          <circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>
        </svg>
      </div>
    {/if}
  </div>
{/snippet}

{#if lib.tracksLoading}
  <p class="status">Loading…</p>
{:else if lib.tracksError}
  <p class="status error">{lib.tracksError}</p>
{:else}
  <div class="toolbar">
    <div class="search-wrap">
      <i class="lni lni-search-1" aria-hidden="true"></i>
      <input class="search" placeholder="Search tracks or artists" bind:value={lib.trackSearch} />
      <kbd>S</kbd>
    </div>

    <div class="filters">
      <button class="pill" class:on={lib.trackFilter === 'all'} onclick={() => (lib.trackFilter = 'all')}>All</button>
      <button class="pill" class:on={lib.trackFilter === 'review'} onclick={() => (lib.trackFilter = 'review')}>Needs review · {lib.needsReviewCount}</button>
      <button class="pill" class:on={lib.trackFilter === 'lossless'} onclick={() => (lib.trackFilter = 'lossless')}>Lossless</button>
      <button class="pill" class:on={lib.trackFilter === 'liked'} onclick={() => (lib.trackFilter = 'liked')}>Liked</button>
    </div>

    <div class="tools">
      <div class="sort-tabs">
        {#each sortTabs as tab}
          <button class="sort-tab" class:on={lib.tracksSortBy === tab.value} onclick={() => (lib.tracksSortBy = tab.value)}>{tab.label}</button>
        {/each}
        <button class="sort-dir" onclick={() => (lib.tracksSortDir = lib.tracksSortDir === 'asc' ? 'desc' : 'asc')} aria-label="Toggle sort direction" title={lib.tracksSortDir === 'asc' ? 'Ascending' : 'Descending'}>{lib.tracksSortDir === 'asc' ? '↑' : '↓'}</button>
      </div>
      <div class="view-toggle">
        <button class:active={lib.tracksView === 'list'} onclick={() => (lib.tracksView = 'list')} title="List">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/>
            <circle cx="3" cy="6" r="1" fill="currentColor" stroke="none"/>
            <circle cx="3" cy="12" r="1" fill="currentColor" stroke="none"/>
            <circle cx="3" cy="18" r="1" fill="currentColor" stroke="none"/>
          </svg>
        </button>
        <button class:active={lib.tracksView === 'grid'} onclick={() => (lib.tracksView = 'grid')} title="Grid">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/>
            <rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/>
          </svg>
        </button>
      </div>
    </div>
  </div>

  {#if lib.tracksView === 'list'}
    <TrackTable tracks={lib.filteredTracks} showAlbumCol={true} />
    {#if lib.filteredTracks.length === 0}<p class="status">No tracks found.</p>{/if}
  {:else}
    <div class="card-grid">
      {#each lib.filteredTracks as t (t.id)}
         <div class="card"
           class:warn-border={t.needs_validation}
           class:playing={player?.isCurrent(t.id)}
           role="button"
           tabindex="0"
           onmouseenter={() => (lib.hoveredItem = { type: 'track', id: t.id })}
           onmouseleave={() => (lib.hoveredItem = null)}>
          {@render coverWrap(t.cover, t.title)}
          <div class="card-body">
            <div class="card-title" title={t.title}>{t.title}</div>
            <div class="card-sub">{t.artists.map(a => a.name).join(', ') || '\u2014'}</div>
            {#if t.duration != null || t.quality}
              <div class="card-foot">
                {#if t.duration != null}<span class="card-meta mono">{lib.fmtDuration(t.duration)}</span>{/if}
                <QualityBadge quality={t.quality} />
              </div>
            {/if}
          </div>
          {#if t.needs_validation}<span class="card-badge badge-warn" title="Awaiting validation">!</span>{/if}
          <div class="card-hover-actions">
            {#if player}
              <!-- The title lives on the wrapper so it still shows when the button is disabled. -->
              <span class="play-slot" title={t.file_path ? null : 'Not downloaded yet'}>
                <button
                  class="btn-play"
                  onclick={(e) => { e.stopPropagation(); player?.play(t, lib.filteredTracks); }}
                  disabled={!t.file_path}
                >
                  {player.isPlaying(t.id) ? 'Pause' : 'Play'}
                </button>
              </span>
            {/if}
            <button class="btn-rate" class:active-like={t.rating === 'liked'} title="Like" aria-label="Like" onclick={(e) => { e.stopPropagation(); lib.setRating(t, t.rating === 'liked' ? null : 'liked'); }}><i class="lni lni-thumbs-up-1"></i></button>
            <button class="btn-rate" class:active-dislike={t.rating === 'disliked'} title="Dislike" aria-label="Dislike" onclick={(e) => { e.stopPropagation(); lib.setRating(t, t.rating === 'disliked' ? null : 'disliked'); }}><i class="lni lni-thumbs-down-1"></i></button>
            <button class="btn-edit" onclick={(e) => { e.stopPropagation(); lib.startEditTrack(t); }}>Edit</button>
          </div>
        </div>
      {/each}
    </div>
    {#if lib.filteredTracks.length === 0}<p class="status">No tracks found.</p>{/if}
  {/if}
{/if}

<style>
  .toolbar { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .search-wrap {
    display: flex; align-items: center; gap: 8px;
    flex: 1 1 260px; max-width: 420px; height: 38px;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 10px; padding: 0 10px;
  }
  .search-wrap .lni { color: var(--muted-2); font-size: 15px; flex: 0 0 auto; }
  .search-wrap .search {
    flex: 1; min-width: 0; max-width: none; height: 100%;
    background: none; border: none; outline: none; padding: 0;
    color: var(--text); font: inherit;
  }
  .search-wrap kbd {
    flex: 0 0 auto; font-family: var(--font-mono); font-size: 11px;
    color: var(--muted-2); background: var(--surface-2);
    border-radius: 5px; padding: 1px 6px;
  }
  .filters { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .pill {
    display: inline-flex; align-items: center; height: 30px; padding: 0 13px;
    font-family: var(--font-display); font-size: 12.5px; font-weight: 600;
    color: var(--muted); background: transparent;
    border: 1px solid transparent; border-radius: 999px;
    cursor: pointer; white-space: nowrap;
    transition: background 0.12s, color 0.12s;
  }
  .pill:hover { color: var(--text-bright); background: var(--surface); }
  .pill.on { color: #c9bcf7; background: color-mix(in srgb, var(--accent) 18%, transparent); }
  .tools { display: flex; align-items: center; gap: 10px; margin-left: auto; }
  .sort-tabs {
    display: flex; align-items: center; gap: 2px;
    background: var(--surface); border-radius: 8px; padding: 3px;
  }
  .sort-tab {
    font-family: var(--font-display); font-size: 12px; font-weight: 500;
    color: var(--muted); background: transparent; border: none;
    border-radius: 6px; padding: 5px 10px; cursor: pointer;
  }
  .sort-tab:hover { color: var(--text-bright); }
  .sort-tab.on { color: var(--text-bright); background: var(--surface-2); }
  .sort-dir {
    display: flex; align-items: center; justify-content: center;
    width: 26px; height: 26px; line-height: 1; font-size: 14px;
    color: var(--muted); background: transparent; border: none;
    border-radius: 6px; cursor: pointer;
  }
  .sort-dir:hover { color: var(--text-bright); background: var(--surface-2); }

  .card.playing {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .card.playing .card-title {
    color: var(--accent);
  }

  .card-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
    margin-top: 0.25rem;
  }

  .card-foot .card-meta {
    margin-top: 0;
  }

  .play-slot {
    display: flex;
  }

  .play-slot .btn-play {
    width: 100%;
  }

  .card-hover-actions .btn-play {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    color: var(--accent);
    border-color: var(--accent);
  }

  .card-hover-actions .btn-play:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .card-hover-actions .btn-play:disabled {
    opacity: 0.5;
    cursor: default;
  }
  @media (max-width: 860px) {
    .toolbar .tools { display: none; }
    .search-wrap { max-width: none; flex: 1 1 100%; }
  }
</style>
