<script lang="ts">
  import { getContext } from 'svelte';
  import type { LibraryTrackDto } from '../types';
  import { lib, LIBRARY_PLAYER, type LibraryPlayer } from './store.svelte';
  import QualityBadge from './QualityBadge.svelte';

  let { tracks, showAlbumCol = true, showDelete = false }: {
    tracks: LibraryTrackDto[];
    showAlbumCol?: boolean;
    showDelete?: boolean;
  } = $props();

  const player = getContext<LibraryPlayer | undefined>(LIBRARY_PLAYER);
</script>

<div class="table-wrap">
  <table>
    <colgroup>
      <col class="c-idx" />
      <col class="c-title" />
      <col class="c-artist" />
      {#if showAlbumCol}<col class="c-album" />{/if}
      <col class="c-genre" />
      <col class="c-dur" />
      <col class="c-quality" />
      <col class="c-actions" />
    </colgroup>
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
            <span class="row-title">{t.title}{#if t.needs_validation}<span class="badge-warn" title="Awaiting validation">!</span>{/if}</span>
            <span class="row-sub">{t.artists.map(a => a.name).join(', ') || '\u2014'}</span>
          </td>
          <td class="muted col-artist">{t.artists.map(a => a.name).join(', ') || '\u2014'}</td>
          {#if showAlbumCol}<td class="muted col-album">{t.album?.title ?? '\u2014'}</td>{/if}
          <td class="muted col-genre">{t.genre ?? '\u2014'}</td>
          <td class="muted mono col-dur">{lib.fmtDuration(t.duration)}</td>
          <td class="col-quality"><QualityBadge quality={t.quality} /></td>
          <td class="actions">
            <button
              class="btn-rate"
              class:active-like={t.rating === 'liked'}
              title="Like"
              aria-label="Like"
              onclick={(e) => { e.stopPropagation(); lib.setRating(t, t.rating === 'liked' ? null : 'liked'); }}
            ><i class="lni lni-thumbs-up-1"></i></button>
            <button
              class="btn-rate"
              class:active-dislike={t.rating === 'disliked'}
              title="Dislike"
              aria-label="Dislike"
              onclick={(e) => { e.stopPropagation(); lib.setRating(t, t.rating === 'disliked' ? null : 'disliked'); }}
            ><i class="lni lni-thumbs-down-1"></i></button>
            <button class="btn-edit" onclick={(e) => { e.stopPropagation(); lib.startEditTrack(t); }}>Edit</button>
            {#if showDelete}
              <button class="btn-delete" onclick={(e) => { e.stopPropagation(); lib.handleDeleteTrack(t.id); }}>Delete</button>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .table-wrap tbody tr.playable { cursor: pointer; }
  .table-wrap tbody tr.playing {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .table-wrap tbody tr.playing td.idx { color: var(--accent); }
  .table-wrap tbody tr.playing .title-cell {
    color: var(--accent);
    font-weight: 600;
  }

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

  /* Fixed layout so the table always fits its panel and the rightmost Actions
     column (like / dislike / edit) never overflows off-screen on narrower
     desktops. Long text truncates instead of widening the table. */
  table { width: 100%; table-layout: fixed; }
  .c-idx { width: 3em; }
  .c-artist { width: 11%; }
  .c-album { width: 11%; }
  .c-genre { width: 6%; }
  .c-dur { width: 4.5em; }
  .c-quality { width: 6.5em; }
  .c-actions { width: 11em; }
  .title-cell,
  td.col-artist,
  td.col-album,
  td.col-genre {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  td.actions { white-space: nowrap; text-align: right; }
  /* Bigger, clearer like/dislike than the default row icon. */
  td.actions .btn-rate { font-size: 18px; }
  td.actions .btn-rate .lni { display: block; }

  /* ── Mobile: collapse the table into a tap-to-play list ────────────────── */
  .row-sub { display: none; }
  @media (max-width: 860px) {
    .table-wrap { overflow-x: hidden; }
    .table-wrap thead { display: none; }
    .table-wrap table,
    .table-wrap tbody { display: block; width: 100%; }
    .table-wrap tbody tr {
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 9px 2px;
      border-bottom: 1px solid var(--border);
    }
    .table-wrap td { padding: 0; border: none; }
    .col-artist, .col-album, .col-genre, .col-dur, .col-quality, .actions { display: none; }
    .table-wrap td.idx { flex: 0 0 auto; width: 2.4em; text-align: center; }
    .table-wrap td.title-cell { flex: 1; min-width: 0; display: flex; flex-direction: column; align-items: stretch; gap: 2px; text-align: left; }
    .row-title { display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text-bright); }
    .row-sub {
      display: block;
      font-size: 0.8rem;
      color: var(--muted);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  }
</style>
