<script lang="ts">
  import { pixelCover } from './pixel-art';
  import { theme } from './theme.svelte';

  interface Props {
    /** Real artwork; the generated sprite stands in when it is missing or fails to load. */
    src?: string | null;
    /** Stable identity for the sprite, e.g. `album:42`, `track:7`, `artist:3`. */
    seed: string;
    alt?: string;
    loading?: 'lazy' | 'eager';
  }

  let { src = null, seed, alt = '', loading = 'lazy' }: Props = $props();
  let failedSrc = $state<string | null>(null);
  const art = $derived(src && src !== failedSrc ? src : null);
</script>

{#if art}
  <img class="cover-art" src={art} {alt} {loading} decoding="async" onerror={() => (failedSrc = art)} />
{:else}
  <img class="cover-art pixel" src={pixelCover(seed, theme.current)} {alt} decoding="async" />
{/if}

<style>
  .cover-art {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .pixel { image-rendering: pixelated; }
</style>
