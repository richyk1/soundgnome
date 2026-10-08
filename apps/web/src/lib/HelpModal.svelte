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
    <h2>Soundgnome help</h2>
    <button class="dialog-close" onclick={onClose} aria-label="Close">
      <i class="pxi pxi-close" aria-hidden="true"></i>
    </button>
  </div>

  <div class="dialog-body">

    <!-- ── Pages ────────────────────────────────────────────────────────── -->
    <section>
      <h3>Pages</h3>
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
      <h3>Keyboard shortcuts</h3>

      <h4>Library</h4>
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

      <h4>Validations</h4>
      <table class="shortcut-table">
        <tbody>
          <tr><td><kbd>E</kbd></td><td>Open inline edit for the hovered card</td></tr>
          <tr><td><kbd>Esc</kbd></td><td>Close the inline edit form</td></tr>
        </tbody>
      </table>

      <h4>Edit modal</h4>
      <table class="shortcut-table">
        <tbody>
          <tr><td><kbd>Enter</kbd></td><td>Save changes</td></tr>
          <tr><td><kbd>Esc</kbd></td><td>Cancel without saving</td></tr>
        </tbody>
      </table>

      <h4>Global</h4>
      <table class="shortcut-table">
        <tbody>
          <tr><td><kbd>Space</kbd></td><td>Play / pause the current track</td></tr>
          <tr><td><kbd>?</kbd></td><td>Open / close this help panel</td></tr>
        </tbody>
      </table>
    </section>

    <!-- ── Tips ─────────────────────────────────────────────────────────── -->
    <section>
      <h3>Tips</h3>
      <ul class="tips-list">
        <li>Playlist URLs queue a background sync task. Follow progress in <strong>Tasks</strong>.</li>
        <li>Tracks marked <span class="tag-review">review</span> in the recent-downloads list are waiting in <strong>Validations</strong>.</li>
        <li>In the Validations page, <strong>Show matches</strong> fetches alternative metadata candidates when the reason is a partial match.</li>
        <li>In Library, under Artists or Albums, the <strong>Similar</strong> filter highlights items whose names or titles are close. This is useful for spotting duplicates before merging.</li>
        <li>Full API docs are available at <a href="/swagger" target="_blank" rel="noopener noreferrer">/swagger</a>.</li>
      </ul>
    </section>

  </div>
</dialog>

<style>
  /* Float panel. Global CSS skips its default dialog animation for .help-dialog. */
  .help-dialog {
    position: fixed;
    top: calc(var(--app-top, 0px) + 16px);
    bottom: auto;
    margin: 0 auto;
    width: min(560px, calc(100vw - 32px));
    max-height: calc(var(--app-height, 100dvh) - 32px);
    padding: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    box-sizing: border-box;
    background: var(--float);
    border: 1px solid var(--float-border);
    border-radius: var(--radius-panel);
    box-shadow: var(--float-shadow);
    color: var(--text);
    font-family: var(--font-body);
  }
  .help-dialog::backdrop { background: var(--overlay); }
  .help-dialog[open] { animation: help-in var(--motion-normal) var(--ease-out); }
  .help-dialog[open]::backdrop { animation: help-backdrop-in var(--motion-fast) var(--ease-out); }
  @keyframes help-in {
    from { opacity: 0; translate: 0 8px; }
    to { opacity: 1; translate: 0 0; }
  }
  @keyframes help-backdrop-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @media (prefers-reduced-motion: reduce) {
    .help-dialog[open],
    .help-dialog[open]::backdrop { animation: none; }
  }

  .dialog-header {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 16px 16px 14px 24px;
    background: var(--float);
    border-bottom: 1px solid var(--border);
  }
  .dialog-header h2 {
    margin: 0;
    font-size: 18px;
    line-height: 1.3;
  }
  .dialog-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background: none;
    color: var(--muted);
    font-size: 16px;
    cursor: pointer;
  }
  .dialog-close:hover { background: var(--surface-2); color: var(--text-bright); }
  .dialog-close:active { border-color: var(--border-heavy); }

  .dialog-body {
    display: flex;
    flex-direction: column;
    gap: 28px;
    padding: 20px 24px 24px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    line-height: 1.35;
  }
  h4 {
    margin: 12px 0 0;
    font-family: var(--font-body);
    font-size: 13px;
    font-weight: 500;
    letter-spacing: 0;
    color: var(--muted-2);
  }

  /* Hairline rows */
  .help-table,
  .shortcut-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
    line-height: 1.45;
  }
  .help-table td,
  .shortcut-table td {
    padding: 10px 0;
    vertical-align: top;
    border-bottom: 1px solid var(--border-soft);
    color: var(--muted);
  }
  .help-table tr:last-child td,
  .shortcut-table tr:last-child td { border-bottom: none; }
  .shortcut-table td { vertical-align: middle; padding: 8px 0; }
  .page-name,
  .shortcut-table td:first-child {
    width: 1%;
    padding-right: 20px;
    white-space: nowrap;
  }
  .page-name { font-weight: 500; color: var(--text-bright); }
  .shortcut-table td:first-child { color: var(--muted-2); font-size: 13px; }

  kbd {
    display: inline-block;
    min-width: 22px;
    padding: 1px 6px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    background: var(--surface);
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 18px;
    text-align: center;
  }

  /* Tips */
  .tips-list {
    margin: 0;
    padding: 0 0 0 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }
  .tips-list li::marker { color: var(--muted-2); }
  .tips-list strong { font-weight: 600; color: var(--text); }
  .tips-list a { color: var(--accent); }
  .tips-list a:hover { color: var(--accent-2); }
  .tag-review {
    padding: 1px 5px;
    border: 1px solid color-mix(in srgb, var(--warning) 45%, transparent);
    border-radius: 4px;
    background: var(--warning-bg);
    color: var(--warning);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .dialog-header { padding: 12px 12px 12px 16px; }
    .dialog-body { padding: 16px; }
    .dialog-close { width: 44px; height: 44px; font-size: 24px; }
    .page-name { white-space: normal; }
    .shortcut-table td:first-child { white-space: normal; padding-right: 12px; }
  }
</style>
