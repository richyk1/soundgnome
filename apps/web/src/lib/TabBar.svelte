<script lang="ts">
  interface NavigationItem {
    id: string;
    label: string;
    /** pixel icon name: renders `pxi pxi-<icon>` */
    icon: string;
    selected: boolean;
    current?: boolean;
    expanded?: boolean;
    controls?: string;
    onSelect: (trigger: HTMLButtonElement) => void;
  }

  let { items }: { items: readonly NavigationItem[] } = $props();
</script>

<nav class="tab-bar" aria-label="Main navigation">
  {#each items as item (item.id)}
    <button
      type="button"
      class="tab-item"
      class:selected={item.selected}
      aria-label={item.label}
      title={item.label}
      aria-current={item.current ? 'page' : undefined}
      aria-expanded={item.expanded}
      aria-controls={item.controls}
      onclick={(event) => item.onSelect(event.currentTarget)}
    >
      <i class="pxi pxi-{item.icon}" aria-hidden="true"></i>
    </button>
  {/each}
</nav>

<style>
  .tab-bar {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    height: var(--nav-height);
  }

  /* Icons only: the label is the button's accessible name and tooltip. */
  .tab-item {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--muted-2);
    cursor: pointer;
  }
  /* 24px keeps every icon pixel on whole device pixels at 2× and 3×. */
  .tab-item .pxi { font-size: 24px; }

  /* Selection: a short violet bar riding the dock's top hairline. */
  .tab-item::before {
    content: '';
    position: absolute;
    top: -1px;
    left: 50%;
    width: 24px;
    height: 2px;
    margin-left: -12px;
    background: var(--accent);
    transform: scaleX(0);
    transition: transform var(--motion-fast) var(--ease-out);
  }
  .tab-item.selected { color: var(--text-bright); }
  .tab-item.selected::before { transform: scaleX(1); }
  .tab-item:active { color: var(--text); }
  .tab-item:focus-visible { outline-offset: -4px; }

  @media (forced-colors: active) {
    .tab-item.selected { color: Highlight; }
    .tab-item::before { background: Highlight; }
  }
</style>
