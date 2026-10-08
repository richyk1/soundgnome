<script lang="ts">
  import { onMount, setContext } from 'svelte';
  import { getActiveTasksCount, getVersion, getSoundcloudStreamUrl } from './lib/api';
  import { lib } from './lib/library/store.svelte';
  import Home from './pages/Home.svelte';
  import Validations from './pages/Validations.svelte';
  import Tasks from './pages/Tasks.svelte';
  import Library from './pages/Library.svelte';
  import Tools from './pages/Tools.svelte';
  import Ingest from './pages/Ingest.svelte';
  import Likes from './pages/Likes.svelte';
  import Search from './pages/Search.svelte';
  import HelpModal from './lib/HelpModal.svelte';
  import PWAUpdatePrompt from './components/PWAUpdatePrompt.svelte';
  import InstallPrompt from './components/InstallPrompt.svelte';
  import AudioPlayer from './lib/AudioPlayer.svelte';
  import TabBar from './lib/TabBar.svelte';
  import BrandMark from './lib/BrandMark.svelte';
  import PixelCover from './lib/PixelCover.svelte';
  import { observeViewport } from './lib/viewport';
  import { observeTheme } from './lib/theme.svelte';
  import { runNavigation } from './lib/navigation-motion';
  import {
    GLOBAL_PLAYER,
    type GlobalPlayer,
    type PlayerTrack,
    type PlayerHandle,
  } from './lib/player';
  type Page = 'download' | 'validations' | 'tasks' | 'library' | 'tools' | 'ingest' | 'likes' | 'search';
  type LibraryTab = 'artists' | 'albums' | 'tracks' | 'playlists';

  let page: Page = $state('library');
  let activeTasksCount = $state(0);
  let helpOpen = $state(false);
  let version = $state('');
  let moreOpen = $state(false);
  let moreTrigger: HTMLButtonElement | undefined;
  let contentPanel: HTMLElement | undefined = $state();

  // One audio player for the whole app, mounted in the shell (below) so playback
  // and the player bar persist across navigation. Pages drive it via context.
  let player: PlayerHandle | null = $state(null);
  let playError: string | null = $state(null);
  let hasTrack = $state(false);
  let upNext: PlayerTrack[] = $state([]);

  $effect(() => {
    document.documentElement.toggleAttribute('data-player-loaded', hasTrack);
  });

  function resolveSrc(track: PlayerTrack): string | Promise<string> {
    return track.source === 'soundcloud'
      ? getSoundcloudStreamUrl(track.id)
      : `/api/tracks/${track.id}/audio`;
  }

  setContext<GlobalPlayer>(GLOBAL_PLAYER, {
    play: (track, queue) => {
      playError = null;
      player?.toggle(track, queue);
    },
    isCurrent: (id, source) => player?.isCurrent(id, source) ?? false,
    isPlaying: (id, source) => player?.isPlaying(id, source) ?? false,
    isResolving: (id, source) => player?.isResolving(id, source) ?? false,
  });

  async function refreshCounts() {
    try {
      activeTasksCount = await getActiveTasksCount();
    } catch {
      // ignore
    }
  }

  onMount(() => {
    const stopViewport = observeViewport();
    const stopTheme = observeTheme();
    refreshCounts();
    lib.loadAll();
    getVersion().then((v) => (version = v));
    const interval = setInterval(refreshCounts, 5_000);

    function onKeydown(e: KeyboardEvent) {
      const tgt = e.target as HTMLElement;
      if (tgt.tagName === 'INPUT' || tgt.tagName === 'TEXTAREA' || tgt.tagName === 'SELECT' || tgt.isContentEditable)
        return;
      if (e.key === '?') {
        e.preventDefault();
        helpOpen = !helpOpen;
        return;
      }
      if (e.key === ' ' || e.code === 'Space') {
        if (e.defaultPrevented || tgt.closest('button, a, [role="button"], summary')) return;
        e.preventDefault();
        player?.playPause();
        return;
      }
    }
    document.addEventListener('keydown', onKeydown);

    return () => {
      stopViewport();
      stopTheme();
      clearInterval(interval);
      document.removeEventListener('keydown', onKeydown);
      document.documentElement.removeAttribute('data-player-loaded');
    };
  });

  function navigate(to: Page) {
    void runNavigation(() => {
      page = to;
      contentPanel?.scrollTo({ top: 0 });
      if (to === 'validations') refreshCounts();
    });
  }

  // Select the tab and page in one snapshot, preserving library hash history.
  function goLibraryTab(tab: LibraryTab) {
    void runNavigation(() => {
      lib.switchTab(tab);
      page = 'library';
      contentPanel?.scrollTo({ top: 0 });
    });
  }

  function showMore(node: HTMLDialogElement) {
    node.showModal();
    return { destroy: () => {
      node.close();
      moreTrigger?.focus({ preventScroll: true });
    } };
  }

  function closeMoreBackdrop(event: MouseEvent) {
    if (event.target !== event.currentTarget) return;
    const bounds = (event.currentTarget as HTMLDialogElement).getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right
      || event.clientY < bounds.top || event.clientY > bounds.bottom) moreOpen = false;
  }

  const primaryNav: { id: Page; label: string; icon: string }[] = [
    { id: 'library', label: 'Library', icon: 'library' },
    { id: 'download', label: 'Download', icon: 'download' },
    { id: 'ingest', label: 'Ingest', icon: 'upload' },
    { id: 'validations', label: 'Validations', icon: 'checkbox-on' },
  ];
  const secondaryNav: { id: Page; label: string; icon: string }[] = [
    { id: 'likes', label: 'Liked', icon: 'heart' },
    { id: 'tasks', label: 'Activity', icon: 'bell' },
    { id: 'tools', label: 'Tools', icon: 'gear' },
  ];
  const mobileTabs: { id: Page; label: string; icon: string }[] = [
    { id: 'download', label: 'Home', icon: 'home' },
    { id: 'search', label: 'Search', icon: 'search' },
    { id: 'library', label: 'Library', icon: 'library' },
  ];
  const footerItems = $derived([
    ...mobileTabs.map((tab) => ({
      ...tab,
      selected: page === tab.id && !moreOpen,
      current: page === tab.id,
      onSelect: () => navigate(tab.id),
    })),
    {
      id: 'more',
      label: 'More',
      icon: 'menu',
      selected: moreOpen || !mobileTabs.some((tab) => tab.id === page),
      current: !mobileTabs.some((tab) => tab.id === page),
      expanded: moreOpen,
      controls: moreOpen ? 'more-menu' : undefined,
      onSelect: (trigger: HTMLButtonElement) => { moreTrigger = trigger; moreOpen = true; },
    },
  ]);
  const libraryTabs: { id: LibraryTab; label: string; icon: string; count: () => number }[] = [
    { id: 'artists', label: 'Artists', icon: 'mic', count: () => lib.artists.length },
    { id: 'albums', label: 'Albums', icon: 'album', count: () => lib.albums.length },
    { id: 'tracks', label: 'Tracks', icon: 'music', count: () => lib.tracks.length },
    { id: 'playlists', label: 'Playlists', icon: 'bulletlist', count: () => lib.playlists.length },
  ];
