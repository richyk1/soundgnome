<script module lang="ts">
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
</script>

<script lang="ts">
  let {
    waveformUrl,
    currentTime,
    duration,
    onSeek,
    available = $bindable(false),
  }: {
    waveformUrl: string | null | undefined;
    currentTime: number;
    duration: number;
    /** Ask the player to seek to `secs`. */
    onSeek: (secs: number) => void;
    /** Whether usable peaks are loaded, so the caller can fall back. Bindable. */
    available?: boolean;
  } = $props();

  let canvas: HTMLCanvasElement | null = $state(null);
  let peaks: number[] | null = $state(null);
  let sizeTick = $state(0);
  let dragging = $state(false);

  let progress = $derived(
    duration > 0 ? Math.min(1, Math.max(0, currentTime / duration)) : 0,
  );

  // Fetch (or reuse) peaks whenever the url changes. Depends only on the url,
  // so it never refetches when the player re-renders for time updates.
  $effect(() => {
    const url = waveformUrl;
    if (!url) {
      peaks = null;
      return;
    }
    if (peaksCache.has(url)) {
      peaks = peaksCache.get(url) ?? null;
      return;
    }
    let cancelled = false;
    peaks = null;
    loadPeaks(url).then((p) => {
      if (!cancelled) peaks = p;
    });
    return () => {
      cancelled = true;
    };
  });

  // Report availability so the caller can fall back to a plain range.
  $effect(() => {
    available = peaks != null;
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

    const p = peaks;
    if (!p || p.length === 0) return;

    const styles = getComputedStyle(cvs);
    const playedColor = styles.getPropertyValue('--accent').trim() || '#7c6ef5';
    const restColor = styles.getPropertyValue('--muted').trim() || '#7b7f9e';

    const barWidth = 2;
    const gap = 1;
    const step = barWidth + gap;
    const bars = Math.max(1, Math.floor((cssW + gap) / step));
    const mid = cssH / 2;
    const played = progress;

    for (let i = 0; i < bars; i += 1) {
      const start = Math.floor((i / bars) * p.length);
      const end = Math.max(start + 1, Math.floor(((i + 1) / bars) * p.length));
      let sum = 0;
      for (let j = start; j < end && j < p.length; j += 1) sum += p[j];
      const value = sum / (end - start);
      const height = Math.max(2, value * cssH);
      const centre = (i + 0.5) / bars;
      ctx.fillStyle = centre <= played ? playedColor : restColor;
      ctx.fillRect(i * step, mid - height / 2, barWidth, height);
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

{#if peaks}
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
