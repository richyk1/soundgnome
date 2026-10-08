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
      {#if response}
        <p class="header-sub">{response.files.length} file{response.files.length === 1 ? '' : 's'} in ingest directory{#if up.total > 0} · {up.total} queued for upload{/if}</p>
      {/if}
      <p class="lede">
        Upload songs or whole folders from your device, or ingest files already in the server's
        ingest directory. Duplicates are detected and sorted from new tracks automatically.
      </p>
    </div>
    <div class="header-actions">
      <button class="btn-secondary" onclick={loadFiles} disabled={loadingFiles}>
        <i class="pxi {loadingFiles ? 'pxi-loader pxi-spin' : 'pxi-refresh'}" aria-hidden="true"></i>Refresh
      </button>
      <button class="btn-primary" disabled={batchIngesting || loadingFiles} onclick={handleIngestAll}>
        {#if batchIngesting}
          <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Ingesting
        {:else}
          <i class="pxi pxi-upload" aria-hidden="true"></i>Ingest all
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
      <i class="pxi pxi-upload dz-icon" aria-hidden="true"></i>
      <p class="dz-title">Drop songs or folders here</p>
      <p class="dz-hint">MP3, FLAC, M4A, OGG, WAV and more. Duplicates are sorted out on ingest.</p>
      <div class="dz-actions">
        <button class="btn-secondary" onclick={(e) => { e.stopPropagation(); fileInput.click(); }}>
          <i class="pxi pxi-file" aria-hidden="true"></i>Choose files
        </button>
        <button class="btn-secondary" onclick={(e) => { e.stopPropagation(); folderInput.click(); }}>
          <i class="pxi pxi-folder" aria-hidden="true"></i>Choose folder
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
      <p class="pick-note" role="status">{pickNote}</p>
    {/if}

    {#if up.total > 0}
      <div class="upload-panel">
        <div class="up-head">
          <div class="up-status">
            {#if up.phase === 'uploading'}
              <span class="status-tag running"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Uploading</span>
              <span class="up-count">{up.uploadedCount}/{up.total}</span>
            {:else if up.phase === 'ingesting'}
              <span class="status-tag running"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Ingesting…</span>
            {:else if up.phase === 'done'}
              <span class="status-tag completed"><i class="pxi pxi-check" aria-hidden="true"></i>Upload complete</span>
            {:else}
              <span class="up-ready">{up.total} file{up.total === 1 ? '' : 's'} ready · {formatBytes(up.totalBytes)}</span>
            {/if}
          </div>
          <div class="up-actions">
            {#if up.phase === 'idle'}
              <button class="btn-secondary btn-sm" onclick={() => up.reset()}>Clear</button>
              <button class="btn-primary btn-sm" onclick={() => up.start()}>
                <i class="pxi pxi-upload" aria-hidden="true"></i>Upload {up.total}
              </button>
            {:else if up.phase === 'uploading'}
              <button class="btn-ghost btn-sm" onclick={() => up.cancel()}>
                <i class="pxi pxi-close" aria-hidden="true"></i>Cancel
              </button>
            {:else if up.phase === 'done'}
              {#if up.errorCount > 0}
                <button class="btn-secondary btn-sm" onclick={() => up.retryFailed()}>
                  <i class="pxi pxi-redo" aria-hidden="true"></i>Retry {up.errorCount} failed
                </button>
              {/if}
              <button class="btn-secondary btn-sm" onclick={() => up.reset()}>Clear</button>
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
              <span class="stat stat-ok"><i class="pxi pxi-check" aria-hidden="true"></i>{t.stats.downloaded} added</span>
              <span class="stat stat-neutral"><i class="pxi pxi-copy" aria-hidden="true"></i>{t.stats.skipped} duplicate{t.stats.skipped === 1 ? '' : 's'}</span>
              <span class="stat stat-warn"><i class="pxi pxi-flag" aria-hidden="true"></i>{t.stats.to_validate} to review</span>
              {#if t.stats.errors.length > 0}<span class="stat stat-err"><i class="pxi pxi-square-alert" aria-hidden="true"></i>{t.stats.errors.length} errors</span>{/if}
            </div>
          {/if}
          {#if up.ingestError}
            <div class="callout callout-error" role="alert">
              <i class="pxi pxi-square-alert" aria-hidden="true"></i>
              <div class="callout-body"><strong>Ingest failed to start.</strong><span>{up.ingestError}</span></div>
            </div>
          {/if}
        {/if}

        {#if up.uploading.length > 0 || up.errored.length > 0}
          <ul class="up-list">
            {#each up.uploading as it (it.id)}
              <li class="up-row">
                <i class="pxi pxi-file up-ic" aria-hidden="true"></i>
                <div class="up-file">
                  <span class="up-name">{it.relativePath}</span>
                  <div class="mini-track"><div class="mini-fill" style="transform: scaleX({it.size ? it.loaded / it.size : 0})"></div></div>
                </div>
                <span class="up-pct">{it.size ? Math.round((it.loaded / it.size) * 100) : 0}%</span>
              </li>
            {/each}
            {#each up.errored as it (it.id)}
              <li class="up-row err">
                <i class="pxi pxi-square-alert up-ic" aria-hidden="true"></i>
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
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>
      <div class="callout-body"><strong>Batch ingest failed.</strong><span>{batchError}</span></div>
    </div>
  {/if}

  {#if batchTask}
    <div class="task-panel">
      <div class="task-head">
        <span class="task-label">Task #{batchTask.id}</span>
        <span
          class="status-tag"
          class:completed={batchTask.status === 'Completed'}
          class:running={batchTask.status === 'Running' || batchTask.status === 'Pending'}
          class:failed={batchTask.status === 'Failed'}
        >
          {#if batchTask.status === 'Running' || batchTask.status === 'Pending'}
            <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>
          {:else if batchTask.status === 'Completed'}
            <i class="pxi pxi-check" aria-hidden="true"></i>
          {:else if batchTask.status === 'Failed'}
            <i class="pxi pxi-square-alert" aria-hidden="true"></i>
          {/if}
          {batchTask.status}
        </span>
      </div>

      {#if batchTask.total !== null && batchTask.total > 0}
        <div class="progress-row">
          <div class="progress-track">
            <div class="progress-fill" style="transform: scaleX({taskProgress(batchTask) / 100})"></div>
          </div>
          <span class="progress-label">{batchTask.progress} / {batchTask.total}</span>
        </div>
      {/if}

      {#if batchTask.stats}
        <div class="stats-row">
          <span class="stat stat-ok"><i class="pxi pxi-check" aria-hidden="true"></i>{batchTask.stats.downloaded} ingested</span>
          <span class="stat stat-warn"><i class="pxi pxi-flag" aria-hidden="true"></i>{batchTask.stats.to_validate} to validate</span>
          {#if batchTask.stats.errors.length > 0}
            <span class="stat stat-err"><i class="pxi pxi-square-alert" aria-hidden="true"></i>{batchTask.stats.errors.length} errors</span>
          {/if}
        </div>
      {/if}

      {#if batchTask.error}
        <p class="task-error">{batchTask.error}</p>
      {/if}
    </div>
  {/if}

  <section class="schedule">
    <i class="pxi pxi-clock schedule-ic" aria-hidden="true"></i>
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
    {#if pollMsg}<span class="schedule-msg" role="status">{pollMsg}</span>{/if}
  </section>

  <section class="files">
    <div class="files-head">
      <h2>Ingest directory{#if response}<span class="count-num">{response.files.length}</span>{/if}</h2>
      {#if response}
        <span class="dir-path"><i class="pxi pxi-folder" aria-hidden="true"></i><span class="dir-text">{response.ingest_dir}</span></span>
      {/if}
    </div>

    {#if filesError}
      <div class="callout callout-error" role="alert">
        <i class="pxi pxi-square-alert" aria-hidden="true"></i>
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
        <i class="pxi pxi-folder" aria-hidden="true"></i>
        <p class="empty-title">Nothing to ingest</p>
        <p class="empty-hint">Drop audio files into the ingest directory, then refresh.</p>
      </div>
    {:else}
      <ul class="file-list">
        {#each response.files as file (file.path)}
          {@const result = fileResults[file.path]}
          {@const expanded = expandedPath === file.path}

          <li class="file-row" class:expanded class:done={result?.ok} class:failed={result && !result.ok}>
            <div class="file-header">
              <button
                type="button"
                class="file-expand"
                aria-expanded={expanded}
                onclick={() => toggleExpand(file.path)}
              >
                <i class="pxi {expanded ? 'pxi-chevron-down' : 'pxi-chevron-right'} chevron" aria-hidden="true"></i>
                {#if result?.ok}
                  <i class="pxi pxi-check file-ic ok" aria-hidden="true"></i>
                {:else if result}
                  <i class="pxi pxi-square-alert file-ic error" aria-hidden="true"></i>
                {:else}
                  <i class="pxi pxi-file file-ic" aria-hidden="true"></i>
                {/if}
                <span class="file-text">
                  <span class="file-name">
                    {#if file.relative_path !== file.name}
                      <span class="file-subdir">{file.relative_path.slice(0, file.relative_path.lastIndexOf('/') + 1)}</span>
                    {/if}{file.name}
                  </span>
                  <span class="file-meta">
                    <span class="file-size">{formatBytes(file.size_bytes)}</span>
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
                  class="btn-secondary btn-sm"
                  disabled={!!ingestingFile || !!result?.ok}
                  aria-busy={ingestingFile === file.path}
                  onclick={(e) => handleIngestFile(e, file)}
                >
                  {#if ingestingFile === file.path}
                    <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Ingest
                  {:else if result?.ok}
                    <i class="pxi pxi-check" aria-hidden="true"></i>Done
                  {:else}
                    <i class="pxi pxi-upload" aria-hidden="true"></i>Ingest
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
                    {#if t.date}<dt>Date</dt><dd class="num">{t.date}</dd>{/if}
                    {#if t.genre}<dt>Genre</dt><dd>{t.genre}</dd>{/if}
                    {#if t.track_number}<dt>Track #</dt><dd class="num">{t.track_number}</dd>{/if}
                    {#if t.duration_secs}<dt>Duration</dt><dd class="num">{formatSeconds(t.duration_secs)}</dd>{/if}
                  </dl>
                {:else}
                  <p class="no-tags">No readable tags found in this file.</p>
                {/if}
                <div class="detail-path">
                  <span class="detail-path-label">Path</span>
                  <span class="detail-path-text">{file.path}</span>
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
    display: flex;
    flex-direction: column;
    gap: 24px;
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-page);
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-header {
    margin: 0;
    justify-content: space-between;
  }
  @media (min-width: 640px) {
    .page-header { align-items: flex-start; }
  }
  .header-text {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 60ch;
  }
  h1 {
    margin: 0;
    font-size: 32px;
    line-height: 1.05;
  }
  .header-sub {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }
  .lede {
    margin: 4px 0 0;
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }
  .header-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  /* ── Mono status tag (shared look with Activity) ─────────────────────── */
  .status-tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 24px;
    padding: 0 8px;
    border: 1px solid color-mix(in srgb, currentColor 35%, transparent);
    border-radius: var(--radius-chip);
    background: color-mix(in srgb, currentColor 10%, transparent);
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    white-space: nowrap;
  }
  .status-tag .pxi { font-size: 16px; }
  .status-tag.running { color: var(--live); }
  .status-tag.completed { color: var(--success); }
  .status-tag.failed { color: var(--error); }

  /* ── Progress: thin hairline track, live fill ────────────────────────── */
  .progress-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .progress-track {
    flex: 1;
    height: 4px;
    overflow: hidden;
    background: var(--border);
  }
  .progress-fill,
  .mini-fill {
    width: 100%;
    height: 100%;
    background: var(--live);
    transform-origin: left;
    transition: transform var(--motion-normal) var(--ease-out);
  }
  .progress-label {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }

  /* ── Stats: pixel icon + mono tabular figure ─────────────────────────── */
  .stats-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--text);
  }
  .stat .pxi {
    font-size: 16px;
    color: var(--muted-2);
  }
  .stat-ok .pxi { color: var(--success); }
  .stat-warn .pxi { color: var(--warning); }
  .stat-neutral { color: var(--muted); }
  .stat-err,
  .stat-err .pxi { color: var(--error); }

  /* ── Batch task panel ────────────────────────────────────────────────── */
  .task-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }
  .task-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .task-label {
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .task-error {
    margin: 0;
    font-size: 14px;
    line-height: 1.45;
    color: var(--error);
  }

  /* ── Schedule strip ──────────────────────────────────────────────────── */
  .schedule {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px 16px;
    padding: 12px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }
  .schedule-ic {
    font-size: 16px;
    color: var(--muted);
  }
  .switch-label {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    color: var(--text);
    cursor: pointer;
  }
  .switch-label input {
    width: 16px;
    height: 16px;
    margin: 0;
    cursor: pointer;
  }
  .interval {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    transition: opacity var(--motion-fast) var(--ease-out);
  }
  .interval.disabled {
    opacity: 0.5;
  }
  .interval input {
    width: 56px;
    padding: 6px 8px;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    text-align: center;
  }
  .unit {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--muted);
  }
  .schedule-note {
    font-size: 13px;
    color: var(--muted-2);
  }
  .schedule-msg {
    margin-left: auto;
    font-size: 13px;
    color: var(--muted);
  }

  /* ── Ingest directory: hairline tree rows ────────────────────────────── */
  .files {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .files-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 16px;
  }
  h2 {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 0;
    font-size: 18px;
    line-height: 1.3;
  }
  .count-num {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 400;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0;
    color: var(--muted-2);
  }
  .dir-path {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    max-width: 55%;
    color: var(--muted);
  }
  .dir-path .pxi {
    flex-shrink: 0;
    font-size: 16px;
    color: var(--muted-2);
  }
  .dir-text {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--border);
  }
  .file-row {
    border-bottom: 1px solid var(--border-soft);
    transition: background-color var(--motion-fast) var(--ease-out);
  }
  .file-row:hover,
  .file-row.expanded {
    background: var(--surface);
  }
  .file-header {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 56px;
    padding: 8px 8px 8px 4px;
  }
  .file-expand {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 4px;
    border: none;
    border-radius: var(--radius-chip);
    background: none;
    color: inherit;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
  }
  .chevron {
    flex-shrink: 0;
    font-size: 16px;
    color: var(--muted-2);
    transition: color var(--motion-fast) var(--ease-out);
  }
  .file-expand:hover .chevron,
  .file-row.expanded .chevron {
    color: var(--text-bright);
  }
  .file-ic {
    flex-shrink: 0;
    font-size: 16px;
    color: var(--muted-2);
  }
  .file-ic.ok { color: var(--success); }
  .file-ic.error { color: var(--error); }
  .file-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .file-name {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-bright);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-subdir {
    color: var(--muted-2);
  }
  .file-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    min-width: 0;
    font-size: 13px;
    color: var(--muted);
  }
  .file-size,
  .file-dur {
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }
  .file-tag {
    max-width: 40ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-actions {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 10px;
  }
  .result-msg {
    max-width: 26ch;
    overflow: hidden;
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result-msg.ok { color: var(--success); }
  .result-msg.error { color: var(--error); }

  .file-detail {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 16px 16px 56px;
  }
  .tags-grid {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
    margin: 0;
  }
  .tags-grid dt,
  .detail-path-label {
    align-self: baseline;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted-2);
  }
  .tags-grid dd {
    margin: 0;
    font-size: 14px;
    color: var(--text);
  }
  .num {
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .no-tags {
    margin: 0;
    font-size: 14px;
    color: var(--muted);
  }
  .detail-path {
    display: flex;
    align-items: baseline;
    gap: 16px;
    min-width: 0;
  }
  .detail-path-label {
    flex-shrink: 0;
  }
  .detail-path-text {
    min-width: 0;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-row.skeleton .file-header {
    min-height: 56px;
    padding: 0 8px;
  }
  .sk {
    height: 12px;
    border-radius: 4px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s ease-in-out infinite;
  }
  .sk-name {
    width: 45%;
  }
  @keyframes sk-pulse {
    50% { opacity: 0.45; }
  }

  /* ── Browser upload ──────────────────────────────────────────────────── */
  .upload {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .hidden-input {
    display: none;
  }
  .dropzone {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 40px 24px;
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius-card);
    background: transparent;
    text-align: center;
    cursor: pointer;
    transition:
      border-color var(--motion-fast) var(--ease-out),
      background-color var(--motion-fast) var(--ease-out);
  }
  .dropzone:hover {
    border-color: var(--border-heavy);
    background: var(--surface);
  }
  .dropzone.drag {
    border-color: var(--accent);
    background: var(--accent-muted);
  }
  .dz-icon {
    margin-bottom: 10px;
    font-size: 48px;
    color: var(--muted-2);
    transition: color var(--motion-fast) var(--ease-out);
  }
  .dropzone:hover .dz-icon {
    color: var(--text);
  }
  .dropzone.drag .dz-icon {
    color: var(--accent);
  }
  .dz-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-bright);
  }
  .dz-hint {
    margin: 0;
    max-width: 48ch;
    font-size: 14px;
    line-height: 1.45;
    color: var(--muted);
  }
  .dz-actions {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }

  .upload-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }
  .up-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .up-status {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }
  .up-count,
  .up-ready {
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .up-ready {
    color: var(--text);
  }
  .up-actions {
    display: flex;
    gap: 8px;
  }
  .up-sub {
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .up-list {
    display: flex;
    flex-direction: column;
    max-height: 320px;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
    border-top: 1px solid var(--border);
  }
  .up-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 2px;
    border-bottom: 1px solid var(--border-soft);
  }
  .up-ic {
    flex-shrink: 0;
    font-size: 16px;
    color: var(--muted-2);
  }
  .up-row.err .up-ic {
    color: var(--error);
  }
  .up-file {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .up-name {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .up-err {
    overflow: hidden;
    font-size: 12px;
    color: var(--error);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mini-track {
    height: 2px;
    overflow: hidden;
    background: var(--border);
  }
  .mini-fill {
    transition-duration: var(--motion-fast);
  }
  .up-pct {
    flex-shrink: 0;
    min-width: 40px;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    color: var(--muted-2);
  }
  .up-more {
    margin: 0;
    font-size: 13px;
    color: var(--muted-2);
  }
  .pick-note {
    margin: 0;
    font-size: 14px;
    color: var(--muted);
  }

  @media (prefers-reduced-motion: reduce) {
    .sk { animation: none; }
  }

  @media (max-width: 640px) {
    .header-actions {
      width: 100%;
    }
    .header-actions .btn-primary {
      flex: 1;
    }
    .result-msg {
      display: none;
    }
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    h1 { font-size: 28px; }
    .interval input { min-height: 44px; }
    .dropzone { padding: 32px 16px; }
    .header-actions, .dz-actions, .up-actions { flex-wrap: wrap; }
    .file-header { flex-wrap: wrap; padding: 8px 4px; }
    .file-expand { flex-basis: 100%; min-height: 44px; }
    .file-actions { flex-wrap: wrap; width: 100%; justify-content: flex-end; }
    .file-detail { padding: 0 8px 16px 36px; }
    .tags-grid { grid-template-columns: minmax(0, max-content) minmax(0, 1fr); }
    .tags-grid dd, .callout-body { overflow-wrap: anywhere; }
    .files-head { flex-wrap: wrap; }
    .dir-path { max-width: 100%; }
    .switch-label { min-height: 44px; }
    .result-msg { display: block; white-space: normal; overflow-wrap: anywhere; }
  }
</style>
