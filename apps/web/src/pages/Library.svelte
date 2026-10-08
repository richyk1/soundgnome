<script lang="ts">
  import { setContext, getContext } from 'svelte';
  import { lib, LIBRARY_PLAYER, createLibraryPlayer, type LibraryPlayer, type Tab } from '../lib/library/store.svelte';
  import ArtistTab from '../lib/library/ArtistTab.svelte';
  import AlbumTab from '../lib/library/AlbumTab.svelte';
  import TracksTab from '../lib/library/TracksTab.svelte';
  import PlaylistsTab from '../lib/library/PlaylistsTab.svelte';
  import EditModal from '../lib/library/EditModal.svelte';
  import { GLOBAL_PLAYER, type GlobalPlayer } from '../lib/player';
  import { runNavigation } from '../lib/navigation-motion';
  let { onNavigateLiked }: { onNavigateLiked?: () => void } = $props();

  const LIB_TABS: { id: Tab; label: string }[] = [
    { id: 'tracks', label: 'Tracks' },
    { id: 'playlists', label: 'Playlists' },
    { id: 'albums', label: 'Albums' },
    { id: 'artists', label: 'Artists' },
  ];

  // ── Playback: driven by the single app-wide player mounted in the shell,
  // so it keeps playing as the user navigates away from the library. ─────────
  const player = getContext<GlobalPlayer>(GLOBAL_PLAYER);

  setContext<LibraryPlayer>(LIBRARY_PLAYER, createLibraryPlayer(player, () => lib.filteredTracks));

  // Refresh all collections when arriving on this page.
  $effect(() => {
    lib.loadAll();
  });

  // Browser back / forward
  $effect(() => {
    function onPopState() { runNavigation(() => lib.applyHash()); }
    window.addEventListener('popstate', onPopState);
    return () => window.removeEventListener('popstate', onPopState);
  });

  // Keyboard shortcuts
  $effect(() => {
    function onKeydown(e: KeyboardEvent) {
      const tgt = e.target as HTMLElement;
      const inInput = tgt.tagName === 'INPUT' || tgt.tagName === 'TEXTAREA' || tgt.tagName === 'SELECT';
      if (lib.editState || inInput || tgt.closest('dialog')) return;
      if (e.key === 's') {
        e.preventDefault();
        document.querySelector<HTMLInputElement>('.library-page .search')?.focus();
      } else if (e.key === 'e' && lib.hoveredItem) {
        e.preventDefault();
        lib.openEditForHovered();
      } else if (e.key === 'Backspace' && !e.metaKey && !e.ctrlKey) {
        if (lib.drillAlbumId != null) { e.preventDefault(); runNavigation(() => lib.navigate(lib.tab, lib.drillArtistId ?? undefined)); }
        else if (lib.drillArtistId != null) { e.preventDefault(); runNavigation(() => lib.navigate(lib.tab)); }
      } else if (e.key === 'Escape') {
        if (lib.mergePicking) { e.preventDefault(); lib.cancelMergePicking(); }
        else if (lib.selectedArtistIds.size > 0) { e.preventDefault(); lib.clearArtistSelection(); }
        else if (lib.albumMergePicking) { e.preventDefault(); lib.cancelAlbumMergePicking(); }
        else if (lib.selectedAlbumIds.size > 0) { e.preventDefault(); lib.clearAlbumSelection(); }
      } else if (e.key === 'm' && lib.tab === 'artists' && lib.selectedArtistIds.size >= 2) {
        e.preventDefault();
        if (lib.mergePicking) lib.cancelMergePicking();
        else lib.startMergePicking();
      } else if (e.key === 'm' && lib.tab === 'albums' && lib.selectedAlbumIds.size >= 2) {
        e.preventDefault();
        if (lib.albumMergePicking) lib.cancelAlbumMergePicking();
        else lib.startAlbumMergePicking();
      }
    }
    document.addEventListener('keydown', onKeydown);
    return () => document.removeEventListener('keydown', onKeydown);
  });

  function formatLastRefreshed(d: Date | null): string {
    if (!d) return '';
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  function formatSynced(d: Date | null): string {
    if (!d) return '';
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function headerSub(): string {
    const synced = lib.lastRefreshed ? ` · synced ${formatSynced(lib.lastRefreshed)}` : '';
    if (lib.tab === 'tracks') return `${lib.tracks.length} tracks · ${lib.needsReviewCount} need validation${synced}`;
    if (lib.tab === 'artists') return `${lib.artists.length} artists${synced}`;
    if (lib.tab === 'albums') return `${lib.albums.length} albums${synced}`;
    return `${lib.playlists.length} playlists${synced}`;
  }
</script>

<div class="library-page">
  <!-- Title row and section tabs stay pinned while the collection scrolls. -->
  <div class="lib-top">
  <header class="page-header">
    <div class="header-titles">
      <h1>{lib.tab === 'artists' ? 'Artists' : lib.tab === 'albums' ? 'Albums' : lib.tab === 'tracks' ? 'Tracks' : 'Playlists'}</h1>
      <p class="header-sub">{headerSub()}</p>
      <p class="mobile-count">
        {lib.tab === 'tracks' ? lib.filteredTracks.length : lib.tab === 'artists' ? lib.filteredArtists.length : lib.tab === 'albums' ? lib.filteredAlbums.length : lib.filteredPlaylists.length}
        {lib.tab}{lib.tab === 'tracks' && lib.trackFilter === 'review' ? ' to review' : ''}
      </p>
    </div>
    <div class="header-right">
      <button class="btn-header mobile-liked" aria-label="Open liked tracks" onclick={() => onNavigateLiked?.()}>
        <i class="pxi pxi-heart" aria-hidden="true"></i>
      </button>
      <button class="btn-header" aria-label={lib.refreshing ? 'Refreshing library' : 'Refresh library'} onclick={lib.handleRefresh} disabled={lib.refreshing}>
        <i class="pxi pxi-refresh" class:pxi-spin={lib.refreshing} aria-hidden="true"></i>
        <span class="refresh-label">{lib.refreshing ? 'Refreshing…' : 'Refresh'}</span>
      </button>
    </div>
  </header>

  <nav class="lib-tabs" aria-label="Library sections">
    {#each LIB_TABS as t}
      <button class="lib-tab" class:active={lib.tab === t.id} aria-current={lib.tab === t.id ? 'page' : undefined} onclick={() => runNavigation(() => lib.switchTab(t.id))}>{t.label}</button>
    {/each}
  </nav>
  </div>


  {#if (lib.tab === 'artists' || lib.tab === 'albums') && (lib.batchFetchingArtists || lib.batchFetchingAlbums || lib.batchFetchResult)}
    <div class="batch-tools">
      {#if lib.tab === 'artists'}
        <button class="btn-header" onclick={() => lib.batchFetchArtistIconsAction()} disabled={lib.batchFetchingArtists}>
          <i class="pxi {lib.batchFetchingArtists ? 'pxi-loader pxi-spin' : 'pxi-image'}" aria-hidden="true"></i>
          {lib.batchFetchingArtists ? 'Fetching artist photos…' : 'Fetch artist photos from references'}
        </button>
      {:else if lib.tab === 'albums'}
        <button class="btn-header" onclick={() => lib.batchFetchAlbumCoversAction()} disabled={lib.batchFetchingAlbums}>
          <i class="pxi {lib.batchFetchingAlbums ? 'pxi-loader pxi-spin' : 'pxi-image'}" aria-hidden="true"></i>
          {lib.batchFetchingAlbums ? 'Fetching album covers…' : 'Fetch album covers from references'}
        </button>
      {/if}
      {#if lib.batchFetchResult}
        <span class="batch-result">{lib.batchFetchResult.count} fetched · {lib.batchFetchResult.skipped} not found</span>
      {/if}
    </div>
  {/if}

  {#if lib.drillArtist || lib.drillAlbum || lib.drillArtistId || lib.drillAlbumId}
    <nav class="breadcrumb" aria-label="Breadcrumb">
      <button class="crumb-btn" onclick={() => runNavigation(lib.backToRoot)}>
        {lib.tab === 'artists' ? 'Artists' : 'Albums'}
      </button>
      {#if lib.drillArtist}
        <i class="pxi pxi-chevron-right crumb-sep" aria-hidden="true"></i>
        {#if lib.drillAlbum}
          <button class="crumb-btn" onclick={() => runNavigation(lib.backToArtist)}>{lib.drillArtist.name}</button>
          <i class="pxi pxi-chevron-right crumb-sep" aria-hidden="true"></i>
          <span class="crumb-current" aria-current="page">{lib.drillAlbum.title}</span>
        {:else}
          <span class="crumb-current" aria-current="page">{lib.drillArtist.name}</span>
        {/if}
      {:else if lib.drillAlbum}
        <i class="pxi pxi-chevron-right crumb-sep" aria-hidden="true"></i>
        <span class="crumb-current" aria-current="page">{lib.drillAlbum.title}</span>
      {:else}
        <i class="pxi pxi-chevron-right crumb-sep" aria-hidden="true"></i>
        <span class="crumb-current muted">Loading…</span>
      {/if}
    </nav>
  {:else if lib.drillPlaylistId != null}
    <nav class="breadcrumb" aria-label="Breadcrumb">
      <button class="crumb-btn" onclick={() => runNavigation(() => lib.navigate('playlists'))}>Playlists</button>
      <i class="pxi pxi-chevron-right crumb-sep" aria-hidden="true"></i>
      {#if lib.drillPlaylist}
        <span class="crumb-current" aria-current="page">{lib.drillPlaylist.name}</span>
      {:else}
        <span class="crumb-current muted">Loading…</span>
      {/if}
    </nav>
  {/if}

  {#if lib.tab === 'artists'}
    <ArtistTab />
  {:else if lib.tab === 'albums'}
    <AlbumTab />
  {:else if lib.tab === 'playlists'}
    <PlaylistsTab />
  {:else}
    <TracksTab />
  {/if}
</div>

<EditModal />


<style>
  .library-page {
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-page);
  }

  /* Pinned to the top of the scrolling content panel: it spans the page
     padding edge to edge on an opaque ground so rows pass cleanly beneath. */
  .lib-top {
    position: sticky;
    top: 0;
    z-index: 20;
    margin: calc(-1 * var(--space-page)) calc(-1 * var(--space-page)) 20px;
    padding: var(--space-page) var(--space-page) 16px;
    background: var(--bg);
  }
  .page-header {
    display: flex;
    flex-direction: row;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
  }
  .header-titles { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  h1 {
    margin: 0;
    font-size: 32px;
    line-height: 1.05;
  }
  .header-sub {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }
  .header-right { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
  .mobile-liked, .mobile-count { display: none; }

  /* Phones only: the sidebar carries these sections on desktop. */
  .lib-tabs { display: none; }
  .lib-tab {
    position: relative;
    min-width: 0;
    min-height: 44px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted-2);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .lib-tab::after {
    content: '';
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: -1px;
    height: 2px;
    background: var(--accent);
    transform: scaleX(0);
    transition: transform var(--motion-fast) var(--ease-out);
  }
  .lib-tab.active { color: var(--text-bright); }
  .lib-tab.active::after { transform: scaleX(1); }

  .batch-tools {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    margin-bottom: 24px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }
  .batch-result {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }

  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 20px;
    font-size: 14px;
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
  }
  .crumb-btn {
    padding: 0;
    border: none;
    background: none;
    color: var(--muted);
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
  }
  .crumb-btn:hover { color: var(--text-bright); text-decoration: underline; }
  .crumb-sep { font-size: 12px; color: var(--muted-2); }
  .crumb-current { color: var(--text-bright); font-weight: 500; }
  .muted { color: var(--muted); }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .library-page { min-width: 0; }
    .lib-top { margin-bottom: 12px; padding-top: 12px; padding-bottom: 0; }
    .page-header { align-items: center; gap: 8px; margin-bottom: 4px; }
    h1 { font-size: 28px; }
    .header-sub, .refresh-label { display: none; }
    .mobile-count {
      display: block;
      margin: 0;
      font-family: var(--font-mono);
      font-size: 12px;
      font-variant-numeric: tabular-nums;
      color: var(--muted-2);
    }
    .mobile-liked { display: inline-flex; }
    .header-right { gap: 6px; }
    .header-right .btn-header { width: 44px; height: 44px; padding: 0; }
    /* 24px icons land on whole device pixels at 3×. */
    .header-right .btn-header .pxi { font-size: 24px; }
    .lib-tabs {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      margin: 0 calc(-1 * var(--space-page));
      padding: 0 var(--space-page);
      border-bottom: 1px solid var(--border);
    }
    .breadcrumb { flex-wrap: wrap; overflow-wrap: anywhere; }
    .crumb-btn { white-space: normal; text-align: left; }
  }
</style>
