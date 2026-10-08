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
  <header class="page-head">
    <h1>Search</h1>
    {#if lib.tracks.length > 0}
      <p class="page-meta">{lib.tracks.length} tracks in library</p>
    {/if}
  </header>

  <div class="search-head">
    <i class="pxi pxi-search field-icon" aria-hidden="true"></i>
    <input
      type="text"
      class="search-input"
      placeholder="Search your library"
      aria-label="Search your library"
      autocomplete="off"
      spellcheck="false"
      bind:value={q}
    />
    {#if q}
      <button type="button" class="clear" onclick={() => (q = '')} aria-label="Clear search">
        <i class="pxi pxi-close" aria-hidden="true"></i>
      </button>
    {/if}
  </div>

  {#if !q.trim()}
    <div class="empty">
      <i class="pxi pxi-search" aria-hidden="true"></i>
      <p class="empty-hint">Search across your tracks, artists, and albums.</p>
    </div>
  {:else if results.length === 0}
    <div class="empty">
      <i class="pxi pxi-search" aria-hidden="true"></i>
      <p class="empty-title">No matches for "{q}".</p>
    </div>
  {:else}
    <section class="group" aria-labelledby="search-tracks">
      <h2 class="group-title" id="search-tracks">Tracks <span class="group-count">{results.length}</span></h2>
      <TrackTable tracks={results} showAlbumCol={true} />
    </section>
  {/if}
</div>

<style>
  .search-page {
    padding: var(--space-page);
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .page-head {
    display: flex;
    flex-direction: column;
    gap: 6px;
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

  /* The input carries the global field look; icon and clear sit inside it. */
  .search-head {
    position: relative;
    min-width: 0;
  }
  .field-icon {
    position: absolute;
    top: 50%;
    left: 14px;
    margin-top: -8px;
    font-size: 16px;
    line-height: 1;
    color: var(--muted-2);
    pointer-events: none;
    transition: color var(--motion-fast) var(--ease-out);
  }
  .search-head:focus-within .field-icon { color: var(--accent); }
  .search-input {
    min-height: 48px;
    padding-left: 40px;
    padding-right: 48px;
  }
  .clear {
    position: absolute;
    top: 50%;
    right: 4px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    margin-top: -20px;
    padding: 0;
    border: none;
    border-radius: var(--radius-chip);
    background: none;
    color: var(--muted);
    font-size: 16px;
    cursor: pointer;
    transition: color var(--motion-fast) var(--ease-out), background-color var(--motion-fast) var(--ease-out);
  }
  .clear:hover { color: var(--text-bright); background: var(--surface-2); }

  /* ── Result groups ─────────────────────────────────────────────────────── */
  .group {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .group-title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0 0 4px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--text-bright);
  }
  .group-count {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 400;
    letter-spacing: 0;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .search-input { min-height: 48px; }
    .clear { width: 44px; height: 44px; margin-top: -22px; right: 2px; }
  }
</style>
