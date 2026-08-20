<script lang="ts">
  import { slide } from 'svelte/transition';
  import { getMatchCandidates, getYoutubeCandidates, cleanTrackWithAI } from './api';
  import type { PendingValidationDto, PatchValidationBody, MatchCandidateDto } from './types';
  import ArtistMultiSelect from './library/ArtistMultiSelect.svelte';
  import StatefulButton from './StatefulButton.svelte';

  interface Props {
    track: PendingValidationDto;
    onApprove?: (id: number, patch: PatchValidationBody) => Promise<void>;
    onReject?: (id: number) => Promise<void>;
  }

  let { track, onApprove, onReject }: Props = $props();

  const reduce =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  let editing = $state(false);
  let actionError: string | null = $state(null);

  // Match candidates. MusicBrainz metadata matches are cheap and load
  // automatically when the row scrolls in. The YouTube search (yt-dlp, several
  // subprocesses per track) is expensive and would fire a burst for every
  // visible DRM row, so those load on demand instead.
  let matchesRequested = $state(false);
  let matchesLoading = $state(false);
  let matchCandidates: MatchCandidateDto[] = $state([]);
  let matchesError: string | null = $state(null);

  // editable copies — reset whenever we open the form
  let editTitle = $state('');
  let editArtists = $state<string[]>([]);
  let editAlbum = $state('');
  let editGenre = $state('');
  let editDate = $state('');
  let editTrackNumber = $state('');
  let editDiscNumber = $state('');
  let editLabel = $state('');
  let aiCleaning = $state(false);
  let aiError: string | null = $state(null);

  let cardEl: HTMLElement | undefined = $state();

  let sourceUrl = $derived(
    track.references.find((r) => r.ref_type === 'Source' && r.external_url)?.external_url ?? null,
  );
  let isPartialMatch = $derived(track.validation_reason === 'metadata_partial_match');
  let isDrmProtected = $derived(track.validation_reason === 'soundcloud_drm_protected');
  let showsCandidates = $derived(isPartialMatch || isDrmProtected);

  // Keep edit fields in sync with the track.
  $effect(() => {
    editTitle = track.title;
    editArtists = track.artists.map((a) => a.name);
    editAlbum = track.album?.title ?? '';
    editGenre = track.genre ?? '';
    editDate = track.date ?? '';
    editTrackNumber = track.track_number?.toString() ?? '';
    editDiscNumber = track.disc_number?.toString() ?? '';
    editLabel = track.label ?? '';
  });

  // Auto-load metadata candidates when the row nears the viewport (avoids firing
  // a rate-limited MusicBrainz lookup for every pending track at once). DRM rows
  // are skipped here; their expensive YouTube search loads on demand.
  $effect(() => {
    if (!cardEl || !showsCandidates || isDrmProtected) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          io.disconnect();
          loadMatches();
        }
      },
      { rootMargin: '300px' },
    );
    io.observe(cardEl);
    return () => io.disconnect();
  });

  // 'e' to edit / Escape to close, only while hovered / editing.
  let hovered = $state(false);
  $effect(() => {
    if (!hovered || editing) return;
    function onKeydown(e: KeyboardEvent) {
      const tgt = e.target as HTMLElement;
      if (tgt.tagName === 'INPUT' || tgt.tagName === 'TEXTAREA' || tgt.tagName === 'SELECT') return;
      if (e.key === 'e') {
        e.preventDefault();
        startEdit();
      }
    }
    document.addEventListener('keydown', onKeydown);
    return () => document.removeEventListener('keydown', onKeydown);
  });
  $effect(() => {
    if (!editing) return;
    function onKeydown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        e.preventDefault();
        editing = false;
      }
    }
    document.addEventListener('keydown', onKeydown);
    return () => document.removeEventListener('keydown', onKeydown);
  });

  async function loadMatches() {
    if (matchesRequested) return;
    matchesRequested = true;
    matchesLoading = true;
    matchesError = null;
    try {
      matchCandidates = isDrmProtected
        ? await getYoutubeCandidates(track.id)
        : await getMatchCandidates(track.id);
    } catch (e: unknown) {
      matchesError = e instanceof Error ? e.message : String(e);
    } finally {
      matchesLoading = false;
    }
  }

  function startEdit() {
    editing = true;
  }

  // These throw on failure; the StatefulButton catches it to show its error state
  // and reports the reason via onError for the inline note.
  async function approve() {
    if (!onApprove) return;
    const patch: PatchValidationBody = {};
    if (editing) {
      const t = editTitle.trim();
      if (t && t !== track.title) patch.title = t;
      const origArtists = track.artists.map((a) => a.name);
      if (JSON.stringify(editArtists) !== JSON.stringify(origArtists) && editArtists.length > 0)
        patch.artists = editArtists;
      const al = editAlbum.trim();
      if (al !== (track.album?.title ?? '')) patch.album_title = al || undefined;
      const g = editGenre.trim();
      if (g !== (track.genre ?? '')) patch.genre = g || undefined;
      const d = editDate.trim();
      if (d !== (track.date ?? '')) patch.date = d || undefined;
      const tn = parseInt(editTrackNumber);
      if (!isNaN(tn) && tn !== track.track_number) patch.track_number = tn;
      const dn = parseInt(editDiscNumber);
      if (!isNaN(dn) && dn !== track.disc_number) patch.disc_number = dn;
      const lb = editLabel.trim();
      if (lb !== (track.label ?? '')) patch.label = lb || undefined;
    }
    await onApprove(track.id, patch);
  }

  async function reject() {
    if (!onReject) return;
    await onReject(track.id);
  }

  // Ask the AI backend to clean the messy title/artists into a review-ready
  // suggestion, then fill the edit form with it. Non-destructive: the user still
  // reviews and clicks Save & approve. Errors surface inline (e.g. AI not
  // configured) without touching the fields.
  async function aiClean() {
    aiCleaning = true;
    aiError = null;
    try {
      const res = await cleanTrackWithAI(track.id, { title: editTitle, artists: editArtists });
      editTitle = res.title;
      editArtists = res.artists;
    } catch (e: unknown) {
      aiError = e instanceof Error ? e.message : String(e);
    } finally {
      aiCleaning = false;
    }
  }

  async function selectCandidate(candidate: MatchCandidateDto) {
    if (!onApprove) return;
    const patch: PatchValidationBody = {};
    if (isDrmProtected) {
      const providerRef = candidate.references.find(
        (r) => r.ref_type === 'Provider' && r.external_url,
      );
      if (!providerRef?.external_url) throw new Error('This result has no downloadable source.');
      patch.provider_url = providerRef.external_url;
    } else {
      patch.title = candidate.title;
      patch.artists = candidate.artists.map((a) => a.name);
      patch.album_title = candidate.album?.title ?? undefined;
      patch.genre = candidate.genre ?? undefined;
      patch.date = candidate.date ?? undefined;
      patch.track_number = candidate.track_number ?? undefined;
      patch.disc_number = candidate.disc_number ?? undefined;
      patch.label = candidate.label ?? undefined;
    }
    await onApprove(track.id, patch);
  }

  function dur(seconds: number | null | undefined): string | null {
    if (!seconds) return null;
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
  function artistNames(): string {
    return track.artists.map((a) => a.name).join(', ') || '—';
  }
  function trackMeta(): string {
    return [track.album?.title, track.date, dur(track.duration)].filter(Boolean).join('  ·  ');
  }
  function candMeta(c: MatchCandidateDto): string {
    return [c.album?.title, c.date, dur(c.duration), c.provider].filter(Boolean).join('  ·  ');
  }
  function scoreLevel(score: number): 'high' | 'mid' | 'low' {
    if (score >= 0.75) return 'high';
    if (score >= 0.5) return 'mid';
    return 'low';
  }
  function candProviderUrl(c: MatchCandidateDto): string | null {
    return c.references?.find((r) => r.external_url)?.external_url ?? null;
  }
</script>

<article
  class="vrow"
  class:editing
  bind:this={cardEl}
  onmouseenter={() => (hovered = true)}
  onmouseleave={() => (hovered = false)}
  out:slide={{ duration: reduce ? 0 : 240 }}
>
  <div class="vrow-head">
    <div class="cover">
      {#if track.cover}
        <img src={track.cover} alt="" />
      {:else}
        <i class="lni lni-music-note" aria-hidden="true"></i>
      {/if}
    </div>

    <div class="main">
      {#if editing}
        <div class="edit-form">
          <div class="field">
            <label for="edit-title-{track.id}">Title</label>
            <input id="edit-title-{track.id}" bind:value={editTitle} placeholder="Title" />
          </div>
          <div class="field">
            <label for="edit-artists-{track.id}">Artists</label>
            <ArtistMultiSelect value={editArtists} onChange={(names) => (editArtists = names)} />
          </div>
          <button type="button" class="ai-clean" onclick={aiClean} disabled={aiCleaning}>
            {#if aiCleaning}
              <span class="mini-spin" aria-hidden="true"></span>Cleaning…
            {:else}
              Clean title &amp; artists with AI
            {/if}
          </button>
          {#if aiError}<p class="ai-err" role="alert">{aiError}</p>{/if}
          <div class="field">
            <label for="edit-album-{track.id}">Album</label>
            <input id="edit-album-{track.id}" bind:value={editAlbum} placeholder="Album" />
          </div>
          <div class="field-row">
            <div class="field">
              <label for="edit-genre-{track.id}">Genre</label>
              <input id="edit-genre-{track.id}" bind:value={editGenre} placeholder="Genre" />
            </div>
            <div class="field">
              <label for="edit-date-{track.id}">Date</label>
              <input id="edit-date-{track.id}" bind:value={editDate} placeholder="YYYY-MM-DD" />
            </div>
            <div class="field narrow">
              <label for="edit-tn-{track.id}">Track #</label>
              <input id="edit-tn-{track.id}" bind:value={editTrackNumber} type="number" min="1" />
            </div>
            <div class="field narrow">
              <label for="edit-dn-{track.id}">Disc #</label>
              <input id="edit-dn-{track.id}" bind:value={editDiscNumber} type="number" min="1" />
            </div>
          </div>
          <div class="field">
            <label for="edit-label-{track.id}">Label</label>
            <input id="edit-label-{track.id}" bind:value={editLabel} placeholder="Label" />
          </div>
        </div>
      {:else}
        <div class="title-line">
          {#if sourceUrl}
            <a class="title" href={sourceUrl} target="_blank" rel="noopener noreferrer"
              >{track.title}</a
            >
          {:else}
            <span class="title">{track.title}</span>
          {/if}
          <span class="artist">{artistNames()}</span>
        </div>
        {#if trackMeta()}
          <div class="meta">{trackMeta()}</div>
        {/if}
      {/if}
    </div>

    <div class="actions">
      {#if editing}
        <button class="lbtn" onclick={() => (editing = false)}>Cancel</button>
        {#if onApprove}
          <StatefulButton
            variant="primary"
            label="Save & approve"
            action={approve}
            onError={(m) => (actionError = m)}
          />
        {/if}
      {:else}
        <button class="lbtn" onclick={startEdit} title="Edit metadata (e)">Edit</button>
        {#if onReject}
          <StatefulButton
            variant="danger"
            label="Reject"
            action={reject}
            onError={(m) => (actionError = m)}
          />
        {/if}
        {#if onApprove}
          <StatefulButton
            variant="primary"
            label="Approve"
            action={approve}
            onError={(m) => (actionError = m)}
          />
        {/if}
      {/if}
    </div>
  </div>

  {#if actionError}
    <p class="row-error" role="alert">
      <i class="lni lni-xmark-circle" aria-hidden="true"></i>{actionError}
    </p>
  {/if}

  {#if showsCandidates && !editing}
    <div class="cands">
      {#if isDrmProtected && !matchesRequested}
        <button class="find-sources" onclick={loadMatches}>
          <i class="lni lni-search-1" aria-hidden="true"></i>Find YouTube sources
        </button>
      {:else if matchesLoading}
        <p class="cand-status"><span class="mini-spin" aria-hidden="true"></span>Finding matches…</p>
      {:else if matchesError}
        <p class="cand-status is-err">{matchesError}</p>
      {:else if matchCandidates.length === 0}
        <p class="cand-status">No candidates found</p>
      {:else}
        {#each matchCandidates as candidate (candidate.title + candidate.provider + candidate.score)}
          {@const purl = candProviderUrl(candidate)}
          <div class="cand">
            <div class="cand-main">
              <div class="cand-title-line">
                {#if purl}
                  <a class="cand-title" href={purl} target="_blank" rel="noopener noreferrer"
                    >{candidate.title}</a
                  >
                {:else}
                  <span class="cand-title">{candidate.title}</span>
                {/if}
                <span class="cand-artist">{candidate.artists.map((a) => a.name).join(', ')}</span>
                <span class="cand-score" data-lvl={scoreLevel(candidate.score)}
                  >{Math.round(candidate.score * 100)}%</span
                >
              </div>
              <div class="cand-meta">{candMeta(candidate)}</div>
            </div>
            <StatefulButton
              variant="primary"
              size="sm"
              label="Select"
              action={() => selectCandidate(candidate)}
              onError={(m) => (actionError = m)}
            />
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</article>

<style>
  /* Flat row: no nested cards, no surface stacking. Rows are separated by a
     hairline and grouped by indentation, not by boxes. */
  .vrow {
    padding: 1rem 0.5rem 1.1rem;
    border-bottom: 1px solid var(--border-soft);
    transition: background 0.12s ease;
  }
  .vrow:hover {
    background: color-mix(in srgb, var(--surface) 55%, transparent);
  }
  .vrow.editing {
    background: color-mix(in srgb, var(--accent) 5%, transparent);
  }

  .vrow-head {
    display: flex;
    align-items: flex-start;
    gap: 0.9rem;
  }

  .cover {
    flex-shrink: 0;
    width: 48px;
    height: 48px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--surface-2);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted-2);
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .cover .lni {
    font-size: 20px;
  }

  .main {
    flex: 1;
    min-width: 0;
    padding-top: 0.1rem;
  }
  .title-line {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .title {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-bright);
    text-decoration: none;
  }
  a.title:hover {
    text-decoration: underline;
  }
  .artist {
    font-size: 0.85rem;
    color: var(--muted);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    margin-top: 0.25rem;
    font-size: 0.8rem;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
  }

  .actions {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  /* Plain (non-async) button for Edit / Cancel */
  .lbtn {
    height: 34px;
    padding: 0 0.8rem;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    transition:
      background 0.12s ease,
      color 0.12s ease;
  }
  .lbtn:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .row-error {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0.5rem 0 0 3.9rem;
    font-size: 0.82rem;
    color: var(--error);
  }
  .row-error .lni {
    font-size: 15px;
    flex-shrink: 0;
  }

  /* Candidates: indented under the title, flat rows split by hairlines. */
  .cands {
    margin: 0.6rem 0 0 3.9rem;
  }
  .cand-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0;
    padding: 0.5rem 0;
    font-size: 0.82rem;
    color: var(--muted);
  }
  .cand-status.is-err {
    color: var(--error);
  }
  .find-sources {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.4rem 0.7rem;
    font-size: 0.82rem;
    font-family: inherit;
    color: var(--accent);
    background: transparent;
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.12s ease;
  }
  .find-sources:hover {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .cand {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.55rem 0;
    border-top: 1px solid var(--border-soft);
  }
  .cand:first-child {
    border-top: none;
  }
  .cand-main {
    min-width: 0;
  }
  .cand-title-line {
    display: flex;
    align-items: baseline;
    gap: 0.55rem;
    flex-wrap: wrap;
  }
  .cand-title {
    font-size: 0.88rem;
    font-weight: 500;
    color: var(--text);
    text-decoration: none;
  }
  a.cand-title:hover {
    text-decoration: underline;
  }
  .cand-artist {
    font-size: 0.8rem;
    color: var(--muted);
  }
  .cand-score {
    font-size: 0.72rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }
  .cand-score[data-lvl='high'] {
    color: var(--success);
  }
  .cand-score[data-lvl='mid'] {
    color: var(--warning, #d9a441);
  }
  .cand-meta {
    margin-top: 0.2rem;
    font-size: 0.76rem;
    color: var(--muted-2);
    font-variant-numeric: tabular-nums;
  }

  .mini-spin {
    width: 13px;
    height: 13px;
    border: 2px solid color-mix(in srgb, currentColor 30%, transparent);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: mini-rot 0.7s linear infinite;
  }
  @keyframes mini-rot {
    to {
      transform: rotate(360deg);
    }
  }

  /* Edit form */
  .edit-form {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    max-width: 620px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
    flex: 1;
  }
  .field.narrow {
    max-width: 90px;
  }
  .field label {
    font-size: 0.72rem;
    color: var(--muted-2);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .field input {
    height: 32px;
    padding: 0 0.6rem;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface-2);
    color: var(--text-bright);
    font: inherit;
    font-size: 0.85rem;
  }
  .field input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .field-row {
    display: flex;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  @media (prefers-reduced-motion: reduce) {
    .mini-spin {
      animation-duration: 1.3s;
    }
  }

  @media (max-width: 640px) {
    .vrow-head {
      flex-wrap: wrap;
    }
    .actions {
      width: 100%;
      justify-content: flex-end;
    }
    .row-error,
    .cands {
      margin-left: 0;
    }
  }
  .ai-clean {
    align-self: flex-start;
    margin: 2px 0 4px;
    padding: 5px 10px;
    font-size: 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
    border-radius: 6px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .ai-clean:hover:not(:disabled) { background: color-mix(in srgb, var(--accent) 22%, transparent); }
  .ai-clean:disabled { opacity: 0.6; cursor: default; }
  .ai-err { margin: 2px 0 0; font-size: 12px; color: #e5684d; }
</style>
