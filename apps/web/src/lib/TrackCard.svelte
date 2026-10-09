<script lang="ts">
  import { slide } from 'svelte/transition';
  import { getMatchCandidates, getYoutubeCandidates, cleanTrackWithAI } from './api';
  import type { PendingValidationDto, PatchValidationBody, MatchCandidateDto } from './types';
  import ArtistMultiSelect from './library/ArtistMultiSelect.svelte';
  import StatefulButton from './StatefulButton.svelte';
  import PixelCover from './PixelCover.svelte';

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
  function scoreLevel(score: number): 'high' | 'mid' | 'low' {
    if (score >= 0.75) return 'high';
    if (score >= 0.5) return 'mid';
    return 'low';
  }
  function candProviderUrl(c: MatchCandidateDto): string | null {
    return c.references?.find((r) => r.external_url)?.external_url ?? null;
  }
  let coverSrc = $derived(track.cover && /^(https?:\/\/|\/)/.test(track.cover) ? track.cover : null);
</script>

<article
  class="vcard"
  class:editing
  bind:this={cardEl}
  onmouseenter={() => (hovered = true)}
  onmouseleave={() => (hovered = false)}
  out:slide={{ duration: reduce ? 0 : 240 }}
>
  <div class="vcard-head">
    <div class="cover-wrap vcover">
      <PixelCover src={coverSrc} seed="track:{track.id}" />
    </div>

    <div class="main">
      {#if editing}
        <div class="edit-form">
          <div class="field">
            <label for="edit-title-{track.id}">Title</label>
            <input id="edit-title-{track.id}" type="text" bind:value={editTitle} placeholder="Title" />
          </div>
          <div class="field">
            <label for="edit-artists-{track.id}">Artists</label>
            <ArtistMultiSelect value={editArtists} onChange={(names) => (editArtists = names)} />
          </div>
          <button type="button" class="btn-secondary btn-sm ai-clean" onclick={aiClean} disabled={aiCleaning}>
            {#if aiCleaning}
              <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Cleaning…
            {:else}
              Clean title &amp; artists with AI
            {/if}
          </button>
          {#if aiError}
            <p class="inline-error" role="alert">
              <i class="pxi pxi-square-alert" aria-hidden="true"></i>{aiError}
            </p>
          {/if}
          <div class="field">
            <label for="edit-album-{track.id}">Album</label>
            <input id="edit-album-{track.id}" type="text" bind:value={editAlbum} placeholder="Album" />
          </div>
          <div class="field-row">
            <div class="field">
              <label for="edit-genre-{track.id}">Genre</label>
              <input id="edit-genre-{track.id}" type="text" bind:value={editGenre} placeholder="Genre" />
            </div>
            <div class="field">
              <label for="edit-date-{track.id}">Date</label>
              <input id="edit-date-{track.id}" type="text" class="mono-input" bind:value={editDate} placeholder="YYYY-MM-DD" />
            </div>
            <div class="field narrow">
              <label for="edit-tn-{track.id}">Track #</label>
              <input id="edit-tn-{track.id}" class="mono-input" bind:value={editTrackNumber} type="number" min="1" />
            </div>
            <div class="field narrow">
              <label for="edit-dn-{track.id}">Disc #</label>
              <input id="edit-dn-{track.id}" class="mono-input" bind:value={editDiscNumber} type="number" min="1" />
            </div>
          </div>
          <div class="field">
            <label for="edit-label-{track.id}">Label</label>
            <input id="edit-label-{track.id}" type="text" bind:value={editLabel} placeholder="Label" />
          </div>
        </div>
      {:else}
        {#if sourceUrl}
          <a class="title" href={sourceUrl} target="_blank" rel="noopener noreferrer">{track.title}</a>
        {:else}
          <span class="title">{track.title}</span>
        {/if}
        <div class="artist">{artistNames()}</div>
        {#if track.album?.title || track.date || dur(track.duration)}
          <div class="meta">
            {#if track.album?.title}<span class="meta-text">{track.album.title}</span>{/if}
            {#if track.date}<span class="meta-data">{track.date}</span>{/if}
            {#if dur(track.duration)}<span class="meta-data">{dur(track.duration)}</span>{/if}
          </div>
        {/if}
      {/if}
    </div>

    <div class="actions">
      {#if editing}
        <button type="button" class="btn-cancel" onclick={() => (editing = false)}>Cancel</button>
        {#if onApprove}
          <StatefulButton
            variant="primary"
            label="Save & approve"
            action={approve}
            onError={(m) => (actionError = m)}
          />
        {/if}
      {:else}
        <button type="button" class="btn-secondary" onclick={startEdit} title="Edit metadata (e)">
          <i class="pxi pxi-pencil" aria-hidden="true"></i>Edit
        </button>
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
    <p class="inline-error row-error" role="alert">
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>{actionError}
    </p>
  {/if}

  {#if showsCandidates && !editing}
    <div class="cands">
      {#if isDrmProtected && !matchesRequested}
        <button type="button" class="btn-secondary btn-sm" onclick={loadMatches}>
          <i class="pxi pxi-search" aria-hidden="true"></i>Find YouTube sources
        </button>
      {:else if matchesLoading}
        <p class="cand-status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Finding matches…</p>
      {:else if matchesError}
        <p class="cand-status inline-error"><i class="pxi pxi-square-alert" aria-hidden="true"></i>{matchesError}</p>
      {:else if matchCandidates.length === 0}
        <p class="cand-status">No candidates found</p>
      {:else}
        <ul class="cand-list">
          {#each matchCandidates as candidate (candidate.title + candidate.provider + candidate.score)}
            {@const purl = candProviderUrl(candidate)}
            <li class="cand">
              <div class="cand-main">
                <div class="cand-title-line">
                  {#if purl}
                    <a class="cand-title" href={purl} target="_blank" rel="noopener noreferrer">{candidate.title}</a>
                  {:else}
                    <span class="cand-title">{candidate.title}</span>
                  {/if}
                  <span class="cand-artist">{candidate.artists.map((a) => a.name).join(', ')}</span>
                </div>
                <div class="meta">
                  <span class="cand-score" data-lvl={scoreLevel(candidate.score)}>{Math.round(candidate.score * 100)}%</span>
                  {#if candidate.provider}<span class="cand-provider">{candidate.provider}</span>{/if}
                  {#if candidate.album?.title}<span class="meta-text">{candidate.album.title}</span>{/if}
                  {#if candidate.date}<span class="meta-data">{candidate.date}</span>{/if}
                  {#if dur(candidate.duration)}<span class="meta-data">{dur(candidate.duration)}</span>{/if}
                </div>
              </div>
              <StatefulButton
                variant="primary"
                size="sm"
                label="Select"
                action={() => selectCandidate(candidate)}
                onError={(m) => (actionError = m)}
              />
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</article>

<style>
  /* One hairline panel per pending track; candidates are hairline rows inside it. */
  .vcard {
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    transition: border-color var(--motion-fast) var(--ease-out);
  }
  .vcard:hover { border-color: var(--border-strong); }
  .vcard.editing { border-color: var(--border-heavy); }

  .vcard-head {
    display: flex;
    align-items: flex-start;
    gap: 14px;
  }
  .vcover {
    flex-shrink: 0;
    width: 56px;
    height: 56px;
  }

  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font-size: 15px;
    font-weight: 600;
    line-height: 1.35;
    letter-spacing: -0.01em;
    color: var(--text-bright);
    text-decoration: none;
    overflow-wrap: anywhere;
  }
  a.title:hover { color: var(--accent); text-decoration: underline; }
  .artist {
    font-size: 14px;
    line-height: 1.35;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Metadata line: words in Geist, data in Geist Mono, split by a quiet dot. */
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 0;
    margin-top: 4px;
    font-size: 13px;
    line-height: 1.4;
    color: var(--muted-2);
  }
  .meta > * + *::before {
    content: '·';
    margin: 0 8px;
    font-family: var(--font-body);
    color: var(--muted-2);
  }
  .meta-text { color: var(--muted); min-width: 0; overflow-wrap: anywhere; }
  .meta-data,
  .cand-score,
  .cand-provider {
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .cand-provider {
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .actions {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .inline-error {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    font-size: 13px;
    line-height: 1.4;
    color: var(--error);
    overflow-wrap: anywhere;
  }
  .inline-error .pxi {
    flex-shrink: 0;
    margin-top: 1px;
    font-size: 16px;
  }
  .row-error { margin: 10px 0 0 70px; }

  /* Candidates: indented under the title, flat rows split by hairlines. */
  .cands {
    margin: 12px 0 0 70px;
  }
  .cand-status {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 8px 0;
    font-size: 13px;
    color: var(--muted);
  }
  .cand-status .pxi { font-size: 16px; }
  .cand-status.inline-error { color: var(--error); }
  .cand-list {
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--border-soft);
  }
  .cand {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border-soft);
  }
  .cand:last-child { border-bottom: none; padding-bottom: 0; }
  .cand-main { min-width: 0; }
  .cand-title-line {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 2px 10px;
  }
  .cand-title {
    font-size: 14px;
    font-weight: 500;
    color: var(--text-bright);
    text-decoration: none;
  }
  a.cand-title:hover { color: var(--accent); text-decoration: underline; }
  .cand-artist {
    font-size: 13px;
    color: var(--muted);
  }
  .cand .meta { margin-top: 2px; }
  .cand-score { color: var(--muted-2); }
  .cand-score[data-lvl='high'] { color: var(--success); }
  .cand-score[data-lvl='mid'] { color: var(--warning); }

  /* Edit form: global fields, layout only here. */
  .edit-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 620px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    flex: 1;
  }
  .field.narrow { max-width: 96px; }
  .field label {
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .field-row {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .mono-input {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .ai-clean { align-self: flex-start; }

  @media (max-width: 640px) {
    .row-error,
    .cands { margin-left: 0; }
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .vcard { padding: 14px; }
    .vcard-head { flex-wrap: wrap; }
    .main { flex-basis: calc(100% - 70px); }
    .actions { width: 100%; flex-wrap: wrap; justify-content: flex-end; }
    .cand-title, .cand-artist { overflow-wrap: anywhere; }
    .cand { flex-wrap: wrap; gap: 8px; }
    .cand-main { flex-basis: 100%; }
    .cand :global(.sbtn) { margin-left: auto; }
    .field-row .field { flex: 1 1 120px; }
    .field.narrow { max-width: none; }
    .ai-clean { min-height: 44px; }
  }
</style>
