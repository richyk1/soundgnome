<script lang="ts">
  import { onMount, setContext } from 'svelte';
  import { fly, fade } from 'svelte/transition';
  import { getPendingCount, getActiveTasksCount, getVersion, getSoundcloudStreamUrl } from './lib/api';
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
  import {
    GLOBAL_PLAYER,
    type GlobalPlayer,
    type PlayerTrack,
    type PlayerHandle,
  } from './lib/player';
  type Page = 'download' | 'validations' | 'tasks' | 'library' | 'tools' | 'ingest' | 'likes' | 'search';
  type LibraryTab = 'artists' | 'albums' | 'tracks' | 'playlists';

  let page: Page = $state('library');
  let pendingCount = $state(0);
  let activeTasksCount = $state(0);
  let helpOpen = $state(false);
  let version = $state('');
  let mobileMenuOpen = $state(false);
  let moreOpen = $state(false);

  // One audio player for the whole app, mounted in the shell (below) so playback
  // and the player bar persist across navigation. Pages drive it via context.
  let player: PlayerHandle | null = $state(null);
  let playError: string | null = $state(null);
  let playing = $state(false);
  let upNext: PlayerTrack[] = $state([]);

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
      pendingCount = await getPendingCount();
    } catch {
      // API might not be up yet in dev
    }
    try {
      activeTasksCount = await getActiveTasksCount();
    } catch {
      // ignore
    }
  }

  onMount(() => {
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
      // Space toggles play/pause (media convention). Let real controls (buttons,
      // links, role=button cards) keep their native Space activation; otherwise
      // stop the page from scrolling and toggle the current track.
      if ((e.key === ' ' || e.code === 'Space') && playing) {
        if (tgt.closest('button, a, [role="button"]')) return;
        e.preventDefault();
        player?.playPause();
      }
    }
    document.addEventListener('keydown', onKeydown);

    return () => {
      clearInterval(interval);
      document.removeEventListener('keydown', onKeydown);
    };
  });

  function navigate(to: Page) {
    page = to;
    mobileMenuOpen = false;
    if (to === 'validations') refreshCounts();
  }

  // The sidebar "Your library" sub-nav navigates to the library and selects a tab.
  function goLibraryTab(tab: LibraryTab) {
    lib.switchTab(tab);
    navigate('library');
  }

  const primaryNav: { id: Page; label: string; icon: string }[] = [
    { id: 'library', label: 'Library', icon: 'lni-library' },
    { id: 'download', label: 'Download', icon: 'lni-download-1' },
    { id: 'ingest', label: 'Ingest', icon: 'lni-folder-upload' },
    { id: 'validations', label: 'Validations', icon: 'lni-check-square-1' },
  ];
  const mobileTabs: { id: Page; label: string; icon: string }[] = [
    { id: 'download', label: 'Home', icon: 'lni-home-2' },
    { id: 'search', label: 'Search', icon: 'lni-search-1' },
    { id: 'library', label: 'Library', icon: 'lni-library' },
  ];
  const libraryTabs: { id: LibraryTab; label: string; icon: string; count: () => number }[] = [
    { id: 'artists', label: 'Artists', icon: 'lni-microphone-1', count: () => lib.artists.length },
    { id: 'albums', label: 'Albums', icon: 'lni-layers-1', count: () => lib.albums.length },
    { id: 'tracks', label: 'Tracks', icon: 'lni-music-note', count: () => lib.tracks.length },
    { id: 'playlists', label: 'Playlists', icon: 'lni-list-music-4', count: () => lib.playlists.length },
  ];
</script>

