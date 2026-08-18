// In-app graphic equalizer built on the Web Audio API. This is the same
// technique browser EQ extensions (e.g. Ears) use: the audio element is routed
// through a chain of peaking BiquadFilters, one per band, plus a preamp gain.
//
// It is opt-in. Until the user enables it, no AudioContext is created and the
// <audio> element plays untouched, so default playback (including cross-origin
// SoundCloud streams) is unaffected. Enabling it routes the element through the
// graph, which requires the audio to be same-origin or CORS-enabled — library
// files (`/api/tracks/<id>/audio`) are same-origin, so they always work.

/** Band center frequencies (Hz). A standard 10-band graphic EQ layout. */
export const EQ_BANDS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;
export const EQ_MIN_DB = -12;
export const EQ_MAX_DB = 12;

export interface EqState {
  enabled: boolean;
  /** Output makeup gain in dB, applied before the bands. */
  preamp: number;
  /** Per-band gain in dB; length === EQ_BANDS.length. */
  gains: number[];
}

/** Named starting points. `Flat` is the identity (transparent). */
export const EQ_PRESETS: Record<string, number[]> = {
  Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  'Bass boost': [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
  'Treble boost': [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
  Vocal: [-2, -1, 0, 2, 4, 4, 3, 1, 0, -1],
  Loudness: [5, 4, 2, 0, -1, 0, 1, 3, 4, 5],
  Podcast: [-4, -3, 0, 2, 3, 3, 2, 1, -1, -3],
};

const STORAGE_KEY = 'soundgnome:eq:v1';

export function flatGains(): number[] {
  return EQ_BANDS.map(() => 0);
}

export function defaultEqState(): EqState {
  return { enabled: false, preamp: 0, gains: flatGains() };
}

export function loadEqState(): EqState {
  if (typeof localStorage === 'undefined') return defaultEqState();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultEqState();
    const parsed = JSON.parse(raw) as Partial<EqState>;
    const gains = Array.isArray(parsed.gains) ? parsed.gains : [];
    return {
      enabled: !!parsed.enabled,
      preamp: clampDb(Number(parsed.preamp) || 0),
      // Reconcile length in case the band layout ever changes.
      gains: EQ_BANDS.map((_, i) => clampDb(Number(gains[i]) || 0)),
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
    /* storage full / unavailable — non-fatal */
  }
}

export function clampDb(db: number): number {
  if (!Number.isFinite(db)) return 0;
  return Math.max(EQ_MIN_DB, Math.min(EQ_MAX_DB, db));
}

function dbToGain(db: number): number {
  return Math.pow(10, db / 20);
}

/**
 * Owns the Web Audio graph. Built once (lazily) from the shared <audio> element;
 * `createMediaElementSource` can only be called once per element, and once built
 * the element's audio permanently flows through the graph. When disabled, the
 * bands are set flat so the chain is transparent.
 */
export class Equalizer {
  private ctx: AudioContext | null = null;
  private source: MediaElementAudioSourceNode | null = null;
  private preampNode: GainNode | null = null;
  private filters: BiquadFilterNode[] = [];
  private built = false;

  get isBuilt(): boolean {
    return this.built;
  }

  /**
   * Route `el` through the filter chain. Idempotent. Sets `crossOrigin` so the
   * graph can read the audio (required for CORS-enabled cross-origin sources;
   * harmless for same-origin library files).
   */
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
    this.filters = EQ_BANDS.map((freq) => {
      const filter = this.ctx!.createBiquadFilter();
      filter.type = 'peaking';
      filter.frequency.value = freq;
      filter.Q.value = 1.0;
      filter.gain.value = 0;
      return filter;
    });

    // source -> preamp -> band0 -> band1 -> ... -> destination
    let node: AudioNode = this.source;
    node.connect(this.preampNode);
    node = this.preampNode;
    for (const filter of this.filters) {
      node.connect(filter);
      node = filter;
    }
    node.connect(this.ctx.destination);

    this.built = true;
    this.apply(state);
  }

  /** AudioContexts start suspended; resume after a user gesture (play/click). */
  resume(): void {
    if (this.ctx && this.ctx.state === 'suspended') {
      void this.ctx.resume();
    }
  }

  /** Push the full state onto the graph. Disabled === flat (transparent). */
  apply(state: EqState): void {
    if (!this.built) return;
    const active = state.enabled;
    if (this.preampNode) {
      this.preampNode.gain.value = active ? dbToGain(state.preamp) : 1;
    }
    this.filters.forEach((filter, i) => {
      filter.gain.value = active ? clampDb(state.gains[i] ?? 0) : 0;
    });
  }

  setBand(index: number, db: number): void {
    const filter = this.filters[index];
    if (filter) filter.gain.value = clampDb(db);
  }

  setPreamp(db: number): void {
    if (this.preampNode) this.preampNode.gain.value = dbToGain(clampDb(db));
  }

  /** Set every band flat (used when disabling without discarding saved gains). */
  flatten(): void {
    if (this.preampNode) this.preampNode.gain.value = 1;
    this.filters.forEach((f) => (f.gain.value = 0));
  }

  /** Analyser tapped off the output, for verification/metering. */
  createAnalyser(): AnalyserNode | null {
    if (!this.ctx || !this.built) return null;
    const analyser = this.ctx.createAnalyser();
    // Tap the last node's signal without disturbing the chain to destination.
    (this.filters[this.filters.length - 1] ?? this.preampNode)?.connect(analyser);
    return analyser;
  }
}
