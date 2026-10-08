<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  let { search, children }: { search: Snippet; children: Snippet } = $props();
  let expanded = $state(false);

  onMount(() => {
    const mobile = window.matchMedia('(max-width: 860px), (hover: none) and (pointer: coarse)');
    const syncLayout = () => { expanded = !mobile.matches; };
    syncLayout();
    mobile.addEventListener('change', syncLayout);
    return () => mobile.removeEventListener('change', syncLayout);
  });
</script>

<div class="collection-toolbar">
  <div class="search-slot">{@render search()}</div>
  <details bind:open={expanded}>
    <summary aria-label="Sort and view options" title="Sort and view options">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true"><path d="M4 7h16M4 17h16"/><circle cx="9" cy="7" r="3" fill="var(--surface)"/><circle cx="15" cy="17" r="3" fill="var(--surface)"/></svg>
    </summary>
    <div class="options">{@render children()}</div>
  </details>
</div>

<style>
  .collection-toolbar { display: flex; align-items: center; flex-wrap: wrap; gap: 12px; margin-bottom: 16px; min-width: 0; }
  .search-slot { flex: 1 1 240px; min-width: 0; }
  .search-slot :global(.search) { width: 100%; box-sizing: border-box; }
  details { display: contents; }
  summary { display: none; }
  .options { display: flex; align-items: center; flex-wrap: wrap; gap: 12px; min-width: 0; }
  .options :global(button), .options :global(select) { min-width: 44px; min-height: 44px; }
  .options :global(.view-toggle button) { justify-content: center; }
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .collection-toolbar { display: block; position: relative; margin-bottom: 12px; }
    .search-slot { position: absolute; top: 0; left: 0; right: 52px; }
    .search-slot :global(.search) { width: 100%; min-width: 0; max-width: none; min-height: 44px; font-size: 16px; }
    details { display: block; width: 100%; min-width: 0; }
    summary { display: flex; align-items: center; justify-content: center; margin-left: auto; width: 44px; height: 44px; padding: 0; list-style: none; border: 1px solid var(--border); border-radius: 10px; box-sizing: border-box; color: var(--text); background: var(--surface); cursor: pointer; transition: background var(--motion-fast, 160ms) var(--ease-out, ease-out), border-color var(--motion-fast, 160ms) var(--ease-out, ease-out); }
    summary::-webkit-details-marker { display: none; }
    details[open] summary { color: var(--accent-2); border-color: var(--accent); background: var(--surface-2); }
    .options { margin-top: 8px; padding: 12px; border: 1px solid var(--border); border-radius: 12px; background: var(--surface); gap: 8px; }
    details[open] .options { animation: options-open var(--motion-fast, 160ms) var(--ease-out, ease-out); }
    .options :global(button), .options :global(select) { font-size: 14px; }
    .options :global(select) { font-size: 16px; max-width: 100%; }
    .options :global(.count) { display: none; }
  }
  @keyframes options-open { from { opacity: 0; } to { opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { summary { transition: none; } details[open] .options { animation: none; } }
</style>
