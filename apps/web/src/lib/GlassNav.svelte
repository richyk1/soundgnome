<script lang="ts">
  import { onMount } from 'svelte';
  import { createLensMap } from './glass-lens';

  interface NavigationItem {
    id: string;
    label: string;
    icon: string;
    selected: boolean;
    current?: boolean;
    expanded?: boolean;
    controls?: string;
    onSelect: (trigger: HTMLButtonElement) => void;
  }

  let { items }: { items: readonly NavigationItem[] } = $props();
  let nav: HTMLElement;
  let lensWidth = $state(0);
  let stripWidth = $state(0);
  let map = $state('');
  let filterId = $state('');
  const instanceId = $props.id();
  let generation = 0;
  const selectedIndex = $derived(items.findIndex((item) => item.selected));
  const offset = $derived(Math.max(0, selectedIndex) * stripWidth / 4);

  onMount(() => {
    function resize() {
      const available = Math.max(0, nav.clientWidth - 12);
      const width = Math.floor(available / 4);
      stripWidth = available;
      if (width === lensWidth || width === 0) return;
      lensWidth = width;
      map = createLensMap(width, 48, 24);
      // Safari can retain a stale feImage when an existing filter changes.
      filterId = `${instanceId}-glass-${++generation}`;
    }

    const observer = new ResizeObserver(resize);
    observer.observe(nav);
    resize();
    return () => observer.disconnect();
  });
</script>

<nav
  bind:this={nav}
  class="glass-nav"
  class:has-map={map !== ''}
  aria-label="Main navigation"
  style:--lens-width={`${lensWidth}px`}
  style:--strip-width={`${stripWidth}px`}
  style:--selection-offset={`${offset}px`}
>
  {#if map}
    <svg class="filter-definitions" width="1" height="1" aria-hidden="true" focusable="false">
      <defs>
        <filter
          id={filterId}
          filterUnits="userSpaceOnUse"
          primitiveUnits="userSpaceOnUse"
          x="0"
          y="0"
          width={lensWidth}
          height="48"
          color-interpolation-filters="sRGB"
        >
          <feImage href={map} x="0" y="0" width={lensWidth} height="48" result="lens-map" />
          <feDisplacementMap
            in="SourceGraphic"
            in2="lens-map"
            scale="8"
            xChannelSelector="R"
            yChannelSelector="G"
          />
        </filter>
      </defs>
    </svg>
  {/if}

  {#if selectedIndex >= 0}
    <div class="selection-lens" aria-hidden="true" inert>
      {#if map}
        <div class="lens-source" style:filter={`url(#${filterId})`}>
          <div class="decorative-strip">
            {#each items as item (item.id)}
              <span class="option-copy">
                <i class={`lni lni-${item.icon}`}></i>
                <span>{item.label}</span>
              </span>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}

  {#each items as item (item.id)}
    <button
      type="button"
      class:selected={item.selected}
      aria-current={item.current ? 'page' : undefined}
      aria-expanded={item.expanded}
      aria-controls={item.controls}
      onclick={(event) => item.onSelect(event.currentTarget)}
    >
      <i class={`lni lni-${item.icon}`} aria-hidden="true"></i>
      <span>{item.label}</span>
    </button>
  {/each}
</nav>

<style>
  .glass-nav {
    position: relative;
    isolation: isolate;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    box-sizing: border-box;
    height: 60px;
    padding: 6px;
  }

  .filter-definitions {
    position: absolute;
    pointer-events: none;
    overflow: hidden;
  }

  button,
  .option-copy {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    height: 48px;
    min-width: 0;
    font: inherit;
    font-size: 10px;
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
  }

  button {
    position: relative;
    z-index: 1;
    min-height: 44px;
    padding: 0;
    border: 0;
    border-radius: 24px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
  }

  button.selected { color: var(--text-bright); }
  .has-map button.selected { color: transparent; }
  button:hover:not(.selected) { color: var(--text); }
  button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  button i, .option-copy i { font-size: 19px; }

  .selection-lens {
    position: absolute;
    top: 6px;
    left: 6px;
    width: var(--lens-width);
    height: 48px;
    overflow: hidden;
    border-radius: 24px;
    background: color-mix(in srgb, var(--accent) 12%, var(--surface));
    box-shadow: inset 0 1px 0 var(--glass-highlight),
      inset 0 -1px 0 var(--glass-border);
    transform: translateX(var(--selection-offset));
    transition: transform var(--motion-normal) var(--ease-out);
    pointer-events: none;
  }

  .lens-source {
    width: var(--lens-width);
    height: 48px;
    overflow: hidden;
    pointer-events: none;
    background: linear-gradient(145deg,
      color-mix(in srgb, var(--text-bright) 14%, transparent),
      transparent 45%,
      color-mix(in srgb, var(--accent) 18%, transparent));
  }

  .decorative-strip {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    width: var(--strip-width);
    height: 48px;
    color: var(--text-bright);
    transform: translateX(calc(-1 * var(--selection-offset)));
    transition: transform var(--motion-normal) var(--ease-out);
    pointer-events: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .selection-lens, .decorative-strip { transition: none; }
  }

  @media (prefers-reduced-transparency: reduce) {
    .lens-source { display: none; }
    .selection-lens { background: var(--surface); box-shadow: inset 0 0 0 1px var(--border); }
    .has-map button.selected { color: var(--text-bright); }
  }

  @media (forced-colors: active) {
    .selection-lens { display: none; }
    button { color: ButtonText; }
    .has-map button.selected, button.selected {
      color: HighlightText;
      background: Highlight;
      forced-color-adjust: none;
    }
    button:focus-visible { outline-color: ButtonText; }
  }
</style>
