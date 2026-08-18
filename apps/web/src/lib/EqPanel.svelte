<script lang="ts">
  import {
    GRAPHIC_FREQS,
    GRAPHIC_PRESETS,
    DEVICE_PRESETS,
    EQ_MIN_DB,
    EQ_MAX_DB,
    activeBands,
    curveFreqs,
    flatGains,
    isDevicePreset,
    responseCurveDb,
    type EqState,
  } from './equalizer';

  let {
    state = $bindable(),
    onUpdate,
  }: {
    state: EqState;
    /** Called after any change so the parent can push it to the graph + persist. */
    onUpdate: (s: EqState) => void;
  } = $props();

  const graphicNames = Object.keys(GRAPHIC_PRESETS);
  const deviceNames = Object.keys(DEVICE_PRESETS);

  // ── Response curve geometry (SVG viewBox units) ────────────────────────────
  const W = 336;
  const H = 78;
  const DB_SPAN = 12;
  const freqs = curveFreqs(96);

  let device = $derived(isDevicePreset(state.preset));
  let curveDb = $derived(
    responseCurveDb(activeBands(state), freqs, state.enabled ? state.preamp : 0),
  );
  let pathD = $derived(buildPath(curveDb));
  let areaD = $derived(`${pathD} L${W},${H / 2} L0,${H / 2} Z`);

  function buildPath(db: number[]): string {
    const n = db.length;
    return db
      .map((d, i) => {
        const x = (i / (n - 1)) * W;
        const clamped = Math.max(-DB_SPAN, Math.min(DB_SPAN, d));
        const y = H / 2 - (clamped / DB_SPAN) * (H / 2 - 4);
        return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(' ');
  }

  function fmtFreq(hz: number): string {
    return hz >= 1000 ? `${hz / 1000}k` : `${hz}`;
  }
  function fmtDb(db: number): string {
    return `${db > 0 ? '+' : ''}${Math.round(db * 10) / 10}`;
  }

  function choosePreset(name: string) {
    if (name === 'custom') {
      state.preset = 'custom';
    } else if (GRAPHIC_PRESETS[name]) {
      state.gains = [...GRAPHIC_PRESETS[name]];
      state.preset = name;
      state.enabled = true;
    } else if (DEVICE_PRESETS[name]) {
      state.preset = name;
      state.enabled = true;
    }
    onUpdate(state);
  }
  function setBand(i: number, v: number) {
    state.gains[i] = v;
    state.preset = 'custom';
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
  function reset() {
    state.gains = flatGains();
    state.preamp = 0;
    state.preset = 'custom';
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
      value={state.preset}
      onchange={(e) => choosePreset(e.currentTarget.value)}
    >
      <option value="custom">Custom</option>
      <optgroup label="Presets">
        {#each graphicNames as name}<option value={name}>{name}</option>{/each}
      </optgroup>
      <optgroup label="Device correction">
        {#each deviceNames as name}<option value={name}>{name}</option>{/each}
      </optgroup>
    </select>
    <button class="eq-reset" onclick={reset} title="Reset to flat">Reset</button>
  </div>

  <!-- Live frequency response of the active EQ -->
  <div class="eq-curve" class:muted={!state.enabled}>
    <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="Frequency response">
      <line class="axis" x1="0" y1={H / 2} x2={W} y2={H / 2} />
      <path class="area" d={areaD} />
      <path class="line" d={pathD} />
    </svg>
    <span class="eq-scale top">+{DB_SPAN}</span>
    <span class="eq-scale bot">-{DB_SPAN}</span>
  </div>

  <div class="eq-preamp" class:muted={!state.enabled}>
    <span class="lbl">Preamp</span>
    <input
      type="range"
      min={EQ_MIN_DB}
      max={EQ_MAX_DB}
      step="0.5"
      value={state.preamp}
      oninput={(e) => setPreamp(+e.currentTarget.value)}
      aria-label="Preamp"
    />
    <span class="val">{fmtDb(state.preamp)}</span>
  </div>

  {#if device}
    <div class="eq-device">
      <i class="lni lni-headphone-bluetooth" aria-hidden="true"></i>
      <div class="eq-device-text">
        <strong>{state.preset}</strong>
        <span>Calibrated correction curve. Choose Custom to shape it by hand.</span>
      </div>
    </div>
  {:else}
    <div class="eq-bands" class:muted={!state.enabled}>
      {#each GRAPHIC_FREQS as freq, i}
        <div class="band">
          <span class="db">{fmtDb(state.gains[i] ?? 0)}</span>
          <input
            class="slider"
            type="range"
            min={EQ_MIN_DB}
            max={EQ_MAX_DB}
            step="0.5"
            value={state.gains[i] ?? 0}
            oninput={(e) => setBand(i, +e.currentTarget.value)}
            aria-label={`${fmtFreq(freq)} Hz`}
          />
          <span class="freq">{fmtFreq(freq)}</span>
        </div>
      {/each}
    </div>
  {/if}
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
    transition:
      background 0.15s ease,
      box-shadow 0.15s ease;
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
    min-width: 0;
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

  /* ── Response curve ──────────────────────────────────────────────────── */
  .eq-curve {
    position: relative;
    height: 78px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    transition: opacity 0.15s ease;
  }
  .eq-curve.muted {
    opacity: 0.45;
  }
  .eq-curve svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  .axis {
    stroke: var(--border);
    stroke-width: 1;
    stroke-dasharray: 3 3;
    vector-effect: non-scaling-stroke;
  }
  .area {
    fill: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }
  .eq-scale {
    position: absolute;
    right: 5px;
    font-family: var(--font-mono);
    font-size: 8.5px;
    color: var(--muted-2);
    pointer-events: none;
  }
  .eq-scale.top {
    top: 3px;
  }
  .eq-scale.bot {
    bottom: 3px;
  }

  /* ── Preamp ──────────────────────────────────────────────────────────── */
  .eq-preamp {
    display: flex;
    align-items: center;
    gap: 10px;
    transition: opacity 0.15s ease;
  }
  .eq-preamp.muted {
    opacity: 0.55;
  }
  .eq-preamp .lbl {
    font-size: 11px;
    color: var(--muted);
    width: 46px;
    flex-shrink: 0;
  }
  .eq-preamp input {
    flex: 1;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .eq-preamp .val {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--muted-2);
    width: 34px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* ── Band sliders ────────────────────────────────────────────────────── */
  .eq-bands {
    display: flex;
    justify-content: space-between;
    gap: 2px;
    transition: opacity 0.15s ease;
  }
  .eq-bands.muted {
    opacity: 0.45;
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
    font-size: 9px;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
    min-height: 12px;
  }
  .freq {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--muted);
  }
  .slider {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 6px;
    height: 92px;
    accent-color: var(--accent);
    cursor: pointer;
  }

  /* ── Device-correction note ──────────────────────────────────────────── */
  .eq-device {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    background: color-mix(in srgb, var(--accent) 8%, var(--surface-2));
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 8px;
  }
  .eq-device .lni {
    font-size: 22px;
    color: var(--accent);
    flex-shrink: 0;
  }
  .eq-device-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .eq-device-text strong {
    font-size: 13px;
    color: var(--text-bright);
  }
  .eq-device-text span {
    font-size: 11px;
    color: var(--muted);
    line-height: 1.4;
  }
</style>
