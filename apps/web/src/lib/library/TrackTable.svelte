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
        <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
        <tr
          class:needs-validation={t.needs_validation}
          class:playing={player?.isCurrent(t.id)}
          class:playable={!!(player && t.file_path)}
          onmouseenter={() => (lib.hoveredItem = { type: 'track', id: t.id })}
          onmouseleave={() => (lib.hoveredItem = null)}
          onclick={() => { if (player && t.file_path) player.play(t, tracks); }}
        >
          <td class="idx muted">
            <span class="idx-num">{t.id}</span>
            {#if player && t.file_path}
              <button
                class="idx-play"
                onclick={(e) => { e.stopPropagation(); player.play(t, tracks); }}
                aria-label={player.isPlaying(t.id) ? 'Pause' : 'Play'}
              ><i class="lni {player.isPlaying(t.id) ? 'lni-pause' : 'lni-play'}"></i></button>
            {/if}
          </td>
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
            <button class="btn-edit" onclick={(e) => { e.stopPropagation(); lib.startEditTrack(t); }}>Edit</button>
            <button class="btn-delete" onclick={(e) => { e.stopPropagation(); lib.handleDeleteTrack(t.id); }}>Delete</button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .table-wrap tbody tr.playable { cursor: pointer; }
  .table-wrap tbody tr.playing {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .table-wrap tbody tr.playing td.idx { color: var(--accent); }

  /* The # cell doubles as the play affordance: the number morphs into a
     play/pause icon on row hover (and while this row is the current track),
     so there is no separate per-row play CTA competing for attention. */
  td.idx { position: relative; }
  .idx-play {
    position: absolute;
    inset: 0;
    display: none;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-bright);
    cursor: pointer;
    padding: 0;
  }
  .idx-play .lni { font-size: 15px; }
  .table-wrap tbody tr.playable:hover .idx-num { visibility: hidden; }
  .table-wrap tbody tr.playable:hover .idx-play { display: flex; }
  .table-wrap tbody tr.playing .idx-num { visibility: hidden; }
  .table-wrap tbody tr.playing .idx-play { display: flex; color: var(--accent); }
</style>
