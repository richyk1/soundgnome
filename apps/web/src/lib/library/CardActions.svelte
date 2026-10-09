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
    <i class="pxi pxi-more-horizontal" aria-hidden="true"></i>
  </button>
  {#if open}
    <dialog id={`${headingId}-dialog`} bind:this={dialog} use:showActions class="actions-sheet" aria-labelledby={headingId} onclick={closeBackdrop} onclose={() => (open = false)} onkeydown={(e) => e.stopPropagation()} oncancel={(e) => e.stopPropagation()}>
      <header>
        <h3 id={headingId}>{title}</h3>
        <button class="close" aria-label="Close actions" onclick={() => dialog?.close()}><i class="pxi pxi-close" aria-hidden="true"></i></button>
      </header>
      <div class="action-rows">
        {#each actions as action}
          <button class="action-row" class:danger={action.danger} aria-pressed={action.pressed} onclick={() => choose(action)}>
            <span>{action.label}</span>
            {#if action.pressed}<i class="pxi pxi-check" aria-hidden="true"></i>{/if}
          </button>
        {/each}
      </div>
    </dialog>
  {/if}
</div>

<style>
  .card-actions { position: absolute; top: 6px; right: 6px; z-index: 3; }
  .card-actions.inline { position: static; }
  /* A tinted ground chip, readable on art and sprites without a blur layer. */
  .actions-trigger, .close {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-chip);
    background: color-mix(in srgb, var(--bg) 78%, transparent);
    color: var(--text-bright);
    font: inherit;
    font-size: 24px;
    cursor: pointer;
    flex: 0 0 auto;
  }
  .actions-trigger:hover, .close:hover { background: var(--surface-2); }
  .close { background: transparent; border-color: transparent; color: var(--muted); }
  /* Mouse and trackpad: the chip appears with hover or keyboard focus, so a grid
     of covers is not stamped with a thousand buttons. Touch keeps it visible. */
  @media (hover: hover) and (pointer: fine) {
    .card-actions:not(.inline) {
      opacity: 0;
      transition: opacity var(--motion-fast) var(--ease-out);
    }
    :global(.card:hover) .card-actions,
    :global(.card:focus-within) .card-actions,
    .card-actions:has([aria-expanded="true"]) {
      opacity: 1;
    }
  }
  .actions-sheet {
    position: fixed;
    inset: auto;
    top: calc(var(--app-top, 0px) + var(--app-height, 100dvh) / 2);
    left: calc((100vw + var(--safe-left, 0px) - var(--safe-right, 0px)) / 2);
    transform: translate(-50%, -50%);
    margin: 0;
    width: min(360px, calc(100vw - var(--safe-left, 0px) - var(--safe-right, 0px) - 32px));
    max-height: calc(var(--app-height, 100dvh) - var(--safe-top, 0px) - var(--safe-bottom, 0px) - 32px);
    box-sizing: border-box;
    padding: 8px 16px 12px;
    overflow-y: auto;
    overscroll-behavior: contain;
    border: 1px solid var(--float-border);
    border-radius: var(--radius-panel);
    background: var(--float);
    color: var(--text);
    box-shadow: var(--float-shadow);
  }
  .actions-sheet::backdrop { background: var(--overlay); }
  header { display: flex; align-items: center; gap: 12px; padding: 4px 0 8px; }
  h3 { margin: 0; flex: 1; min-width: 0; overflow-wrap: anywhere; font-size: 15px; font-weight: 600; letter-spacing: -0.01em; color: var(--text-bright); }
  .action-rows { display: grid; }
  .action-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    min-height: 48px;
    padding: 0 4px;
    border: 0;
    border-top: 1px solid var(--border-soft);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 15px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
  }
  .action-row .pxi { font-size: 16px; }
  .action-row:hover { color: var(--text-bright); background: var(--surface); }
  .action-row[aria-pressed="true"] { color: var(--accent); }
  .action-row.danger { color: var(--error); }
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .actions-sheet {
      top: calc(var(--app-top, 0px) + var(--app-height, 100dvh) - var(--safe-bottom, 0px) - 8px);
      transform: translate(-50%, -100%);
      width: calc(100vw - var(--safe-left, 0px) - var(--safe-right, 0px) - 16px);
    }
    /* A quiet 28px chip inside the full 44px touch target, tucked into the corner. */
    .card-actions:not(.inline) { top: 0; right: 0; }
    .actions-trigger {
      position: relative;
      isolation: isolate;
      width: 44px;
      height: 44px;
      border: 0;
      background: transparent;
      font-size: 16px;
    }
    .actions-trigger::before {
      content: '';
      position: absolute;
      inset: 8px;
      z-index: -1;
      border-radius: var(--radius-chip);
      /* Opaque, so sprite pixels under the corner never show through. */
      background: var(--surface);
      box-shadow: inset 0 0 0 1px var(--border-strong);
    }
    .actions-trigger:hover { background: transparent; }
  }
</style>
