<script lang="ts">
  import { getContext } from 'svelte';
  import { lib, LIBRARY_PLAYER, type LibraryPlayer, type TrackSortBy } from './store.svelte';
  import TrackTable from './TrackTable.svelte';
  import QualityBadge from './QualityBadge.svelte';
  import CardActions from './CardActions.svelte';
  import LibraryOptions from './LibraryOptions.svelte';
  import SortDropdown from './SortDropdown.svelte';
  import PixelCover from '../PixelCover.svelte';
  import { trackCoverSeed } from '../pixel-art';

  const player = getContext<LibraryPlayer | undefined>(LIBRARY_PLAYER);

  const sortOptions: { value: TrackSortBy; label: string }[] = [
    { value: 'title', label: 'Title' },
    { value: 'artist', label: 'Artist' },
    { value: 'duration', label: 'Duration' },
  ];

  function coverUrl(src: string | null): string | null {
    return src && /^(https?:\/\/|\/)/.test(src) ? src : null;
  }
</script>

{#if lib.tracksLoading}
  <p class="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Loading…</p>
{:else if lib.tracksError}
  <div class="callout callout-error" role="alert">
    <i class="pxi pxi-square-alert" aria-hidden="true"></i>
    <div class="callout-body"><strong>Couldn't load tracks</strong><span>{lib.tracksError}</span></div>
  </div>
{:else}
  <LibraryOptions>
    {#snippet search()}
      <i class="pxi pxi-search" aria-hidden="true"></i>
      <input class="search" aria-label="Search tracks or artists" placeholder="Search tracks or artists" bind:value={lib.trackSearch} />
      <kbd class="search-kbd">S</kbd>
    {/snippet}
    {#snippet children()}
      <div class="opt-row filter-group" role="group" aria-label="Filter tracks">
        <button class="filter-btn" class:active={lib.trackFilter === 'all'} aria-pressed={lib.trackFilter === 'all'} onclick={() => (lib.trackFilter = 'all')}>All</button>
        <button class="filter-btn" class:active={lib.trackFilter === 'review'} aria-pressed={lib.trackFilter === 'review'} onclick={() => (lib.trackFilter = 'review')}>
          Needs review <span class="filter-count">{lib.needsReviewCount}</span>
        </button>
        <button class="filter-btn" class:active={lib.trackFilter === 'lossless'} aria-pressed={lib.trackFilter === 'lossless'} onclick={() => (lib.trackFilter = 'lossless')}>Lossless</button>
        <button class="filter-btn" class:active={lib.trackFilter === 'liked'} aria-pressed={lib.trackFilter === 'liked'} onclick={() => (lib.trackFilter = 'liked')}>Liked</button>
      </div>
      <div class="opt-row tools">
        <SortDropdown
          value={lib.tracksSortBy}
          direction={lib.tracksSortDir}
          options={sortOptions}
          onChange={(val) => (lib.tracksSortBy = val as TrackSortBy)}
          onDirectionChange={(dir) => (lib.tracksSortDir = dir)}
        />
        <div class="view-toggle" role="group" aria-label="View">
          <button class:active={lib.tracksView === 'list'} aria-pressed={lib.tracksView === 'list'} onclick={() => (lib.tracksView = 'list')} title="List" aria-label="List view">
            <i class="pxi pxi-list-box" aria-hidden="true"></i>
          </button>
          <button class:active={lib.tracksView === 'grid'} aria-pressed={lib.tracksView === 'grid'} onclick={() => (lib.tracksView = 'grid')} title="Grid" aria-label="Grid view">
            <i class="pxi pxi-grid-2x2-2" aria-hidden="true"></i>
          </button>
        </div>
      </div>
      <span class="count">{lib.filteredTracks.length} track{lib.filteredTracks.length !== 1 ? 's' : ''}</span>
    {/snippet}
  </LibraryOptions>

  {#if lib.hiddenReviewMatches > 0}
    <button class="review-hint" onclick={() => (lib.trackFilter = 'review')}>
      <i class="pxi pxi-warning-diamond" aria-hidden="true"></i>
      <span>{lib.hiddenReviewMatches} matching {lib.hiddenReviewMatches === 1 ? 'track is' : 'tracks are'} awaiting review</span>
      <span class="review-hint-action">View <i class="pxi pxi-arrow-right" aria-hidden="true"></i></span>
    </button>
  {/if}

  {#if lib.tracksView === 'list'}
    <TrackTable tracks={lib.filteredTracks} showAlbumCol={true} />
  {:else}
    <div class="card-grid">
      {#each lib.filteredTracks as t (t.id)}
        <div class="card"
          role="group" aria-label={t.title}
          class:warn-border={t.needs_validation}
          class:playing={player?.isCurrent(t.id)}
          onmouseenter={() => (lib.hoveredItem = { type: 'track', id: t.id })}
          onmouseleave={() => (lib.hoveredItem = null)}>
          <div class="cover-wrap">
            <PixelCover src={coverUrl(t.cover)} seed={trackCoverSeed(t)} alt={t.title} />
          </div>
          {#if player}
            <span class="play-slot" title={t.file_path ? undefined : 'Not downloaded yet'}>
              <button class="artwork-play" aria-label={`${player.isPlaying(t.id) ? 'Pause' : 'Play'} ${t.title}`}
                onclick={() => player?.play(t, lib.filteredTracks)} disabled={!t.file_path}>
                <i class="pxi {player.isPlaying(t.id) ? 'pxi-pause' : 'pxi-play'}" aria-hidden="true"></i>
              </button>
            </span>
          {/if}
          <div class="card-body">
            <div class="card-title" title={t.title}>
              {#if player?.isCurrent(t.id)}<i class="pxi pxi-volume-3 live-glyph" aria-hidden="true"></i>{/if}{t.title}
            </div>
            <div class="card-sub">{t.artists.map(a => a.name).join(', ') || '\u2014'}</div>
            {#if t.duration != null || t.quality}
              <div class="card-foot">
                {#if t.duration != null}<span class="card-meta">{lib.fmtDuration(t.duration)}</span>{/if}
                <QualityBadge quality={t.quality} />
              </div>
            {/if}
          </div>
          {#if t.needs_validation}<span class="card-badge badge-warn" title="Awaiting validation" aria-label="Awaiting validation"><i class="pxi pxi-warning-diamond" aria-hidden="true"></i></span>{/if}
          <CardActions title={t.title} actions={[
            { label: t.rating === 'liked' ? 'Remove like' : 'Like track', pressed: t.rating === 'liked', onSelect: () => lib.setRating(t, t.rating === 'liked' ? null : 'liked') },
            { label: t.rating === 'disliked' ? 'Remove dislike' : 'Dislike track', pressed: t.rating === 'disliked', onSelect: () => lib.setRating(t, t.rating === 'disliked' ? null : 'disliked') },
            { label: 'Edit track', onSelect: () => lib.startEditTrack(t) },
          ]} />
        </div>
      {/each}
    </div>
  {/if}
  {#if lib.filteredTracks.length === 0}
    <div class="empty">
      <i class="pxi pxi-music" aria-hidden="true"></i>
      <p class="empty-title">No tracks found.</p>
    </div>
  {/if}
{/if}

<style>
  .status { display: flex; align-items: center; justify-content: center; gap: 8px; }
  .status .pxi { font-size: 16px; }

  .search { padding-right: 36px; }
  .search-kbd {
    position: absolute;
    right: 8px;
    top: 50%;
    translate: 0 -50%;
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--muted-2);
    font-family: var(--font-mono);
    font-size: 11px;
    pointer-events: none;
  }
  .filter-count { font-variant-numeric: tabular-nums; color: var(--muted-2); }
  .filter-btn.active .filter-count { color: var(--text-bright); }
  .tools { margin-left: auto; }

  .review-hint {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 44px;
    margin: 0 0 12px;
    padding: 0 14px;
    border: 1px solid color-mix(in srgb, var(--warning) 35%, transparent);
    border-radius: var(--radius-control);
    background: var(--warning-bg);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .review-hint > .pxi { font-size: 16px; color: var(--warning); flex: 0 0 auto; }
  .review-hint-action {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    color: var(--text-bright);
    font-weight: 500;
    white-space: nowrap;
  }
  .review-hint-action .pxi { font-size: 16px; }
  .review-hint:hover { border-color: color-mix(in srgb, var(--warning) 60%, transparent); }

  /* The playing card: cyan marks what is happening now. */
  .card.playing .cover-wrap::after { box-shadow: inset 0 0 0 2px var(--live); }
  .card.playing .card-title { color: var(--live); }
  .live-glyph { font-size: 16px; margin-right: 6px; vertical-align: -3px; }

  .card-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    margin-top: 4px;
  }
  .card-foot .card-meta { margin-top: 0; }

  .play-slot { position: absolute; top: 8px; left: 8px; z-index: 2; display: flex; }
  .artwork-play {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background: var(--text-bright);
    color: var(--bg);
    font-size: 24px;
    cursor: pointer;
  }
  .artwork-play:hover:not(:disabled) { background: color-mix(in srgb, var(--text-bright) 86%, var(--bg)); }
  .artwork-play:disabled { border-color: var(--border-strong); background: color-mix(in srgb, var(--bg) 78%, transparent); color: var(--muted); cursor: default; }
  .card-badge { top: 60px; left: 8px; }
  .badge-warn .pxi { font-size: 16px; }
  .badge-warn { width: 24px; height: 24px; }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .search-kbd { display: none; }
    .tools { margin-left: 0; }
  }
</style>
