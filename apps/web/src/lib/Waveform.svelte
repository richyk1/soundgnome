<script module lang="ts">
  import { getCachedPeaks, putCachedPeaks } from './peaksCache';

  /**
   * Precomputed peaks payload as served by SoundCloud's waveform CDN, for
   * example https://wave.sndcdn.com/NoTs1dxKHSIR_m.json
   */
  interface PeaksPayload {
    width: number;
    height: number;
    samples: number[];
  }

  // Peaks are immutable per url, so cache them for the whole session. A cached
  // `null` marks a url we already failed to load, so it is never refetched.
  const peaksCache = new Map<string, number[] | null>();

  async function loadPeaks(url: string): Promise<number[] | null> {
    if (peaksCache.has(url)) return peaksCache.get(url) ?? null;
    try {
      const res = await fetch(url);
      if (!res.ok) throw new Error(`waveform request failed: ${res.status}`);
      const data: PeaksPayload = await res.json();
      const samples = data.samples;
      if (!Array.isArray(samples) || samples.length === 0) throw new Error('empty waveform');
      const scale = data.height > 0 ? data.height : Math.max(...samples) || 1;
      const normalised = samples.map((s) => Math.min(1, Math.max(0, s / scale)));
      peaksCache.set(url, normalised);
      return normalised;
    } catch {
      peaksCache.set(url, null);
      return null;
    }
  }

  // Waveforms computed from the audio itself, for local tracks that have no
  // precomputed peaks. Cached per url for the session (null = decode failed).
  const computedCache = new Map<string, number[] | null>();

  async function computePeaks(url: string): Promise<number[] | null> {
    if (computedCache.has(url)) return computedCache.get(url) ?? null;
    try {
      const res = await fetch(url);
      if (!res.ok) throw new Error(`audio request failed: ${res.status}`);
      const bytes = await res.arrayBuffer();
      const Ctor =
        window.OfflineAudioContext ||
        (window as unknown as { webkitOfflineAudioContext: typeof OfflineAudioContext })
          .webkitOfflineAudioContext;
      // Decode at a low sample rate: a ~900-bar overview needs nothing near CD
      // quality, and decoding to 8 kHz is roughly 3x faster than full rate.
      const ac = new Ctor(1, 1, 8000);
      const buf = await ac.decodeAudioData(bytes);
      const ch = buf.getChannelData(0);
      const N = 900;
      const block = Math.max(1, Math.floor(ch.length / N));
      const out: number[] = [];
      let max = 0;
      for (let i = 0; i < N; i += 1) {
        const s = i * block;
        const e = Math.min(ch.length, s + block);
        let peak = 0;
        for (let j = s; j < e; j += 1) {
          const v = Math.abs(ch[j]);
          if (v > peak) peak = v;
        }
        out.push(peak);
        if (peak > max) max = peak;
      }
      const norm = max > 0 ? out.map((v) => Math.min(1, v / max)) : out;
      computedCache.set(url, norm);
      return norm;
    } catch {
      computedCache.set(url, null);
      return null;
    }
  }
</script>

