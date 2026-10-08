<script lang="ts">
  import { getContext } from 'svelte';
  import { lib, LIBRARY_PLAYER, type LibraryPlayer, type TrackSortBy } from './store.svelte';
  import TrackTable from './TrackTable.svelte';
  import QualityBadge from './QualityBadge.svelte';
  import CardActions from './CardActions.svelte';
  import LibraryOptions from './LibraryOptions.svelte';

  const player = getContext<LibraryPlayer | undefined>(LIBRARY_PLAYER);

  const sortTabs: { value: TrackSortBy; label: string }[] = [
    { value: 'title', label: 'Title' },
    { value: 'artist', label: 'Artist' },
    { value: 'duration', label: 'Duration' },
  ];
</script>

{#snippet coverWrap(src: string | null | undefined, alt: string)}
  <div class="cover-wrap">
    {#if src && (src.startsWith('http://') || src.startsWith('https://') || src.startsWith('/'))}
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
  <LibraryOptions>
    {#snippet search()}
    <div class="search-wrap">
      <i class="lni lni-search-1" aria-hidden="true"></i>
      <input class="search" aria-label="Search tracks or artists" placeholder="Search tracks or artists" bind:value={lib.trackSearch} />
      <kbd>S</kbd>
    </div>
    {/snippet}
    {#snippet children()}

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
    {/snippet}
  </LibraryOptions>

  {#if lib.hiddenReviewMatches > 0}
    <button class="review-hint" onclick={() => (lib.trackFilter = 'review')}>
      {lib.hiddenReviewMatches} matching {lib.hiddenReviewMatches === 1 ? 'track is' : 'tracks are'} awaiting review — view
    </button>
  {/if}

  {#if lib.tracksView === 'list'}
    <TrackTable tracks={lib.filteredTracks} showAlbumCol={true} />
    {#if lib.filteredTracks.length === 0}<p class="status">No tracks found.</p>{/if}
  {:else}
    <div class="card-grid">
      {#each lib.filteredTracks as t (t.id)}
         <div class="card"
           role="group" aria-label={t.title}
           class:warn-border={t.needs_validation}
           class:playing={player?.isCurrent(t.id)}
           onmouseenter={() => (lib.hoveredItem = { type: 'track', id: t.id })}
           onmouseleave={() => (lib.hoveredItem = null)}>
          {@render coverWrap(t.cover, t.title)}
          {#if player}
            <span class="play-slot" title={t.file_path ? undefined : 'Not downloaded yet'}>
              <button class="artwork-play" aria-label={`${player.isPlaying(t.id) ? 'Pause' : 'Play'} ${t.title}`}
                onclick={() => player?.play(t, lib.filteredTracks)} disabled={!t.file_path}>
                {#if player.isPlaying(t.id)}
                  <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
                {:else}
                  <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="m8 4 12 8-12 8z"/></svg>
                {/if}
              </button>
            </span>
          {/if}
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
          <CardActions title={t.title} actions={[
            { label: t.rating === 'liked' ? 'Remove like' : 'Like track', pressed: t.rating === 'liked', onSelect: () => lib.setRating(t, t.rating === 'liked' ? null : 'liked') },
            { label: t.rating === 'disliked' ? 'Remove dislike' : 'Dislike track', pressed: t.rating === 'disliked', onSelect: () => lib.setRating(t, t.rating === 'disliked' ? null : 'disliked') },
            { label: 'Edit track', onSelect: () => lib.startEditTrack(t) },
          ]} />
        </div>
      {/each}
    </div>
    {#if lib.filteredTracks.length === 0}<p class="status">No tracks found.</p>{/if}
  {/if}
{/if}

<style>
  .review-hint {
    display: block;
    width: 100%;
    margin: 10px 0 0;
    padding: 9px 13px;
    text-align: left;
    font: inherit;
    font-size: 13px;
    color: var(--accent-2);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 10px;
    cursor: pointer;
    transition: background var(--motion-fast, 160ms) var(--ease-out, ease-out);
  }
  .review-hint:hover { background: color-mix(in srgb, var(--accent) 20%, transparent); }
  .search-wrap {
    display: flex; align-items: center; gap: 8px;
    flex: 1 1 260px; max-width: 420px; height: 44px; box-sizing: border-box;
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
    transition: background var(--motion-fast, 160ms) var(--ease-out, ease-out), color var(--motion-fast, 160ms) var(--ease-out, ease-out);
  }
  .pill:hover { color: var(--text-bright); background: var(--surface); }
  .pill.on { color: var(--accent-2); background: color-mix(in srgb, var(--accent) 18%, transparent); }
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

  .card { position: relative; }
  .play-slot { position: absolute; top: 8px; left: 8px; z-index: 2; display: flex; }
  .artwork-play { width: 44px; height: 44px; display: flex; align-items: center; justify-content: center; padding: 0; border: 1px solid var(--border); border-radius: 50%; background: var(--accent); color: var(--on-accent); cursor: pointer; box-shadow: var(--shadow-sm); }
  .artwork-play:disabled { background: var(--surface); color: var(--muted); cursor: default; }
  .card-badge { top: 58px; left: 8px; }
  .review-hint { min-height: 44px; }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    button { min-height: 44px; min-width: 44px; }
    input:not([type="checkbox"]):not([type="radio"]):not([type="range"]) { font-size: 16px; min-height: 44px; }
    .search-wrap { min-width: 0; height: 44px; max-width: none; } .search-wrap kbd { display: none; } .sort-tabs { flex-wrap: wrap; max-width: 100%; } .sort-dir { width: 44px; height: 44px; } .pill { height: 44px; font-size: 0.875rem; } .tools { flex-wrap: wrap; margin-left: 0; max-width: 100%; }
  }
  @media (prefers-reduced-motion: reduce) { .review-hint, .pill { transition: none; } }
</style>