<div class="app-shell">

  <div class="app-body">
    <aside class="sidebar" class:mobile-open={mobileMenuOpen}>
      <div class="side-panel brand-panel">
        <button class="brand" onclick={() => navigate('library')}>
          <span class="brand-name">Soundgnome</span>
          {#if version}<span class="brand-ver">v{version}</span>{/if}
        </button>
        <nav class="nav">
          {#each primaryNav as item}
            <button class="nav-item" class:active={page === item.id} onclick={() => navigate(item.id)}>
              <span class="nav-label"><i class="lni {item.icon}"></i>{item.label}</span>
              {#if item.id === 'validations' && pendingCount > 0}
                <span class="badge badge-amber">{pendingCount}</span>
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
              onclick={() => goLibraryTab(t.id)}
            >
              <span class="nav-label"><i class="lni {t.icon}"></i>{t.label}</span>
              <span class="counts">
                <span class="mono dim">{t.count()}</span>
                {#if t.id === 'tracks' && pendingCount > 0}
                  <span class="badge badge-amber sm">{pendingCount}</span>
                {/if}
              </span>
            </button>
          {/each}
        </div>

        <div class="divider"></div>

        <div class="eyebrow">Queue</div>
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

    <main class="content-panel">
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

  <div class="player-bar">
    <AudioPlayer
      bind:this={player}
      bind:active={playing}
      bind:upNext
      resolveSrc={resolveSrc}
      onError={(_track, msg) => (playError = msg)}
    />
  </div>

    <nav class="tabbar">
      {#each mobileTabs as t}
        <button class="tab-btn" class:active={page === t.id} onclick={() => navigate(t.id)}>
          <i class="lni {t.icon}"></i>
          <span>{t.label}</span>
        </button>
      {/each}
      <button class="tab-btn tab-more" onclick={() => (moreOpen = true)} aria-label="More">
        <i class="lni lni-gear-1"></i>
        <span>More</span>
      </button>
    </nav>

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
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <button class="more-backdrop" transition:fade={{ duration: 200 }} onclick={() => (moreOpen = false)} aria-label="Close more menu"></button>
  <div class="more-sheet" transition:fly={{ y: 300, duration: 280 }} role="dialog" aria-label="More">
    <div class="more-grabber"></div>
    <div class="more-title">More</div>
    <button class="more-item" onclick={() => { navigate('ingest'); moreOpen = false; }}><i class="lni lni-folder-upload"></i>Ingest local files</button>
    <button class="more-item" onclick={() => { navigate('validations'); moreOpen = false; }}><i class="lni lni-check-square-1"></i>Validations{#if pendingCount > 0}<span class="badge badge-amber sm">{pendingCount}</span>{/if}</button>
    <button class="more-item" onclick={() => { navigate('tasks'); moreOpen = false; }}><i class="lni lni-bell-1"></i>Activity{#if activeTasksCount > 0}<span class="badge badge-red sm">{activeTasksCount}</span>{/if}</button>
    <button class="more-item" onclick={() => { navigate('tools'); moreOpen = false; }}><i class="lni lni-gear-1"></i>Tools</button>
    <button class="more-item" onclick={() => { helpOpen = true; moreOpen = false; }}><i class="lni lni-question-mark-circle"></i>Help</button>
  </div>
{/if}

<style>
  .app-shell {
    width: 100%;
    height: 100dvh;
    background: var(--bg);
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: calc(8px + env(safe-area-inset-top)) calc(8px + env(safe-area-inset-right))
      calc(8px + env(safe-area-inset-bottom)) calc(8px + env(safe-area-inset-left));
    box-sizing: border-box;
    overflow: hidden;
  }

  .app-body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 288px 1fr;
    gap: 8px;
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
    transition: background 0.12s, color 0.12s;
  }
  .sub-item { padding: 9px 10px; }
  .nav-item:hover,
  .sub-item:hover { background: #141418; color: var(--text); }
  .nav-item.active { background: #1c1b22; color: var(--text-bright); }
  .sub-item.active { background: #17161c; color: var(--text-bright); }
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
    background: linear-gradient(135deg, #241f33, #15131c);
    background-size: cover;
    background-position: center;
    flex-shrink: 0;
    border: 1px solid #232330;
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
    color: #d6d6de;
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
    transition: color 0.12s;
  }
  .side-link:hover { color: var(--text); }
  .side-link .lni { font-size: 15px; }

  /* ── Main content ────────────────────────────────────────────────────── */
  .content-panel {
    min-height: 0;
    background: linear-gradient(var(--panel) 0, var(--panel) 100%);
    border-radius: 14px;
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
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
    transform: translateX(-50%);
    bottom: 128px;
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
  .play-error-x {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 1.1rem;
    line-height: 1;
    cursor: pointer;
    padding: 0 0.2rem;
  }
  .play-error-x:hover { color: var(--text); }

  /* ── Mobile bottom tab bar + More sheet ──────────────────────────────── */
  .tabbar {
    display: none;
    flex-shrink: 0;
    align-items: stretch;
    background: var(--panel);
    border-top: 1px solid var(--border);
    padding-bottom: env(safe-area-inset-bottom);
  }
  .tab-btn {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    background: none;
    border: none;
    color: var(--muted);
    font-family: var(--font-display);
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.01em;
    padding: 8px 0 6px;
    cursor: pointer;
  }
  .tab-btn .lni { font-size: 20px; }
  .tab-btn.active { color: var(--accent); }
  .tab-more {
    flex: 0 0 auto;
    padding-left: 16px;
    padding-right: 16px;
    border-left: 1px solid var(--border);
    color: var(--muted-2);
  }

  .more-backdrop {
    position: fixed;
    inset: 0;
    z-index: 320;
    background: rgba(0, 0, 0, 0.5);
    border: none;
    cursor: pointer;
  }
  .more-sheet {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 321;
    background: var(--surface);
    border-top-left-radius: 16px;
    border-top-right-radius: 16px;
    padding: 10px 14px calc(env(safe-area-inset-bottom) + 14px);
    display: flex;
    flex-direction: column;
    gap: 2px;
    box-shadow: 0 -10px 40px rgba(0, 0, 0, 0.5);
  }
  .more-grabber {
    width: 40px;
    height: 5px;
    border-radius: 999px;
    background: var(--surface-2);
    align-self: center;
    margin: 2px 0 10px;
  }
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
  }
  .more-item:hover,
  .more-item:active { background: var(--surface-2); }
  .more-item .lni { font-size: 19px; color: var(--muted); width: 22px; text-align: center; }
  .more-item .badge { margin-left: auto; }

  /* ── Mobile ──────────────────────────────────────────────────────────── */
  .mobile-toggle {
    display: none;
    align-items: center;
    gap: 10px;
    background: var(--panel);
    border: none;
    border-radius: 12px;
    color: var(--text-bright);
    font-family: var(--font-display);
    font-weight: 800;
    font-size: 17px;
    letter-spacing: -0.02em;
    padding: 12px 16px;
    cursor: pointer;
  }
  .mobile-toggle .lni { font-size: 20px; }

  @media (max-width: 860px) {
    /* Edge-to-edge on phones: drop the desktop black frame + rounded panels. */
    .app-shell {
      padding: 0;
      gap: 0;
    }
    .app-body {
      grid-template-columns: 1fr;
    }
    .content-panel {
      border-radius: 0;
    }
    .player-bar {
      border-radius: 0;
      height: 64px;
    }
    .sidebar { display: none; }
    .tabbar { display: flex; }
  }
</style>
