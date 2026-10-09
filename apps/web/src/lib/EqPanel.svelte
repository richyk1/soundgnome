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
  const H = 80;
  const DB_SPAN = 12;
  const freqs = curveFreqs(96);
  // The curve spans 20 Hz–20 kHz on a log axis; decade rules mark 100, 1k, 10k.
  const LOG_LO = Math.log10(20);
  const LOG_SPAN = Math.log10(20000) - LOG_LO;
  const DECADES = [100, 1000, 10000];
  const DB_RULES = [6, -6];

  let device = $derived(isDevicePreset(state.preset));
  let curveDb = $derived(
    responseCurveDb(activeBands(state), freqs, state.enabled ? state.preamp : 0),
  );
  let pathD = $derived(buildPath(curveDb));
  let areaD = $derived(`${pathD} L${W},${H / 2} L0,${H / 2} Z`);
  // Preamp fill runs from the 0 dB centre to the current value.
  let preampPct = $derived(((state.preamp - EQ_MIN_DB) / (EQ_MAX_DB - EQ_MIN_DB)) * 100);

  function freqX(hz: number): number {
    return ((Math.log10(hz) - LOG_LO) / LOG_SPAN) * W;
  }
  function dbY(db: number): number {
    const clamped = Math.max(-DB_SPAN, Math.min(DB_SPAN, db));
    return H / 2 - (clamped / DB_SPAN) * (H / 2 - 4);
  }

  function buildPath(db: number[]): string {
    const n = db.length;
    return db
      .map((d, i) => {
        const x = (i / (n - 1)) * W;
        const y = dbY(d);
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

<fieldset class="eq" aria-label="Equalizer">
  <div class="eq-head">
    <button
      class="filter-btn eq-toggle"
      class:active={state.enabled}
      onclick={toggle}
      aria-pressed={state.enabled}
    >
      <span class="led" aria-hidden="true"></span>{state.enabled ? 'On' : 'Off'}
    </button>
    <button class="filter-btn eq-reset" onclick={reset} title="Reset to flat">Reset</button>
  </div>

  <!-- Preset keys: the manual curves, then calibrated device corrections. -->
  <div class="eq-presets" role="group" aria-label="Preset">
    <div class="filter-group" role="group" aria-label="Presets">
      <button
        class="filter-btn"
        class:active={state.preset === 'custom'}
        aria-pressed={state.preset === 'custom'}
        onclick={() => choosePreset('custom')}
      >Custom</button>
      {#each graphicNames as name}
        <button
          class="filter-btn"
          class:active={state.preset === name}
          aria-pressed={state.preset === name}
          onclick={() => choosePreset(name)}
        >{name}</button>
      {/each}
    </div>
    <div class="filter-group" role="group" aria-label="Device correction">
      {#each deviceNames as name}
        <button
          class="filter-btn"
          class:active={state.preset === name}
          aria-pressed={state.preset === name}
          onclick={() => choosePreset(name)}
        ><i class="pxi pxi-headphone" aria-hidden="true"></i>{name}</button>
      {/each}
    </div>
  </div>

  <!-- Live frequency response of the active EQ, on a hairline graph grid. -->
  <div class="eq-graph">
    <div class="eq-curve" class:muted={!state.enabled}>
      <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="Frequency response">
        {#each DECADES as hz}
          <line class="grid" x1={freqX(hz)} y1="0" x2={freqX(hz)} y2={H} />
        {/each}
        {#each DB_RULES as db}
          <line class="grid" x1="0" y1={dbY(db)} x2={W} y2={dbY(db)} />
        {/each}
        <line class="axis" x1="0" y1={H / 2} x2={W} y2={H / 2} />
        <path class="area" d={areaD} />
        <path class="line" d={pathD} />
      </svg>
      <span class="eq-scale top">+{DB_SPAN}</span>
      <span class="eq-scale mid">0</span>
      <span class="eq-scale bot">-{DB_SPAN}</span>
    </div>
    <div class="eq-freq-axis" aria-hidden="true">
      {#each DECADES as hz}
        <span style="left: {(freqX(hz) / W) * 100}%">{fmtFreq(hz)}</span>
      {/each}
    </div>
  </div>

  <div class="eq-preamp" class:muted={!state.enabled}>
    <span class="lbl">Preamp</span>
    <input
      type="range"
      min={EQ_MIN_DB}
      max={EQ_MAX_DB}
      step="0.5"
      value={state.preamp}
      style="--lo: {Math.min(50, preampPct)}%; --hi: {Math.max(50, preampPct)}%"
      oninput={(e) => setPreamp(+e.currentTarget.value)}
      aria-label="Preamp"
    />
    <span class="val">{fmtDb(state.preamp)}</span>
  </div>

  {#if device}
    <div class="eq-device">
      <i class="pxi pxi-headphone" aria-hidden="true"></i>
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
          <div class="fader">
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
          </div>
          <span class="freq">{fmtFreq(freq)}</span>
        </div>
      {/each}
    </div>
  {/if}
</fieldset>

<style>
  .eq {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: 100%;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .eq :disabled { cursor: not-allowed; opacity: 0.65; }

  /* ── Power + reset ───────────────────────────────────────────────────── */
  .eq-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .eq-reset { margin-left: auto; }
  /* A square LED: dark when off, violet when the EQ is on. */
  .led {
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    background: var(--muted-2);
  }
  .eq-toggle.active .led { background: var(--accent); }

  /* ── Preset keys ─────────────────────────────────────────────────────── */
  .eq-presets {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .eq-presets .pxi { font-size: 16px; }

  /* ── Response graph ──────────────────────────────────────────────────── */
  .eq-graph { display: flex; flex-direction: column; gap: 4px; }
  .eq-curve {
    position: relative;
    height: 80px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    overflow: hidden;
    transition: opacity var(--motion-fast) var(--ease-out);
  }
  .eq-curve.muted { opacity: 0.45; }
  .eq-curve svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  .grid {
    stroke: var(--border-soft);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
    shape-rendering: crispEdges;
  }
  .axis {
    stroke: var(--border-strong);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
    shape-rendering: crispEdges;
  }
  .area { fill: var(--accent-muted); }
  .line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }
  .eq-scale,
  .eq-freq-axis span {
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
    pointer-events: none;
  }
  .eq-scale { position: absolute; right: 6px; }
  .eq-scale.top { top: 4px; }
  .eq-scale.mid { top: calc(50% - 12px); }
  .eq-scale.bot { bottom: 4px; }
  .eq-freq-axis {
    position: relative;
    height: 12px;
  }
  .eq-freq-axis span {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
  }

  /* ── Preamp ──────────────────────────────────────────────────────────── */
  .eq-preamp {
    display: flex;
    align-items: center;
    gap: 12px;
    transition: opacity var(--motion-fast) var(--ease-out);
  }
  .eq-preamp.muted { opacity: 0.55; }
  .eq-preamp .lbl {
    flex-shrink: 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .eq-preamp .val {
    flex-shrink: 0;
    width: 36px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  /* Hairline track; the violet fill runs from 0 dB to the value; square cap. */
  .eq-preamp input {
    flex: 1;
    min-width: 0;
    height: 44px;
    margin: 0;
    padding: 0;
    -webkit-appearance: none;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }
  .eq-preamp input::-webkit-slider-runnable-track {
    height: 2px;
    background: linear-gradient(
      to right,
      var(--border-strong) var(--lo, 50%),
      var(--accent) var(--lo, 50%),
      var(--accent) var(--hi, 50%),
      var(--border-strong) var(--hi, 50%)
    );
  }
  .eq-preamp input::-moz-range-track {
    height: 2px;
    background: var(--border-strong);
  }
  .eq-preamp input::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 10px;
    height: 10px;
    margin-top: -4px;
    border: 0;
    border-radius: 0;
    background: var(--accent);
  }
  .eq-preamp input::-moz-range-thumb {
    width: 10px;
    height: 10px;
    border: 0;
    border-radius: 0;
    background: var(--accent);
  }
  .eq-preamp input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* ── Band faders on a hairline faceplate (±12 rails, 0 dB centre) ────── */
  .eq-bands {
    display: flex;
    justify-content: space-between;
    transition: opacity var(--motion-fast) var(--ease-out);
  }
  .eq-bands.muted { opacity: 0.45; }
  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }
  .fader {
    position: relative;
    display: flex;
    justify-content: center;
    width: 100%;
    border-top: 1px solid var(--border-soft);
    border-bottom: 1px solid var(--border-soft);
  }
  .fader::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    border-top: 1px solid var(--border);
    pointer-events: none;
  }
  .db,
  .freq {
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 12px;
    font-variant-numeric: tabular-nums;
  }
  .db { min-height: 12px; color: var(--muted-2); }
  .freq { color: var(--muted); }
  .slider {
    position: relative;
    writing-mode: vertical-lr;
    direction: rtl;
    width: 28px;
    height: 92px;
    margin: 0;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .slider:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* ── Device-correction line ──────────────────────────────────────────── */
  .eq-device {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
  }
  .eq-device .pxi {
    flex-shrink: 0;
    font-size: 24px;
    color: var(--accent);
  }
  .eq-device-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .eq-device-text strong {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-bright);
  }
  .eq-device-text span {
    font-size: 12px;
    line-height: 1.45;
    color: var(--muted);
  }

  @media (max-width: 600px) {
    .eq-bands { flex-wrap: wrap; row-gap: 14px; }
    .band { flex: 0 0 20%; }
    .slider { width: 44px; height: 108px; }
  }

  @media (forced-colors: active) {
    .eq-preamp input { -webkit-appearance: auto; appearance: auto; }
    .led { border: 1px solid CanvasText; }
  }
</style>
