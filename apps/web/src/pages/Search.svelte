<script lang="ts">
  import { onMount, getContext, setContext } from 'svelte';
  import { lib, LIBRARY_PLAYER, createLibraryPlayer } from '../lib/library/store.svelte';
  import { GLOBAL_PLAYER, type GlobalPlayer } from '../lib/player';
  import TrackTable from '../lib/library/TrackTable.svelte';

  let q = $state('');

  let results = $derived.by(() => {
    const query = q.trim().toLowerCase();
    if (!query) return [];
    return lib.tracks
      .filter(
        (t) =>
          t.title.toLowerCase().includes(query) ||
          t.artists.some((a) => a.name.toLowerCase().includes(query)) ||
          (t.album?.title ?? '').toLowerCase().includes(query),
      )
      .slice(0, 200);
  });

  // Share the app-wide player via the same bridge the Library uses.
  const player = getContext<GlobalPlayer>(GLOBAL_PLAYER);
  setContext(LIBRARY_PLAYER, createLibraryPlayer(player, () => results));

  onMount(() => {
    if (lib.tracks.length === 0) lib.loadTracks();
  });
</script>

<div class="search-page">
  <div class="search-head">
    <i class="lni lni-search-1"></i>
    <input class="search-input" placeholder="Search your library" bind:value={q} />
    {#if q}<button class="clear" onclick={() => (q = '')} aria-label="Clear search">×</button>{/if}
  </div>

  {#if !q.trim()}
    <p class="hint">Search across your tracks, artists, and albums.</p>
  {:else if results.length === 0}
    <p class="hint">No matches for "{q}".</p>
  {:else}
    <TrackTable tracks={results} showAlbumCol={true} />
  {/if}
</div>

<style>
  .search-page {
    padding: 1.25rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .search-head {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 0 12px;
  }
  .search-head .lni {
    color: var(--muted);
    font-size: 18px;
    flex-shrink: 0;
  }
  .search-input {
    flex: 1;
    background: none;
    border: none;
    color: var(--text);
    font-size: 1rem;
    padding: 13px 0;
    outline: none;
  }
  .search-input::placeholder {
    color: var(--muted);
  }
  .clear {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 1.4rem;
    line-height: 1;
    cursor: pointer;
    padding: 0 4px;
  }
  .hint {
    color: var(--muted);
    text-align: center;
    padding: 2rem 1rem;
    font-size: 0.9rem;
  }
  @media (max-width: 640px) {
    .search-page {
      padding: 1rem;
    }
  }
</style>
