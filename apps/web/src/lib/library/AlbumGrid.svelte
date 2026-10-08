<script lang="ts">
  import type { Snippet } from 'svelte';
  import { lib } from './store.svelte';
  import VirtualCardGrid from './VirtualCardGrid.svelte';
  import CardActions from './CardActions.svelte';
  import { runNavigation } from '../navigation-motion';

  let { cover }: { cover: Snippet<[string | null | undefined, string]> } = $props();

  function clearHoveredAlbum() {
    if (lib.hoveredItem?.type === 'album') lib.hoveredItem = null;
  }
</script>

<VirtualCardGrid items={lib.filteredAlbums} onWindowChange={clearHoveredAlbum}>
  {#snippet card(a)}
      {@const sel = lib.selectedAlbumIds.has(a.id)}
      {@const dimmed = lib.albumSimilarFilterActive && !lib.similarAlbumIds.has(a.id)}
      {@const pickable = lib.albumMergePicking && sel}
      <div class="card"
        class:clickable={!lib.albumMergePicking}
        class:card-selected={sel}
        class:card-dimmed={dimmed}
        class:card-pickable={pickable}
        style={lib.albumMergePicking && !sel ? 'opacity:0.3;pointer-events:none' : ''}
        onmouseenter={() => (lib.hoveredItem = { type: 'album', id: a.id })}
        onmouseleave={() => (lib.hoveredItem = null)}
        onclick={(e) => {
          if (lib.albumMergePicking) { if (sel) lib.pickAlbumMergeTarget(a.id); }
          else if (e.shiftKey) { e.preventDefault(); lib.toggleAlbumSelection(a.id); }
          else { runNavigation(() => lib.drillIntoAlbum(a)); }
        }}
        role="button" tabindex="0"
        onkeydown={(e) => {
          if (e.target !== e.currentTarget) return;
          if (e.key === 'Enter') {
            if (lib.albumMergePicking && sel) lib.pickAlbumMergeTarget(a.id);
            else if (!lib.albumMergePicking) runNavigation(() => lib.drillIntoAlbum(a));
          } else if (e.key === ' ') { e.preventDefault(); lib.toggleAlbumSelection(a.id); }
        }}>
        {@render cover(a.cover, a.title)}
        <div class="card-body">
          <div class="card-title" title={a.title}>{a.title}</div>
          <div class="card-sub">{a.artists.map(x => x.name).join(', ') || '\u2014'}</div>
          {#if a.date}<div class="card-meta">{a.date.slice(0, 4)}</div>{/if}
        </div>
        {#if sel}<span class="card-sel-badge">✓</span>{/if}
        {#if !lib.albumMergePicking}
          <CardActions title={a.title} actions={[
            { label: sel ? 'Selected' : 'Select', pressed: sel, onSelect: () => lib.toggleAlbumSelection(a.id) },
            { label: 'Edit album', onSelect: () => lib.startEditAlbum(a) },
            { label: 'Delete album', danger: true, onSelect: () => lib.handleDeleteAlbum(a.id) },
          ]} />
        {/if}
      </div>
  {/snippet}
</VirtualCardGrid>

<style>
  /* Uniform metadata height keeps virtual rows stable, including albums without dates. */
  .card-body { height: 6rem; box-sizing: border-box; }
  .card { position: relative; }
  .card-selected { outline: 2px solid var(--accent); outline-offset: -2px; }
  .card-dimmed { opacity: 0.2; }
  .card-pickable { cursor: pointer; }
  .card-pickable:hover { outline-color: var(--success) !important; background: color-mix(in srgb, var(--success) 10%, var(--surface)); }
  .card-sel-badge {
    position: absolute; top: 0.5rem; left: 0.5rem; z-index: 2;
    background: var(--accent); color: var(--on-accent); border-radius: 50%;
    width: 1.2rem; height: 1.2rem; font-size: 0.65rem; font-weight: 700;
    display: flex; align-items: center; justify-content: center;
  }
  .card-pickable .card-sel-badge { background: var(--success); color: var(--on-success); }
</style>
