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
  .quality-badge {
    display: inline-flex;
    align-items: baseline;
    gap: 0.3rem;
    padding: 0.1rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.4;
    white-space: nowrap;
  }

  .quality-badge.lossless {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
  }

  .fmt {
    font-weight: 600;
    letter-spacing: 0.03em;
  }

  .rate {
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    font-variant-numeric: tabular-nums;
  }

  .unit {
    margin-left: 0.15rem;
    opacity: 0.7;
  }
</style>
