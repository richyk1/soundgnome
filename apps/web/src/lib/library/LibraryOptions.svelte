<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  let { search, children }: { search: Snippet; children: Snippet } = $props();
  let expanded = $state(false);
  let mobile = $state(false);
  let detailsEl: HTMLDetailsElement | undefined = $state();

  onMount(() => {
    const query = window.matchMedia('(max-width: 860px), (hover: none) and (pointer: coarse)');
    const syncLayout = () => { mobile = query.matches; expanded = !query.matches; };
    syncLayout();
    query.addEventListener('change', syncLayout);
    return () => query.removeEventListener('change', syncLayout);
  });

  // On phones the options float over the content, so a tap outside or Escape dismisses them.
  $effect(() => {
    if (!mobile || !expanded) return;
    function onPointerDown(e: PointerEvent) {
      if (detailsEl && !detailsEl.contains(e.target as Node)) expanded = false;
    }
    function onKeydown(e: KeyboardEvent) {
      if (e.key === 'Escape') { expanded = false; detailsEl?.querySelector('summary')?.focus(); }
    }
    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeydown);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown);
      document.removeEventListener('keydown', onKeydown);
    };
  });
</script>

<div class="collection-toolbar">
  <div class="search-slot">{@render search()}</div>
  <details bind:this={detailsEl} bind:open={expanded}>
    <summary aria-label="Sort and view options" title="Sort and view options">
      <i class="pxi pxi-sliders-vertical" aria-hidden="true"></i>
    </summary>
    <div class="options">{@render children()}</div>
  </details>
</div>

<style>
  .collection-toolbar { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin-bottom: 20px; min-width: 0; }

  /* Search: one field with a 16px pixel glyph; tabs render `.pxi-search` + `input.search`. */
  .search-slot { position: relative; flex: 1 1 240px; max-width: 360px; min-width: 0; }
  .search-slot :global(.search) { width: 100%; max-width: none; box-sizing: border-box; padding-left: 36px; }
  .search-slot :global(.pxi-search) {
    position: absolute;
    left: 12px;
    top: 50%;
    translate: 0 -50%;
    z-index: 1;
    font-size: 16px;
    color: var(--muted-2);
    pointer-events: none;
  }

  details { display: contents; }
  summary { display: none; }
  .options { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; flex: 1 1 auto; min-width: 0; }
  .options :global(.opt-row) { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; min-width: 0; }
  .options :global(.view-toggle button) { justify-content: center; min-width: 36px; min-height: 34px; font-size: 16px; }
  .options :global(.count) { margin-left: 4px; white-space: nowrap; }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .collection-toolbar { display: block; position: relative; margin-bottom: 12px; }
    .search-slot { position: absolute; top: 0; left: 0; right: 52px; max-width: none; }
    .search-slot :global(.search) { min-width: 0; min-height: 44px; font-size: 16px; }
    details { display: block; width: 100%; min-width: 0; }
    summary {
      display: flex;
      align-items: center;
      justify-content: center;
      margin-left: auto;
      width: 44px;
      height: 44px;
      padding: 0;
      list-style: none;
      border: 1px solid var(--border-strong);
      border-radius: var(--radius-control);
      box-sizing: border-box;
      color: var(--text);
      background: transparent;
      font-size: 16px;
      cursor: pointer;
      transition: background-color var(--motion-fast) var(--ease-out), border-color var(--motion-fast) var(--ease-out), color var(--motion-fast) var(--ease-out);
    }
    summary::-webkit-details-marker { display: none; }
    summary:active { background: var(--surface-2); }
    details[open] summary {
      color: var(--accent);
      border-color: color-mix(in srgb, var(--accent) 45%, transparent);
      background: var(--accent-muted);
    }

    /* Floating panel with hairline rows, like the shell's More sheet. */
    .options {
      position: absolute;
      top: 52px;
      left: 0;
      right: 0;
      z-index: 30;
      display: flex;
      flex-direction: column;
      align-items: stretch;
      gap: 0;
      padding: 4px 16px;
      border: 1px solid var(--float-border);
      border-radius: var(--radius-panel);
      background: var(--float);
      box-shadow: var(--float-shadow);
    }
    .options :global(.opt-row) { min-height: 56px; padding: 6px 0; }
    .options :global(.opt-row + .opt-row) { border-top: 1px solid var(--border-soft); }
    .options :global(.sort-controls) { flex: 1 1 auto; }
    .options :global(.view-toggle button) { min-width: 44px; min-height: 42px; }
    .options :global(.count) { display: none; }
    details[open] .options { animation: options-open var(--motion-fast) var(--ease-out); }
  }
  @keyframes options-open { from { opacity: 0; } to { opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { summary { transition: none; } details[open] .options { animation: none; } }
</style>
