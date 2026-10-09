<script lang="ts">
  import { lib } from './store.svelte';
  import TrackTable from './TrackTable.svelte';
  import SortDropdown from './SortDropdown.svelte';
  import AlbumGrid from './AlbumGrid.svelte';
  import LibraryOptions from './LibraryOptions.svelte';
  import PixelCover from '../PixelCover.svelte';
  import { runNavigation } from '../navigation-motion';
  import type { LibraryAlbumDto, LibraryTrackDto } from '../types';

  const albumSortOptions = [
    { value: 'title', label: 'Title' },
    { value: 'date', label: 'Date' },
    { value: 'artist', label: 'Artist' },
    { value: 'track_count', label: 'Tracks' },
  ];

  function remoteCover(src: string | null | undefined): string | null {
    return src && /^https?:\/\//.test(src) ? src : null;
  }

  /** Total running time, e.g. 48:31 or 1:12:05. */
  function totalDuration(tracks: LibraryTrackDto[]): string | null {
    const secs = tracks.reduce((sum, t) => sum + (t.duration ?? 0), 0);
    if (secs <= 0) return null;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = String(Math.floor(secs % 60)).padStart(2, '0');
    return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
  }

  function albumMeta(a: LibraryAlbumDto): string {
    const parts: (string | null | undefined)[] = [a.album_type, a.date?.slice(0, 4)];
    if (!lib.tracksLoading) {
      parts.push(`${lib.albumTracks.length} track${lib.albumTracks.length !== 1 ? 's' : ''}`);
      parts.push(totalDuration(lib.albumTracks));
    }
    return parts.filter(Boolean).join(' · ');
  }
</script>

