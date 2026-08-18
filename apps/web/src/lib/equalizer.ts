// In-app parametric equalizer built on the Web Audio API — the same technique
// browser EQ extensions (e.g. Ears) use: the <audio> element is routed through a
// chain of peaking BiquadFilters, one per band, plus a preamp gain.
//
// Each band is fully parametric (frequency + gain + Q), which is what lets a
// calibrated device-correction curve (e.g. AirPods Pro 2, exported from Ears) be
// reproduced faithfully rather than approximated onto fixed graphic bands.
//
// It is opt-in: until enabled, no AudioContext is created and the element plays
// untouched, so default playback (including cross-origin SoundCloud streams) is
// unaffected. Enabling routes the element through the graph, which needs the
// audio to be same-origin or CORS-enabled — library files (`/api/tracks/<id>/
// audio`) are same-origin, so they always work.

export interface Band {
  freq: number;
  /** dB */
  gain: number;
  q: number;
}

/** Fixed layout for the manual graphic EQ (Custom mode). */
export const GRAPHIC_FREQS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;
export const GRAPHIC_Q = 1.0;
export const EQ_MIN_DB = -12;
export const EQ_MAX_DB = 12;

/** Manual graphic presets: name -> per-band gain (dB), aligned to GRAPHIC_FREQS. */
export const GRAPHIC_PRESETS: Record<string, number[]> = {
  Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  'Bass boost': [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
  'Treble boost': [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
  Vocal: [-2, -1, 0, 2, 4, 4, 3, 1, 0, -1],
  Loudness: [5, 4, 2, 0, -1, 0, 1, 3, 4, 5],
  Podcast: [-4, -3, 0, 2, 3, 3, 2, 1, -1, -3],
};

/**
 * Calibrated device-correction curves (parametric). AirPods Pro 2 is the curve
 * from the Ears preset export (oratory1990-style correction): arbitrary
 * frequencies and Q per band, applied verbatim.
 */
export const DEVICE_PRESETS: Record<string, Band[]> = {
  'AirPods Pro 2': [
    { freq: 24, gain: -0.76, q: 1.165 },
    { freq: 62, gain: 2.11, q: 1.042 },
    { freq: 84, gain: 1.28, q: 1.348 },
    { freq: 428, gain: -1.6, q: 1.796 },
    { freq: 3781, gain: 6.49, q: 0.4 },
    { freq: 6268, gain: -2.28, q: 3.972 },
    { freq: 7202, gain: -3.1, q: 3.008 },
    { freq: 9965, gain: 5.36, q: 1.292 },
    { freq: 16000, gain: 2.59, q: 0.917 },
    { freq: 16000, gain: 2.06, q: 1.737 },
  ],
};

/** 'custom' = manual graphic sliders; otherwise a preset name. */
export type PresetName = 'custom' | string;

export interface EqState {
  enabled: boolean;
  /** dB makeup gain applied before the bands. */
  preamp: number;
  preset: PresetName;
  /** Graphic-band gains (dB), used when preset is 'custom' or a graphic preset. */
  gains: number[];
}

const STORAGE_KEY = 'soundgnome:eq:v2';

export function flatGains(): number[] {
  return GRAPHIC_FREQS.map(() => 0);
}

export function defaultEqState(): EqState {
  return { enabled: false, preamp: 0, preset: 'custom', gains: flatGains() };
}

export function isDevicePreset(preset: PresetName): boolean {
  return preset !== 'custom' && preset in DEVICE_PRESETS;
}

/** The parametric bands currently in effect for a state. */
export function activeBands(state: EqState): Band[] {
  const device = DEVICE_PRESETS[state.preset];
  if (device) return device;
  return GRAPHIC_FREQS.map((freq, i) => ({
    freq,
    gain: clampDb(state.gains[i] ?? 0),
    q: GRAPHIC_Q,
  }));
}

export function loadEqState(): EqState {
  if (typeof localStorage === 'undefined') return defaultEqState();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultEqState();
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) return defaultEqState();

    const enabled = 'enabled' in parsed && !!parsed.enabled;
    const preamp = 'preamp' in parsed ? clampDb(Number(parsed.preamp) || 0) : 0;
    const presetRaw =
      'preset' in parsed && typeof parsed.preset === 'string' ? parsed.preset : 'custom';
    const preset =
      presetRaw === 'custom' || presetRaw in GRAPHIC_PRESETS || presetRaw in DEVICE_PRESETS
        ? presetRaw
        : 'custom';
    const gainsRaw = 'gains' in parsed && Array.isArray(parsed.gains) ? parsed.gains : [];
    return {
      enabled,
      preamp,
      preset,
      gains: GRAPHIC_FREQS.map((_, i) => clampDb(Number(gainsRaw[i]) || 0)),
    };
  } catch {
    return defaultEqState();
  }
}

export function saveEqState(state: EqState): void {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    /* storage unavailable — non-fatal */
  }
}

