<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getTasks, retryTask, cancelTask } from '../lib/api';
  import type { TaskDto } from '../lib/types';

  interface Props {
    onNavigateValidations?: () => void;
  }
  let { onNavigateValidations }: Props = $props();

  let tasks: TaskDto[] = $state([]);
  let loading = $state(true);
  let retrying: Set<number> = $state(new Set());
  let cancelling: Set<number> = $state(new Set());
  // Track which task error lists are expanded
  let expandedErrors: Set<number> = $state(new Set());
  let expandedValidations: Set<number> = $state(new Set());
  let interval: ReturnType<typeof setInterval>;

  async function refresh() {
    try {
      tasks = await getTasks();
      // Clear cancelling state for tasks that have transitioned to Cancelled/Cancelling
      if (cancelling.size > 0) {
        const resolved = tasks
          .filter((t) => cancelling.has(t.id) && (t.status === 'Cancelled' || t.status === 'Cancelling'))
          .map((t) => t.id);
        if (resolved.length > 0) {
          cancelling = new Set([...cancelling].filter((id) => !resolved.includes(id)));
        }
      }
    } catch {
      // silent
    } finally {
      loading = false;
    }
  }

  async function handleRetry(task: TaskDto) {
    retrying = new Set([...retrying, task.id]);
    try {
      await retryTask(task.id);
      await refresh();
     } catch (e) {
       alert(`Retry failed: ${e instanceof Error ? e.message : e}`);
     } finally {
      retrying = new Set([...retrying].filter((id) => id !== task.id));
    }
  }

  async function handleCancel(task: TaskDto) {
    cancelling = new Set([...cancelling, task.id]);
    try {
      await cancelTask(task.id);
      await refresh();
     } catch (e) {
       alert(`Cancel failed: ${e instanceof Error ? e.message : e}`);
       cancelling = new Set([...cancelling].filter((id) => id !== task.id));
    }
    // Keep in cancelling state until task status reflects cancellation (handled in refresh)
  }

  function toggleErrors(taskId: number) {
    if (expandedErrors.has(taskId)) {
      expandedErrors = new Set([...expandedErrors].filter((id) => id !== taskId));
    } else {
      expandedErrors = new Set([...expandedErrors, taskId]);
    }
  }

  function toggleValidations(taskId: number) {
    if (expandedValidations.has(taskId)) {
      expandedValidations = new Set([...expandedValidations].filter((id) => id !== taskId));
    } else {
      expandedValidations = new Set([...expandedValidations, taskId]);
    }
  }

  onMount(() => {
    refresh();
    interval = setInterval(refresh, 3_000);
  });

  onDestroy(() => clearInterval(interval));

  function statusLabel(status: TaskDto['status']) {
    return { Pending: 'Pending', Running: 'Running', Completed: 'Completed', Failed: 'Failed', Cancelled: 'Cancelled', Cancelling: 'Cancelling…' }[status] ?? status;
  }

  function statusClass(status: TaskDto['status']) {
    return { Pending: 'pending', Running: 'running', Completed: 'completed', Failed: 'failed', Cancelled: 'cancelled', Cancelling: 'cancelling' }[status] ?? '';
  }

  function progressPercent(task: TaskDto) {
    if (!task.total || task.total === 0) return 0;
    return Math.round((task.progress / task.total) * 100);
  }

  function taskLabel(task: TaskDto) {
    if (task.label) return task.label;
    if (task.task_type === 'SyncPlaylist') return 'Sync playlist';
    if (task.task_type === 'SyncArtist') return 'Sync artist';
    if (task.task_type === 'SyncAlbum') return 'Sync album';
    return 'Download track';
  }

  function canRetry(status: TaskDto['status']) {
    return (
      status === 'Pending' ||
      status === 'Failed' ||
      status === 'Running' ||
      status === 'Cancelled' ||
      status === 'Completed'
    );
  }

  function canCancel(status: TaskDto['status']) {
    return status === 'Running' || status === 'Pending';
  }

  function hasStats(task: TaskDto) {
    return task.stats != null && (
      task.stats.downloaded > 0 ||
      task.stats.to_validate > 0 ||
      task.stats.skipped > 0 ||
      task.stats.errors.length > 0
    );
  }

  function reasonLabel(reason: string | null): string {
    if (reason === 'soundcloud_drm_protected') return 'DRM protected';
    if (reason === 'metadata_partial_match') return 'partial metadata match';
    if (reason === 'metadata_no_match') return 'no metadata match';
    return reason ?? 'needs review';
  }