<!-- ── ALBUM DETAIL ──────────────────────────────────────────────────────── -->
{#if lib.drillAlbum}
  {@const album = lib.drillAlbum}
  <header class="detail-hero">
    <div class="cover-wrap detail-cover">
      <PixelCover src={remoteCover(album.cover)} seed="album:{album.id}" alt={album.title} loading="eager" />
    </div>
    <div class="detail-info">
      <h2>{album.title}</h2>
      {#if album.artists.length}<p class="detail-sub">{album.artists.map(a => a.name).join(', ')}</p>{/if}
      <p class="detail-meta">{albumMeta(album)}</p>
      <div class="detail-actions">
        <button class="btn-edit" onclick={() => lib.startEditAlbum(album)}><i class="pxi pxi-pencil" aria-hidden="true"></i>Edit</button>
        <button class="btn-delete" onclick={() => lib.handleDeleteAlbum(album.id)}><i class="pxi pxi-trash" aria-hidden="true"></i>Delete</button>
      </div>
    </div>
  </header>
  <h3 class="section-title">
    Tracks
    {#if lib.tracksLoading}<i class="pxi pxi-loader pxi-spin section-count" aria-label="Loading tracks"></i>{:else}<span class="section-count">{lib.albumTracks.length}</span>{/if}
  </h3>
  <TrackTable tracks={lib.albumTracks} showAlbumCol={false} />
  {#if !lib.tracksLoading && lib.albumTracks.length === 0}<p class="status">No tracks in this album.</p>{/if}

<!-- ── LOADING / ERROR ────────────────────────────────────────────────────── -->
{:else if (lib.drillAlbumId != null && !lib.albumsLoaded) || lib.albumsLoading}
  <p class="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Loading…</p>
{:else if lib.albumsError}
  <div class="callout callout-error" role="alert">
    <i class="pxi pxi-square-alert" aria-hidden="true"></i>
    <div class="callout-body"><strong>Couldn't load albums</strong><span>{lib.albumsError}</span></div>
  </div>

<!-- ── ALBUMS LIST / GRID ────────────────────────────────────────────────── -->
{:else}
  <LibraryOptions>
    {#snippet search()}
      <i class="pxi pxi-search" aria-hidden="true"></i>
      <input class="search" aria-label="Search albums or artists" placeholder="Search albums or artists…" bind:value={lib.albumSearch} />
    {/snippet}
    {#snippet children()}
      <div class="opt-row">
        <button
          class="filter-btn"
          class:active={lib.albumSimilarFilterActive}
          aria-pressed={lib.albumSimilarFilterActive}
          onclick={() => { lib.albumSimilarFilterActive = !lib.albumSimilarFilterActive; }}
          title="Dim albums that have no similar-titled peer — helps spot duplicates"
        >
          <i class="pxi pxi-copy" aria-hidden="true"></i>
          Similar
          {#if lib.albumSimilarFilterActive && lib.similarAlbumIds.size > 0}
            <span class="mini-badge">{lib.similarAlbumIds.size}</span>
          {/if}
        </button>
      </div>
      <div class="opt-row tools">
        <SortDropdown
          value={lib.albumsSortBy}
          direction={lib.albumsSortDir}
          options={albumSortOptions}
          onChange={(val) => lib.albumsSortBy = val as any}
          onDirectionChange={(dir) => lib.albumsSortDir = dir}
        />
        <div class="view-toggle" role="group" aria-label="View">
          <button class:active={lib.albumsView === 'list'} aria-pressed={lib.albumsView === 'list'} onclick={() => (lib.albumsView = 'list')} title="List" aria-label="List view">
            <i class="pxi pxi-list-box" aria-hidden="true"></i>
          </button>
          <button class:active={lib.albumsView === 'grid'} aria-pressed={lib.albumsView === 'grid'} onclick={() => (lib.albumsView = 'grid')} title="Grid" aria-label="Grid view">
            <i class="pxi pxi-grid-2x2-2" aria-hidden="true"></i>
          </button>
        </div>
      </div>
      <span class="count">{lib.filteredAlbums.length} album{lib.filteredAlbums.length !== 1 ? 's' : ''}</span>
    {/snippet}
  </LibraryOptions>

  {#if lib.albumsView === 'list'}
    <div class="table-wrap">
      <table>
        <thead><tr><th>#</th><th>Title</th><th>Artists</th><th>Type</th><th>Date</th><th class="col-actions">Actions</th></tr></thead>
        <tbody>
          {#each lib.filteredAlbums as a (a.id)}
            {@const sel = lib.selectedAlbumIds.has(a.id)}
            {@const dimmed = lib.albumSimilarFilterActive && !lib.similarAlbumIds.has(a.id)}
            {@const pickable = lib.albumMergePicking && sel}
            <tr
              class:row-selected={sel}
              class:row-dimmed={dimmed}
              class:row-pickable={pickable}
              style={lib.albumMergePicking && !sel ? 'opacity:0.3;pointer-events:none' : ''}
              onmouseenter={() => (lib.hoveredItem = { type: 'album', id: a.id })}
              onmouseleave={() => (lib.hoveredItem = null)}
              onclick={(e) => {
                if (lib.albumMergePicking) { if (sel) lib.pickAlbumMergeTarget(a.id); }
                else if (e.shiftKey) { e.preventDefault(); lib.toggleAlbumSelection(a.id); }
              }}
            >
              <td class="mono">{a.id}</td>
              <td>
                <span class="row-title">
                  <span class="title-link" class:muted={dimmed} onclick={(e) => { if (!lib.albumMergePicking && !e.shiftKey) runNavigation(() => lib.drillIntoAlbum(a)); }} role="button" tabindex="0"
                    onkeydown={(e) => e.key === 'Enter' && runNavigation(() => lib.drillIntoAlbum(a))}>{a.title}</span>
                  {#if sel}<span class="badge-sel" aria-label="Selected"><i class="pxi pxi-check" aria-hidden="true"></i></span>{/if}
                </span>
              </td>
              <td class="muted">{a.artists.map(x => x.name).join(', ') || '\u2014'}</td>
              <td class="muted">{a.album_type}</td>
              <td class="mono">{a.date ?? '\u2014'}</td>
              <td class="row-actions">
                {#if !lib.albumMergePicking}
                  <button class="btn-header btn-sm btn-select" aria-pressed={sel} onclick={(e) => { e.stopPropagation(); lib.toggleAlbumSelection(a.id); }}>{sel ? 'Selected' : 'Select'}</button>
                  <button class="btn-edit btn-sm" onclick={(e) => { e.stopPropagation(); lib.startEditAlbum(a); }}>Edit</button>
                  <button class="btn-delete btn-sm" onclick={(e) => { e.stopPropagation(); lib.handleDeleteAlbum(a.id); }}>Delete</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <AlbumGrid />
  {/if}
  {#if lib.filteredAlbums.length === 0}
    <div class="empty">
      <i class="pxi pxi-album" aria-hidden="true"></i>
      <p class="empty-title">No albums found.</p>
    </div>
  {/if}

  <!-- Floating merge / pick-target button -->
  {#if lib.selectedAlbumIds.size >= 2}
    <div class="merge-fab" class:fab-picking={lib.albumMergePicking}>
      {#if lib.albumMergePicking}
        <span class="fab-hint">
          <i class="pxi pxi-check" aria-hidden="true"></i>
          Click the album to keep
        </span>
        <button class="btn-cancel btn-sm" onclick={lib.cancelAlbumMergePicking} disabled={lib.albumMergeSaving}>Cancel</button>
      {:else}
        <span class="fab-count">{lib.selectedAlbumIds.size}</span>
        <button class="btn-save btn-sm" onclick={lib.startAlbumMergePicking} disabled={lib.albumMergeSaving}>
          {#if lib.albumMergeSaving}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Merging…{:else}Merge{/if}
        </button>
        <button class="btn-ghost btn-sm fab-close" onclick={lib.clearAlbumSelection} title="Cancel selection" aria-label="Cancel selection">
          <i class="pxi pxi-close" aria-hidden="true"></i>
        </button>
      {/if}
    </div>
  {/if}
{/if}

<style>
  /* Detail hero: no card chrome; the cover carries the hairline. */
  .detail-hero { display: flex; align-items: flex-end; gap: 20px; margin-bottom: 32px; }
  .detail-cover { width: 120px; height: 120px; flex: 0 0 auto; }
  .detail-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .detail-info h2 { margin: 0; font-size: 28px; line-height: 1.15; letter-spacing: -0.03em; overflow-wrap: anywhere; }
  .detail-sub { margin: 0; font-size: 14px; color: var(--muted); }
  .detail-meta { margin: 0; font-family: var(--font-mono); font-size: 12px; font-variant-numeric: tabular-nums; color: var(--muted-2); }
  .detail-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }

  .section-title { display: flex; align-items: baseline; gap: 8px; }
  .section-count { font-family: var(--font-mono); font-size: 12px; font-weight: 400; font-variant-numeric: tabular-nums; color: var(--muted-2); }
  i.section-count { font-size: 16px; align-self: center; }

  .status { display: flex; align-items: center; justify-content: center; gap: 8px; }
  .status .pxi { font-size: 16px; }
  .tools { margin-left: auto; }

  /* Row selection */
  .row-title { display: inline-flex; align-items: center; gap: 8px; }
  .muted { color: var(--muted); }
  tr.row-selected td { background: var(--accent-muted); }
  tr.row-dimmed { opacity: 0.2; }
  tr.row-pickable { cursor: pointer; }
  tr.row-pickable:hover td { background: color-mix(in srgb, var(--success) 14%, transparent); }
  .badge-sel {
    display: inline-grid; place-items: center;
    width: 18px; height: 18px; border-radius: 4px;
    background: var(--accent); color: var(--on-accent); font-size: 16px;
  }
  tr.row-pickable .badge-sel { background: var(--success); color: var(--on-success); }
  .row-actions { white-space: nowrap; }
  .row-actions button + button { margin-left: 4px; }
  .btn-select[aria-pressed="true"] { color: var(--accent); border-color: color-mix(in srgb, var(--accent) 45%, transparent); background: var(--accent-muted); }

  /* Floating merge bar */
  .merge-fab {
    position: fixed;
    left: 50%;
    top: calc(var(--app-top, 0px) + var(--app-height, 100dvh) - var(--app-bottom-clearance, 144px));
    transform: translate(-50%, -100%);
    z-index: 200;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: max-content;
    max-width: calc(100vw - 2rem);
    max-height: calc(var(--app-height, 100dvh) - var(--app-bottom-clearance, 144px) - 2rem);
    overflow-y: auto;
    box-sizing: border-box;
    padding: 8px 8px 8px 12px;
    border: 1px solid var(--float-border);
    border-radius: var(--radius-panel);
    background: var(--float);
    box-shadow: var(--float-shadow);
    font-size: 14px;
  }
  .merge-fab.fab-picking { border-color: color-mix(in srgb, var(--success) 50%, transparent); }
  .fab-count {
    min-width: 24px; padding: 2px 6px; border-radius: 4px;
    background: var(--accent); color: var(--on-accent);
    font-family: var(--font-mono); font-size: 12px; font-weight: 600; font-variant-numeric: tabular-nums; text-align: center;
  }
  .fab-hint { display: flex; align-items: center; gap: 8px; color: var(--success); font-weight: 500; }
  .fab-hint .pxi { font-size: 16px; }
  .fab-close { padding: 0; width: 30px; border-color: transparent; }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .detail-hero { align-items: flex-start; gap: 16px; margin-bottom: 24px; }
    .detail-info h2 { font-size: 24px; }
    .fab-close { width: 44px; }
  }
</style>