</script>

<div class="app-shell">
  <aside class="sidebar">
    <button class="brand" onclick={() => navigate('library')} aria-label="Soundgnome: open the library">
      <BrandMark scale={2} />
      <span class="brand-name">Soundgnome</span>
      {#if version}<span class="brand-ver">v{version}</span>{/if}
    </button>

    <nav class="nav" aria-label="Main">
      {#each primaryNav as item}
        <button class="nav-item" class:active={page === item.id} aria-current={page === item.id ? 'page' : undefined} onclick={() => navigate(item.id)}>
          <i class="pxi pxi-{item.icon}" aria-hidden="true"></i>
          <span class="nav-text">{item.label}</span>
          {#if item.id === 'validations' && lib.needsReviewCount > 0}
            <span class="badge badge-amber" aria-label="{lib.needsReviewCount} to review">{lib.needsReviewCount}</span>
          {/if}
        </button>
      {/each}
    </nav>

    <section class="side-group" aria-labelledby="side-library">
      <h2 class="side-head" id="side-library">
        <span>Your library</span>
        <span class="side-count">{lib.tracks.length} tracks</span>
      </h2>
      {#each libraryTabs as t}
        <button
          class="nav-item"
          class:active={page === 'library' && lib.tab === t.id}
          aria-current={page === 'library' && lib.tab === t.id ? 'page' : undefined}
          onclick={() => goLibraryTab(t.id)}
        >
          <i class="pxi pxi-{t.icon}" aria-hidden="true"></i>
          <span class="nav-text">{t.label}</span>
          {#if t.id === 'tracks' && lib.needsReviewCount > 0}
            <span class="badge badge-amber" aria-label="{lib.needsReviewCount} to review">{lib.needsReviewCount}</span>
          {/if}
          <span class="side-count">{t.count()}</span>
        </button>
      {/each}
    </section>

    <section class="side-group queue-group" aria-labelledby="side-queue">
      <h2 class="side-head" id="side-queue">
        <span>Up next</span>
        {#if upNext.length}<span class="side-count">{upNext.length}</span>{/if}
      </h2>
      <div class="queue">
        {#if upNext.length === 0}
          <p class="queue-empty">Nothing queued. Play an album or a list of tracks to fill it.</p>
        {:else}
          {#each upNext.slice(0, 8) as q}
            <div class="queue-row">
              <div class="queue-art">
                <PixelCover src={q.artwork} seed={q.coverSeed ?? `track:${q.id}`} />
              </div>
              <div class="queue-meta">
                <div class="queue-title">{q.title}</div>
                <div class="queue-artist">{q.artist}</div>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </section>

    <div class="side-foot">
      <InstallPrompt />
      {#each secondaryNav as item}
        <button class="nav-item" class:active={page === item.id} aria-current={page === item.id ? 'page' : undefined} onclick={() => navigate(item.id)}>
          <i class="pxi pxi-{item.icon}" aria-hidden="true"></i>
          <span class="nav-text">{item.label}</span>
          {#if item.id === 'tasks' && activeTasksCount > 0}
            <span class="badge badge-live" aria-label="{activeTasksCount} running">{activeTasksCount}</span>
          {/if}
        </button>
      {/each}
      <button class="nav-item" onclick={() => (helpOpen = true)}>
        <i class="pxi pxi-circle-question" aria-hidden="true"></i>
        <span class="nav-text">Help</span>
        <kbd aria-hidden="true">?</kbd>
      </button>
    </div>
  </aside>

  <main class="content-panel" bind:this={contentPanel}>
    {#if page === 'download'}
      <Home onNavigateTasks={() => navigate('tasks')} />
    {:else if page === 'library'}
      <Library onNavigateLiked={() => navigate('likes')} />
    {:else if page === 'tools'}
      <Tools />
    {:else if page === 'validations'}
      <Validations onDownloaded={refreshCounts} />
    {:else if page === 'ingest'}
      <Ingest />
    {:else if page === 'likes'}
      <Likes />
    {:else if page === 'search'}
      <Search />
    {:else}
      <Tasks onNavigateValidations={() => navigate('validations')} />
    {/if}
  </main>

  <footer class="footer-dock" class:loaded={hasTrack} aria-label="Player and navigation">
    <div class="player-bar" class:idle={!hasTrack}>
      <AudioPlayer
        bind:this={player}
        bind:active={hasTrack}
        bind:upNext
        resolveSrc={resolveSrc}
        onError={(_track, msg) => (playError = msg)}
      />
    </div>
    <div class="mobile-navigation">
      <TabBar items={footerItems} />
    </div>
  </footer>

  {#if playError}
    <div class="play-error" role="alert">
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>
      <span>{playError}</span>
      <button class="play-error-x" onclick={() => (playError = null)} aria-label="Dismiss">
        <i class="pxi pxi-close" aria-hidden="true"></i>
      </button>
    </div>
  {/if}
</div>

<HelpModal open={helpOpen} onClose={() => (helpOpen = false)} />
<PWAUpdatePrompt />

{#if moreOpen}
  <dialog id="more-menu" class="more-sheet" aria-labelledby="more-title" use:showMore
    oncancel={() => (moreOpen = false)}
    onclick={closeMoreBackdrop}>
    <div class="more-header">
      <h2 class="more-title" id="more-title">More</h2>
      <button class="more-close" onclick={() => (moreOpen = false)} aria-label="Close">
        <i class="pxi pxi-close" aria-hidden="true"></i>
      </button>
    </div>
    <button class="more-item" onclick={() => { navigate('ingest'); moreOpen = false; }}>
      <i class="pxi pxi-upload" aria-hidden="true"></i>Ingest local files
    </button>
    <button class="more-item" onclick={() => { navigate('validations'); moreOpen = false; }}>
      <i class="pxi pxi-checkbox-on" aria-hidden="true"></i>Validations
      {#if lib.needsReviewCount > 0}<span class="badge badge-amber" aria-label="{lib.needsReviewCount} to review">{lib.needsReviewCount}</span>{/if}
    </button>
    <button class="more-item" onclick={() => { navigate('likes'); moreOpen = false; }}>
      <i class="pxi pxi-heart" aria-hidden="true"></i>Liked and disliked
    </button>
    <button class="more-item" onclick={() => { navigate('tasks'); moreOpen = false; }}>
      <i class="pxi pxi-bell" aria-hidden="true"></i>Activity
      {#if activeTasksCount > 0}<span class="badge badge-live" aria-label="{activeTasksCount} running">{activeTasksCount}</span>{/if}
    </button>
    <button class="more-item" onclick={() => { navigate('tools'); moreOpen = false; }}>
      <i class="pxi pxi-gear" aria-hidden="true"></i>Tools
    </button>
    <button class="more-item" onclick={() => { helpOpen = true; moreOpen = false; }}>
      <i class="pxi pxi-circle-question" aria-hidden="true"></i>Help
    </button>
  </dialog>
{/if}

<style>
  /* ── Frame: hairline grid on the bare ground ─────────────────────────── */
  .app-shell {
    position: fixed;
    top: var(--app-top);
    left: 0;
    width: 100%;
    height: var(--app-height, 100dvh);
    display: grid;
    grid-template-columns: 244px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr) auto;
    grid-template-areas:
      'side main'
      'dock dock';
    padding: var(--safe-top) var(--safe-right) var(--safe-bottom) var(--safe-left);
    background: var(--bg);
    overflow: hidden;
  }

  /* ── Sidebar ─────────────────────────────────────────────────────────── */
  .sidebar {
    grid-area: side;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 28px;
    padding: 16px 12px 12px;
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 0 8px;
    border: none;
    border-radius: var(--radius-control);
    background: none;
    cursor: pointer;
  }
  .brand-name {
    font-family: var(--font-display);
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--text-bright);
  }
  .brand-ver {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--muted-2);
  }

  .nav,
  .side-group,
  .side-foot {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 36px;
    padding: 0 10px;
    border: none;
    border-radius: var(--radius-control);
    background: none;
    color: var(--muted);
    font-family: var(--font-body);
    font-size: 14px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
  }
  .nav-item .pxi { font-size: 16px; color: var(--muted-2); }
  .nav-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nav-item:hover { background: var(--surface); color: var(--text-bright); }
  .nav-item.active { background: var(--surface-2); color: var(--text-bright); }
  .nav-item.active .pxi { color: var(--accent); }

  .side-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    margin: 0 0 6px;
    padding: 0 10px;
    font-family: var(--font-body);
    font-size: 12px;
    font-weight: 500;
    letter-spacing: 0;
    color: var(--muted-2);
  }
  .side-count {
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }

  .queue-group { flex: 1; min-height: 0; }
  .queue {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    padding: 4px 10px 0;
    overflow-y: auto;
  }
  .queue-empty { margin: 0; font-size: 13px; line-height: 1.45; color: var(--muted-2); }
  .queue-row { display: flex; align-items: center; gap: 10px; }
  .queue-art {
    position: relative;
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    overflow: hidden;
    border-radius: 4px;
    background: var(--surface);
  }
  .queue-art::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .queue-meta { min-width: 0; }
  .queue-title,
  .queue-artist {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .queue-title { font-size: 13px; font-weight: 500; color: var(--text); }
  .queue-artist { font-size: 12px; color: var(--muted-2); }

  .side-foot {
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }
  kbd {
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--muted-2);
  }

  /* Counts are data: mono, tabular, tinted by meaning. */
  .badge {
    min-width: 20px;
    padding: 0 6px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    line-height: 18px;
    font-variant-numeric: tabular-nums;
    text-align: center;
  }
  .badge-amber { background: var(--warning-bg); color: var(--warning); }
  .badge-live { background: color-mix(in srgb, var(--live) 14%, transparent); color: var(--live); }

  /* ── Main content ────────────────────────────────────────────────────── */
  .content-panel {
    grid-area: main;
    min-height: 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior-y: contain;
    scroll-padding-block: var(--space-page);
    background: var(--bg);
    view-transition-name: app-content;
  }

  /* ── Dock: player bar on desktop; player + tabs on phones ───────────── */
  .footer-dock {
    grid-area: dock;
    border-top: 1px solid var(--border);
    background: var(--bg);
  }
  .player-bar { height: var(--mini-player-height); }
  .mobile-navigation { display: none; }

  .play-error {
    position: fixed;
    left: 50%;
    top: calc(var(--app-top) + var(--app-height) - var(--app-bottom-clearance));
    transform: translate(-50%, -100%);
    z-index: 200;
    display: flex;
    align-items: center;
    gap: 10px;
    /* Centered with left: 50%, so size to content explicitly instead of half the viewport. */
    width: max-content;
    max-width: min(560px, calc(100vw - 32px));
    max-height: calc(var(--app-height) - var(--app-bottom-clearance) - 2rem);
    overflow-y: auto;
    padding: 6px 6px 6px 14px;
    border: 1px solid color-mix(in srgb, var(--error) 45%, transparent);
    border-radius: var(--radius-control);
    background: var(--float);
    box-shadow: var(--float-shadow);
    color: var(--text);
    font-size: 14px;
  }
  .play-error > .pxi { font-size: 16px; color: var(--error); }
  .play-error > span { min-width: 0; overflow-wrap: anywhere; }
  .play-error-x {
    display: grid;
    place-items: center;
    min-width: 36px;
    min-height: 36px;
    border: none;
    border-radius: var(--radius-chip);
    background: none;
    color: var(--muted);
    font-size: 16px;
    cursor: pointer;
  }
  .play-error-x:hover { color: var(--text-bright); background: var(--surface-2); }

  /* ── More sheet ──────────────────────────────────────────────────────── */
  .more-sheet::backdrop { background: var(--overlay); }
  .more-sheet {
    position: fixed;
    left: var(--safe-left);
    right: var(--safe-right);
    top: calc(var(--app-top) + var(--app-height));
    bottom: auto;
    transform: translateY(-100%);
    width: auto;
    max-width: none;
    max-height: calc(var(--app-height) - var(--safe-top) - 16px);
    margin: 0;
    padding: 8px 16px calc(var(--safe-bottom) + 12px);
    overflow-y: auto;
    border: none;
    border-top: 1px solid var(--border-strong);
    border-radius: var(--radius-panel) var(--radius-panel) 0 0;
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--float-shadow);
  }
  .more-sheet[open] { animation: sheet-up var(--motion-sheet) var(--ease-drawer); }
  @keyframes sheet-up {
    from { translate: 0 100%; }
    to { translate: 0 0; }
  }
  .more-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 4px;
  }
  .more-title {
    margin: 0;
    padding-left: 4px;
    font-size: 15px;
    font-weight: 600;
  }
  .more-close {
    display: grid;
    place-items: center;
    min-width: 44px;
    min-height: 44px;
    border: none;
    border-radius: var(--radius-control);
    background: none;
    color: var(--muted);
    font-size: 24px;
    cursor: pointer;
  }
  .more-item {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    min-height: 52px;
    padding: 0 4px;
    border: none;
    border-top: 1px solid var(--border-soft);
    background: none;
    color: var(--text);
    font-size: 16px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
  }
  .more-item .pxi { font-size: 24px; color: var(--muted-2); }
  .more-item:active { background: var(--surface); }
  .more-item .badge { margin-left: auto; }

  @media (prefers-reduced-motion: reduce) {
    .more-sheet[open] { animation: none; }
  }

  /* ── Phones ──────────────────────────────────────────────────────────── */
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .app-shell {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 1fr);
      grid-template-areas: 'main';
      padding: var(--safe-top) var(--safe-right) 0 var(--safe-left);
    }
    .sidebar { display: none; }
    .content-panel {
      padding-bottom: var(--app-bottom-clearance);
      scroll-padding-bottom: var(--app-bottom-clearance);
    }
    /* Docked, opaque, edge to edge: no blur layer over the scrolling grid. */
    .footer-dock {
      position: absolute;
      z-index: 100;
      left: 0;
      right: 0;
      bottom: 0;
      padding: 0 var(--safe-right) var(--safe-bottom) var(--safe-left);
    }
    .player-bar { height: 64px; border-bottom: 1px solid var(--border-soft); }
    .player-bar.idle { display: none; }
    .mobile-navigation { display: block; }
  }
</style>
