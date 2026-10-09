<script lang="ts">
  import type { ReferenceDto, AddReferenceBody } from '../types';

  interface Props {
    references: ReferenceDto[];
    onAdd: (body: AddReferenceBody) => Promise<void>;
    onDelete: (ref: ReferenceDto) => Promise<void>;
  }

  let { references, onAdd, onDelete }: Props = $props();

  // Form state — the primary flow is just "type" + "link": platform and id are
  // inferred server-side straight from the link (see `Reference::infer_platform_and_id`).
  // "Advanced" is only needed to override the inferred platform or to add a
  // reference that has no URL at all (id-only), e.g. a SoundCloud/Bandcamp id
  // grabbed from elsewhere.
  let formOpen = $state(false);
  let advancedOpen = $state(false);
  let saving = $state(false);
  let refType = $state('Metadata');
  let link = $state('');
  let platformOverride = $state('Auto');
  let externalId = $state('');

  const REF_TYPES = ['Source', 'Provider', 'Metadata', 'Reference'] as const;
  const PLATFORM_OVERRIDES = ['Auto', 'Spotify', 'SoundCloud', 'MusicBrainz', 'YoutubeMusic', 'Youtube', 'Bandcamp', 'Unknown'] as const;

  function platformLabel(p: string): string {
    const map: Record<string, string> = {
      soundcloud: 'SoundCloud',
      musicbrainz: 'MusicBrainz',
      youtubemusic: 'YT Music',
      youtube: 'YouTube',
      bandcamp: 'Bandcamp',
      spotify: 'Spotify',
      unknown: 'Unknown',
      auto: 'Auto (from link)',
    };
    return map[p.toLowerCase()] ?? p;
  }

  function refTypeLabel(t: string): string {
    const map: Record<string, string> = {
      Source: 'Source',
      Provider: 'Provider',
      Metadata: 'Metadata',
      Reference: 'Reference',
    };
    return map[t] ?? t;
  }

  function resetForm() {
    refType = 'Metadata';
    link = '';
    platformOverride = 'Auto';
    externalId = '';
    advancedOpen = false;
    formOpen = false;
  }

  async function handleAdd() {
    const trimmedLink = link.trim();
    const trimmedId = externalId.trim();
    if (!trimmedLink && !trimmedId) return;
    saving = true;
    try {
      await onAdd({
        ref_type: refType,
        // Only sent when "Advanced" was opened and set to something other than
        // "Auto" — otherwise the backend infers the platform (and id, when
        // possible) directly from the link.
        platform: advancedOpen && platformOverride !== 'Auto' ? platformOverride : null,
        external_id: trimmedId || null,
        external_url: trimmedLink || null,
      });
      resetForm();
    } catch (err) {
      alert(err instanceof Error ? err.message : String(err));
    } finally {
      saving = false;
    }
  }

  async function handleDelete(ref: ReferenceDto) {
    if (!confirm('Remove this reference?')) return;
    try {
      await onDelete(ref);
    } catch (err) {
      alert(err instanceof Error ? err.message : String(err));
    }
  }
</script>

