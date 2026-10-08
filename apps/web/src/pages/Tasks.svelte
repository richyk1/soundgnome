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

  const runningCount = $derived(tasks.filter((t) => t.status === 'Running' || t.status === 'Pending').length);
  const failedCount = $derived(tasks.filter((t) => t.status === 'Failed').length);

  function statusIcon(status: TaskDto['status']) {
    return { Pending: 'pxi-hourglass', Running: 'pxi-loader pxi-spin', Completed: 'pxi-check', Failed: 'pxi-square-alert', Cancelled: 'pxi-close', Cancelling: 'pxi-loader pxi-spin' }[status] ?? '';
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
      {#if !loading}
        <p class="header-sub">{tasks.length} task{tasks.length === 1 ? '' : 's'} · {runningCount} running{failedCount > 0 ? ` · ${failedCount} failed` : ''}</p>
      {/if}
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
      <i class="pxi pxi-bulletlist" aria-hidden="true"></i>
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
              <span class="status-tag {statusClass(task.status)}">
                <i class="pxi {statusIcon(task.status)}" aria-hidden="true"></i>{statusLabel(task.status)}
              </span>
            </div>
            <div class="task-actions">
              {#if canCancel(task.status)}
                <button
                  class="btn-ghost btn-sm"
                  disabled={cancelling.has(task.id)}
                  onclick={() => handleCancel(task)}
                >
                  {#if cancelling.has(task.id)}
                    <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Cancelling…
                  {:else}
                    <i class="pxi pxi-close" aria-hidden="true"></i> Cancel
                  {/if}
                </button>
              {/if}
              {#if canRetry(task.status)}
                <button
                  class="btn-secondary btn-sm"
                  disabled={retrying.has(task.id)}
                  aria-busy={retrying.has(task.id)}
                  onclick={() => handleRetry(task)}
                >
                  {#if retrying.has(task.id)}
                    <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Retry
                  {:else}
                    <i class="pxi pxi-redo" aria-hidden="true"></i> Retry
                  {/if}
                </button>
              {/if}
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
              <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>
              <span>
                Curating metadata with AI: <span class="num">{task.stats.ai_curation.processed} / {task.stats.ai_curation.total}</span> tracks
              </span>
            </div>
          {:else if task.status === 'Running' && (task.stats?.downloaded ?? 0) === 0}
            <div class="status-line">
              <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>
              <span>Fetching tracks…</span>
            </div>
          {/if}

          {#if hasStats(task)}
            <div class="stats-row">
              {#if task.stats!.downloaded > 0}
                <span class="stat stat-ok">
                  <i class="pxi pxi-check" aria-hidden="true"></i>
                  {task.stats!.downloaded} downloaded
                </span>
              {/if}
              {#if task.stats!.to_validate > 0}
                {#if task.stats!.to_validate_tracks && task.stats!.to_validate_tracks.length > 0}
                  <button
                    class="stat stat-warn stat-btn"
                    onclick={() => toggleValidations(task.id)}
                    aria-expanded={expandedValidations.has(task.id)}
                    title="View tracks pending validation"
                  >
                    <i class="pxi pxi-flag" aria-hidden="true"></i>
                    {task.stats!.to_validate} pending validation
                    <i class="pxi {expandedValidations.has(task.id) ? 'pxi-chevron-down' : 'pxi-chevron-right'} chevron" aria-hidden="true"></i>
                  </button>
                {:else}
                  <button
                    class="stat stat-warn stat-btn"
                    onclick={() => onNavigateValidations?.()}
                    title="Go to Validations"
                  >
                    <i class="pxi pxi-flag" aria-hidden="true"></i>
                    {task.stats!.to_validate} pending validation
                    <i class="pxi pxi-arrow-right chevron" aria-hidden="true"></i>
                  </button>
                {/if}
              {/if}
              {#if task.stats!.skipped > 0}
                <span class="stat stat-muted">
                  <i class="pxi pxi-minus" aria-hidden="true"></i>
                  {task.stats!.skipped} skipped
                </span>
              {/if}
              {#if task.stats!.errors.length > 0}
                <button
                  class="stat stat-err stat-btn"
                  onclick={() => toggleErrors(task.id)}
                  aria-expanded={expandedErrors.has(task.id)}
                  title="View error details"
                >
                  <i class="pxi pxi-square-alert" aria-hidden="true"></i>
                  {task.stats!.errors.length} error{task.stats!.errors.length > 1 ? 's' : ''}
                  <i class="pxi {expandedErrors.has(task.id) ? 'pxi-chevron-down' : 'pxi-chevron-right'} chevron" aria-hidden="true"></i>
                </button>
              {/if}
            </div>

            {#if task.stats!.errors.length > 0 && expandedErrors.has(task.id)}
              <ul class="detail-list">
                {#each task.stats!.errors as err}
                  <li class="detail-row error-row">
                    {#if err.provider_url}
                      <a href={err.provider_url} target="_blank" rel="noopener noreferrer" class="detail-track detail-link">
                        <span class="detail-text">{err.track}</span>
                        <i class="pxi pxi-external-link ext-icon" aria-hidden="true"></i>
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
                  <button class="btn-secondary btn-sm" onclick={() => onNavigateValidations?.()}>
                    Review in Validations <i class="pxi pxi-arrow-right" aria-hidden="true"></i>
                  </button>
                </li>
              </ul>
            {/if}
          {/if}

          {#if task.error}
            <div class="callout callout-error" role="alert">
              <i class="pxi pxi-square-alert" aria-hidden="true"></i>
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
    display: flex;
    flex-direction: column;
    gap: 24px;
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-page);
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    margin: 0;
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

  /* ── Task list + panels ──────────────────────────────────────────────── */
  .task-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .task-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    transition: border-color var(--motion-fast) var(--ease-out);
  }
  .task-panel:hover {
    border-color: var(--border-strong);
  }
  .task-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .task-ident {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
    min-width: 0;
  }
  .task-label {
    min-width: 0;
    font-size: 15px;
    font-weight: 600;
    line-height: 1.35;
    color: var(--text-bright);
  }
  .task-id {
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }
  .task-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  /* ── Status tag: mono uppercase, tinted by its own color ─────────────── */
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
  .status-tag .pxi {
    font-size: 16px;
  }
  .status-tag.running { color: var(--live); }
  .status-tag.completed { color: var(--success); }
  .status-tag.failed { color: var(--error); }
  .status-tag.pending,
  .status-tag.cancelled,
  .status-tag.cancelling { color: var(--muted); }

  /* ── Progress ────────────────────────────────────────────────────────── */
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
  .progress-fill {
    width: 100%;
    height: 100%;
    background: var(--live);
    transform-origin: left;
    transition: transform var(--motion-normal) var(--ease-out);
  }
  .progress-fill.completed { background: var(--success); }
  .progress-fill.cancelled { background: var(--muted-2); }
  .progress-label {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }

  /* ── Transient status line ───────────────────────────────────────────── */
  .status-line {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--muted);
  }
  .status-line .pxi {
    flex-shrink: 0;
    font-size: 16px;
    color: var(--live);
  }
  .num {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  /* ── Stats: pixel icon + mono tabular figure ─────────────────────────── */
  .stats-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 16px;
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .stat .pxi {
    font-size: 16px;
    color: var(--muted-2);
  }
  .stat-btn {
    min-height: 30px;
    margin: 0 -6px;
    padding: 0 6px;
    border: 0;
    border-radius: var(--radius-chip);
    background: none;
    cursor: pointer;
  }
  .stat-btn:hover {
    background: var(--surface-2);
    color: var(--text-bright);
  }
  .stat-ok .pxi { color: var(--success); }
  .stat-warn .pxi { color: var(--warning); }
  .stat-muted { color: var(--muted); }
  .stat-err { color: var(--error); }
  .stat-err .pxi { color: var(--error); }
  .stat .chevron { color: var(--muted-2); }

  /* ── Detail lists (errors + validations) ─────────────────────────────── */
  .detail-list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
  }
  .detail-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 40px;
    padding: 8px 12px;
    font-size: 13px;
  }
  .detail-row + .detail-row {
    border-top: 1px solid var(--border-soft);
  }
  .detail-track {
    min-width: 0;
    overflow: hidden;
    color: var(--text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail-link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    text-decoration: none;
  }
  .detail-text {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .detail-link:hover {
    color: var(--accent);
  }
  .detail-link:hover .detail-text {
    text-decoration: underline;
  }
  .ext-icon {
    flex-shrink: 0;
    font-size: 16px;
    color: var(--muted-2);
  }
  .detail-link:hover .ext-icon {
    color: var(--accent);
  }
  .detail-reason {
    flex-shrink: 0;
    font-size: 12px;
    text-align: right;
    color: var(--muted);
  }
  .error-row .detail-reason { color: var(--error); }
  .detail-action {
    justify-content: flex-end;
  }

  .task-date {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }

  /* ── Loading skeleton ────────────────────────────────────────────────── */
  .sk {
    border-radius: 4px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s ease-in-out infinite;
  }
  .sk-head { width: 40%; height: 16px; }
  .sk-bar { width: 100%; height: 4px; }
  .sk-stats { width: 60%; height: 14px; }
  @keyframes sk-pulse {
    50% { opacity: 0.45; }
  }

  @media (prefers-reduced-motion: reduce) {
    .sk { animation: none; }
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    h1 { font-size: 28px; }
    .stat-btn { min-height: 44px; }
    .task-ident { flex: 1 1 100%; }
    .task-actions { flex-wrap: wrap; }
    .detail-row { flex-wrap: wrap; }
    .detail-reason { text-align: left; overflow-wrap: anywhere; }
    .callout-body { overflow-wrap: anywhere; }
  }
</style>
