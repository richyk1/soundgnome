<script lang="ts">
  import { onMount, getContext, setContext } from 'svelte';
  import { lib, LIBRARY_PLAYER, createLibraryPlayer } from '../lib/library/store.svelte';
  import { GLOBAL_PLAYER, type GlobalPlayer } from '../lib/player';
  import { deleteTrack } from '../lib/api';
  import TrackTable from '../lib/library/TrackTable.svelte';

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
  <div class="tabs" role="tablist">
    <button
      class="tab"
      class:active={tab === 'liked'}
      role="tab"
      aria-selected={tab === 'liked'}
      onclick={() => (tab = 'liked')}
    >
      <i class="lni lni-thumbs-up-1"></i> Liked
      {#if lib.likedTracks.length > 0}<span class="tab-count">{lib.likedTracks.length}</span>{/if}
    </button>
    <button
      class="tab"
      class:active={tab === 'disliked'}
      role="tab"
      aria-selected={tab === 'disliked'}
      onclick={() => (tab = 'disliked')}
    >
      <i class="lni lni-thumbs-down-1"></i> Disliked
      {#if lib.dislikedTracks.length > 0}<span class="tab-count">{lib.dislikedTracks.length}</span>{/if}
    </button>
  </div>

  {#if lib.tracksLoading}
    <p class="status">Loading…</p>
  {:else if lib.tracksError}
    <p class="status error">{lib.tracksError}</p>
  {:else}
    <div class="toolbar">
      <input class="search" placeholder="Search these tracks…" bind:value={search} />
      {#if tab === 'disliked' && lib.dislikedTracks.length > 0}
        <button class="btn-delete-all" onclick={deleteAllDisliked} disabled={deleting}>
          {deleting ? 'Deleting…' : `Delete all (${lib.dislikedTracks.length})`}
        </button>
      {/if}
      <span class="count">{filtered.length} track{filtered.length !== 1 ? 's' : ''}</span>
    </div>

    {#if filtered.length === 0}
      <p class="status empty">
        {#if search.trim()}
          No matches.
        {:else if tab === 'liked'}
          No liked tracks yet. Tap the <i class="lni lni-thumbs-up-1"></i> on any track to like it.
        {:else}
          Nothing disliked. Tap the <i class="lni lni-thumbs-down-1"></i> on a track to send it here for cleanup.
        {/if}
      </p>
    {:else}
      <TrackTable tracks={filtered} showAlbumCol={true} showDelete={tab === 'disliked'} />
    {/if}
  {/if}
</div>

<style>
  .liked-page {
    padding: 1.25rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .tabs {
    display: flex;
    gap: 0.5rem;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.5rem 0.9rem;
    border-radius: 999px;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--muted);
    cursor: pointer;
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: -0.01em;
    font-size: 0.95rem;
  }
  .tab:hover {
    color: var(--text-bright);
  }
  .tab.active {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    color: var(--text);
  }
  .tab .lni {
    font-size: 1rem;
  }
  .tab-count {
    font-size: 0.7rem;
    font-weight: 600;
    background: var(--surface-2);
    color: var(--muted);
    border-radius: 999px;
    padding: 0.05rem 0.45rem;
  }
  .tab.active .tab-count {
    background: color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--text);
  }

  .btn-delete-all {
    background: transparent;
    color: var(--error);
    border: 1px solid var(--error);
    border-radius: 999px;
    padding: 0.4rem 0.85rem;
    cursor: pointer;
    font-size: 0.8rem;
    white-space: nowrap;
  }
  .btn-delete-all:hover:not(:disabled) {
    background: color-mix(in srgb, var(--error) 12%, transparent);
  }
  .btn-delete-all:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .empty :global(.lni) {
    color: var(--muted);
  }

  @media (max-width: 640px) {
    .liked-page {
      padding: 1rem;
    }
  }
</style>
