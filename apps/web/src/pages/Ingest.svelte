<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    listIngestFiles,
    ingestFile,
    ingestAll,
    getTasks,
    type IngestFileEntry,
    type IngestFilesResponse,
    type IngestResult,
  } from '../lib/api';
  import type { TaskDto } from '../lib/types';
  import { uploadManager } from '../lib/uploadStore.svelte';

  // ── State ──────────────────────────────────────────────────────────────────

  let response: IngestFilesResponse | null = $state(null);
  let loadingFiles = $state(true);
  let filesError: string | null = $state(null);

  // Which file row is expanded (by absolute path)
  let expandedPath: string | null = $state(null);

  // Per-file ingest state
  let ingestingFile: string | null = $state(null);
  let fileResults: Record<string, { ok: boolean; message: string }> = $state({});

  // Batch ingest state
  let batchTaskId: number | null = $state(null);
  let batchTask: TaskDto | null = $state(null);
  let batchError: string | null = $state(null);
  let batchIngesting = $state(false);

  // Poll interval for task progress
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  // Scheduled auto-ingest
  let pollHours = $state(0);
  let pollMinutes = $state(30);
  let pollEnabled = $state(false);
  let pollTimerId: ReturnType<typeof setInterval> | null = null;
  let pollMsg: string | null = $state(null);

  // ── Files ──────────────────────────────────────────────────────────────────

  async function loadFiles() {
    loadingFiles = true;
    filesError = null;
    try {
      response = await listIngestFiles();
    } catch (e: unknown) {
      filesError = e instanceof Error ? e.message : String(e);
    } finally {
      loadingFiles = false;
    }
  }

  onMount(loadFiles);

  function toggleExpand(path: string) {
    expandedPath = expandedPath === path ? null : path;
  }

  // ── Single-file ingest ─────────────────────────────────────────────────────

  async function handleIngestFile(e: Event, file: IngestFileEntry) {
    e.stopPropagation(); // don't toggle the expand panel
    if (ingestingFile) return;
    ingestingFile = file.path;
    const prev = { ...fileResults };
    delete prev[file.path];
    fileResults = prev;

    try {
      const result: IngestResult = await ingestFile(file.path);
      fileResults = {
        ...fileResults,
        [file.path]: {
          ok: true,
          message: result.needs_validation
            ? `Staged for validation — "${result.title}"`
            : `Ingested — "${result.title}"`,
        },
      };
      await loadFiles();
    } catch (err: unknown) {
      fileResults = {
        ...fileResults,
        [file.path]: {
          ok: false,
          message: err instanceof Error ? err.message : String(err),
        },
      };
    } finally {
      ingestingFile = null;
    }
  }

  // ── Batch ingest ───────────────────────────────────────────────────────────

  async function handleIngestAll() {
    if (batchIngesting) return;
    batchIngesting = true;
    batchError = null;
    batchTask = null;
    batchTaskId = null;
    stopTaskPoll();

    try {
      const res = await ingestAll();
      batchTaskId = res.task_id;
      startTaskPoll();
      await loadFiles();
    } catch (e: unknown) {
      batchError = e instanceof Error ? e.message : String(e);
      batchIngesting = false;
    }
  }

  function startTaskPoll() {
    pollInterval = setInterval(async () => {
      if (batchTaskId === null) return;
      try {
        const tasks = await getTasks();
        const t = tasks.find((x) => x.id === batchTaskId) ?? null;
        batchTask = t;
        if (t && (t.status === 'Completed' || t.status === 'Failed' || t.status === 'Cancelled')) {
          stopTaskPoll();
          batchIngesting = false;
          await loadFiles();
        }
      } catch {
        // ignore transient poll errors
      }
    }, 1500);
  }

  function stopTaskPoll() {
    if (pollInterval !== null) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  // ── Scheduled auto-ingest ──────────────────────────────────────────────────

  function applyPollSchedule() {
    stopSchedule();
    const ms = (pollHours * 3600 + pollMinutes * 60) * 1000;
    if (!pollEnabled || ms <= 0) return;
    pollTimerId = setInterval(async () => {
      try {
        const res = await ingestAll();
        pollMsg = `Auto-ingest started (task #${res.task_id}) at ${new Date().toLocaleTimeString()}`;
        batchTaskId = res.task_id;
        batchIngesting = true;
        startTaskPoll();
        await loadFiles();
      } catch (e: unknown) {
        pollMsg = `Auto-ingest failed: ${e instanceof Error ? e.message : String(e)}`;
      }
    }, ms);
    pollMsg = `Auto-ingest scheduled every ${formatDuration(ms)}.`;
  }

  function stopSchedule() {
    if (pollTimerId !== null) {
      clearInterval(pollTimerId);
      pollTimerId = null;
    }
  }

  onDestroy(() => {
    stopTaskPoll();
    stopSchedule();
  });

  // ── Helpers ────────────────────────────────────────────────────────────────

  function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
    if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
    return `${(n / 1024 ** 3).toFixed(2)} GB`;
  }

  function formatDuration(ms: number): string {
    const s = ms / 1000;
    if (s < 60) return `${s}s`;
    if (s < 3600) return `${Math.floor(s / 60)}m`;
    return `${(s / 3600).toFixed(1)}h`;
  }

  function formatSeconds(s: number): string {
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m}:${sec.toString().padStart(2, '0')}`;
  }

  function taskProgress(t: TaskDto): number {
    if (!t.total || t.total === 0) return 0;
    return Math.round((t.progress / t.total) * 100);
  }

  // ── Browser upload ──────────────────────────────────────────────────────────
  type DropEntry = { file: File; relativePath: string };

  const up = uploadManager;
  let fileInput: HTMLInputElement;
  let folderInput: HTMLInputElement;
  let dragOver = $state(false);
  let pickNote: string | null = $state(null);

  function summarizePick(r: { added: number; skippedNonAudio: number; skippedDuplicate: number }) {
    if (r.added === 0 && r.skippedNonAudio === 0 && r.skippedDuplicate === 0) {
      pickNote = null;
      return;
    }
    const parts: string[] = [];
    if (r.added > 0) parts.push(`Added ${r.added} song${r.added === 1 ? '' : 's'}`);
    if (r.skippedNonAudio > 0)
      parts.push(`skipped ${r.skippedNonAudio} non-audio file${r.skippedNonAudio === 1 ? '' : 's'}`);
    if (r.skippedDuplicate > 0) parts.push(`${r.skippedDuplicate} already queued`);
    pickNote = r.added === 0 ? `No songs added — ${parts.join(', ')}.` : `${parts.join(' · ')}.`;
  }

  function pickedFiles(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const entries: DropEntry[] = Array.from(input.files ?? []).map((file) => ({
      file,
      relativePath: (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name,
    }));
    summarizePick(uploadManager.addFiles(entries));
    input.value = '';
  }

  async function droppedFiles(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
    if (!e.dataTransfer) return;
    summarizePick(uploadManager.addFiles(await readDataTransfer(e.dataTransfer)));
  }

  async function readDataTransfer(dt: DataTransfer): Promise<DropEntry[]> {
    const roots = Array.from(dt.items)
      .filter((it) => it.kind === 'file')
      .map((it) => it.webkitGetAsEntry?.())
      .filter((entry): entry is FileSystemEntry => !!entry);
    if (roots.length === 0) {
      return Array.from(dt.files).map((file) => ({ file, relativePath: file.name }));
    }
    const out: DropEntry[] = [];
    await Promise.all(roots.map((entry) => walkEntry(entry, '', out)));
    return out;
  }

  // Recurse a dropped folder tree. readEntries returns in chunks, so keep reading
  // until it yields an empty batch.
  function walkEntry(entry: FileSystemEntry, prefix: string, out: DropEntry[]): Promise<void> {
    return new Promise((resolve) => {
      if (entry.isFile) {
        (entry as FileSystemFileEntry).file(
          (file) => {
            out.push({ file, relativePath: prefix + file.name });
            resolve();
          },
          () => resolve(),
        );
      } else if (entry.isDirectory) {
        const reader = (entry as FileSystemDirectoryEntry).createReader();
        const dirPrefix = `${prefix}${entry.name}/`;
        const readBatch = () => {
          reader.readEntries(
            (batch) => {
              if (batch.length === 0) {
                resolve();
                return;
              }
              Promise.all(batch.map((child) => walkEntry(child, dirPrefix, out))).then(readBatch);
            },
            () => resolve(),
          );
        };
        readBatch();
      } else {
        resolve();
      }
    });
  }
</script>

<div class="ingest-page">
  <header class="page-header">
    <div class="header-text">
      <h1>Ingest</h1>
      <p class="lede">
        Upload songs or whole folders from your device, or ingest files already in the server's
        ingest directory. Duplicates are detected and sorted from new tracks automatically.
      </p>
    </div>
    <div class="header-actions">
      <button class="btn-ghost" onclick={loadFiles} disabled={loadingFiles}>
        {#if loadingFiles}<span class="spinner"></span>{/if}Refresh
      </button>
      <button class="btn-accent" disabled={batchIngesting || loadingFiles} onclick={handleIngestAll}>
        {#if batchIngesting}
          <span class="spinner"></span>Ingesting
        {:else}
          <i class="lni lni-cloud-upload" aria-hidden="true"></i>Ingest all
        {/if}
      </button>
    </div>
  </header>

  <!-- ── Browser upload ─────────────────────────────────────────────────────── -->
  <section class="upload">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="dropzone"
      class:drag={dragOver}
      role="button"
      tabindex="0"
      onclick={() => fileInput.click()}
      onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), fileInput.click())}
      ondragover={(e) => {
        e.preventDefault();
        dragOver = true;
      }}
      ondragleave={() => (dragOver = false)}
      ondrop={droppedFiles}
    >
      <i class="lni lni-cloud-upload dz-icon" aria-hidden="true"></i>
      <p class="dz-title">Drop songs or folders here</p>
      <p class="dz-hint">MP3, FLAC, M4A, OGG, WAV and more. Duplicates are sorted out on ingest.</p>
      <div class="dz-actions">
        <button class="btn-accent" onclick={(e) => { e.stopPropagation(); fileInput.click(); }}>
          Choose files
        </button>
        <button class="btn-ghost" onclick={(e) => { e.stopPropagation(); folderInput.click(); }}>
          Choose folder
        </button>
      </div>
    </div>
    <input
      bind:this={fileInput}
      type="file"
      multiple
      accept="audio/*,.mp3,.flac,.m4a,.mp4,.aac,.ogg,.opus,.wav"
      class="hidden-input"
      onchange={pickedFiles}
    />
    <!-- svelte-ignore a11y_missing_attribute -->
    <input
      bind:this={folderInput}
      type="file"
      webkitdirectory
      multiple
      class="hidden-input"
      onchange={pickedFiles}
    />
    {#if pickNote}
      <p class="pick-note">{pickNote}</p>
    {/if}

    {#if up.total > 0}
      <div class="upload-panel">
        <div class="up-head">
          <div class="up-status">
            {#if up.phase === 'uploading'}
              <span class="spinner"></span>Uploading {up.uploadedCount}/{up.total}
            {:else if up.phase === 'ingesting'}
              <span class="spinner"></span>Ingesting…
            {:else if up.phase === 'done'}
              <i class="lni lni-check-circle-1" aria-hidden="true"></i>Upload complete
            {:else}
              {up.total} file{up.total === 1 ? '' : 's'} ready · {formatBytes(up.totalBytes)}
            {/if}
          </div>
          <div class="up-actions">
            {#if up.phase === 'idle'}
              <button class="btn-ghost btn-sm" onclick={() => up.reset()}>Clear</button>
              <button class="btn-accent btn-sm" onclick={() => up.start()}>Upload {up.total}</button>
            {:else if up.phase === 'uploading'}
              <button class="btn-ghost btn-sm" onclick={() => up.cancel()}>Cancel</button>
            {:else if up.phase === 'done'}
              {#if up.errorCount > 0}
                <button class="btn-ghost btn-sm" onclick={() => up.retryFailed()}>Retry {up.errorCount} failed</button>
              {/if}
              <button class="btn-ghost btn-sm" onclick={() => up.reset()}>Clear</button>
            {/if}
          </div>
        </div>

        {#if up.phase === 'uploading'}
          <div class="progress-track"><div class="progress-fill" style="transform: scaleX({up.bytePct / 100})"></div></div>
          <span class="up-sub">{formatBytes(up.uploadedBytes)} / {formatBytes(up.totalBytes)} · {up.bytePct}%{#if up.errorCount > 0} · {up.errorCount} failed{/if}</span>
        {/if}

        {#if up.phase === 'ingesting' || up.phase === 'done'}
          {@const t = up.ingestTask}
          {#if t && t.total}
            <div class="progress-track"><div class="progress-fill" style="transform: scaleX({t.progress / t.total})"></div></div>
            <span class="up-sub">Ingesting {t.progress} / {t.total}</span>
          {/if}
          {#if t?.stats}
            <div class="stats-row">
              <span class="stat stat-ok">{t.stats.downloaded} added</span>
              <span class="stat stat-neutral">{t.stats.skipped} duplicate{t.stats.skipped === 1 ? '' : 's'}</span>
              <span class="stat stat-warn">{t.stats.to_validate} to review</span>
              {#if t.stats.errors.length > 0}<span class="stat stat-err">{t.stats.errors.length} errors</span>{/if}
            </div>
          {/if}
          {#if up.ingestError}
            <div class="callout callout-error" role="alert">
              <i class="lni lni-xmark-circle" aria-hidden="true"></i>
              <div class="callout-body"><strong>Ingest failed to start.</strong><span>{up.ingestError}</span></div>
            </div>
          {/if}
        {/if}

        {#if up.uploading.length > 0 || up.errored.length > 0}
          <ul class="up-list">
            {#each up.uploading as it (it.id)}
              <li class="up-row">
                <i class="lni lni-file-audio up-ic" aria-hidden="true"></i>
                <div class="up-file">
                  <span class="up-name">{it.relativePath}</span>
                  <div class="mini-track"><div class="mini-fill" style="transform: scaleX({it.size ? it.loaded / it.size : 0})"></div></div>
                </div>
                <span class="up-pct">{it.size ? Math.round((it.loaded / it.size) * 100) : 0}%</span>
              </li>
            {/each}
            {#each up.errored as it (it.id)}
              <li class="up-row err">
                <i class="lni lni-xmark-circle up-ic" aria-hidden="true"></i>
                <div class="up-file">
                  <span class="up-name">{it.relativePath}</span>
                  <span class="up-err">{it.error}</span>
                </div>
              </li>
            {/each}
          </ul>
        {/if}

        {#if up.phase === 'idle' && up.total > 8}
          <p class="up-more">{up.total} files queued — only uploads and errors are listed while running.</p>
        {/if}
      </div>
    {/if}
  </section>

  {#if batchError}
    <div class="callout callout-error" role="alert">
      <i class="lni lni-xmark-circle" aria-hidden="true"></i>
      <div class="callout-body"><strong>Batch ingest failed.</strong><span>{batchError}</span></div>
    </div>
  {/if}

  {#if batchTask}
    <div class="task-panel">
      <div class="task-head">
        <span class="task-label">Task #{batchTask.id}</span>
        <span
          class="status-badge"
          class:ok={batchTask.status === 'Completed'}
          class:running={batchTask.status === 'Running' || batchTask.status === 'Pending'}
          class:error={batchTask.status === 'Failed'}
        >
          {batchTask.status}
        </span>
      </div>

      {#if batchTask.total !== null && batchTask.total > 0}
        <div class="progress-track">
          <div class="progress-fill" style="transform: scaleX({taskProgress(batchTask) / 100})"></div>
        </div>
        <span class="progress-label">{batchTask.progress} / {batchTask.total}</span>
      {/if}

      {#if batchTask.stats}
        <div class="stats-row">
          <span class="stat stat-ok">{batchTask.stats.downloaded} ingested</span>
          <span class="stat stat-warn">{batchTask.stats.to_validate} to validate</span>
          {#if batchTask.stats.errors.length > 0}
            <span class="stat stat-err">{batchTask.stats.errors.length} errors</span>
          {/if}
        </div>
      {/if}

      {#if batchTask.error}
        <p class="task-error">{batchTask.error}</p>
      {/if}
    </div>
  {/if}

  <section class="schedule">
    <label class="switch-label">
      <input type="checkbox" bind:checked={pollEnabled} onchange={applyPollSchedule} />
      <span>Auto-ingest every</span>
    </label>
    <div class="interval" class:disabled={!pollEnabled}>
      <input
        type="number"
        min="0"
        max="23"
        bind:value={pollHours}
        disabled={!pollEnabled}
        onchange={applyPollSchedule}
        aria-label="Hours"
      /><span class="unit">h</span>
      <input
        type="number"
        min="0"
        max="59"
        step="5"
        bind:value={pollMinutes}
        disabled={!pollEnabled}
        onchange={applyPollSchedule}
        aria-label="Minutes"
      /><span class="unit">m</span>
    </div>
    <span class="schedule-note">Runs in your browser; resets on reload.</span>
    {#if pollMsg}<span class="schedule-msg">{pollMsg}</span>{/if}
  </section>

  <section class="files">
    <div class="files-head">
      <h2>Ingest directory{#if response}<span class="count">{response.files.length}</span>{/if}</h2>
      {#if response}<code class="dir-path">{response.ingest_dir}</code>{/if}
    </div>

    {#if filesError}
      <div class="callout callout-error" role="alert">
        <i class="lni lni-xmark-circle" aria-hidden="true"></i>
        <div class="callout-body"><strong>Couldn't read the ingest directory.</strong><span>{filesError}</span></div>
      </div>
    {:else if loadingFiles}
      <ul class="file-list" aria-hidden="true">
        {#each { length: 4 } as _}
          <li class="file-row skeleton">
            <div class="file-header">
              <span class="sk sk-name"></span>
            </div>
          </li>
        {/each}
      </ul>
    {:else if !response || response.files.length === 0}
      <div class="empty">
        <i class="lni lni-folder-1" aria-hidden="true"></i>
        <p class="empty-title">Nothing to ingest</p>
        <p class="empty-hint">Drop audio files into the ingest directory, then refresh.</p>
      </div>
    {:else}
      <ul class="file-list">
        {#each response.files as file (file.path)}
          {@const result = fileResults[file.path]}
          {@const expanded = expandedPath === file.path}

          <li class="file-row" class:done={result?.ok} class:failed={result && !result.ok}>
            <div class="file-header">
              <button
                type="button"
                class="file-expand"
                aria-expanded={expanded}
                onclick={() => toggleExpand(file.path)}
              >
                <i class="lni lni-chevron-right chevron" class:open={expanded} aria-hidden="true"></i>
                <i class="lni lni-file-audio file-ic" aria-hidden="true"></i>
                <span class="file-text">
                  <span class="file-name">
                    {#if file.relative_path !== file.name}
                      <span class="file-subdir">{file.relative_path.slice(0, file.relative_path.lastIndexOf('/') + 1)}</span>
                    {/if}{file.name}
                  </span>
                  <span class="file-meta">
                    <span>{formatBytes(file.size_bytes)}</span>
                    {#if file.tags?.title}
                      <span class="file-tag">{file.tags.artists.length > 0 ? `${file.tags.artists[0]} — ` : ''}{file.tags.title}</span>
                    {/if}
                    {#if file.tags?.duration_secs}
                      <span class="file-dur">{formatSeconds(file.tags.duration_secs)}</span>
                    {/if}
                  </span>
                </span>
              </button>

              <div class="file-actions">
                {#if result}
                  <span class="result-msg" class:ok={result.ok} class:error={!result.ok}>{result.message}</span>
                {/if}
                <button
                  class="btn-accent btn-sm"
                  disabled={!!ingestingFile || !!result?.ok}
                  onclick={(e) => handleIngestFile(e, file)}
                >
                  {#if ingestingFile === file.path}
                    <span class="spinner"></span>
                  {:else if result?.ok}
                    <i class="lni lni-check" aria-hidden="true"></i>Done
                  {:else}
                    Ingest
                  {/if}
                </button>
              </div>
            </div>

            {#if expanded}
              <div class="file-detail">
                {#if file.tags}
                  {@const t = file.tags}
                  <dl class="tags-grid">
                    {#if t.title}<dt>Title</dt><dd>{t.title}</dd>{/if}
                    {#if t.artists.length > 0}<dt>Artists</dt><dd>{t.artists.join(', ')}</dd>{/if}
                    {#if t.album}<dt>Album</dt><dd>{t.album}</dd>{/if}
                    {#if t.date}<dt>Date</dt><dd>{t.date}</dd>{/if}
                    {#if t.genre}<dt>Genre</dt><dd>{t.genre}</dd>{/if}
                    {#if t.track_number}<dt>Track #</dt><dd>{t.track_number}</dd>{/if}
                    {#if t.duration_secs}<dt>Duration</dt><dd>{formatSeconds(t.duration_secs)}</dd>{/if}
                  </dl>
                {:else}
                  <p class="no-tags">No readable tags found in this file.</p>
                {/if}
                <div class="detail-path">
                  <span class="detail-path-label">Path</span>
                  <code>{file.path}</code>
                </div>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .ingest-page {
    width: 100%;
    box-sizing: border-box;
    padding: 1.5rem 2rem 2rem;
    display: flex;
    flex-direction: column;
    gap: 1.75rem;
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-header {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  @media (min-width: 640px) {
    .page-header {
      flex-direction: row;
      align-items: flex-start;
      justify-content: space-between;
    }
  }
  .header-text {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: 60ch;
  }
  h1 {
    font-size: 1.25rem;
    font-weight: 700;
    margin: 0;
  }
  @media (min-width: 768px) {
    h1 {
      font-size: 1.5rem;
    }
  }
  .lede {
    margin: 0;
    color: var(--muted);
    font-size: 0.95rem;
    line-height: 1.55;
  }
  .header-actions {
    display: flex;
    gap: 0.6rem;
    flex-shrink: 0;
  }

  /* ── Buttons ─────────────────────────────────────────────────────────── */
  .btn-accent,
  .btn-ghost {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 1rem;
    border-radius: 8px;
    font-family: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition:
      filter 0.12s ease,
      background 0.12s ease,
      opacity 0.12s ease;
  }
  .btn-accent {
    border: none;
    background: var(--accent);
    color: #fff;
  }
  .btn-accent .lni {
    font-size: 16px;
  }
  .btn-accent:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .btn-ghost {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
  }
  .btn-ghost:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .btn-accent:disabled,
  .btn-ghost:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn-sm {
    padding: 0.4rem 0.8rem;
    font-size: 0.82rem;
  }

  /* ── Callout (errors) ────────────────────────────────────────────────── */
  .callout {
    display: flex;
    align-items: flex-start;
    gap: 0.7rem;
    padding: 0.85rem 1rem;
    border-radius: 10px;
    border: 1px solid transparent;
    font-size: 0.9rem;
  }
  .callout .lni {
    font-size: 19px;
    flex-shrink: 0;
  }
  .callout-body {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .callout-body strong {
    font-weight: 600;
    color: var(--text-bright);
  }
  .callout-error {
    background: var(--error-bg);
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
    color: var(--error);
  }

  /* ── Batch task panel ────────────────────────────────────────────────── */
  .task-panel {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 1rem 1.1rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
  }
  .task-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .task-label {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--muted);
  }
  .status-badge {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0.15rem 0.55rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
  }
  .status-badge.ok {
    background: color-mix(in srgb, var(--success) 20%, transparent);
    color: var(--success);
  }
  .status-badge.running {
    background: var(--accent-muted);
    color: var(--accent-2);
  }
  .status-badge.error {
    background: var(--error-bg);
    color: var(--error);
  }
  .progress-track {
    height: 6px;
    border-radius: 999px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    width: 100%;
    background: var(--accent);
    transform-origin: left;
    transition: transform 0.3s ease;
  }
  .progress-label {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--muted-2);
  }
  .stats-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .stat {
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.15rem 0.55rem;
    border-radius: 999px;
  }
  .stat-ok {
    background: color-mix(in srgb, var(--success) 16%, transparent);
    color: var(--success);
  }
  .stat-warn {
    background: var(--warning-bg);
    color: var(--warning);
  }
  .stat-err {
    background: var(--error-bg);
    color: var(--error);
  }
  .task-error {
    margin: 0;
    font-size: 0.85rem;
    color: var(--error);
  }

  /* ── Schedule strip ──────────────────────────────────────────────────── */
  .schedule {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.75rem 1rem;
    padding: 0.75rem 1rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
  }
  .switch-label {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
    color: var(--text);
    cursor: pointer;
  }
  .switch-label input {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .interval {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    transition: opacity 0.12s ease;
  }
  .interval.disabled {
    opacity: 0.5;
  }
  .interval input {
    width: 3rem;
    padding: 0.35rem 0.4rem;
    text-align: center;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 0.85rem;
    outline: none;
  }
  .interval input:focus {
    border-color: var(--accent);
  }
  .unit {
    font-size: 0.8rem;
    color: var(--muted);
  }
  .schedule-note {
    font-size: 0.8rem;
    color: var(--muted-2);
  }
  .schedule-msg {
    margin-left: auto;
    font-size: 0.8rem;
    color: var(--muted);
  }

  /* ── Files ───────────────────────────────────────────────────────────── */
  .files {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .files-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--muted-2);
    margin: 0;
  }
  .count {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    letter-spacing: 0;
    padding: 0.1rem 0.4rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
  }
  .dir-path {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--muted);
    background: var(--surface);
    border: 1px solid var(--border-soft);
    padding: 0.2rem 0.55rem;
    border-radius: 6px;
    max-width: 55%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .file-row {
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 10px;
    overflow: hidden;
    transition: border-color 0.12s ease;
  }
  .file-row.done {
    background: color-mix(in srgb, var(--success) 7%, var(--surface));
  }
  .file-row.failed {
    background: color-mix(in srgb, var(--error) 7%, var(--surface));
  }
  .file-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 0.8rem;
  }
  .file-expand {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    color: inherit;
    font-family: inherit;
    cursor: pointer;
  }
  .chevron {
    font-size: 14px;
    color: var(--muted-2);
    flex-shrink: 0;
    transition: transform 0.15s ease;
  }
  .chevron.open {
    transform: rotate(90deg);
  }
  .file-ic {
    font-size: 18px;
    color: var(--muted-2);
    flex-shrink: 0;
  }
  .file-text {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .file-name {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file-subdir {
    color: var(--muted-2);
  }
  .file-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    font-size: 0.78rem;
    color: var(--muted);
    min-width: 0;
  }
  .file-tag {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 40ch;
  }
  .file-dur {
    font-family: var(--font-mono);
    color: var(--muted-2);
  }
  .file-actions {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-shrink: 0;
  }
  .result-msg {
    font-size: 0.78rem;
    max-width: 26ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result-msg.ok {
    color: var(--success);
  }
  .result-msg.error {
    color: var(--error);
  }

  .file-detail {
    padding: 0 0.9rem 0.9rem 2.15rem;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }
  .tags-grid {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.3rem 1rem;
    margin: 0;
  }
  .tags-grid dt {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted-2);
    align-self: baseline;
  }
  .tags-grid dd {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text);
  }
  .no-tags {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .detail-path {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-width: 0;
  }
  .detail-path-label {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted-2);
    flex-shrink: 0;
  }
  .detail-path code {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--muted);
    background: var(--surface-2);
    padding: 0.15rem 0.5rem;
    border-radius: 5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ── Empty + loading ─────────────────────────────────────────────────── */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.35rem;
    padding: 2.5rem 1rem;
  }
  .empty .lni {
    font-size: 30px;
    color: var(--muted-2);
    margin-bottom: 0.3rem;
  }
  .empty-title {
    margin: 0;
    font-weight: 600;
    color: var(--text);
  }
  .empty-hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .file-row.skeleton .file-header {
    padding: 0.85rem 0.8rem;
  }
  .sk {
    height: 0.75rem;
    border-radius: 4px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s ease-in-out infinite;
  }
  .sk-name {
    width: 45%;
  }
  @keyframes sk-pulse {
    50% {
      opacity: 0.45;
    }
  }

  .spinner {
    width: 13px;
    height: 13px;
    border: 2px solid color-mix(in srgb, currentColor 30%, transparent);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* ── Browser upload ──────────────────────────────────────────────────── */
  .upload {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .hidden-input {
    display: none;
  }
  .dropzone {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.4rem;
    padding: 2.25rem 1.5rem;
    border: 1.5px dashed var(--border);
    border-radius: 14px;
    background: var(--surface);
    cursor: pointer;
    transition:
      border-color 0.15s ease,
      background 0.15s ease;
  }
  .dropzone:hover {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  }
  .dropzone.drag {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, var(--surface));
  }
  .dz-icon {
    font-size: 34px;
    color: var(--accent);
    margin-bottom: 0.3rem;
  }
  .dz-title {
    margin: 0;
    font-weight: 600;
    color: var(--text-bright);
  }
  .dz-hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .dz-actions {
    display: flex;
    gap: 0.6rem;
    margin-top: 0.7rem;
  }

  .upload-panel {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 1rem 1.1rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
  }
  .up-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .up-status {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text);
  }
  .up-status .lni {
    color: var(--success);
    font-size: 18px;
  }
  .up-actions {
    display: flex;
    gap: 0.5rem;
  }
  .up-sub {
    font-size: 0.8rem;
    color: var(--muted);
    font-family: var(--font-mono);
  }
  .stat-neutral {
    background: var(--surface-2);
    color: var(--muted);
  }
  .up-list {
    list-style: none;
    margin: 0.2rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 320px;
    overflow-y: auto;
  }
  .up-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.4rem 0.5rem;
    border-radius: 8px;
    background: var(--surface-2);
  }
  .up-row.err {
    background: color-mix(in srgb, var(--error) 10%, var(--surface-2));
  }
  .up-ic {
    font-size: 16px;
    color: var(--muted-2);
    flex-shrink: 0;
  }
  .up-row.err .up-ic {
    color: var(--error);
  }
  .up-file {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .up-name {
    font-size: 0.82rem;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .up-err {
    font-size: 0.75rem;
    color: var(--error);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mini-track {
    height: 3px;
    border-radius: 999px;
    background: var(--border);
    overflow: hidden;
  }
  .mini-fill {
    height: 100%;
    width: 100%;
    transform-origin: left;
    background: var(--accent);
    transition: transform 0.15s ease;
  }
  .up-pct {
    font-size: 0.75rem;
    font-family: var(--font-mono);
    color: var(--muted-2);
    flex-shrink: 0;
    min-width: 2.5rem;
    text-align: right;
  }
  .up-more {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted-2);
  }

  @media (prefers-reduced-motion: reduce) {
    .sk,
    .spinner,
    .progress-fill,
    .mini-fill,
    .chevron {
      animation: none;
      transition: none;
    }
  }

  @media (max-width: 640px) {
    .ingest-page {
      padding: 1.25rem 1rem 1.5rem;
    }
    .header-actions {
      width: 100%;
    }
    .header-actions .btn-accent {
      flex: 1;
      justify-content: center;
    }
    .dir-path {
      display: none;
    }
    .result-msg {
      display: none;
    }
  }
  .pick-note {
    margin: 0.75rem 0 0;
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
