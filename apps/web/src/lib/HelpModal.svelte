<script lang="ts">
  interface Props {
    open: boolean;
    onClose: () => void;
  }
  let { open, onClose }: Props = $props();

  let dialogEl: HTMLDialogElement | undefined = $state(undefined);

  $effect(() => {
    if (!dialogEl) return;
    if (open) { if (!dialogEl.open) dialogEl.showModal(); }
    else { if (dialogEl.open) dialogEl.close(); }
  });
</script>

<dialog
  bind:this={dialogEl}
  onclose={onClose}
  onkeydown={(e) => { if (e.key === 'Escape') onClose(); }}
  class="help-dialog"
>
  <div class="dialog-header">
    <h3>Soundgnome help</h3>
    <button class="dialog-close" onclick={onClose} aria-label="Close">
      <i class="lni lni-xmark" aria-hidden="true"></i>
    </button>
  </div>

  <div class="dialog-body">

    <!-- ── Pages ────────────────────────────────────────────────────────── -->
    <section>
      <h4>Pages</h4>
      <table class="help-table">
        <tbody>
          <tr>
            <td class="page-name">Download</td>
            <td>Paste a track, album, or playlist URL. Result appears inline; tracks needing review are flagged.</td>
          </tr>
          <tr>
            <td class="page-name">Library</td>
            <td>Browse and edit artists, albums, tracks, and playlists. Drill into an artist or album to see its content. Merge duplicate artists via multi-select.</td>
          </tr>
          <tr>
            <td class="page-name">Validations</td>
            <td>Tracks whose metadata could not be matched automatically. Review, optionally edit, then approve or reject each one.</td>
          </tr>
          <tr>
            <td class="page-name">Tasks</td>
            <td>Background jobs (playlist syncs, downloads). Shows progress, status, and allows retry or cancellation.</td>
          </tr>
          <tr>
            <td class="page-name">Sync</td>
            <td>Scheduled playlist sync jobs. Add a URL with an interval; pause, resume, or trigger a sync manually.</td>
          </tr>
        </tbody>
      </table>
    </section>

    <!-- ── Keyboard shortcuts ───────────────────────────────────────────── -->
    <section>
      <h4>Keyboard shortcuts</h4>

      <h5>Library</h5>
      <table class="shortcut-table">
        <tbody>
          <tr><td><kbd>S</kbd></td><td>Focus the search field</td></tr>
          <tr><td><kbd>E</kbd></td><td>Edit the item under the cursor</td></tr>
          <tr><td><kbd>Backspace</kbd></td><td>Go up one level (album to artist to list)</td></tr>
          <tr><td><kbd>Shift</kbd> + click</td><td>Select an artist or album for merge</td></tr>
          <tr><td><kbd>M</kbd></td><td>Start merge (requires 2 or more artists or albums selected)</td></tr>
          <tr><td><kbd>Esc</kbd></td><td>Cancel merge / clear selection</td></tr>
        </tbody>
      </table>

      <h5>Validations</h5>
      <table class="shortcut-table">
        <tbody>
          <tr><td><kbd>E</kbd></td><td>Open inline edit for the hovered card</td></tr>
          <tr><td><kbd>Esc</kbd></td><td>Close the inline edit form</td></tr>
        </tbody>
      </table>

      <h5>Edit modal</h5>
      <table class="shortcut-table">
        <tbody>
          <tr><td><kbd>Enter</kbd></td><td>Save changes</td></tr>
          <tr><td><kbd>Esc</kbd></td><td>Cancel without saving</td></tr>
        </tbody>
      </table>

      <h5>Global</h5>
      <table class="shortcut-table">
        <tbody>
          <tr><td><kbd>Space</kbd></td><td>Play / pause the current track</td></tr>
          <tr><td><kbd>?</kbd></td><td>Open / close this help panel</td></tr>
        </tbody>
      </table>
    </section>

    <!-- ── Tips ─────────────────────────────────────────────────────────── -->
    <section>
      <h4>Tips</h4>
      <ul class="tips-list">
        <li>Playlist URLs queue a background sync task. Follow progress in <strong>Tasks</strong>.</li>
        <li>Tracks marked <span class="badge-warn">review</span> in the recent-downloads list are waiting in <strong>Validations</strong>.</li>
        <li>In the Validations page, <strong>Show matches</strong> fetches alternative metadata candidates when the reason is a partial match.</li>
        <li>In Library, under Artists or Albums, the <strong>Similar</strong> filter highlights items whose names or titles are close. This is useful for spotting duplicates before merging.</li>
        <li>Full API docs are available at <a href="/swagger" target="_blank" rel="noopener noreferrer">/swagger</a>.</li>
      </ul>
    </section>

  </div>
