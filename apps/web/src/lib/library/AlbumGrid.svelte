<script lang="ts">
  import { lib } from './store.svelte';
  import VirtualCardGrid from './VirtualCardGrid.svelte';
  import CardActions from './CardActions.svelte';
  import PixelCover from '../PixelCover.svelte';
  import { runNavigation } from '../navigation-motion';

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
        <div class="cover-wrap">
          <PixelCover src={a.cover && /^https?:\/\//.test(a.cover) ? a.cover : null} seed="album:{a.id}" alt={a.title} />
        </div>
        <div class="card-body">
          <div class="card-title" title={a.title}>{a.title}</div>
          <div class="card-sub">{a.artists.map(x => x.name).join(', ') || '\u2014'}</div>
          {#if a.date}<div class="card-meta">{a.date.slice(0, 4)}</div>{/if}
        </div>
        {#if sel}<span class="card-sel-badge" aria-label="Selected"><i class="pxi pxi-check" aria-hidden="true"></i></span>{/if}
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
  /* Fixed metadata height keeps virtual rows stable, including albums without dates. */
  .card-body { height: 4.5rem; }
  .card-selected :global(.cover-wrap)::after { box-shadow: inset 0 0 0 2px var(--accent); }
  .card-dimmed { opacity: 0.2; }
  .card-pickable { cursor: pointer; }
  .card-pickable:hover :global(.cover-wrap)::after { box-shadow: inset 0 0 0 2px var(--success); }
  .card-sel-badge {
    position: absolute;
    top: 8px;
    left: 8px;
    z-index: 2;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 16px;
  }
  .card-pickable .card-sel-badge { background: var(--success); color: var(--on-success); }
</style>
