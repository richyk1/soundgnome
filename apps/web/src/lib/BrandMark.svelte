<script lang="ts">
  import { MARK_HEIGHT, MARK_WIDTH, markPaths } from './brand-mark.js';

  // `scale` is CSS pixels per mark pixel; whole numbers keep every pixel crisp.
  let { scale = 2 }: { scale?: number } = $props();
  const paths = markPaths();
</script>

<svg
  class="brand-mark"
  viewBox="0 0 {MARK_WIDTH} {MARK_HEIGHT}"
  width={MARK_WIDTH * scale}
  height={MARK_HEIGHT * scale}
  shape-rendering="crispEdges"
  aria-hidden="true"
  focusable="false"
>
  <path class="hat" d={paths.hat} />
  <path class="beard" d={paths.beard} />
  <path class="nose" d={paths.nose} />
</svg>

<style>
  .brand-mark { display: block; flex: none; }
  .hat { fill: var(--accent); }
  .beard { fill: var(--text-bright); }
  .nose { fill: var(--brand-warm); }
  @media (forced-colors: active) {
    .hat, .beard, .nose { fill: CanvasText; }
  }
</style>
