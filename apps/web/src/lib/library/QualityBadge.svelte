<script lang="ts">
  import type { TrackQualityDto } from '../types';

  let { quality }: { quality?: TrackQualityDto | null } = $props();

  // Full detail for the tooltip, e.g. "FLAC, 1002 kbps, lossless".
  const detail = $derived(
    quality
      ? [
          quality.format,
          quality.bitrate_kbps != null ? `${quality.bitrate_kbps} kbps` : null,
          quality.lossless ? 'lossless' : 'lossy',
        ]
          .filter(Boolean)
          .join(', ')
      : ''
  );
</script>

{#if quality}
  <span class="quality-badge" class:lossless={quality.lossless} title={detail}>
    <span class="fmt">{quality.format}</span>
    {#if quality.bitrate_kbps != null}<span class="rate">{quality.bitrate_kbps}<span class="unit">kbps</span></span>{/if}
  </span>
{/if}

<style>
  /* Mono data tag. Tiers: lossless = accent tint, lossy = neutral. */
  .quality-badge {
    display: inline-flex;
    align-items: baseline;
    gap: 5px;
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--surface-2);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.06em;
    line-height: 1.5;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .quality-badge.lossless {
    background: var(--accent-muted);
    color: var(--accent);
  }

  .fmt { font-weight: 600; }
  .unit { margin-left: 2px; text-transform: none; letter-spacing: 0; }
</style>
