<script lang="ts">
  import { ditherFadeMask, pixelCover } from './pixel-art';
  import { theme } from './theme.svelte';

  // A pixel mosaic of the artwork's own colors radiating from behind it,
  // dissolving outward through a dither mask (the Stencil planet glow, in the
  // cover's palette). Drawing a cross-origin image is allowed even though its
  // pixels can't be read back, so any artwork works without CORS.
  let { src = null, seed }: { src?: string | null; seed: string } = $props();

  const MOSAIC = 20;
  const mask = `url("${ditherFadeMask()}")`;
  let canvas: HTMLCanvasElement;

  $effect(() => {
    const sprite = pixelCover(seed, theme.current);
    const image = new Image();
    let cancelled = false;
    image.onload = () => {
      const context = canvas?.getContext('2d');
      if (cancelled || !context) return;
      context.imageSmoothingQuality = 'high';
      context.clearRect(0, 0, MOSAIC, MOSAIC);
      context.drawImage(image, 0, 0, MOSAIC, MOSAIC);
    };
    image.onerror = () => {
      if (!cancelled && image.src !== sprite) image.src = sprite;
    };
    image.src = src ?? sprite;
    return () => { cancelled = true; };
  });
</script>

<canvas bind:this={canvas} class="art-glow" width={MOSAIC} height={MOSAIC} style:--glow-mask={mask} aria-hidden="true"></canvas>

<style>
  .art-glow {
    position: absolute;
    left: 50%;
    top: var(--glow-y, 50%);
    width: var(--glow-size, 150%);
    aspect-ratio: 1;
    translate: -50% -50%;
    z-index: 0;
    opacity: 0.75;
    image-rendering: pixelated;
    pointer-events: none;
    -webkit-mask: var(--glow-mask) center / 100% 100% no-repeat;
    mask: var(--glow-mask) center / 100% 100% no-repeat;
  }
  /* On paper dissolving cells read as a transparency checkerboard, so by day the
     same downsampled cover becomes a faint, soft wash of its colors instead. */
  :global([data-theme='light']) .art-glow {
    opacity: 0.4;
    image-rendering: auto;
    filter: blur(36px) saturate(1.5);
    -webkit-mask: radial-gradient(closest-side, #000 45%, transparent);
    mask: radial-gradient(closest-side, #000 45%, transparent);
  }
  @media (prefers-reduced-transparency: reduce), (forced-colors: active) {
    .art-glow { display: none; }
  }
</style>
