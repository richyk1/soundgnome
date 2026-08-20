<script lang="ts">
  import { getContext } from 'svelte';
  import type { LibraryTrackDto } from '../types';
  import { lib, LIBRARY_PLAYER, type LibraryPlayer } from './store.svelte';

  let { tracks, showAlbumCol = true, showDelete = false }: {
    tracks: LibraryTrackDto[];
    showAlbumCol?: boolean;
    showDelete?: boolean;
  } = $props();

  const player = getContext<LibraryPlayer | undefined>(LIBRARY_PLAYER);

  function coverUrl(t: LibraryTrackDto): string | null {
    return t.cover && /^(https?:\/\/|\/)/.test(t.cover) ? t.cover : null;
  }
  function qualityLabel(t: LibraryTrackDto): string {
    const q = t.quality;
    if (!q) return '';
    return q.bitrate_kbps ? `${q.format} ${q.bitrate_kbps}` : q.format;
  }
  /** Secondary line: artist, then album (when shown) or genre. */
  function secondary(t: LibraryTrackDto): string {
    const artist = t.artists.map((a) => a.name).join(', ') || '\u2014';
    const extra = showAlbumCol && t.album?.title ? t.album.title : t.genre;
    return extra ? `${artist} · ${extra}` : artist;
  }
</script>

<div class="track-list">
  {#each tracks as t, i (t.id)}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div
      class="trow"
      class:needs-validation={t.needs_validation}
      class:playing={player?.isCurrent(t.id)}
      class:playable={!!(player && t.file_path)}
      onmouseenter={() => (lib.hoveredItem = { type: 'track', id: t.id })}
      onmouseleave={() => (lib.hoveredItem = null)}
      onclick={() => { if (player && t.file_path) player.play(t, tracks); }}
    >
      <span class="trow-idx">{String(i + 1).padStart(2, '0')}</span>

      <div class="trow-art">
        {#if coverUrl(t)}
          <img src={coverUrl(t)} alt="" loading="lazy" />
        {:else}
          <div class="trow-ph"><i class="lni lni-music-note"></i></div>
        {/if}
        {#if player && t.file_path}
          <span class="trow-play" aria-hidden="true"><i class="lni {player.isPlaying(t.id) ? 'lni-pause' : 'lni-play'}"></i></span>
        {/if}
      </div>

      <div class="trow-main">
        <span class="trow-title">
          <span class="trow-title-text">{t.title}</span>
          {#if t.needs_validation}<span class="trow-dot" title="Awaiting validation"></span>{/if}
        </span>
        <span class="trow-sub">{secondary(t)}</span>
      </div>

      <span class="trow-fmt">{qualityLabel(t)}</span>
      <span class="trow-dur">{lib.fmtDuration(t.duration)}</span>

      <div class="trow-actions">
        <button class="btn-edit trow-hover" onclick={(e) => { e.stopPropagation(); lib.startEditTrack(t); }}>Edit</button>
        {#if showDelete}
          <button class="btn-delete trow-hover" onclick={(e) => { e.stopPropagation(); lib.handleDeleteTrack(t.id); }}>Delete</button>
        {/if}
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
      </div>
    </div>
  {/each}
</div>

<style>
  .track-list { display: flex; flex-direction: column; }

  .trow {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 10px;
    border-radius: 10px;
    min-width: 0;
    /* Skip layout/paint for rows outside the viewport so a multi-thousand-row
       list scrolls and re-renders cheaply without a virtual-list library.
       `auto` lets the browser remember each row's real height once measured. */
    content-visibility: auto;
    contain-intrinsic-size: auto 60px;
  }
  .trow.playable { cursor: pointer; }
  .trow:hover { background: var(--surface); }
  .trow.playing { background: color-mix(in srgb, var(--accent) 12%, transparent); }

  .trow-idx {
    flex: 0 0 auto;
    width: 26px;
    text-align: right;
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: #4a4a54;
    font-variant-numeric: tabular-nums;
  }
  .trow.playing .trow-idx { color: var(--accent); }

  .trow-art {
    position: relative;
    flex: 0 0 auto;
    width: 44px;
    height: 44px;
    border-radius: 7px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .trow-art img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .trow-ph {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted-2);
    background: linear-gradient(135deg, #241f33, #15131c);
  }
  .trow-ph .lni { font-size: 18px; }
  .trow-play {
    position: absolute;
    inset: 0;
    display: none;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.45);
    color: #fff;
    pointer-events: none;
  }
  .trow-play .lni { font-size: 18px; }
  .trow.playable:hover .trow-play,
  .trow.playing .trow-play { display: flex; }

  .trow-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .trow-title {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .trow-title-text {
    font-family: var(--font-display);
    font-size: 14.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: #ececef;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .trow.playing .trow-title-text { color: var(--accent); }
  .trow-dot {
    flex: 0 0 auto;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warning);
  }
  .trow-sub {
    font-family: var(--font-display);
    font-size: 12.5px;
    font-weight: 500;
    color: #8b8b96;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .trow-fmt {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.42px;
    color: var(--muted-2);
    text-align: right;
    min-width: 4.75em;
  }
  .trow-dur {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 12px;
    color: #8b8b96;
    text-align: right;
    min-width: 3em;
    font-variant-numeric: tabular-nums;
  }

  .trow-actions {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .trow-actions .btn-rate { font-size: 17px; }
  .trow-actions .btn-rate .lni { display: block; }
  /* Curation actions replace format/duration on row hover: the resting row keeps
     like/dislike pinned to the right edge (no reserved gap), and Edit/Delete swap
     in without shoving the thumbs around. */
  .trow-hover { display: none; }
  .trow:hover .trow-hover { display: inline-flex; }
  .trow:hover .trow-fmt,
  .trow:hover .trow-dur { display: none; }

  /* ── Mobile: artwork-led list, tap to play ─────────────────────────────── */
  @media (max-width: 860px) {
    .trow { gap: 10px; padding: 8px 2px; border-radius: 0; }
    .trow-idx,
    .trow-fmt,
    .trow-actions { display: none; }
    .trow-art { width: 48px; height: 48px; }
  }
</style>
