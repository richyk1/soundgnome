<script lang="ts">
  import { lib } from './store.svelte';
  import ReferencesPanel from './ReferencesPanel.svelte';
  import ArtistMultiSelect from './ArtistMultiSelect.svelte';

  let dialogEl: HTMLDialogElement | undefined = $state(undefined);

  function handleFileInput(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (file) lib.uploadImage(file);
    input.value = '';
  }

  function onDropZoneDrop(e: DragEvent) {
    e.preventDefault();
    const file = e.dataTransfer?.files?.[0];
    if (file) lib.uploadImage(file);
  }

  function onDropZoneDragOver(e: DragEvent) {
    e.preventDefault();
  }

  $effect(() => {
    if (!dialogEl) return;
    if (lib.editState) { if (!dialogEl.open) dialogEl.showModal(); }
    else { if (dialogEl.open) dialogEl.close(); }
  });
</script>

<dialog
  bind:this={dialogEl}
  onclose={() => (lib.editState = null)}
  onkeydown={(e) => {
    if (e.key === 'Enter' && !lib.editSaving) {
      const t = e.target as HTMLElement;
      if (t.tagName !== 'BUTTON') { e.preventDefault(); lib.saveEdit(); }
    }
  }}
  class="edit-dialog modal-content"
>
  {#if lib.editState}
    <div class="dialog-header">
      <h3>
        {lib.editState.type === 'track' ? 'Edit Track'
          : lib.editState.type === 'album' ? 'Edit Album' : 'Edit Artist'}
      </h3>
      <button class="dialog-close" onclick={() => (lib.editState = null)} aria-label="Close"><i class="pxi pxi-close" aria-hidden="true"></i></button>
    </div>

    {#if lib.editState.type === 'track'}
      <div class="dialog-body">
        <div class="image-section">
          <h4 class="field-heading">Cover</h4>
          <div
            class="image-drop-zone"
            class:uploading={lib.imageUploading}
            ondrop={onDropZoneDrop}
            ondragover={onDropZoneDragOver}
            role="button"
            tabindex="0"
            aria-label="Upload cover image"
          >
            {#if lib.imageUploading}
              <i class="pxi pxi-loader pxi-spin upload-loader" aria-hidden="true"></i>
            {:else if lib.editState.item.cover}
              <img src={lib.editState.item.cover} alt="cover" class="image-preview" />
              <span class="image-overlay">Change</span>
            {:else}
              <span class="upload-placeholder">
                <i class="pxi pxi-upload" aria-hidden="true"></i>
                <span>Upload image</span>
                <span class="drop-hint">or drop here</span>
              </span>
            {/if}
            <input
              type="file"
              accept="image/jpeg,image/png,image/webp,image/gif"
              class="file-input"
              onchange={handleFileInput}
              disabled={lib.imageUploading}
            />
          </div>
        </div>
        <label class="field-label">Title
          <input type="text" value={lib.trackDraft.title ?? ''}
            oninput={(e) => { lib.trackDraft.title = (e.currentTarget as HTMLInputElement).value; }} />
        </label>
        <label class="field-label">Artists
          <ArtistMultiSelect
            value={lib.trackDraft.artists ?? []}
            onChange={(names) => { lib.trackDraft.artists = names; }}
          />
        </label>
        <button
          type="button"
          class="btn-secondary btn-sm ai-clean-btn"
          onclick={() => lib.aiCleanTrack()}
          disabled={lib.aiCleaning}
          title="Use AI to clean up the title and extract the real artists"
        >
          {#if lib.aiCleaning}
            <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Cleaning…
          {:else}
            Clean title & artists with AI
          {/if}
        </button>
        <label class="field-label">Album
          <input type="text" value={lib.trackDraft.album_title ?? ''}
            oninput={(e) => { lib.trackDraft.album_title = (e.currentTarget as HTMLInputElement).value || undefined; }}
            placeholder="Album" />
        </label>
        <div class="field-row">
          <label class="field-label half">Genre
            <input type="text" value={lib.trackDraft.genre ?? ''}
              oninput={(e) => { lib.trackDraft.genre = (e.currentTarget as HTMLInputElement).value || undefined; }}
              placeholder="Genre" />
          </label>
          <label class="field-label half">Date
            <input type="text" value={lib.trackDraft.date ?? ''}
              oninput={(e) => { lib.trackDraft.date = (e.currentTarget as HTMLInputElement).value || undefined; }}
              placeholder="YYYY-MM-DD" />
          </label>
        </div>
        <div class="field-row">
          <label class="field-label third">Track #
            <input type="number" value={lib.trackDraft.track_number ?? ''}
              oninput={(e) => { const v = (e.currentTarget as HTMLInputElement).valueAsNumber; lib.trackDraft.track_number = isNaN(v) ? undefined : v; }} />
          </label>
          <label class="field-label third">Disc #
            <input type="number" value={lib.trackDraft.disc_number ?? ''}
              oninput={(e) => { const v = (e.currentTarget as HTMLInputElement).valueAsNumber; lib.trackDraft.disc_number = isNaN(v) ? undefined : v; }} />
          </label>
          <label class="field-label third">Label
            <input type="text" value={lib.trackDraft.label ?? ''}
              oninput={(e) => { lib.trackDraft.label = (e.currentTarget as HTMLInputElement).value || undefined; }}
              placeholder="Label" />
          </label>
        </div>
        <ReferencesPanel
          references={lib.editState.item.references}
          onAdd={(body) => lib.addReference('tracks', lib.editState!.item.id, body)}
          onDelete={(ref) => lib.deleteReference('tracks', lib.editState!.item.id, ref)}
        />
      </div>

    {:else if lib.editState.type === 'album'}
      <div class="dialog-body">
        <div class="image-section">
          <h4 class="field-heading">Cover</h4>
          <div class="image-row">
            <div
              class="image-drop-zone"
              class:uploading={lib.imageUploading}
              ondrop={onDropZoneDrop}
              ondragover={onDropZoneDragOver}
              role="button"
              tabindex="0"
              aria-label="Upload cover image"
            >
              {#if lib.imageUploading}
                <i class="pxi pxi-loader pxi-spin upload-loader" aria-hidden="true"></i>
              {:else if lib.albumDraft.cover}
                <img src={lib.albumDraft.cover} alt="cover" class="image-preview" />
                <span class="image-overlay">Change</span>
              {:else}
                <span class="upload-placeholder">
                  <i class="pxi pxi-upload" aria-hidden="true"></i>
                  <span>Upload</span>
                  <span class="drop-hint">or drop</span>
                </span>
              {/if}
              <input
                type="file"
                accept="image/jpeg,image/png,image/webp,image/gif"
                class="file-input"
                onchange={handleFileInput}
                disabled={lib.imageUploading || lib.thumbnailFetching}
              />
            </div>
            <label class="field-label url-field">
              Image URL
              <div class="url-input-row">
                <input
                  type="url"
                  value={lib.albumDraft.cover ?? ''}
                  oninput={(e) => { lib.albumDraft.cover = (e.currentTarget as HTMLInputElement).value || undefined; }}
                  placeholder="https://…"
                />
                <button
                  type="button"
                  class="btn-secondary btn-fetch-thumbnail"
                  onclick={() => lib.fetchThumbnailFromReferences()}
                  disabled={lib.imageUploading || lib.thumbnailFetching || lib.editState.item.references.length === 0}
                  title="Fetch cover from this album's references (Spotify, SoundCloud, YouTube Music)"
                >
                  {#if lib.thumbnailFetching}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>{:else}<i class="pxi pxi-download" aria-hidden="true"></i>{/if}
                  From references
                </button>
                <button
                  type="button"
                  class="btn-ghost btn-clear-thumbnail"
                  onclick={() => { lib.albumDraft.cover = undefined; }}
                  disabled={lib.imageUploading || lib.thumbnailFetching || !lib.albumDraft.cover}
                  title="Remove cover image"
                  aria-label="Remove cover image"
                >
                  <i class="pxi pxi-close" aria-hidden="true"></i>
                </button>
              </div>
            </label>
          </div>
        </div>
        <label class="field-label">Title
          <input type="text" value={lib.albumDraft.title ?? ''}
            oninput={(e) => { lib.albumDraft.title = (e.currentTarget as HTMLInputElement).value; }} />
        </label>
        <label class="field-label">Date
          <input type="text" value={lib.albumDraft.date ?? ''}
            oninput={(e) => { lib.albumDraft.date = (e.currentTarget as HTMLInputElement).value || undefined; }}
            placeholder="YYYY-MM-DD" />
        </label>
        <ReferencesPanel
          references={lib.editState.item.references}
          onAdd={(body) => lib.addReference('albums', lib.editState!.item.id, body)}
          onDelete={(ref) => lib.deleteReference('albums', lib.editState!.item.id, ref)}
        />
      </div>

    {:else}
      <div class="dialog-body">
        <div class="image-section">
          <h4 class="field-heading">Photo</h4>
          <div class="image-row">
            <div
              class="image-drop-zone image-drop-zone--round"
              class:uploading={lib.imageUploading}
              ondrop={onDropZoneDrop}
              ondragover={onDropZoneDragOver}
              role="button"
              tabindex="0"
              aria-label="Upload artist photo"
            >
              {#if lib.imageUploading}
                <i class="pxi pxi-loader pxi-spin upload-loader" aria-hidden="true"></i>
              {:else if lib.artistDraft.icon}
                <img src={lib.artistDraft.icon} alt="artist" class="image-preview" />
                <span class="image-overlay">Change</span>
              {:else}
                <span class="upload-placeholder">
                  <i class="pxi pxi-upload" aria-hidden="true"></i>
                  <span>Upload</span>
                  <span class="drop-hint">or drop</span>
                </span>
              {/if}
              <input
                type="file"
                accept="image/jpeg,image/png,image/webp,image/gif"
                class="file-input"
                onchange={handleFileInput}
                disabled={lib.imageUploading || lib.thumbnailFetching}
              />
            </div>
            <label class="field-label url-field">
              Photo URL
              <div class="url-input-row">
                <input
                  type="url"
                  value={lib.artistDraft.icon ?? ''}
                  oninput={(e) => { lib.artistDraft.icon = (e.currentTarget as HTMLInputElement).value || undefined; }}
                  placeholder="https://…"
                />
                <button
                  type="button"
                  class="btn-secondary btn-fetch-thumbnail"
                  onclick={() => lib.fetchThumbnailFromReferences()}
                  disabled={lib.imageUploading || lib.thumbnailFetching || lib.editState.item.references.length === 0}
                  title="Fetch photo from this artist's references (Spotify, SoundCloud, YouTube Music)"
                >
                  {#if lib.thumbnailFetching}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>{:else}<i class="pxi pxi-download" aria-hidden="true"></i>{/if}
                  From references
                </button>
                <button
                  type="button"
                  class="btn-ghost btn-clear-thumbnail"
                  onclick={() => { lib.artistDraft.icon = undefined; }}
                  disabled={lib.imageUploading || lib.thumbnailFetching || !lib.artistDraft.icon}
                  title="Remove photo"
                  aria-label="Remove photo"
                >
                  <i class="pxi pxi-close" aria-hidden="true"></i>
                </button>
              </div>
            </label>
          </div>
        </div>
        <label class="field-label">Name
          <input type="text" value={lib.artistDraft.name ?? ''}
            oninput={(e) => { lib.artistDraft.name = (e.currentTarget as HTMLInputElement).value; }} />
        </label>
        <ReferencesPanel
          references={lib.editState.item.references}
          onAdd={(body) => lib.addReference('artists', lib.editState!.item.id, body)}
          onDelete={(ref) => lib.deleteReference('artists', lib.editState!.item.id, ref)}
        />
      </div>
    {/if}

    <div class="dialog-footer">
      <span class="kbd-hint"><kbd>Enter</kbd> to save · <kbd>Esc</kbd> to cancel</span>
      <button class="btn-cancel" onclick={() => (lib.editState = null)}>Cancel</button>
      <button class="btn-save" onclick={lib.saveEdit} disabled={lib.editSaving}>
        {#if lib.editSaving}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>{/if}Save
      </button>
    </div>
  {/if}
</dialog>

<style>
  .edit-dialog {
    padding: 0;
    width: min(540px, 92vw);
    color: var(--text);
    font-family: inherit;
  }
  .edit-dialog::backdrop { background: var(--overlay); }
  .dialog-header {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 14px 16px 12px 24px; border-bottom: 1px solid var(--border);
  }
  .dialog-header h3 { margin: 0; font-size: 18px; font-weight: 600; letter-spacing: -0.02em; }
  .dialog-close {
    display: grid; place-items: center; width: 36px; height: 36px; padding: 0;
    border: 1px solid transparent; border-radius: var(--radius-control);
    background: none; color: var(--muted); font-size: 16px; cursor: pointer;
  }
  .dialog-close:hover { background: var(--surface-2); color: var(--text-bright); }
  .dialog-body { padding: 20px 24px; display: flex; flex-direction: column; gap: 14px; }
  .field-label { display: flex; flex-direction: column; gap: 6px; font-size: 13px; color: var(--muted); font-weight: 500; }
  .field-row { display: flex; gap: 12px; }
  .field-label.half { flex: 1; }
  .field-label.third { flex: 1; min-width: 0; }
  .dialog-footer {
    display: flex; align-items: center; gap: 8px;
    padding: 14px 24px 16px; border-top: 1px solid var(--border);
  }
  .kbd-hint { margin-right: auto; font-size: 12px; color: var(--muted-2); }
  .kbd-hint kbd {
    padding: 1px 5px; border: 1px solid var(--border); border-radius: 4px;
    font-family: var(--font-mono); font-size: 11px; color: var(--muted);
  }

  /* ── Image upload ─────────────────────────────────────────────────────── */
  .image-section { display: flex; flex-direction: column; gap: 8px; }
  .field-heading { margin: 0; font-size: 13px; font-weight: 600; letter-spacing: -0.01em; color: var(--text-bright); }

  .image-row { display: flex; align-items: flex-start; gap: 12px; }
  .url-field { flex: 1; min-width: 0; }

  .url-input-row { display: flex; gap: 6px; }
  .url-input-row input { flex: 1; min-width: 0; }
  .btn-fetch-thumbnail { flex-shrink: 0; min-height: 42px; }
  .btn-clear-thumbnail { flex-shrink: 0; min-height: 42px; width: 42px; padding: 0; }

  .image-drop-zone {
    position: relative; width: 100px; height: 100px;
    border: 1px dashed var(--border-heavy); border-radius: var(--radius-control);
    background: var(--surface); cursor: pointer; overflow: hidden;
    display: flex; align-items: center; justify-content: center;
    transition: border-color var(--motion-fast) var(--ease-out);
  }
  .image-drop-zone:hover { border-color: var(--accent); }
  .image-drop-zone.uploading { cursor: wait; }
  .image-drop-zone--round { border-radius: 50%; }

  .image-preview { width: 100%; height: 100%; object-fit: cover; display: block; }

  .image-overlay {
    position: absolute; inset: 0;
    background: color-mix(in srgb, var(--bg) 72%, transparent); color: var(--text-bright);
    font-size: 12px; font-weight: 600;
    display: flex; align-items: center; justify-content: center;
    opacity: 0; transition: opacity var(--motion-fast) var(--ease-out);
  }
  .image-drop-zone:hover .image-overlay { opacity: 1; }

  .upload-placeholder {
    display: flex; flex-direction: column; align-items: center; gap: 4px;
    color: var(--muted); font-size: 12px; text-align: center; padding: 8px;
    pointer-events: none;
  }
  .upload-placeholder .pxi { font-size: 24px; color: var(--muted-2); margin-bottom: 2px; }
  .drop-hint { font-size: 11px; color: var(--muted-2); }
  .upload-loader { font-size: 24px; color: var(--live); }

  .file-input {
    position: absolute; inset: 0; opacity: 0; cursor: pointer; width: 100%; height: 100%;
  }
  .file-input:disabled { cursor: wait; }

  .ai-clean-btn { align-self: flex-start; margin: -4px 0 2px; }

  .edit-dialog { position: fixed; top: calc(var(--app-top, 0px) + 1rem); bottom: auto; margin: 0 auto; max-height: calc(var(--app-height, 100dvh) - 2rem); overflow-y: auto; box-sizing: border-box; overscroll-behavior: contain; }
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .dialog-header, .dialog-body, .dialog-footer { padding-left: 16px; padding-right: 16px; }
    .dialog-header { padding-right: 8px; }
    .dialog-close { width: 44px; height: 44px; flex-shrink: 0; }
    .field-row { flex-wrap: wrap; }
    .field-label.half, .field-label.third { flex: 1 1 120px; min-width: 0; }
    .dialog-footer { flex-wrap: wrap; }
    .kbd-hint { flex-basis: 100%; }
    .image-row { flex-wrap: wrap; }
    .url-field { flex-basis: 100%; }
    .url-input-row { flex-wrap: wrap; }
    .url-input-row input { flex-basis: 100%; }
    .btn-fetch-thumbnail, .btn-clear-thumbnail { min-height: 44px; }
    .btn-clear-thumbnail { width: 44px; }
  }
</style>