</dialog>

<style>
  .help-dialog {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 0;
    width: min(560px, 94vw);
    max-height: 88vh;
    overflow-y: auto;
    color: var(--text);
    font-family: var(--font-body);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.55);
  }

  .help-dialog::backdrop {
    background: color-mix(in srgb, #000 70%, transparent);
    backdrop-filter: blur(2px);
  }

  .help-dialog[open] {
    animation: help-in 160ms ease-out;
  }

  .help-dialog[open]::backdrop {
    animation: help-backdrop-in 160ms ease-out;
  }

  @keyframes help-in {
    from { opacity: 0; transform: translateY(6px) scale(0.98); }
    to { opacity: 1; transform: none; }
  }

  @keyframes help-backdrop-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @media (prefers-reduced-motion: reduce) {
    .help-dialog[open],
    .help-dialog[open]::backdrop {
      animation: none;
    }
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1.25rem 1.5rem 1rem;
    border-bottom: 1px solid var(--border-soft);
    position: sticky;
    top: 0;
    background: var(--panel);
    z-index: 1;
  }

  .dialog-header h3 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--text-bright);
  }

  .dialog-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--muted);
    font-size: 1.05rem;
    line-height: 1;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease;
  }

  .dialog-close:hover {
    background: var(--surface-2);
    color: var(--text-bright);
  }

  @media (prefers-reduced-motion: reduce) {
    .dialog-close { transition: none; }
  }

  .dialog-body {
    padding: 1.25rem 1.5rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.75rem;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  h4 {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--muted-2);
    margin: 0;
  }

  h5 {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text);
    margin: 0.5rem 0 0.1rem;
  }

  /* Pages table */
  .help-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }

  .help-table td {
    padding: 0.5rem 0.65rem;
    vertical-align: top;
    border-bottom: 1px solid var(--border-soft);
    color: var(--muted);
    line-height: 1.5;
  }

  .help-table tr:last-child td {
    border-bottom: none;
  }

  .page-name {
    font-weight: 600;
    white-space: nowrap;
    width: 1%;
    padding-right: 1.25rem;
    color: var(--text-bright);
  }

  /* Shortcut table */
  .shortcut-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }

  .shortcut-table td {
    padding: 0.35rem 0.4rem;
    vertical-align: middle;
    color: var(--muted);
  }

  .shortcut-table td:first-child {
    white-space: nowrap;
    width: 1%;
    padding-right: 1.5rem;
  }

  kbd {
    display: inline-block;
    padding: 0.15rem 0.45rem;
    background: var(--surface-2);
    border: 1px solid var(--border-soft);
    border-radius: 5px;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.4;
    color: var(--text);
  }

  /* Tips list */
  .tips-list {
    margin: 0;
    padding: 0 0 0 1.2rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    font-size: 0.875rem;
    color: var(--muted);
  }

  .tips-list li {
    line-height: 1.55;
  }

  .tips-list strong {
    color: var(--text);
    font-weight: 600;
  }

  .tips-list a {
    color: var(--accent-2);
    text-decoration: none;
  }

  .tips-list a:hover {
    text-decoration: underline;
  }

  .badge-warn {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    padding: 0.1rem 0.4rem;
    background: var(--warning-bg);
    border: 1px solid color-mix(in srgb, var(--warning) 45%, transparent);
    color: var(--warning);
    border-radius: 5px;
  }
</style>
