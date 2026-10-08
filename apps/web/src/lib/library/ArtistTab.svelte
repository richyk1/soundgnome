<script lang="ts">
  import { lib } from './store.svelte';
  import TrackTable from './TrackTable.svelte';
  import SortDropdown from './SortDropdown.svelte';
  import VirtualCardGrid from './VirtualCardGrid.svelte';
  import CardActions from './CardActions.svelte';
  import LibraryOptions from './LibraryOptions.svelte';
  import PixelCover from '../PixelCover.svelte';
  import { runNavigation } from '../navigation-motion';
  import type { LibraryAlbumDto, LibraryTrackDto } from '../types';

  function clearHoveredArtist() {
    if (lib.hoveredItem?.type === 'artist') lib.hoveredItem = null;
  }

  const artistSortOptions = [
    { value: 'name', label: 'Name' },
    { value: 'track_count', label: 'Tracks' },
    { value: 'album_count', label: 'Albums' },
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

  function plural(n: number, word: string): string {
    return `${n} ${word}${n !== 1 ? 's' : ''}`;
  }

  function albumMeta(a: LibraryAlbumDto): string {
    const parts: (string | null | undefined)[] = [a.album_type, a.date?.slice(0, 4)];
    if (!lib.tracksLoading) parts.push(plural(lib.albumTracks.length, 'track'), totalDuration(lib.albumTracks));
    return parts.filter(Boolean).join(' · ');
  }

  function artistMeta(): string {
    const parts: (string | null)[] = ['artist'];
    if (!lib.albumsLoading) parts.push(plural(lib.artistAlbums.length, 'album'));
    if (!lib.tracksLoading) parts.push(plural(lib.artistTracks.length, 'track'), totalDuration(lib.artistTracks));
    return parts.filter(Boolean).join(' · ');
  }
</script>

<!-- ── ALBUM DETAIL (within artist context) ──────────────────────────────── -->
{#if lib.drillArtist && lib.drillAlbum}
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

<!-- ── ARTIST DETAIL ─────────────────────────────────────────────────────── -->
{:else if lib.drillArtist}
  {@const artist = lib.drillArtist}
  <header class="detail-hero">
    <div class="cover-wrap artist-avatar detail-cover">
      <PixelCover src={remoteCover(artist.icon)} seed="artist:{artist.id}" alt={artist.name} loading="eager" />
    </div>
    <div class="detail-info">
      <h2>{artist.name}</h2>
      <p class="detail-meta">{artistMeta()}</p>
      <div class="detail-actions">
        <button class="btn-edit" onclick={() => lib.startEditArtist(artist)}><i class="pxi pxi-pencil" aria-hidden="true"></i>Edit</button>
        <button class="btn-delete" onclick={() => lib.handleDeleteArtist(artist.id)}><i class="pxi pxi-trash" aria-hidden="true"></i>Delete</button>
      </div>
    </div>
  </header>

  <!-- Albums compact grid -->
  {#if lib.albumsLoading}
    <p class="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Loading albums…</p>
  {:else if lib.artistAlbums.length > 0}
    <h3 class="section-title">Albums <span class="section-count">{lib.artistAlbums.length}</span></h3>
    <div class="card-grid compact artist-albums">
      {#each lib.artistAlbums as a (a.id)}
        <div class="card clickable"
          onmouseenter={() => (lib.hoveredItem = { type: 'album', id: a.id })}
          onmouseleave={() => (lib.hoveredItem = null)}
          onclick={() => runNavigation(() => lib.drillIntoAlbum(a))} role="button" tabindex="0"
          onkeydown={(e) => { if (e.target === e.currentTarget && e.key === 'Enter') runNavigation(() => lib.drillIntoAlbum(a)); }}>
          <div class="cover-wrap">
            <PixelCover src={remoteCover(a.cover)} seed="album:{a.id}" alt={a.title} />
          </div>
          <div class="card-body">
            <div class="card-title" title={a.title}>{a.title}</div>
            {#if a.date}<div class="card-meta">{a.date.slice(0, 4)}</div>{/if}
          </div>
          <CardActions title={a.title} actions={[
            { label: 'Edit album', onSelect: () => lib.startEditAlbum(a) },
            { label: 'Delete album', danger: true, onSelect: () => lib.handleDeleteAlbum(a.id) },
          ]} />
        </div>
      {/each}
    </div>
  {/if}

  <!-- Tracks grouped by album -->
  {#if lib.tracksLoading}
    <p class="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Loading tracks…</p>
  {:else if lib.artistTracksByAlbum.length > 0}
    <h3 class="section-title tracks-title">Tracks <span class="section-count">{lib.artistTracks.length}</span></h3>
    {#each lib.artistTracksByAlbum as group (group.albumId ?? '__none__')}
      <section class="album-section">
        <div class="album-section-header">
          {#if group.albumId != null}
            <div class="cover-wrap album-section-thumb">
              <PixelCover src={remoteCover(group.albumCover)} seed="album:{group.albumId}" alt="" />
            </div>
            {@const fullAlbum = lib.albums.find(x => x.id === group.albumId)}
            {#if fullAlbum}
              <span class="album-section-name title-link"
                onclick={() => runNavigation(() => lib.drillIntoAlbum(fullAlbum))}
                role="button" tabindex="0"
                onkeydown={(e) => e.key === 'Enter' && runNavigation(() => lib.drillIntoAlbum(fullAlbum))}>
                {group.albumTitle}
              </span>
            {:else}
              <span class="album-section-name">{group.albumTitle}</span>
            {/if}
          {:else}
            <span class="album-section-name muted">No album</span>
          {/if}
          <span class="section-count album-section-count">{plural(group.tracks.length, 'track')}</span>
        </div>
        <TrackTable tracks={group.tracks} showAlbumCol={false} />
      </section>
    {/each}
  {:else if !lib.albumsLoading && lib.artistAlbums.length === 0}
    <div class="empty">
      <i class="pxi pxi-user" aria-hidden="true"></i>
      <p class="empty-title">No content found for this artist.</p>
    </div>
  {/if}

<!-- ── LOADING / ERROR ────────────────────────────────────────────────────── -->
{:else if (lib.drillArtistId != null && !lib.artistsLoaded) || lib.artistsLoading}
  <p class="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Loading…</p>
{:else if lib.artistsError}
  <div class="callout callout-error" role="alert">
    <i class="pxi pxi-square-alert" aria-hidden="true"></i>
    <div class="callout-body"><strong>Couldn't load artists</strong><span>{lib.artistsError}</span></div>
  </div>

<!-- ── ARTISTS LIST / GRID ───────────────────────────────────────────────── -->
{:else}
  <LibraryOptions>
    {#snippet search()}
      <i class="pxi pxi-search" aria-hidden="true"></i>
      <input class="search" aria-label="Search artists" placeholder="Search artists…" bind:value={lib.artistSearch} />
    {/snippet}
    {#snippet children()}
      <div class="opt-row">
        <button
          class="filter-btn"
          class:active={lib.similarFilterActive}
          aria-pressed={lib.similarFilterActive}
          onclick={() => { lib.similarFilterActive = !lib.similarFilterActive; }}
          title="Dim artists that have no similar-named peer — helps spot duplicates"
        >
          <i class="pxi pxi-copy" aria-hidden="true"></i>
          Similar
          {#if lib.similarFilterActive && lib.similarArtistIds.size > 0}
            <span class="mini-badge">{lib.similarArtistIds.size}</span>
          {/if}
        </button>
      </div>
      <div class="opt-row tools">
        <SortDropdown
          value={lib.artistsSortBy}
          direction={lib.artistsSortDir}
          options={artistSortOptions}
          onChange={(val) => lib.artistsSortBy = val as any}
          onDirectionChange={(dir) => lib.artistsSortDir = dir}
        />
        <div class="view-toggle" role="group" aria-label="View">
          <button class:active={lib.artistsView === 'list'} aria-pressed={lib.artistsView === 'list'} onclick={() => (lib.artistsView = 'list')} title="List" aria-label="List view">
            <i class="pxi pxi-list-box" aria-hidden="true"></i>
          </button>
          <button class:active={lib.artistsView === 'grid'} aria-pressed={lib.artistsView === 'grid'} onclick={() => (lib.artistsView = 'grid')} title="Grid" aria-label="Grid view">
            <i class="pxi pxi-grid-2x2-2" aria-hidden="true"></i>
          </button>
        </div>
      </div>
      <span class="count">{lib.filteredArtists.length} artist{lib.filteredArtists.length !== 1 ? 's' : ''}</span>
    {/snippet}
  </LibraryOptions>

  {#if lib.artistsView === 'list'}
    <div class="table-wrap">
      <table>
        <thead><tr><th>#</th><th>Name</th><th class="col-actions">Actions</th></tr></thead>
        <tbody>
          {#each lib.filteredArtists as a (a.id)}
            {@const sel = lib.selectedArtistIds.has(a.id)}
            {@const dimmed = lib.similarFilterActive && !lib.similarArtistIds.has(a.id)}
            {@const pickable = lib.mergePicking && sel}
            <tr
              class:row-selected={sel}
              class:row-dimmed={dimmed}
              class:row-pickable={pickable}
              style={lib.mergePicking && !sel ? 'opacity:0.3;pointer-events:none' : ''}
              onmouseenter={() => (lib.hoveredItem = { type: 'artist', id: a.id })}
              onmouseleave={() => (lib.hoveredItem = null)}
              onclick={(e) => {
                if (lib.mergePicking) { if (sel) lib.pickMergeTarget(a.id); }
                else if (e.shiftKey) { e.preventDefault(); lib.toggleArtistSelection(a.id); }
                else { runNavigation(() => lib.drillIntoArtist(a)); }
              }}
            >
              <td class="mono">{a.id}</td>
              <td>
                <span class="row-title">
                  <span class="artist-name" class:muted={dimmed}>{a.name}</span>
                  {#if sel}<span class="badge-sel" aria-label="Selected"><i class="pxi pxi-check" aria-hidden="true"></i></span>{/if}
                </span>
              </td>
              <td class="row-actions">
                {#if !lib.mergePicking}
                  <button class="btn-header btn-sm btn-select" aria-pressed={sel} onclick={(e) => { e.stopPropagation(); lib.toggleArtistSelection(a.id); }}>{sel ? 'Selected' : 'Select'}</button>
                  <button class="btn-edit btn-sm" onclick={(e) => { e.stopPropagation(); lib.startEditArtist(a); }}>Edit</button>
                  <button class="btn-delete btn-sm" onclick={(e) => { e.stopPropagation(); lib.handleDeleteArtist(a.id); }}>Delete</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <VirtualCardGrid items={lib.filteredArtists} onWindowChange={clearHoveredArtist}>
      {#snippet card(a)}
        {@const sel = lib.selectedArtistIds.has(a.id)}
        {@const dimmed = lib.similarFilterActive && !lib.similarArtistIds.has(a.id)}
        {@const pickable = lib.mergePicking && sel}
        <div class="card artist-card"
          class:clickable={!lib.mergePicking}
          class:card-selected={sel}
          class:card-dimmed={dimmed}
          class:card-pickable={pickable}
          style={lib.mergePicking && !sel ? 'opacity:0.3;pointer-events:none' : ''}
          onmouseenter={() => (lib.hoveredItem = { type: 'artist', id: a.id })}
          onmouseleave={() => (lib.hoveredItem = null)}
          onclick={(e) => {
            if (lib.mergePicking) { if (sel) lib.pickMergeTarget(a.id); }
            else if (e.shiftKey) { e.preventDefault(); lib.toggleArtistSelection(a.id); }
            else { runNavigation(() => lib.drillIntoArtist(a)); }
          }}
          role="button" tabindex="0"
          onkeydown={(e) => {
            if (e.target !== e.currentTarget) return;
            if (e.key === 'Enter') {
              if (lib.mergePicking && sel) lib.pickMergeTarget(a.id);
              else if (!lib.mergePicking) runNavigation(() => lib.drillIntoArtist(a));
            } else if (e.key === ' ') { e.preventDefault(); lib.toggleArtistSelection(a.id); }
          }}
        >
          <div class="cover-wrap artist-avatar">
            <PixelCover src={remoteCover(a.icon)} seed="artist:{a.id}" alt={a.name} />
          </div>
          <div class="card-body artist-card-body">
            <div class="card-title" title={a.name}>{a.name}</div>
          </div>
          {#if sel}<span class="card-sel-badge" aria-label="Selected"><i class="pxi pxi-check" aria-hidden="true"></i></span>{/if}
          {#if !lib.mergePicking}
            <CardActions title={a.name} actions={[
              { label: sel ? 'Selected' : 'Select', pressed: sel, onSelect: () => lib.toggleArtistSelection(a.id) },
              { label: 'Edit artist', onSelect: () => lib.startEditArtist(a) },
              { label: 'Delete artist', danger: true, onSelect: () => lib.handleDeleteArtist(a.id) },
            ]} />
          {/if}
        </div>
      {/snippet}
    </VirtualCardGrid>
  {/if}
  {#if lib.filteredArtists.length === 0}
    <div class="empty">
      <i class="pxi pxi-users" aria-hidden="true"></i>
      <p class="empty-title">No artists found.</p>
    </div>
  {/if}

  <!-- Floating merge / pick-target button -->
  {#if lib.selectedArtistIds.size >= 2}
    <div class="merge-fab" class:fab-picking={lib.mergePicking}>
      {#if lib.mergePicking}
        <span class="fab-hint">
          <i class="pxi pxi-check" aria-hidden="true"></i>
          Click the artist to keep
        </span>
        <button class="btn-cancel btn-sm" onclick={lib.cancelMergePicking} disabled={lib.mergeSaving}>Cancel</button>
      {:else}
        <span class="fab-count">{lib.selectedArtistIds.size}</span>
        <button class="btn-save btn-sm" onclick={lib.startMergePicking} disabled={lib.mergeSaving}>
          {#if lib.mergeSaving}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Merging…{:else}Merge{/if}
        </button>
        <button class="btn-ghost btn-sm fab-close" onclick={lib.clearArtistSelection} title="Cancel selection" aria-label="Cancel selection">
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

  /* Artist detail: albums strip and per-album track sections with hairline headers. */
  .artist-albums { margin-bottom: 40px; }
  .tracks-title { margin-bottom: 16px; padding-bottom: 0; border-bottom: 0; }
  .album-section { margin-bottom: 32px; }
  .album-section-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 8px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
  }
  .album-section-thumb { width: 32px; height: 32px; flex: 0 0 auto; border-radius: var(--radius-chip); }
  .album-section-name { min-width: 0; font-size: 14px; font-weight: 600; letter-spacing: -0.01em; color: var(--text-bright); overflow-wrap: anywhere; }
  .album-section-name.muted { color: var(--muted); font-weight: 500; }
  .album-section-count { margin-left: auto; white-space: nowrap; }
  .artist-name { color: var(--text-bright); font-weight: 500; }

  /* Artist grid: same card grammar as AlbumGrid, circular avatars. */
  .artist-card-body { height: 4rem; box-sizing: border-box; text-align: center; }
  .card-selected :global(.cover-wrap)::after { box-shadow: inset 0 0 0 2px var(--accent); }
  .card-dimmed { opacity: 0.2; }
  .card-pickable { cursor: pointer; }
  .card-pickable:hover :global(.cover-wrap)::after { box-shadow: inset 0 0 0 2px var(--success); }
  .card-sel-badge {
    position: absolute; top: 8px; left: 8px; z-index: 2;
    display: grid; place-items: center;
    width: 24px; height: 24px; border-radius: 4px;
    background: var(--accent); color: var(--on-accent); font-size: 16px;
  }
  .card-pickable .card-sel-badge { background: var(--success); color: var(--on-success); }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .detail-hero { align-items: flex-start; gap: 16px; margin-bottom: 24px; }
    .detail-info h2 { font-size: 24px; }
    .fab-close { width: 44px; }
    .album-section-header { flex-wrap: wrap; }
  }
</style>
