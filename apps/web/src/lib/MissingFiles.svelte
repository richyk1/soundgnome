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
      <h2><i class="pxi pxi-file" aria-hidden="true"></i>Missing files</h2>
      <p class="mf-desc">
        Library tracks whose audio file is gone from disk. Re-sync re-downloads it from the
        original source and re-files it in place, keeping the track's identity.
      </p>
    </div>
    <button class="btn-secondary btn-sm mf-refresh" onclick={load} disabled={loading}>
      <i class="pxi {loading ? 'pxi-loader pxi-spin' : 'pxi-refresh'}" aria-hidden="true"></i>Refresh
    </button>
  </header>

  {#if loading && tracks.length === 0}
    <p class="mf-status" role="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Checking library…</p>
  {:else if error}
    <div class="callout callout-error" role="alert">
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>
      <div class="callout-body"><strong>Couldn't check the library.</strong><span>{error}</span></div>
    </div>
  {:else if tracks.length === 0}
    <div class="empty">
      <i class="pxi pxi-check" aria-hidden="true"></i>
      <p class="empty-title">No missing files</p>
      <p class="empty-hint">Every library file is present. Nothing to re-sync.</p>
    </div>
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
              icon="refresh"
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
    gap: 16px;
  }
  .mf-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  .mf-heading h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 6px;
    font-size: 18px;
    line-height: 1.3;
  }
  .mf-heading h2 .pxi {
    font-size: 16px;
    color: var(--muted);
  }
  .mf-desc {
    margin: 0;
    max-width: 72ch;
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }
  .mf-refresh {
    flex-shrink: 0;
  }

  .mf-status {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 14px;
    color: var(--muted);
  }
  .mf-status .pxi {
    font-size: 16px;
    color: var(--live);
  }
  .mf-count {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }

  .mf-list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--border);
  }
  .mf-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 4px;
    border-bottom: 1px solid var(--border-soft);
    transition: background-color var(--motion-fast) var(--ease-out);
  }
  .mf-row:hover {
    background: var(--surface);
  }
  .mf-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .mf-title {
    font-size: 14px;
    font-weight: 500;
    line-height: 1.35;
    color: var(--text-bright);
  }
  .mf-meta {
    font-size: 13px;
    line-height: 1.35;
    color: var(--muted);
  }
  .mf-path {
    margin-top: 2px;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--muted-2);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mf-nosrc {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted-2);
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .mf-head, .mf-row { flex-wrap: wrap; }
    .mf-main { flex: 1 1 100%; }
    .mf-title, .mf-meta { overflow-wrap: anywhere; }
  }
</style>