<script lang="ts">
  let {
    waveformUrl,
    srcUrl,
    currentTime,
    duration,
    onSeek,
    available = $bindable(false),
  }: {
    waveformUrl: string | null | undefined;
    srcUrl?: string | null;
    currentTime: number;
    duration: number;
    /** Ask the player to seek to `secs`. */
    onSeek: (secs: number) => void;
    /** Whether usable peaks are loaded, so the caller can fall back. Bindable. */
    available?: boolean;
  } = $props();

  let canvas: HTMLCanvasElement | null = $state(null);
  let peaks: number[] | null = $state(null);
  let loading = $state(false);
  let sizeTick = $state(0);
  let dragging = $state(false);

  let progress = $derived(
    duration > 0 ? Math.min(1, Math.max(0, currentTime / duration)) : 0,
  );

  // Resolve peaks for the current track, cheapest source first:
  //   1. in-memory session cache
  //   2. IndexedDB (survives reloads)
  //   3. server-precomputed peaks (waveformUrl) - a tiny JSON, no audio decode
  //   4. fall back to decoding the audio in the browser (computePeaks)
  // Whatever resolves is written back to both caches. `loading` keeps the slot
  // showing a skeleton instead of flashing the plain range line.
  $effect(() => {
    const url = waveformUrl;
    const src = srcUrl;
    if (!url && !src) {
      peaks = null;
      loading = false;
      return;
    }
    // Key by the precomputed-peaks url when present, else the audio url; whatever
    // we end up with (fetched or computed) is cached under this one key.
    const key = url ?? src ?? '';
    const memCache = url ? peaksCache : computedCache;
    if (memCache.has(key)) {
      peaks = memCache.get(key) ?? null;
      loading = false;
      return;
    }
    let cancelled = false;
    peaks = null;
    loading = true;
    (async () => {
      let resolved = await getCachedPeaks(key);
      if (cancelled) return;
      if (!resolved && url) resolved = await loadPeaks(url);
      if (cancelled) return;
      if (!resolved && src) resolved = await computePeaks(src);
      if (cancelled) return;
      memCache.set(key, resolved);
      if (resolved) putCachedPeaks(key, resolved);
      peaks = resolved;
      loading = false;
    })();
    return () => {
      cancelled = true;
    };
  });

  // Show the waveform slot while peaks load (skeleton) and once ready; only fall
  // back to the plain range when there is no source or the decode failed.
  $effect(() => {
    available = loading || peaks != null;
  });

  // Keep the canvas in sync with its rendered size.
  $effect(() => {
    const cvs = canvas;
    if (!cvs) return;
    const ro = new ResizeObserver(() => (sizeTick += 1));
    ro.observe(cvs);
    return () => ro.disconnect();
  });

  // Redraw on peaks, progress or resize changes.
  $effect(() => {
    sizeTick;
    void loading;
    draw();
  });

  function draw() {
    const cvs = canvas;
    if (!cvs) return;
    const ctx = cvs.getContext('2d');
    if (!ctx) return;

    const cssW = cvs.clientWidth;
    const cssH = cvs.clientHeight;
    if (cssW === 0 || cssH === 0) return;

    const dpr = window.devicePixelRatio || 1;
    if (cvs.width !== Math.round(cssW * dpr) || cvs.height !== Math.round(cssH * dpr)) {
      cvs.width = Math.round(cssW * dpr);
      cvs.height = Math.round(cssH * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssW, cssH);

    const styles = getComputedStyle(cvs);
    const playedColor = styles.getPropertyValue('--accent').trim() || '#7c6ef5';
    const restColor = styles.getPropertyValue('--muted-2').trim() || '#6e6e78';

    // Chunky, rounded "voice-memo" bars.
    const barWidth = 4;
    const gap = 3;
    const step = barWidth + gap;
    const radius = barWidth / 2;
    const bars = Math.max(1, Math.floor((cssW + gap) / step));
    const mid = cssH / 2;
    const bar = (x: number, y: number, h: number) => {
      if (ctx.roundRect) {
        ctx.beginPath();
        ctx.roundRect(x, y, barWidth, h, radius);
        ctx.fill();
      } else {
        ctx.fillRect(x, y, barWidth, h);
      }
    };

    const p = peaks;
    if (!p || p.length === 0) {
      // Skeleton while peaks are still computing: flat low bars so the scrubber
      // already reads as a waveform and the real peaks fill in subtly instead of
      // jumping from a plain line.
      ctx.fillStyle = restColor;
      const h = Math.max(barWidth, cssH * 0.22);
      for (let i = 0; i < bars; i += 1) bar(i * step, mid - h / 2, h);
      return;
    }

    const played = progress;
    const minH = barWidth; // quiet passages still show a dot rather than vanish
    for (let i = 0; i < bars; i += 1) {
      const start = Math.floor((i / bars) * p.length);
      const end = Math.max(start + 1, Math.floor(((i + 1) / bars) * p.length));
      let sum = 0;
      for (let j = start; j < end && j < p.length; j += 1) sum += p[j];
      const value = sum / (end - start);
      const height = Math.max(minH, value * cssH);
      const centre = (i + 0.5) / bars;
      ctx.fillStyle = centre <= played ? playedColor : restColor;
      bar(i * step, mid - height / 2, height);
    }
  }

  function seekToClientX(clientX: number) {
    const cvs = canvas;
    if (!cvs || duration <= 0) return;
    const rect = cvs.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    onSeek(ratio * duration);
  }

  function onPointerDown(e: PointerEvent) {
    dragging = true;
    canvas?.setPointerCapture(e.pointerId);
    seekToClientX(e.clientX);
  }

  function onPointerMove(e: PointerEvent) {
    if (dragging) seekToClientX(e.clientX);
  }

  function onPointerUp(e: PointerEvent) {
    dragging = false;
    canvas?.releasePointerCapture(e.pointerId);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (duration <= 0) return;
    if (e.key === 'ArrowRight') {
      onSeek(Math.min(duration, currentTime + 5));
      e.preventDefault();
    } else if (e.key === 'ArrowLeft') {
      onSeek(Math.max(0, currentTime - 5));
      e.preventDefault();
    }
  }
</script>

{#if loading || peaks}
  <canvas
    bind:this={canvas}
    class="waveform"
    class:dragging
    role="slider"
    tabindex="0"
    aria-label="Seek"
    aria-valuemin={0}
    aria-valuemax={Math.round(duration)}
    aria-valuenow={Math.round(currentTime)}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    onkeydown={onKeyDown}
  ></canvas>
{/if}

<style>
  .waveform {
    display: block;
    width: 100%;
    height: 42px;
    cursor: pointer;
    touch-action: none;
  }

  .waveform.dragging {
    cursor: grabbing;
  }

  .waveform:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 3px;
  }
</style>
