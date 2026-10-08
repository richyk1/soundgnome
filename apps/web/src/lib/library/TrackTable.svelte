<script lang="ts">
  import { getContext } from 'svelte';
  import type { LibraryTrackDto } from '../types';
  import { lib, LIBRARY_PLAYER, type LibraryPlayer } from './store.svelte';
  import PixelCover from '../PixelCover.svelte';
  import { trackCoverSeed } from '../pixel-art';
  import { pop } from '../motion';
  import { phone } from '../viewport.svelte';

  let { tracks, showAlbumCol = true, showDelete = false }: {
    tracks: LibraryTrackDto[];
    showAlbumCol?: boolean;
    showDelete?: boolean;
  } = $props();

  const player = getContext<LibraryPlayer | undefined>(LIBRARY_PLAYER);

  let listEl: HTMLElement | undefined = $state();

  // Index of the row currently playing. Reactive: `isCurrent` reads the player's
  // current-track signal, and `tracks` re-orders when shuffle reshuffles the
  // list — so this changes on both advance and reorder, re-focusing either way.
  let playingIndex = $derived(player ? tracks.findIndex((t) => player.isCurrent(t.id)) : -1);

  let scrollRaf = 0;
  function scrollParent(el: HTMLElement): HTMLElement | null {
    let p = el.parentElement;
    while (p) {
      const oy = getComputedStyle(p).overflowY;
      if ((oy === 'auto' || oy === 'scroll') && p.scrollHeight > p.clientHeight) return p;
      p = p.parentElement;
    }
    return null;
  }
  function centerOffset(row: HTMLElement, panel: HTMLElement): number {
    const rr = row.getBoundingClientRect();
    const pr = panel.getBoundingClientRect();
    return rr.top + rr.height / 2 - (pr.top + pr.height / 2);
  }
  // Keep the playing row in view. Since shuffle now reorders the list, the next
  // track is the adjacent row — a short smooth glide. A far jump (a fresh shuffle
  // pins the song to the top, or first entry) settles instantly instead, so there
  // is no long, heavy scroll. Reduced-motion always jumps.
  function focusPlaying() {
    const el = listEl;
    const row = el?.querySelector('.trow.playing');
    if (!el || !(row instanceof HTMLElement)) return;
    const panel = scrollParent(el);
    if (!panel) {
      row.scrollIntoView({ block: 'center' });
      return;
    }
    cancelAnimationFrame(scrollRaf);
    const reduce =
      typeof matchMedia !== 'undefined' &&
      matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduce || Math.abs(centerOffset(row, panel)) > panel.clientHeight * 2) {
      // Instant, but converge: content-visibility rows shift as they render.
      let hops = 0;
      const jump = () => {
        const off = centerOffset(row, panel);
        panel.scrollBy({ top: off });
        if (Math.abs(off) > 2 && hops++ < 8) scrollRaf = requestAnimationFrame(jump);
      };
      jump();
      return;
    }
    let frames = 0;
    const tick = () => {
      const off = centerOffset(row, panel);
      if (Math.abs(off) < 1.5 || frames++ >= 120) {
        panel.scrollBy({ top: off });
        return;
      }
      panel.scrollBy({ top: off * 0.22 }); // ease toward the re-measured centre
      scrollRaf = requestAnimationFrame(tick);
    };
    scrollRaf = requestAnimationFrame(tick);
  }
  $effect(() => {
    const idx = playingIndex;
    if (idx < 0 || !listEl) return;
    requestAnimationFrame(focusPlaying);
    return () => cancelAnimationFrame(scrollRaf);
  });

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

