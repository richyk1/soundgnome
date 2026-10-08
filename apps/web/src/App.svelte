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
  import GlassNav from './lib/GlassNav.svelte';
  import { observeViewport } from './lib/viewport';
  import { observeTheme } from './lib/theme';
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
    { id: 'library', label: 'Library', icon: 'lni-library' },
    { id: 'download', label: 'Download', icon: 'lni-download-1' },
    { id: 'ingest', label: 'Ingest', icon: 'lni-folder-upload' },
    { id: 'validations', label: 'Validations', icon: 'lni-check-square-1' },
  ];
  const mobileTabs: { id: Page; label: string; icon: string }[] = [
    { id: 'download', label: 'Home', icon: 'home-2' },
    { id: 'search', label: 'Search', icon: 'search-1' },
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
      icon: 'menu-bento-1',
      selected: moreOpen || !mobileTabs.some((tab) => tab.id === page),
      current: !mobileTabs.some((tab) => tab.id === page),
      expanded: moreOpen,
      controls: moreOpen ? 'more-menu' : undefined,
      onSelect: (trigger: HTMLButtonElement) => { moreTrigger = trigger; moreOpen = true; },
    },
  ]);
  const libraryTabs: { id: LibraryTab; label: string; icon: string; count: () => number }[] = [
    { id: 'artists', label: 'Artists', icon: 'lni-microphone-1', count: () => lib.artists.length },
    { id: 'albums', label: 'Albums', icon: 'lni-layers-1', count: () => lib.albums.length },
    { id: 'tracks', label: 'Tracks', icon: 'lni-music-note', count: () => lib.tracks.length },
    { id: 'playlists', label: 'Playlists', icon: 'lni-list-music-4', count: () => lib.playlists.length },
  ];
</script>

