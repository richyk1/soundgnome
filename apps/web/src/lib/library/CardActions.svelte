<script lang="ts">
  type Action = { label: string; onSelect: () => unknown; pressed?: boolean; danger?: boolean };
  let { title, actions, inline = false }: { title: string; actions: Action[]; inline?: boolean } = $props();
  const id = $props.id();
  const headingId = `${id}-title`;
  let dialog = $state<HTMLDialogElement>();
  let open = $state(false);

  function showActions(node: HTMLDialogElement) {
    node.showModal();
    return { destroy: () => node.close() };
  }

  function choose(action: Action) {
    // Native close restores focus before a selection, confirmation, or Edit dialog opens.
    dialog?.close();
    action.onSelect();
  }

  function closeBackdrop(e: MouseEvent) {
    const node = e.currentTarget as HTMLDialogElement;
    if (e.target !== node) return;
    const bounds = node.getBoundingClientRect();
    if (e.clientX < bounds.left || e.clientX > bounds.right || e.clientY < bounds.top || e.clientY > bounds.bottom) node.close();
  }
</script>

<div class="card-actions" class:inline onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="presentation">
  <button class="actions-trigger" aria-label={`Actions for ${title}`} aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? `${headingId}-dialog` : undefined} onclick={() => (open = true)}>
    <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="5" cy="12" r="2"/><circle cx="12" cy="12" r="2"/><circle cx="19" cy="12" r="2"/></svg>
  </button>
  {#if open}
    <dialog id={`${headingId}-dialog`} bind:this={dialog} use:showActions class="actions-sheet" aria-labelledby={headingId} onclick={closeBackdrop} onclose={() => (open = false)} onkeydown={(e) => e.stopPropagation()} oncancel={(e) => e.stopPropagation()}>
      <header>
        <h3 id={headingId}>{title}</h3>
        <button class="close" aria-label="Close actions" onclick={() => dialog?.close()}>×</button>
      </header>
      <div class="action-rows">
        {#each actions as action}
          <button class="action-row" class:danger={action.danger} aria-pressed={action.pressed} onclick={() => choose(action)}>
            <span>{action.label}</span>
            {#if action.pressed}<span aria-hidden="true">✓</span>{/if}
          </button>
        {/each}
      </div>
    </dialog>
  {/if}
</div>

<style>
  .card-actions { position: absolute; top: 8px; right: 8px; z-index: 3; }
  .card-actions.inline { position: static; }
  .actions-trigger, .close { display: flex; align-items: center; justify-content: center; width: 44px; height: 44px; padding: 0; border: 1px solid var(--border); border-radius: 50%; background: var(--surface); color: var(--text-bright); cursor: pointer; font: inherit; flex: 0 0 auto; }
  .actions-trigger { box-shadow: var(--shadow-sm); }
  .actions-trigger:hover, .close:hover { background: var(--surface-2); }
  .actions-sheet { position: fixed; inset: auto; top: calc(var(--app-top, 0px) + var(--app-height, 100dvh) / 2); left: calc((100vw + var(--safe-left, 0px) - var(--safe-right, 0px)) / 2); transform: translate(-50%, -50%); margin: 0; width: min(360px, calc(100vw - var(--safe-left, 0px) - var(--safe-right, 0px) - 32px)); max-height: calc(var(--app-height, 100dvh) - var(--safe-top, 0px) - var(--safe-bottom, 0px) - 32px); box-sizing: border-box; padding: 16px; overflow-y: auto; overscroll-behavior: contain; border: 1px solid var(--border); border-radius: 20px; background: var(--panel); color: var(--text); box-shadow: var(--shadow); }
  .actions-sheet::backdrop { background: var(--overlay); }
  header { display: flex; align-items: center; gap: 12px; padding-bottom: 12px; }
  h3 { margin: 0; flex: 1; min-width: 0; overflow-wrap: anywhere; font: 700 1rem var(--font-display); color: var(--text-bright); }
  .close { font-size: 24px; }
  .action-rows { display: grid; gap: 4px; }
  .action-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; width: 100%; min-height: 48px; padding: 10px 12px; text-align: left; font: inherit; font-size: 16px; font-weight: 500; color: var(--text); background: var(--surface); border: 1px solid var(--border); border-radius: 10px; cursor: pointer; transition: background var(--motion-fast, 160ms) var(--ease-out, ease-out); }
  .action-row:hover { background: var(--surface-2); }
  .action-row[aria-pressed="true"] { color: var(--accent-2); }
  .action-row.danger { color: var(--error); }
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .actions-sheet { top: calc(var(--app-top, 0px) + var(--app-height, 100dvh) - var(--safe-bottom, 0px) - 8px); transform: translate(-50%, -100%); width: calc(100vw - var(--safe-left, 0px) - var(--safe-right, 0px) - 16px); padding: 16px; border-radius: 22px; }
  }
  @media (prefers-reduced-motion: reduce) { .action-row { transition: none; } }
</style>
