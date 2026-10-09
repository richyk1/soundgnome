<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getTasks } from './api';
  import type { TaskDto, TaskType } from './types';

  interface Props {
    /** Page heading, e.g. "Fingerprint library". */
    title: string;
    /** One or two sentences explaining what the pass does. */
    description: string;
    /** Pixel icon name (pxi-*) without the prefix, e.g. "audio-waveform". */
    icon: string;
    /** Task type this panel tracks. */
    taskType: TaskType;
    /** Verb for successful items, e.g. "fingerprinted" / "embedded". */
    okLabel: string;
    /** Optional footnote (idempotency, cost, etc.). */
    note?: string;
    /** Short explanation of what "skipped" means for this pass. */
    skipHint?: string;
    /** Kick off the pass; resolves with the tracking task id. */
    start: () => Promise<{ task_id: number }>;
  }

  let { title, description, icon, taskType, okLabel, note, skipHint, start }: Props = $props();

  let task = $state<TaskDto | null>(null);
  let starting = $state(false);
  let errorMsg = $state<string | null>(null);
  let pollTimer: number | null = null;

  const active = $derived(task?.status === 'Pending' || task?.status === 'Running');
  const fill = $derived(task && task.total ? Math.min(1, task.progress / task.total) : 0);
  const pct = $derived(task && task.total ? Math.round(fill * 100) : 0);

  async function refresh() {
    try {
      const tasks = await getTasks();
      const mine = tasks.filter((t) => t.task_type === taskType);
      task = mine.length ? mine.reduce((a, b) => (b.id > a.id ? b : a)) : null;
    } catch {
      /* transient poll failure: keep showing the last known state */
    }
  }

  async function run() {
    starting = true;
    errorMsg = null;
    try {
      await start();
      await refresh();
    } catch (err: unknown) {
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      starting = false;
    }
  }

  onMount(() => {
    refresh();
    pollTimer = setInterval(refresh, 2000);
  });
  onDestroy(() => {
    if (pollTimer !== null) clearInterval(pollTimer);
  });
</script>

<section class="backfill">
  <header class="bf-head">
    <div class="bf-heading">
      <i class="pxi pxi-{icon}" aria-hidden="true"></i>
      <h2>{title}</h2>
    </div>
    <button class="btn-primary bf-run" onclick={run} disabled={active || starting}>
      {#if starting}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Starting{:else if active}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Running{:else}<i class="pxi pxi-play" aria-hidden="true"></i>{task?.status === 'Completed' ? 'Run again' : 'Run'}{/if}
    </button>
  </header>

  <p class="bf-desc">{description}</p>

  {#if errorMsg}
    <div class="callout callout-error" role="alert">
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>
      <div class="callout-body"><strong>Couldn't start.</strong><span>{errorMsg}</span></div>
    </div>
  {/if}

  {#if task}
    <div class="bf-run-state">
      <div class="bf-status">
        {#if task.status === 'Running'}
          <span class="bf-state is-running"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Running</span>
        {:else if task.status === 'Pending'}
          <span class="bf-state is-pending"><i class="pxi pxi-hourglass" aria-hidden="true"></i>Queued</span>
        {:else if task.status === 'Completed'}
          <span class="bf-state is-done"><i class="pxi pxi-check" aria-hidden="true"></i>Completed</span>
        {:else if task.status === 'Failed'}
          <span class="bf-state is-failed"><i class="pxi pxi-square-alert" aria-hidden="true"></i>Failed</span>
        {:else}
          <span class="bf-state">{task.status}</span>
        {/if}
        {#if task.total}
          <span class="bf-count">{task.progress} / {task.total} tracks · {pct}%</span>
        {:else if active}
          <span class="bf-count">Preparing…</span>
        {/if}
      </div>

      {#if task.total}
        <div class="bf-track" role="progressbar" aria-valuenow={pct} aria-valuemin="0" aria-valuemax="100">
          <div class="bf-fill" class:is-done={task.status === 'Completed'} class:is-failed={task.status === 'Failed'} style="transform: scaleX({fill})"></div>
        </div>
      {/if}

      {#if task.stats?.backfill}
        <div class="bf-stats">
          <span class="stat ok"><i class="pxi pxi-check" aria-hidden="true"></i>{task.stats.backfill.ok} {okLabel}</span>
          <span class="stat skip"><i class="pxi pxi-minus" aria-hidden="true"></i>{task.stats.backfill.skipped} skipped</span>
          {#if task.stats.backfill.errors > 0}
            <span class="stat err"><i class="pxi pxi-square-alert" aria-hidden="true"></i>{task.stats.backfill.errors} errors</span>
          {/if}
        </div>
        {#if skipHint && task.stats.backfill.skipped > 0}
          <p class="bf-hint">{skipHint}</p>
        {/if}
      {/if}

      {#if task.error}
        <p class="bf-error">{task.error}</p>
      {/if}
      {#if task.status === 'Completed' && task.updated_at}
        <p class="bf-when">Last run {task.updated_at.replace('T', ' ').slice(0, 19)}</p>
      {/if}
    </div>
  {:else}
    <div class="empty bf-idle">
      <i class="pxi pxi-{icon}" aria-hidden="true"></i>
      <p class="empty-title">Not run yet</p>
      <p class="empty-hint">Press <strong>Run</strong> to start.</p>
    </div>
  {/if}

  {#if note}
    <p class="bf-note">{note}</p>
  {/if}
</section>

<style>
  .backfill {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 720px;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }
  .bf-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .bf-heading {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .bf-heading .pxi {
    flex-shrink: 0;
    font-size: 16px;
    color: var(--muted);
  }
  .bf-heading h2 {
    margin: 0;
    font-size: 18px;
    line-height: 1.3;
  }
  .bf-run {
    flex-shrink: 0;
  }
  .bf-desc {
    margin: 0;
    max-width: 64ch;
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }

  /* The latest run: a hairline-separated section of the panel, not a nested card. */
  .bf-run-state {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  .bf-status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  /* Mono status tag, shared look with the Activity page. */
  .bf-state {
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
  .bf-state .pxi {
    font-size: 16px;
  }
  .bf-state.is-running { color: var(--live); }
  .bf-state.is-pending { color: var(--muted); }
  .bf-state.is-done,
  .bf-state .pxi-check { color: var(--success); }
  .bf-state.is-failed,
  .bf-state .pxi-square-alert { color: var(--error); }
  .bf-count {
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }

  .bf-track {
    height: 4px;
    overflow: hidden;
    background: var(--border);
  }
  .bf-fill {
    width: 100%;
    height: 100%;
    background: var(--live);
    transform-origin: left;
    transition: transform var(--motion-normal) var(--ease-out);
  }
  .bf-fill.is-done { background: var(--success); }
  .bf-fill.is-failed { background: var(--error); }

  .bf-stats {
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
  .stat.ok .pxi { color: var(--success); }
  .stat.err { color: var(--error); }
  .stat.err .pxi { color: var(--error); }

  .bf-error {
    margin: 0;
    font-size: 14px;
    line-height: 1.45;
    color: var(--error);
  }
  .bf-when {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--muted-2);
  }
  .bf-hint {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
    color: var(--muted-2);
  }

  .bf-idle {
    padding: 32px 16px;
    border-top: 1px solid var(--border);
  }

  .bf-note {
    margin: 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted-2);
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .backfill { padding: 16px; }
    .bf-head { flex-wrap: wrap; }
    .bf-desc, .bf-error, .bf-heading h2 { overflow-wrap: anywhere; }
  }
</style>
