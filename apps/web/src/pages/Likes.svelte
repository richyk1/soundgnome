<script lang="ts">
  import { onMount, getContext, setContext } from 'svelte';
  import { lib, LIBRARY_PLAYER, createLibraryPlayer } from '../lib/library/store.svelte';
  import { GLOBAL_PLAYER, type GlobalPlayer } from '../lib/player';
  import { deleteTrack } from '../lib/api';
  import TrackTable from '../lib/library/TrackTable.svelte';
  import { runNavigation } from '../lib/navigation-motion';

  // ── Tabs ──────────────────────────────────────────────────────────────────
  let tab: 'liked' | 'disliked' = $state('liked');
  let search = $state('');
  let deleting = $state(false);

  let base = $derived(tab === 'liked' ? lib.likedTracks : lib.dislikedTracks);
  let filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    if (!q) return base;
    return base.filter(
      (t) => t.title.toLowerCase().includes(q) || t.artists.some((a) => a.name.toLowerCase().includes(q)),
    );
  });

  // ── Playback: share the app-wide player via the same bridge the Library uses.
  const player = getContext<GlobalPlayer>(GLOBAL_PLAYER);
  setContext(LIBRARY_PLAYER, createLibraryPlayer(player, () => filtered));

  onMount(() => {
    lib.loadTracks();
  });

  async function deleteAllDisliked() {
    const ids = lib.dislikedTracks.map((t) => t.id);
    if (ids.length === 0) return;
    if (
      !confirm(
        `Delete all ${ids.length} disliked track${ids.length !== 1 ? 's' : ''} from the library? This removes the files from disk.`,
      )
    )
      return;
    deleting = true;
    try {
      for (const id of ids) await deleteTrack(id);
      await lib.loadTracks();
    } catch (e) {
      alert(e instanceof Error ? e.message : String(e));
    } finally {
      deleting = false;
    }
  }
</script>

<div class="liked-page">
  <header class="page-head">
    <h1>Liked and disliked</h1>
    {#if !lib.tracksLoading && !lib.tracksError}
      <p class="page-meta">{lib.likedTracks.length} liked · {lib.dislikedTracks.length} disliked</p>
    {/if}
  </header>

  <div class="tabs" role="tablist">
    <button
      class="tab"
      class:active={tab === 'liked'}
      role="tab"
      aria-selected={tab === 'liked'}
      onclick={() => { if (tab !== 'liked') void runNavigation(() => { tab = 'liked'; }); }}
    >
      <i class="pxi pxi-thumbs-up" aria-hidden="true"></i>Liked
      {#if lib.likedTracks.length > 0}<span class="tab-count">{lib.likedTracks.length}</span>{/if}
    </button>
    <button
      class="tab"
      class:active={tab === 'disliked'}
      role="tab"
      aria-selected={tab === 'disliked'}
      onclick={() => { if (tab !== 'disliked') void runNavigation(() => { tab = 'disliked'; }); }}
    >
      <i class="pxi pxi-thumbs-down" aria-hidden="true"></i>Disliked
      {#if lib.dislikedTracks.length > 0}<span class="tab-count">{lib.dislikedTracks.length}</span>{/if}
    </button>
  </div>

  {#if lib.tracksLoading}
    <p class="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Loading…</p>
  {:else if lib.tracksError}
    <div class="callout callout-error" role="alert">
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>
      <div class="callout-body"><span>{lib.tracksError}</span></div>
    </div>
  {:else}
    <div class="toolbar">
      <input class="search" placeholder="Search these tracks…" aria-label="Search these tracks" bind:value={search} />
      {#if tab === 'disliked' && lib.dislikedTracks.length > 0}
        <button class="btn-danger btn-sm" onclick={deleteAllDisliked} disabled={deleting}>
          <i class="pxi {deleting ? 'pxi-loader pxi-spin' : 'pxi-trash'}" aria-hidden="true"></i>
          {deleting ? 'Deleting…' : `Delete all (${lib.dislikedTracks.length})`}
        </button>
      {/if}
      <span class="count">{filtered.length} track{filtered.length !== 1 ? 's' : ''}</span>
    </div>

    {#if filtered.length === 0}
      <div class="empty">
        <i class="pxi {search.trim() ? 'pxi-search' : tab === 'liked' ? 'pxi-thumbs-up' : 'pxi-thumbs-down'}" aria-hidden="true"></i>
        {#if search.trim()}
          <p class="empty-title">No matches.</p>
        {:else if tab === 'liked'}
          <p class="empty-hint">No liked tracks yet. Tap the <i class="pxi pxi-thumbs-up inline-icon" role="img" aria-label="like button"></i> on any track to like it.</p>
        {:else}
          <p class="empty-hint">Nothing disliked. Tap the <i class="pxi pxi-thumbs-down inline-icon" role="img" aria-label="dislike button"></i> on a track to send it here for cleanup.</p>
        {/if}
      </div>
    {:else}
      <TrackTable tracks={filtered} showAlbumCol={true} showDelete={tab === 'disliked'} />
    {/if}
  {/if}
</div>

<style>
  .liked-page {
    padding: var(--space-page);
    display: flex;
    flex-direction: column;
  }

  .page-head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 16px;
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

  .tab-count {
    font-variant-numeric: tabular-nums;
    letter-spacing: 0;
    color: var(--muted-2);
  }
  .tab.active .tab-count { color: var(--accent); }

  .status {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }
  .status .pxi { font-size: 16px; }

  /* The thumbs glyph sits in the sentence like a word. */
  .inline-icon {
    font-size: 16px;
    vertical-align: -2px;
    color: var(--text);
  }
</style>
