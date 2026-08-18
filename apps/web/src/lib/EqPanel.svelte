<script lang="ts">
  import { EQ_BANDS, EQ_MIN_DB, EQ_MAX_DB, EQ_PRESETS, flatGains, type EqState } from './equalizer';

  let {
    state = $bindable(),
    onUpdate,
  }: {
    state: EqState;
    /** Called after any change so the parent can push it to the graph + persist. */
    onUpdate: (s: EqState) => void;
  } = $props();

  function fmtFreq(hz: number): string {
    return hz >= 1000 ? `${hz / 1000}k` : `${hz}`;
  }
  function fmtDb(db: number): string {
    return `${db > 0 ? '+' : ''}${db}`;
  }

  function setBand(i: number, v: number) {
    state.gains[i] = v;
    onUpdate(state);
  }
  function setPreamp(v: number) {
    state.preamp = v;
    onUpdate(state);
  }
  function toggle() {
    state.enabled = !state.enabled;
    onUpdate(state);
  }
  function applyPreset(name: string) {
    const preset = EQ_PRESETS[name];
    if (!preset) return;
    state.gains = [...preset];
    state.enabled = true;
    onUpdate(state);
  }
  function reset() {
    state.gains = flatGains();
    state.preamp = 0;
    onUpdate(state);
  }
</script>

<div class="eq" role="group" aria-label="Equalizer">
  <div class="eq-head">
    <button class="eq-toggle" class:on={state.enabled} onclick={toggle} aria-pressed={state.enabled}>
      <span class="dot" aria-hidden="true"></span>{state.enabled ? 'On' : 'Off'}
    </button>
    <select
      class="eq-preset"
      aria-label="Preset"
      onchange={(e) => {
        applyPreset(e.currentTarget.value);
        e.currentTarget.selectedIndex = 0;
      }}
    >
      <option value="" disabled selected>Preset…</option>
      {#each Object.keys(EQ_PRESETS) as name}<option value={name}>{name}</option>{/each}
    </select>
    <button class="eq-reset" onclick={reset}>Reset</button>
  </div>

  <div class="eq-bands" class:muted={!state.enabled}>
    <div class="band">
      <span class="db">{fmtDb(state.preamp)}</span>
      <input
        class="slider"
        type="range"
        min={EQ_MIN_DB}
        max={EQ_MAX_DB}
        step="0.5"
        value={state.preamp}
        oninput={(e) => setPreamp(+e.currentTarget.value)}
        aria-label="Preamp"
      />
      <span class="freq">Pre</span>
    </div>
    {#each EQ_BANDS as freq, i}
      <div class="band">
        <span class="db">{fmtDb(state.gains[i])}</span>
        <input
          class="slider"
          type="range"
          min={EQ_MIN_DB}
          max={EQ_MAX_DB}
          step="0.5"
          value={state.gains[i]}
          oninput={(e) => setBand(i, +e.currentTarget.value)}
          aria-label={`${fmtFreq(freq)} Hz`}
        />
        <span class="freq">{fmtFreq(freq)}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .eq {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 340px;
  }

  .eq-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .eq-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
  }
  .eq-toggle .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted-2);
  }
  .eq-toggle.on {
    color: var(--text-bright);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }
  .eq-toggle.on .dot {
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
  }
  .eq-preset {
    flex: 1;
    font: inherit;
    font-size: 12px;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 8px;
    cursor: pointer;
  }
  .eq-reset {
    font: inherit;
    font-size: 12px;
    color: var(--muted);
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 10px;
    cursor: pointer;
  }
  .eq-reset:hover {
    color: var(--text);
  }

  .eq-bands {
    display: flex;
    justify-content: space-between;
    gap: 2px;
    transition: opacity 0.15s ease;
  }
  .eq-bands.muted {
    opacity: 0.4;
  }

  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    flex: 1;
  }
  .db {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
    min-height: 12px;
  }
  .freq {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--muted);
  }

  /* Vertical range sliders (writing-mode is the modern, un-prefixed way). */
  .slider {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 6px;
    height: 96px;
    accent-color: var(--accent);
    cursor: pointer;
  }
</style>