export function clampDb(db: number): number {
  if (!Number.isFinite(db)) return 0;
  return Math.max(EQ_MIN_DB, Math.min(EQ_MAX_DB, db));
}

function dbToGain(db: number): number {
  return Math.pow(10, db / 20);
}

// ── Frequency-response curve (display; independent of the live graph) ─────────

/** Magnitude (dB) of a single RBJ peaking biquad at frequency `f`. */
function peakingMagDb(f: number, f0: number, gainDb: number, q: number, fs: number): number {
  if (gainDb === 0) return 0;
  const A = Math.pow(10, gainDb / 40);
  const w0 = (2 * Math.PI * f0) / fs;
  const cw0 = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * q);
  const b0 = 1 + alpha * A;
  const b1 = -2 * cw0;
  const b2 = 1 - alpha * A;
  const a0 = 1 + alpha / A;
  const a1 = -2 * cw0;
  const a2 = 1 - alpha / A;
  const w = (2 * Math.PI * f) / fs;
  const cw = Math.cos(w);
  const c2w = Math.cos(2 * w);
  const num = b0 * b0 + b1 * b1 + b2 * b2 + 2 * (b0 * b1 + b1 * b2) * cw + 2 * b0 * b2 * c2w;
  const den = a0 * a0 + a1 * a1 + a2 * a2 + 2 * (a0 * a1 + a1 * a2) * cw + 2 * a0 * a2 * c2w;
  return 10 * Math.log10(num / den);
}

/** Combined response (dB) of the bands + preamp at each frequency in `freqs`. */
export function responseCurveDb(
  bands: Band[],
  freqs: number[],
  preampDb = 0,
  fs = 44100,
): number[] {
  return freqs.map(
    (f) => preampDb + bands.reduce((sum, b) => sum + peakingMagDb(f, b.freq, b.gain, b.q, fs), 0),
  );
}

/** Log-spaced frequency samples from 20 Hz to 20 kHz for the curve. */
export function curveFreqs(points = 96): number[] {
  const lo = Math.log10(20);
  const hi = Math.log10(20000);
  return Array.from({ length: points }, (_, i) => Math.pow(10, lo + ((hi - lo) * i) / (points - 1)));
}

/**
 * Owns the Web Audio graph. Built once (lazily) from the shared <audio> element;
 * `createMediaElementSource` can only be called once per element. The filter
 * segment is rebuilt when the band topology (frequencies/Qs) changes; gain-only
 * changes update the existing nodes in place (no clicks while dragging a slider).
 */
export class Equalizer {
  private ctx: AudioContext | null = null;
  private source: MediaElementAudioSourceNode | null = null;
  private preampNode: GainNode | null = null;
  private filters: BiquadFilterNode[] = [];
  private sig = '';
  private built = false;

  get isBuilt(): boolean {
    return this.built;
  }

  attach(el: HTMLAudioElement, state: EqState): void {
    if (this.built) return;
    // Safari exposes the constructor under a legacy prefix the DOM lib doesn't declare.
    const win: { AudioContext?: typeof AudioContext; webkitAudioContext?: typeof AudioContext } =
      window;
    const Ctor = win.AudioContext ?? win.webkitAudioContext;
    if (!Ctor) return;

    el.crossOrigin = 'anonymous';
    this.ctx = new Ctor();
    this.source = this.ctx.createMediaElementSource(el);
    this.preampNode = this.ctx.createGain();
    this.source.connect(this.preampNode);
    this.built = true;
    this.apply(state);
  }

  /** AudioContexts start suspended; resume after a user gesture (play/click). */
  resume(): void {
    if (this.ctx && this.ctx.state === 'suspended') void this.ctx.resume();
  }

  /** Push the full state onto the graph. Disabled === no bands (transparent). */
  apply(state: EqState): void {
    if (!this.built || !this.ctx || !this.preampNode) return;
    const bands = state.enabled ? activeBands(state) : [];
    this.preampNode.gain.value = state.enabled ? dbToGain(clampDb(state.preamp)) : 1;

    const sig = bands.map((b) => `${b.freq}:${b.q}`).join(',');
    if (sig === this.sig) {
      // Same topology — just update gains (smooth, no reconnection clicks).
      bands.forEach((b, i) => {
        if (this.filters[i]) this.filters[i].gain.value = b.gain;
      });
      return;
    }

    // Topology changed — rebuild the filter segment: preamp -> f0 -> ... -> dest.
    this.preampNode.disconnect();
    this.filters.forEach((f) => f.disconnect());
    this.filters = bands.map((b) => {
      const filter = this.ctx!.createBiquadFilter();
      filter.type = 'peaking';
      filter.frequency.value = b.freq;
      filter.Q.value = b.q;
      filter.gain.value = b.gain;
      return filter;
    });
    let node: AudioNode = this.preampNode;
    for (const filter of this.filters) {
      node.connect(filter);
      node = filter;
    }
    node.connect(this.ctx.destination);
    this.sig = sig;
  }
}