<div class="track-list" bind:this={listEl}>
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

      <div class="cover-wrap trow-art">
        <PixelCover src={coverUrl(t)} seed={trackCoverSeed(t)} />
        {#if player && t.file_path}
          <span class="trow-play" aria-hidden="true"><i class="pxi {player.isPlaying(t.id) ? 'pxi-pause' : 'pxi-play'}" use:pop={player.isPlaying(t.id)}></i></span>
        {/if}
      </div>

      <div class="trow-main">
        <span class="trow-title">
          <span class="trow-title-text">{t.title}</span>
          {#if player?.isCurrent(t.id)}<i class="pxi pxi-volume-3 trow-live" aria-hidden="true"></i>{/if}
          {#if t.needs_validation}<span class="trow-dot" title="Awaiting validation"></span>{/if}
        </span>
        <span class="trow-sub">{secondary(t)}</span>
      </div>

      <span class="trow-fmt">{qualityLabel(t)}</span>
      <span class="trow-dur">{lib.fmtDuration(t.duration)}</span>

      {#if phone.current}
        <!-- Phones: no editing or rating from lists (rate in Now Playing, edit on desktop). -->
        {#if showDelete}
          <button class="btn-delete btn-sm trow-delete" onclick={(e) => { e.stopPropagation(); lib.handleDeleteTrack(t.id); }}>Delete</button>
        {/if}
      {:else}
      <div class="trow-actions">
        <button class="btn-edit btn-sm trow-hover" onclick={(e) => { e.stopPropagation(); lib.startEditTrack(t); }}>Edit</button>
        {#if showDelete}
          <button class="btn-delete btn-sm trow-hover" onclick={(e) => { e.stopPropagation(); lib.handleDeleteTrack(t.id); }}>Delete</button>
        {/if}
        <button
          class="btn-rate"
          class:active-like={t.rating === 'liked'}
          title="Like"
          aria-label="Like"
          aria-pressed={t.rating === 'liked'}
          onclick={(e) => { e.stopPropagation(); lib.setRating(t, t.rating === 'liked' ? null : 'liked'); }}
        ><i class="pxi pxi-thumbs-up" aria-hidden="true" use:pop={t.rating === 'liked'}></i></button>
        <button
          class="btn-rate"
          class:active-dislike={t.rating === 'disliked'}
          title="Dislike"
          aria-label="Dislike"
          aria-pressed={t.rating === 'disliked'}
          onclick={(e) => { e.stopPropagation(); lib.setRating(t, t.rating === 'disliked' ? null : 'disliked'); }}
        ><i class="pxi pxi-thumbs-down" aria-hidden="true" use:pop={t.rating === 'disliked'}></i></button>
      </div>
      {/if}
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
    border-radius: var(--radius-control);
    min-width: 0;
    /* Paint-only feedback: rows are long lists, so no transforms. */
    transition: background-color var(--motion-fast) var(--ease-out);
    /* Skip layout/paint for rows outside the viewport so a multi-thousand-row
       list scrolls and re-renders cheaply without a virtual-list library.
       `auto` lets the browser remember each row's real height once measured. */
    content-visibility: auto;
    /* Content-box height (the 40px cover); padding is added on top. */
    contain-intrinsic-size: auto 40px;
  }
  .trow.playable { cursor: pointer; }
  .trow:hover { background: var(--surface); }

  .trow-idx {
    flex: 0 0 auto;
    width: 26px;
    text-align: right;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
  }
  .trow.playing .trow-idx { color: var(--live); }

  .trow-art {
    flex: 0 0 auto;
    width: 40px;
    height: 40px;
    border-radius: var(--radius-chip);
  }
  .trow-play {
    position: absolute;
    inset: 0;
    display: none;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--bg) 64%, transparent);
    color: var(--text-bright);
    font-size: 16px;
    pointer-events: none;
  }
  .trow.playing .trow-play { color: var(--live); }
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
    font-size: 14px;
    font-weight: 500;
    line-height: 1.35;
    color: var(--text-bright);
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color var(--motion-fast) var(--ease-out);
  }
  .trow.playing .trow-title-text { color: var(--live); }
  .trow-live { flex: 0 0 auto; font-size: 16px; color: var(--live); }
  .trow-dot {
    flex: 0 0 auto;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warning);
  }
  .trow-sub {
    font-size: 13px;
    line-height: 1.35;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .trow-fmt {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.04em;
    color: var(--muted-2);
    text-align: right;
    min-width: 4.75em;
  }
  .trow-dur {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--muted-2);
    text-align: right;
    min-width: 3em;
    font-variant-numeric: tabular-nums;
  }

  .trow-actions {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .trow-actions .btn-rate .pxi { display: block; }
  /* Curation actions replace format/duration on row hover: the resting row keeps
     like/dislike pinned to the right edge (no reserved gap), and Edit/Delete swap
     in without shoving the thumbs around. */
  .trow-hover { display: none; }
  .trow:hover .trow-hover { display: inline-flex; }
  .trow:hover .trow-fmt,
  .trow:hover .trow-dur { display: none; }

  /* ── Phones: artwork-led cozy list, tap to play ────────────────────────── */
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .trow {
      gap: 12px;
      padding: 6px 0;
      border-radius: 0;
    }
    .trow-idx,
    .trow-fmt { display: none; }
    .trow-delete { flex: 0 0 auto; }
  }
</style>
