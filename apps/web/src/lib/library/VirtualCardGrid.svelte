<script lang="ts" generics="T extends { id: number }">
  import { onMount, tick, untrack, type Snippet } from 'svelte';

  let { items, card: renderCard, onWindowChange }: {
    items: T[];
    card: Snippet<[T]>;
    onWindowChange?: () => void;
  } = $props();
  let windowEl: HTMLDivElement | undefined = $state();
  let gridEl: HTMLDivElement | undefined = $state();
  let columns = $state(1);
  let cardHeight = $state(0);
  let gap = $state(0);
  let startRow = $state(0);
  let endRow = $state(12);
  let panel: HTMLElement | null = null;
  let frame = 0;
  let previousItems: T[] | undefined;
  const overscan = 3;

  const rowCount = $derived(Math.ceil(items.length / columns));
  const rowStep = $derived(cardHeight + gap);
  const height = $derived(Math.max(0, rowCount * rowStep - gap));
  const visibleItems = $derived(items.slice(startRow * columns, endRow * columns));

  function updateWindow() {
    if (!panel || !windowEl || rowStep <= 0) return;
    const top = Math.max(0, panel.getBoundingClientRect().top + panel.clientTop - windowEl.getBoundingClientRect().top);
    const first = Math.min(Math.max(0, rowCount - 1), Math.floor(top / rowStep));
    const nextStart = Math.max(0, first - overscan);
    const nextEnd = Math.min(rowCount, Math.ceil((top + panel.clientHeight) / rowStep) + overscan);
    if (nextStart !== startRow || nextEnd !== endRow) {
      onWindowChange?.();
      startRow = nextStart;
      endRow = nextEnd;
    }
  }

  function measure() {
    if (!gridEl) return;
    const card = gridEl.querySelector<HTMLElement>('.card');
    const art = card?.querySelector<HTMLElement>('.cover-wrap');
    const body = card?.querySelector<HTMLElement>('.card-body');
    if (card && art && body) {
      const gridStyle = getComputedStyle(gridEl);
      const cardStyle = getComputedStyle(card);
      columns = gridStyle.gridTemplateColumns.split(' ').length;
      gap = parseFloat(gridStyle.rowGap) || 0;
      cardHeight = art.getBoundingClientRect().height + body.getBoundingClientRect().height
        + parseFloat(cardStyle.borderTopWidth) + parseFloat(cardStyle.borderBottomWidth);
    }
    updateWindow();
  }

  function onScroll() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      updateWindow();
    });
  }

  onMount(() => {
    let ancestor = windowEl?.parentElement;
    while (ancestor) {
      const overflow = getComputedStyle(ancestor).overflowY;
      if (overflow === 'auto' || overflow === 'scroll') { panel = ancestor; break; }
      ancestor = ancestor.parentElement;
    }
    panel?.addEventListener('scroll', onScroll, { passive: true });
    const observer = new ResizeObserver(measure);
    if (gridEl) observer.observe(gridEl);
    if (panel) observer.observe(panel);
    measure();
    return () => {
      observer.disconnect();
      panel?.removeEventListener('scroll', onScroll);
      cancelAnimationFrame(frame);
      panel = null;
    };
  });

  $effect(() => {
    const nextItems = items;
    untrack(() => {
      if (nextItems === previousItems) return;
      previousItems = nextItems;
      if (!panel) return;
      const focused = document.activeElement;
      if (!focused || !panel.contains(focused) || !focused.matches('input, textarea, select')) panel.scrollTop = 0;
      startRow = 0;
      endRow = 12;
      tick().then(() => { if (panel) measure(); });
    });
  });
</script>

<div class="virtual-card-window" bind:this={windowEl} style:height="{height}px">
  <div class="card-grid" bind:this={gridEl} style:top="{startRow * rowStep}px" style:grid-auto-rows={cardHeight ? `${cardHeight}px` : undefined}>
    {#each visibleItems as item (item.id)}
      {@render renderCard(item)}
    {/each}
  </div>
</div>

<style>
  .virtual-card-window { position: relative; overflow-anchor: none; }
  .card-grid { position: absolute; left: 0; right: 0; }
  @media (max-width: 480px) {
    .card-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
