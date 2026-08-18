<script lang="ts">
  import { onMount } from 'svelte';
  import { flip } from 'svelte/animate';
  import { getMissingTracks, resyncTrack, type MissingTrackDto } from './api';
  import StatefulButton from './StatefulButton.svelte';

  let tracks = $state<MissingTrackDto[]>([]);
  let loading = $state(true);
  let error: string | null = $state(null);

  const reduce =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  async function load() {
    loading = true;
    error = null;
    try {
      tracks = await getMissingTracks();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function meta(t: MissingTrackDto): string {
    return [t.artists.join(', '), t.album].filter(Boolean).join(' · ');
  }
</script>

<section class="mf">
  <header class="mf-head">
    <div class="mf-heading">
      <h2>Missing files</h2>
      <p class="mf-desc">
        Library tracks whose audio file is gone from disk. Re-sync re-downloads it from the
        original source and re-files it in place, keeping the track's identity.
      </p>
    </div>
    <button class="mf-refresh" onclick={load} disabled={loading}>
      {#if loading}<span class="mini-spin" aria-hidden="true"></span>{/if}Refresh
    </button>
  </header>

  {#if loading && tracks.length === 0}
    <p class="mf-status"><span class="mini-spin" aria-hidden="true"></span>Checking library…</p>
  {:else if error}
    <p class="mf-status is-err">{error}</p>
  {:else if tracks.length === 0}
    <p class="mf-empty">Every library file is present. Nothing to re-sync.</p>
  {:else}
    <p class="mf-count">{tracks.length} missing file{tracks.length > 1 ? 's' : ''}</p>
    <ul class="mf-list">
      {#each tracks as t (t.id)}
        <li class="mf-row" animate:flip={{ duration: reduce ? 0 : 220 }}>
          <div class="mf-main">
            <span class="mf-title">{t.title}</span>
            {#if meta(t)}<span class="mf-meta">{meta(t)}</span>{/if}
            {#if t.file_path}<span class="mf-path">{t.file_path}</span>{/if}
          </div>
          {#if t.id != null && t.source_url}
            <StatefulButton
              variant="primary"
              size="sm"
              label="Re-sync"
              action={async () => {
                await resyncTrack(t.id!);
              }}
              onSuccess={() => (tracks = tracks.filter((x) => x.id !== t.id))}
            />
          {:else}
            <span class="mf-nosrc" title="No source URL to re-download from">No source</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .mf {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .mf-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }
  .mf-heading h2 {
    margin: 0 0 0.25rem;
    font-size: 1.05rem;
  }
  .mf-desc {
    margin: 0;
    font-size: 0.82rem;
    color: var(--muted);
    line-height: 1.5;
    max-width: 72ch;
  }
  .mf-refresh {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.8rem;
    font: inherit;
    font-size: 0.82rem;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
  }
  .mf-refresh:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .mf-refresh:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .mf-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .mf-status.is-err {
    color: var(--error);
  }
  .mf-empty {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted-2);
  }
  .mf-count {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
  }

  .mf-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .mf-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.75rem 0.25rem;
    border-bottom: 1px solid var(--border-soft);
  }
  .mf-main {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .mf-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-bright);
  }
  .mf-meta {
    font-size: 0.82rem;
    color: var(--muted);
  }
  .mf-path {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--muted-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mf-nosrc {
    flex-shrink: 0;
    font-size: 0.78rem;
    color: var(--muted-2);
  }

  .mini-spin {
    width: 12px;
    height: 12px;
    border: 2px solid color-mix(in srgb, currentColor 30%, transparent);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: mf-spin 0.7s linear infinite;
  }
  @keyframes mf-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