<section class="refs-panel">
  <div class="refs-header">
    <h4 class="refs-title">References <span class="refs-count">{references.length}</span></h4>
    <button class="btn-ghost btn-sm" onclick={() => (formOpen = !formOpen)} aria-expanded={formOpen}>
      {#if formOpen}<i class="pxi pxi-close" aria-hidden="true"></i>Close{:else}<i class="pxi pxi-plus" aria-hidden="true"></i>Add{/if}
    </button>
  </div>

  {#if formOpen}
    <div class="ref-form">
      <div class="ref-form-row">
        <label class="ref-field ref-field-type">
          Type
          <select bind:value={refType}>
            {#each REF_TYPES as t}
              <option value={t}>{refTypeLabel(t)}</option>
            {/each}
          </select>
        </label>
        <label class="ref-field ref-field-link">
          Link
          <input
            bind:value={link}
            type="url"
            placeholder="https://… (platform & id are inferred automatically)"
          />
        </label>
      </div>

      <button type="button" class="btn-toggle-advanced" aria-expanded={advancedOpen} onclick={() => (advancedOpen = !advancedOpen)}>
        <i class="pxi {advancedOpen ? 'pxi-chevron-down' : 'pxi-chevron-right'}" aria-hidden="true"></i>
        Advanced (override platform or set an id without a link)
      </button>

      {#if advancedOpen}
        <div class="ref-form-row">
          <label class="ref-field">
            Platform
            <select bind:value={platformOverride}>
              {#each PLATFORM_OVERRIDES as p}
                <option value={p}>{platformLabel(p)}</option>
              {/each}
            </select>
          </label>
          <label class="ref-field">
            External ID
            <input
              bind:value={externalId}
              type="text"
              placeholder="e.g. spotify track id"
            />
          </label>
        </div>
      {/if}

      <div class="ref-form-actions">
        <button class="btn-cancel btn-sm" onclick={resetForm} disabled={saving}>Cancel</button>
        <button
          class="btn-save btn-sm"
          onclick={handleAdd}
          disabled={saving || (!link.trim() && !externalId.trim())}
        >
          {#if saving}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>{/if}Save
        </button>
      </div>
    </div>
  {/if}

  {#if references.length === 0 && !formOpen}
    <p class="refs-empty">No references.</p>
  {:else}
    <ul class="refs-list">
      {#each references as ref (ref.id ?? `${ref.platform}-${ref.external_id}-${ref.external_url}`)}
        <li class="ref-item">
          <span class="ref-tag ref-type">{refTypeLabel(ref.ref_type)}</span>
          <span class="ref-tag ref-platform">{platformLabel(ref.platform)}</span>
          <span class="ref-target">
            {#if ref.external_url}
              <a href={ref.external_url} target="_blank" rel="noopener noreferrer" class="ref-link">
                <span class="ref-link-text">{ref.external_id ?? ref.external_url}</span>
                <i class="pxi pxi-external-link" aria-hidden="true"></i>
              </a>
            {:else if ref.external_id}
              <span class="ref-id">{ref.external_id}</span>
            {/if}
          </span>
          <button
            class="btn-delete-ref"
            onclick={() => handleDelete(ref)}
            title="Remove reference"
            aria-label="Remove reference"
          ><i class="pxi pxi-close" aria-hidden="true"></i></button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .refs-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 4px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }

  .refs-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .refs-title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--text-bright);
  }
  .refs-count {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 400;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }

  /* ── Form ─────────────────────────────────────────────────────────────── */
  .ref-form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    background: var(--surface);
  }

  .ref-form-row { display: flex; gap: 8px; }

  .ref-field-type { flex: 0 0 auto; min-width: 8rem; }
  .ref-field-link { flex: 2; }

  .ref-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    font-weight: 500;
    color: var(--muted);
    flex: 1;
    min-width: 0;
  }

  /* Selects have no global skin; match the global text inputs. */
  .ref-field select {
    padding: 10px 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text);
  }
  .ref-field input { background: var(--bg); }

  .btn-toggle-advanced {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    padding: 0;
    border: none;
    background: none;
    color: var(--muted);
    font-family: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .btn-toggle-advanced .pxi { font-size: 16px; }
  .btn-toggle-advanced:hover { color: var(--text-bright); }

  .ref-form-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  /* ── List: hairline rows ──────────────────────────────────────────────── */
  .refs-empty {
    margin: 0;
    font-size: 13px;
    color: var(--muted);
  }

  .refs-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .ref-item {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 4px 0;
    border-top: 1px solid var(--border-soft);
    font-size: 13px;
    min-width: 0;
  }
  .ref-item:last-child { border-bottom: 1px solid var(--border-soft); }

  .ref-tag {
    flex: 0 0 auto;
    padding: 1px 5px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.06em;
    line-height: 1.5;
    text-transform: uppercase;
    white-space: nowrap;
  }
  .ref-type { border: 1px solid var(--border-strong); color: var(--muted); }
  .ref-platform { border: 1px solid transparent; background: var(--surface-2); color: var(--text); }

  .ref-target { flex: 1; min-width: 0; display: flex; }

  .ref-link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    max-width: 100%;
    color: var(--accent);
    text-decoration: none;
  }
  .ref-link-text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .ref-link .pxi { flex: 0 0 auto; font-size: 16px; }
  .ref-link:hover .ref-link-text { text-decoration: underline; }

  .ref-id {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--muted);
  }

  .btn-delete-ref {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-chip);
    background: none;
    color: var(--muted-2);
    font-size: 16px;
    cursor: pointer;
  }
  .btn-delete-ref:hover { color: var(--error); background: var(--surface-2); }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .ref-form-row { flex-wrap: wrap; }
    .ref-field-type, .ref-field-link, .ref-field { flex: 1 1 100%; min-width: 0; }
    .ref-form-actions { flex-wrap: wrap; }
    .ref-item { flex-wrap: wrap; }
    .btn-delete-ref { width: 44px; height: 44px; margin-left: auto; }
    .ref-target { order: 3; flex-basis: 100%; }
    .ref-link { min-height: 44px; }
  }
</style>