<div class="app-shell">

  <div class="app-body">
    <aside class="sidebar">
      <div class="side-panel brand-panel">
        <button class="brand" onclick={() => navigate('library')}>
          <span class="brand-name">Soundgnome</span>
          {#if version}<span class="brand-ver">v{version}</span>{/if}
        </button>
        <nav class="nav">
          {#each primaryNav as item}
            <button class="nav-item" class:active={page === item.id} aria-current={page === item.id ? 'page' : undefined} onclick={() => navigate(item.id)}>
              <span class="nav-label"><i class="lni {item.icon}"></i>{item.label}</span>
              {#if item.id === 'validations' && lib.needsReviewCount > 0}
                <span class="badge badge-amber">{lib.needsReviewCount}</span>
              {/if}
            </button>
          {/each}
        </nav>
      </div>

      <div class="side-panel library-panel">
        <div class="panel-head">
          <span class="eyebrow">Your library</span>
          <span class="mono dim">{lib.tracks.length} tracks</span>
        </div>
        <div class="sub-nav">
          {#each libraryTabs as t}
            <button
              class="sub-item"
              class:active={page === 'library' && lib.tab === t.id}
              aria-current={page === 'library' && lib.tab === t.id ? 'page' : undefined}
              onclick={() => goLibraryTab(t.id)}
            >
              <span class="nav-label"><i class="lni {t.icon}"></i>{t.label}</span>
              <span class="counts">
                <span class="mono dim">{t.count()}</span>
                {#if t.id === 'tracks' && lib.needsReviewCount > 0}
                  <span class="badge badge-amber sm">{lib.needsReviewCount}</span>
                {/if}
              </span>
            </button>
          {/each}
        </div>

        <div class="divider"></div>

        <div class="eyebrow">Up next{#if upNext.length} · {upNext.length}{/if}</div>
        <div class="queue">
          {#if upNext.length === 0}
            <p class="queue-empty">Nothing queued.</p>
          {:else}
            {#each upNext.slice(0, 8) as q}
              <div class="queue-row">
                <div class="queue-art" style={q.artwork ? `background-image:url(${q.artwork})` : ''}>
                  {#if !q.artwork}<i class="lni lni-music-note"></i>{/if}
                </div>
                <div class="queue-meta">
                  <div class="queue-title">{q.title}</div>
                  <div class="queue-artist">{q.artist}</div>
                </div>
              </div>
            {/each}
          {/if}
        </div>

        <InstallPrompt />
        <div class="side-links">
          <button class="side-link" onclick={() => navigate('likes')}><i class="lni lni-heart"></i>Liked</button>
          <button class="side-link" onclick={() => navigate('tasks')}>
            <i class="lni lni-bell-1"></i>Activity
            {#if activeTasksCount > 0}<span class="badge badge-red sm">{activeTasksCount}</span>{/if}
          </button>
          <button class="side-link" onclick={() => navigate('tools')}><i class="lni lni-gear-1"></i>Tools</button>
          <button class="side-link" onclick={() => (helpOpen = true)}><i class="lni lni-question-mark-circle"></i>Help</button>
        </div>
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
  </div>

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
      <GlassNav items={footerItems} />
    </div>
  </footer>

  {#if playError}
    <div class="play-error" role="alert">
      <span>{playError}</span>
      <button class="play-error-x" onclick={() => (playError = null)} aria-label="Dismiss">×</button>
    </div>
  {/if}
</div>

<HelpModal open={helpOpen} onClose={() => (helpOpen = false)} />
<PWAUpdatePrompt />

{#if moreOpen}
  <dialog id="more-menu" class="more-sheet" aria-label="More" use:showMore
    oncancel={() => (moreOpen = false)}
    onclick={closeMoreBackdrop}>
    <div class="more-header">
      <div class="more-title">More</div>
      <button class="more-close" onclick={() => (moreOpen = false)} aria-label="Close more menu">×</button>
    </div>
    <button class="more-item" onclick={() => { navigate('ingest'); moreOpen = false; }}><i class="lni lni-folder-upload"></i>Ingest local files</button>
    <button class="more-item" onclick={() => { navigate('validations'); moreOpen = false; }}><i class="lni lni-check-square-1"></i>Validations{#if lib.needsReviewCount > 0}<span class="badge badge-amber sm">{lib.needsReviewCount}</span>{/if}</button>
    <button class="more-item" onclick={() => { navigate('tasks'); moreOpen = false; }}><i class="lni lni-bell-1"></i>Activity{#if activeTasksCount > 0}<span class="badge badge-red sm">{activeTasksCount}</span>{/if}</button>
    <button class="more-item" onclick={() => { navigate('tools'); moreOpen = false; }}><i class="lni lni-gear-1"></i>Tools</button>
    <button class="more-item" onclick={() => { helpOpen = true; moreOpen = false; }}><i class="lni lni-question-mark-circle"></i>Help</button>
  </dialog>
{/if}

<style>
  .app-shell {
    position: fixed;
    top: var(--app-top);
    left: 0;
    width: 100%;
    height: var(--app-height, 100dvh);
    background: var(--bg);
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: calc(8px + var(--safe-top)) calc(8px + var(--safe-right))
      calc(8px + var(--safe-bottom)) calc(8px + var(--safe-left));
    box-sizing: border-box;
    overflow: hidden;
  }

  .app-body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 288px 1fr;
    gap: 8px;
    /* Contain the fallback entrance above the live player/footer. */
    overflow: hidden;
  }

  /* ── Sidebar ─────────────────────────────────────────────────────────── */
  .sidebar {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .side-panel {
    background: var(--panel);
    border-radius: 14px;
  }
  .brand-panel {
    padding: 20px 20px 18px;
    flex-shrink: 0;
  }
  .brand {
    display: flex;
    align-items: baseline;
    gap: 10px;
    background: none;
    min-height: 44px;
    border: none;
    padding: 0;
    cursor: pointer;
  }
  .brand-name {
    font-family: var(--font-display);
    font-size: 25px;
    font-weight: 800;
    letter-spacing: -0.035em;
    color: var(--text-bright);
  }
  .brand-ver {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    color: var(--muted-2);
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 20px;
  }
  .nav-item,
  .sub-item {
    min-height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-radius: 8px;
    background: none;
    border: none;
    color: var(--muted);
    font-family: var(--font-body);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    width: 100%;
    text-align: left;
    transition: background-color var(--motion-fast) var(--ease-out), color var(--motion-fast) var(--ease-out), box-shadow var(--motion-fast) var(--ease-out);
  }
  .sub-item { padding: 9px 10px; }
  .nav-item:hover,
  .sub-item:hover { background: var(--surface); color: var(--text); }
  .nav-item.active { background: var(--surface-2); color: var(--text-bright); box-shadow: inset 3px 0 var(--accent); }
  .sub-item.active { background: var(--surface); color: var(--text-bright); box-shadow: inset 3px 0 var(--accent); }
  .nav-item:active,
  .sub-item:active { background: var(--surface-2); }
  .nav-label { display: flex; align-items: center; gap: 11px; }
  .nav-label .lni { font-size: 16px; color: inherit; }
  .counts { display: flex; align-items: center; gap: 6px; }

  .library-panel {
    flex: 1;
    min-height: 0;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
  }
  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }
  .eyebrow {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--muted-2);
  }
  .sub-nav { display: flex; flex-direction: column; gap: 2px; }
  .divider { height: 1px; background: var(--border-soft); margin: 18px 0; }

  .queue {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 14px;
    overflow-y: auto;
    min-height: 0;
    flex: 1;
  }
  .queue-empty { color: var(--muted-2); font-size: 13px; margin: 6px 0 0; }
  .queue-row { display: flex; gap: 11px; align-items: center; }
  .queue-art {
    width: 38px;
    height: 38px;
    border-radius: 5px;
    background: linear-gradient(135deg, var(--surface-2), var(--surface));
    background-size: cover;
    background-position: center;
    flex-shrink: 0;
    border: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted-2);
    font-size: 15px;
  }
  .queue-meta { min-width: 0; }
  .queue-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .queue-artist {
    font-size: 12px;
    font-weight: 500;
    color: var(--muted-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .side-links {
    display: flex;
    gap: 14px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 18px;
    padding-top: 16px;
    border-top: 1px solid var(--border-soft);
  }
  .side-link {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    color: var(--muted-2);
    font-family: var(--font-body);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    padding: 0;
    transition: color var(--motion-fast) var(--ease-out);
  }
  .side-link:hover { color: var(--text); }
  .side-link .lni { font-size: 15px; }

  /* ── Main content ────────────────────────────────────────────────────── */
  .content-panel {
    min-height: 0;
    min-width: 0;
    background: linear-gradient(var(--panel) 0, var(--panel) 100%);
    border-radius: 14px;
    overflow-y: auto;
    overflow-x: hidden;
    scroll-padding-block: var(--space-page);
    overscroll-behavior-y: contain;
    display: flex;
    view-transition-name: app-content;
    flex-direction: column;
  }

  /* ── Badges ──────────────────────────────────────────────────────────── */
  .badge {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 999px;
    line-height: 1;
  }
  .badge.sm { font-size: 10px; padding: 1px 7px; }
  .badge-amber { background: var(--warning-bg); color: var(--warning); }
  .badge-red { background: var(--error-bg); color: var(--error); }

  .mono { font-family: var(--font-mono); font-size: 11px; }
  .dim { color: var(--muted-2); }

  /* ── Player bar ──────────────────────────────────────────────────────── */
  .footer-dock { flex-shrink: 0; }
  .mobile-navigation { display: none; }
  .player-bar {
    height: 110px;
    flex-shrink: 0;
    background: var(--panel);
    border-radius: 14px;
    overflow: hidden;
  }

  .play-error {
    position: fixed;
    left: 50%;
    top: calc(var(--app-top) + var(--app-height) - var(--app-bottom-clearance));
    bottom: auto;
    transform: translate(-50%, -100%);
    max-height: calc(var(--app-height) - var(--app-bottom-clearance) - 2rem);
    overflow-y: auto;
    z-index: 200;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    max-width: min(560px, calc(100% - 32px));
    padding: 0.55rem 0.9rem;
    background: color-mix(in srgb, var(--error) 20%, var(--panel));
    border: 1px solid color-mix(in srgb, var(--error) 55%, transparent);
    border-radius: 12px;
    box-shadow: var(--float-shadow);
    color: var(--text);
    font-size: 0.85rem;
  }
  .play-error > span { min-width: 0; overflow-wrap: anywhere; }
  .play-error-x {
    min-width: 44px;
    min-height: 44px;
    background: none;
    border: none;
    color: var(--muted);
    font-size: 1.1rem;
    line-height: 1;
    cursor: pointer;
    padding: 0 0.2rem;
  }
  .play-error-x:hover { color: var(--text); }


  .more-sheet::backdrop { background: var(--overlay); }
  .more-sheet {
    position: fixed;
    left: var(--safe-left);
    right: var(--safe-right);
    top: calc(var(--app-top) + var(--app-height));
    bottom: auto;
    transform: translateY(-100%);
    margin: 0;
    width: auto;
    max-width: none;
    max-height: calc(var(--app-height) - var(--safe-top) - 16px);
    overflow-y: auto;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 16px 16px 0 0;
    padding: 12px 16px calc(var(--safe-bottom) + 16px);
    box-shadow: var(--float-shadow);
  }
  .more-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .more-close { min-width: 44px; min-height: 44px; border: none; background: none; color: var(--text); font-size: 24px; cursor: pointer; }
  .more-title {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    padding: 0 4px 6px;
  }
  .more-item {
    width: 100%;
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 12px;
    background: none;
    border: none;
    color: var(--text);
    font-size: 0.95rem;
    font-weight: 500;
    padding: 13px 8px;
    border-radius: 10px;
    cursor: pointer;
    text-align: left;
    transition: background-color var(--motion-fast) var(--ease-out), color var(--motion-fast) var(--ease-out);
  }
  .more-item:hover,
  .more-item:active { background: var(--surface-2); }
  .more-item .lni { font-size: 19px; color: var(--muted); width: 22px; text-align: center; }
  .more-item .badge { margin-left: auto; }

  @media (prefers-reduced-motion: reduce) {
    .nav-item, .sub-item, .side-link, .more-item {
      transition: none;
    }
  }

  /* ── Mobile ──────────────────────────────────────────────────────────── */

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    /* Edge-to-edge on phones: drop the desktop black frame + rounded panels. */
    .app-shell {
      padding: var(--safe-top) var(--safe-right) 0 var(--safe-left);
      gap: 0;
    }
    .app-body {
      grid-template-columns: 1fr;
    }
    .content-panel {
      border-radius: 0;
      padding-bottom: var(--app-bottom-clearance);
      scroll-padding-bottom: var(--app-bottom-clearance);
    }
    .footer-dock {
      position: absolute;
      z-index: 100;
      left: calc(var(--safe-left) + 12px);
      right: calc(var(--safe-right) + 12px);
      bottom: calc(var(--safe-bottom) + 8px);
      max-width: 520px;
      margin-inline: auto;
      isolation: isolate;
      border-radius: 30px;
      box-shadow: var(--float-shadow);
    }
    /* Keep the backdrop on a pseudo-element so Now Playing stays viewport-fixed. */
    .footer-dock::before {
      content: '';
      position: absolute;
      z-index: -1;
      inset: 0;
      border: 1px solid var(--glass-border);
      border-radius: inherit;
      background: var(--glass-tint);
      -webkit-backdrop-filter: blur(12px) saturate(1.3);
      backdrop-filter: blur(12px) saturate(1.3);
      box-shadow: inset 0 1px 0 var(--glass-highlight);
      pointer-events: none;
    }
    .footer-dock.loaded { border-radius: 28px; }
    .player-bar {
      border-radius: 0;
      height: 64px;
      background: transparent;
    }
    .player-bar.idle { display: none; }
    .sidebar { display: none; }
    .mobile-navigation { display: block; }
    @supports not ((backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: blur(1px))) {
      .footer-dock::before { background: var(--panel); }
    }
    @media (prefers-reduced-transparency: reduce) {
      .footer-dock::before {
        background: var(--panel);
        -webkit-backdrop-filter: none;
        backdrop-filter: none;
      }
    }
    @media (forced-colors: active) {
      .footer-dock { box-shadow: none; }
      .footer-dock::before {
        background: Canvas;
        border-color: CanvasText;
        -webkit-backdrop-filter: none;
        backdrop-filter: none;
      }
    }
  }
</style>