</script>

<div class="tasks-page">
  <header class="page-header">
    <div class="header-text">
      <h1>Activity</h1>
      <p class="lede">Background sync and download tasks, with live progress and per-track results.</p>
    </div>
  </header>

  {#if loading}
    <ul class="task-list" aria-hidden="true">
      {#each [0, 1, 2] as _}
        <li class="task-panel skeleton">
          <div class="sk sk-head"></div>
          <div class="sk sk-bar"></div>
          <div class="sk sk-stats"></div>
        </li>
      {/each}
    </ul>
  {:else if tasks.length === 0}
    <div class="empty">
      <i class="lni lni-list-music-4" aria-hidden="true"></i>
      <p class="empty-title">No activity yet</p>
      <p class="empty-hint">Downloads and syncs you start will show up here with live progress.</p>
    </div>
  {:else}
    <ul class="task-list">
      {#each tasks as task (task.id)}
        <li class="task-panel">
          <div class="task-head">
            <div class="task-ident">
              <span class="task-label">{taskLabel(task)}</span>
              <span class="task-id">#{task.id}</span>
            </div>
            <div class="task-actions">
              {#if canCancel(task.status)}
                <button
                  class="btn-danger btn-sm"
                  disabled={cancelling.has(task.id)}
                  onclick={() => handleCancel(task)}
                >
                  {#if cancelling.has(task.id)}
                    <span class="spinner"></span> Cancelling…
                  {:else}
                    <i class="lni lni-xmark" aria-hidden="true"></i> Cancel
                  {/if}
                </button>
              {/if}
              {#if canRetry(task.status)}
                <button
                  class="btn-ghost btn-sm"
                  disabled={retrying.has(task.id)}
                  onclick={() => handleRetry(task)}
                >
                  {#if retrying.has(task.id)}
                    <span class="spinner"></span>
                  {:else}
                    <i class="lni lni-redo" aria-hidden="true"></i> Retry
                  {/if}
                </button>
              {/if}
              <span class="status-badge {statusClass(task.status)}">{statusLabel(task.status)}</span>
            </div>
          </div>

          {#if task.status === 'Running' || task.status === 'Completed' || task.status === 'Cancelled'}
            <div class="progress-row">
              <div class="progress-track">
                <div
                  class="progress-fill {statusClass(task.status)}"
                  style="transform: scaleX({progressPercent(task) / 100})"
                ></div>
              </div>
              <span class="progress-label">
                {task.progress}{task.total != null ? ` / ${task.total}` : ''}
              </span>
            </div>
          {/if}

          {#if task.status === 'Running' && task.stats?.ai_curation && task.source_platform === 'soundcloud'}
            <div class="status-line">
              <span class="spinner"></span>
              <span>
                Curating metadata with AI: {task.stats.ai_curation.processed} / {task.stats.ai_curation.total} tracks
              </span>
            </div>
          {:else if task.status === 'Running' && (task.stats?.downloaded ?? 0) === 0}
            <div class="status-line">
              <span class="spinner"></span>
              <span>Fetching tracks…</span>
            </div>
          {/if}

          {#if hasStats(task)}
            <div class="stats-row">
              {#if task.stats!.downloaded > 0}
                <span class="stat stat-ok">
                  <i class="lni lni-check-circle-1" aria-hidden="true"></i>
                  {task.stats!.downloaded} downloaded
                </span>
              {/if}
              {#if task.stats!.to_validate > 0}
                {#if task.stats!.to_validate_tracks && task.stats!.to_validate_tracks.length > 0}
                  <button
                    class="stat stat-warn stat-btn"
                    onclick={() => toggleValidations(task.id)}
                    title="View tracks pending validation"
                  >
                    <i class="lni lni-flag-1" aria-hidden="true"></i>
                    {task.stats!.to_validate} pending validation
                    <i class="lni {expandedValidations.has(task.id) ? 'lni-chevron-down' : 'lni-chevron-right'} chevron" aria-hidden="true"></i>
                  </button>
                {:else}
                  <button
                    class="stat stat-warn stat-btn"
                    onclick={() => onNavigateValidations?.()}
                    title="Go to Validations"
                  >
                    <i class="lni lni-flag-1" aria-hidden="true"></i>
                    {task.stats!.to_validate} pending validation
                  </button>
                {/if}
              {/if}
              {#if task.stats!.skipped > 0}
                <span class="stat stat-muted">
                  <i class="lni lni-undo" aria-hidden="true"></i>
                  {task.stats!.skipped} skipped
                </span>
              {/if}
              {#if task.stats!.errors.length > 0}
                <button
                  class="stat stat-err stat-btn"
                  onclick={() => toggleErrors(task.id)}
                  title="View error details"
                >
                  <i class="lni lni-xmark-circle" aria-hidden="true"></i>
                  {task.stats!.errors.length} error{task.stats!.errors.length > 1 ? 's' : ''}
                  <i class="lni {expandedErrors.has(task.id) ? 'lni-chevron-down' : 'lni-chevron-right'} chevron" aria-hidden="true"></i>
                </button>
              {/if}
            </div>

            {#if task.stats!.errors.length > 0 && expandedErrors.has(task.id)}
              <ul class="detail-list">
                {#each task.stats!.errors as err}
                  <li class="detail-row error-row">
                    {#if err.provider_url}
                      <a href={err.provider_url} target="_blank" rel="noopener noreferrer" class="detail-track detail-link">
                        {err.track}
                        <i class="lni lni-share-1 ext-icon" aria-hidden="true"></i>
                      </a>
                    {:else}
                      <span class="detail-track">{err.track}</span>
                    {/if}
                    <span class="detail-reason">{err.reason}</span>
                  </li>
                {/each}
              </ul>
            {/if}

            {#if task.stats!.to_validate_tracks && task.stats!.to_validate_tracks.length > 0 && expandedValidations.has(task.id)}
              <ul class="detail-list">
                {#each task.stats!.to_validate_tracks as item}
                  <li class="detail-row validation-row">
                    <span class="detail-track">{item.track}</span>
                    <span class="detail-reason">{reasonLabel(item.reason)}</span>
                  </li>
                {/each}
                <li class="detail-row detail-action">
                  <button class="btn-ghost btn-sm" onclick={() => onNavigateValidations?.()}>
                    Review in Validations <i class="lni lni-arrow-right" aria-hidden="true"></i>
                  </button>
                </li>
              </ul>
            {/if}
          {/if}

          {#if task.error}
            <div class="callout callout-error" role="alert">
              <i class="lni lni-error-circle" aria-hidden="true"></i>
              <div class="callout-body"><span>{task.error}</span></div>
            </div>
          {/if}

          {#if task.updated_at}
            <p class="task-date">{new Date(task.updated_at).toLocaleString()}</p>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .tasks-page {
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
    align-items: flex-start;
    gap: 1rem;
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

  /* ── Buttons ─────────────────────────────────────────────────────────── */
  .btn-ghost,
  .btn-danger {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
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
  .btn-ghost {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
  }
  .btn-ghost:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .btn-danger {
    background: transparent;
    border: 1px solid color-mix(in srgb, var(--error) 45%, transparent);
    color: var(--error);
  }
  .btn-danger:hover:not(:disabled) {
    background: var(--error-bg);
  }
  .btn-ghost:disabled,
  .btn-danger:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn-ghost .lni,
  .btn-danger .lni {
    font-size: 15px;
  }
  .btn-sm {
    padding: 0.4rem 0.8rem;
    font-size: 0.82rem;
  }

  /* ── Task list + panels ──────────────────────────────────────────────── */
  .task-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .task-panel {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 1rem 1.1rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
  }
  .task-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .task-ident {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    min-width: 0;
  }
  .task-label {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-bright);
  }
  .task-id {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--muted-2);
  }
  .task-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  /* ── Status badge ────────────────────────────────────────────────────── */
  .status-badge {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0.2rem 0.6rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    white-space: nowrap;
  }
  .status-badge.completed {
    background: color-mix(in srgb, var(--success) 18%, transparent);
    color: var(--success);
  }
  .status-badge.running,
  .status-badge.pending {
    background: var(--accent-muted);
    color: var(--accent-2);
  }
  .status-badge.failed {
    background: var(--error-bg);
    color: var(--error);
  }
  .status-badge.cancelled,
  .status-badge.cancelling {
    background: var(--surface-2);
    color: var(--muted);
  }

  /* ── Progress ────────────────────────────────────────────────────────── */
  .progress-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .progress-track {
    flex: 1;
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
  .progress-fill.completed {
    background: var(--success);
  }
  .progress-fill.cancelled {
    background: var(--muted-2);
  }
  .progress-label {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--muted-2);
    flex-shrink: 0;
  }

  /* ── Transient status line ───────────────────────────────────────────── */
  .status-line {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.85rem;
    color: var(--muted);
  }

  /* ── Stat pills ──────────────────────────────────────────────────────── */
  .stats-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.2rem 0.6rem;
    border-radius: 999px;
  }
  .stat .lni {
    font-size: 13px;
  }
  .stat-btn {
    border: none;
    font-family: inherit;
    cursor: pointer;
    transition: filter 0.12s ease;
  }
  .stat-btn:hover {
    filter: brightness(1.12);
  }
  .stat-ok {
    background: color-mix(in srgb, var(--success) 16%, transparent);
    color: var(--success);
  }
  .stat-warn {
    background: var(--warning-bg);
    color: var(--warning);
  }
  .stat-muted {
    background: var(--surface-2);
    color: var(--muted);
  }
  .stat-err {
    background: var(--error-bg);
    color: var(--error);
  }
  .stat .chevron {
    font-size: 11px;
  }

  /* ── Detail lists (errors + validations) ─────────────────────────────── */
  .detail-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .detail-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.4rem 0.65rem;
    border-radius: 8px;
    font-size: 0.82rem;
  }
  .error-row {
    background: color-mix(in srgb, var(--error) 10%, var(--panel));
    border: 1px solid color-mix(in srgb, var(--error) 30%, transparent);
  }
  .validation-row {
    background: color-mix(in srgb, var(--warning) 10%, var(--panel));
    border: 1px solid color-mix(in srgb, var(--warning) 28%, transparent);
  }
  .detail-track {
    color: var(--text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail-link {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    text-decoration: none;
  }
  .detail-link:hover {
    color: var(--text-bright);
  }
  .ext-icon {
    font-size: 12px;
    color: var(--muted-2);
    flex-shrink: 0;
  }
  .detail-reason {
    font-size: 0.75rem;
    color: var(--muted);
    flex-shrink: 0;
    text-align: right;
  }
  .detail-action {
    justify-content: flex-end;
    padding: 0.25rem 0;
    background: none;
    border: none;
  }

  /* ── Task error callout ──────────────────────────────────────────────── */
  .callout {
    display: flex;
    align-items: flex-start;
    gap: 0.7rem;
    padding: 0.7rem 0.9rem;
    border-radius: 10px;
    border: 1px solid transparent;
    font-size: 0.88rem;
  }
  .callout .lni {
    font-size: 18px;
    flex-shrink: 0;
  }
  .callout-body {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .callout-error {
    background: var(--error-bg);
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
    color: var(--error);
  }

  .task-date {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--muted-2);
  }

  /* ── Empty state ─────────────────────────────────────────────────────── */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.35rem;
    padding: 3rem 1rem;
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
    max-width: 40ch;
  }

  /* ── Loading skeleton ────────────────────────────────────────────────── */
  .task-panel.skeleton {
    gap: 0.85rem;
  }
  .sk {
    border-radius: 6px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s ease-in-out infinite;
  }
  .sk-head {
    height: 1rem;
    width: 40%;
  }
  .sk-bar {
    height: 6px;
    width: 100%;
  }
  .sk-stats {
    height: 0.9rem;
    width: 60%;
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

  @media (prefers-reduced-motion: reduce) {
    .sk,
    .spinner,
    .progress-fill,
    .chevron {
      animation: none;
      transition: none;
    }
  }

  @media (max-width: 640px) {
    .tasks-page {
      padding: 1.25rem 1rem 1.5rem;
    }
  }
</style>
