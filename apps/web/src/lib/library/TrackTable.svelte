<script lang="ts">
  import { getContext } from 'svelte';
  import type { LibraryTrackDto } from '../types';
  import { lib, LIBRARY_PLAYER, type LibraryPlayer } from './store.svelte';
  import QualityBadge from './QualityBadge.svelte';

  let { tracks, showAlbumCol = true }: {
    tracks: LibraryTrackDto[];
    showAlbumCol?: boolean;
  } = $props();

  const player = getContext<LibraryPlayer | undefined>(LIBRARY_PLAYER);
</script>

<div class="table-wrap">
  <table>
    <thead>
      <tr>
        <th>#</th><th>Title</th><th>Artists</th>
        {#if showAlbumCol}<th>Album</th>{/if}
        <th>Genre</th><th>Duration</th><th>Quality</th><th class="col-actions">Actions</th>
      </tr>
    </thead>
    <tbody>
      {#each tracks as t (t.id)}
        <tr
          class:needs-validation={t.needs_validation}
          class:playing={player?.isCurrent(t.id)}
          onmouseenter={() => (lib.hoveredItem = { type: 'track', id: t.id })}
          onmouseleave={() => (lib.hoveredItem = null)}
        >
          <td class="muted">{t.id}</td>
          <td class="title-cell">
            {t.title}
            {#if t.needs_validation}<span class="badge-warn" title="Awaiting validation">!</span>{/if}
          </td>
          <td class="muted">{t.artists.map(a => a.name).join(', ') || '\u2014'}</td>
          {#if showAlbumCol}<td class="muted">{t.album?.title ?? '\u2014'}</td>{/if}
          <td class="muted">{t.genre ?? '\u2014'}</td>
          <td class="muted mono">{lib.fmtDuration(t.duration)}</td>
          <td><QualityBadge quality={t.quality} /></td>
          <td class="actions">
            {#if player}
              <!-- The title lives on the wrapper so it still shows when the button is disabled. -->
              <span class="play-slot" title={t.file_path ? null : 'Not downloaded yet'}>
                <button
                  class="btn-play"
                  onclick={() => player?.play(t)}
                  disabled={!t.file_path}
                  aria-label={player.isPlaying(t.id) ? 'Pause' : 'Play'}
                >
                  {#if player.isPlaying(t.id)}
                    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                      <rect x="6" y="5" width="4" height="14" rx="1" />
                      <rect x="14" y="5" width="4" height="14" rx="1" />
                    </svg>
                  {:else}
                    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                      <path d="M8 5l11 7-11 7z" />
                    </svg>
                  {/if}
                </button>
              </span>
            {/if}
            <button class="btn-edit" onclick={() => lib.startEditTrack(t)}>Edit</button>
            <button class="btn-delete" onclick={() => lib.handleDeleteTrack(t.id)}>Delete</button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .table-wrap tbody tr.playing {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .play-slot {
    display: inline-block;
    margin-right: 0.3rem;
  }

  .btn-play {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    vertical-align: middle;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface);
    color: var(--text);
  }

  .btn-play:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .btn-play:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .btn-play svg { width: 12px; height: 12px; }
</style>
