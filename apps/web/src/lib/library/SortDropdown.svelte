<script lang="ts">
  interface Option {
    value: string;
    label: string;
  }

  interface Props {
    value: string;
    direction: 'asc' | 'desc';
    options: Option[];
    onChange?: (value: string) => void;
    onDirectionChange?: (dir: 'asc' | 'desc') => void;
  }

  let { value, direction, options, onChange, onDirectionChange }: Props = $props();

  function toggleDirection() {
    const newDir = direction === 'asc' ? 'desc' : 'asc';
    onDirectionChange?.(newDir);
  }

  function handleSelectChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    onChange?.(target.value);
  }
</script>

<div class="sort-controls">
  <span class="sort-field">
    <i class="pxi pxi-sort-vertical lead" aria-hidden="true"></i>
    <select class="sort-select" value={value} onchange={handleSelectChange} title="Sort by" aria-label="Sort by">
      {#each options as opt (opt.value)}
        <option value={opt.value}>{opt.label}</option>
      {/each}
    </select>
    <i class="pxi pxi-chevron-down trail" aria-hidden="true"></i>
  </span>

  <button
    class="sort-direction-btn"
    title={direction === 'asc' ? 'Ascending' : 'Descending'}
    onclick={toggleDirection}
    aria-label={direction === 'asc' ? 'Sort ascending' : 'Sort descending'}
  >
    <i class="pxi {direction === 'asc' ? 'pxi-arrow-up' : 'pxi-arrow-down'}" aria-hidden="true"></i>
  </button>
</div>

<style>
  .sort-controls {
    display: flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }

  /* Hairline control sized like .btn-header; the native select keeps keyboard and picker behavior. */
  .sort-field {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-width: 0;
  }
  .sort-field .pxi {
    position: absolute;
    top: 50%;
    translate: 0 -50%;
    font-size: 16px;
    color: var(--muted-2);
    pointer-events: none;
  }
  .lead { left: 10px; }
  .trail { right: 8px; }

  .sort-select {
    width: auto;
    min-height: 36px;
    padding: 0 30px 0 34px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--text);
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    appearance: none;
    -webkit-appearance: none;
    transition: background-color var(--motion-fast) var(--ease-out), border-color var(--motion-fast) var(--ease-out);
  }
  .sort-select:hover { background: var(--surface-2); border-color: var(--border-heavy); }
  .sort-select option { background: var(--float); color: var(--text); }

  .sort-direction-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--muted);
    font-size: 16px;
    cursor: pointer;
    flex-shrink: 0;
  }
  .sort-direction-btn:hover { background: var(--surface-2); color: var(--text-bright); }
  .sort-direction-btn:active { background: var(--surface-2); border-color: var(--border-heavy); }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .sort-controls { min-width: 0; max-width: 100%; }
    .sort-field { flex: 1 1 auto; }
    .sort-select { min-width: 0; min-height: 44px; font-size: 16px; }
    .sort-direction-btn { width: 44px; height: 44px; }
  }
</style>
