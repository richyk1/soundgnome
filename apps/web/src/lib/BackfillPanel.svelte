<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getTasks } from './api';
  import type { TaskDto, TaskType } from './types';

  interface Props {
    /** Page heading, e.g. "Fingerprint library". */
    title: string;
    /** One or two sentences explaining what the pass does. */
    description: string;
    /** Lineicon class suffix, e.g. "fingerprint-1". */
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
      <i class="lni lni-{icon}" aria-hidden="true"></i>
      <h2>{title}</h2>
    </div>
    <button class="btn-accent" onclick={run} disabled={active || starting}>
      {#if starting}<span class="spinner"></span>Starting{:else if active}<span class="spinner"></span>Running{:else}{task?.status === 'Completed' ? 'Run again' : 'Run'}{/if}
    </button>
  </header>

  <p class="bf-desc">{description}</p>

  {#if errorMsg}
    <div class="callout callout-error" role="alert">
      <i class="lni lni-xmark-circle" aria-hidden="true"></i>
      <div class="callout-body"><strong>Couldn't start.</strong><span>{errorMsg}</span></div>
    </div>
  {/if}

  {#if task}
    <div class="bf-card" class:done={task.status === 'Completed'} class:failed={task.status === 'Failed'}>
      <div class="bf-status">
        <span class="bf-state">
          {#if task.status === 'Running'}
            <span class="spinner"></span>Running
          {:else if task.status === 'Pending'}
            <span class="spinner"></span>Queued
          {:else if task.status === 'Completed'}
            <i class="lni lni-checkmark-circle" aria-hidden="true"></i>Completed
          {:else if task.status === 'Failed'}
            <i class="lni lni-xmark-circle" aria-hidden="true"></i>Failed
          {:else}
            {task.status}
          {/if}
        </span>
        {#if task.total}
          <span class="bf-count">{task.progress} / {task.total} tracks · {pct}%</span>
        {:else if active}
          <span class="bf-count">Preparing…</span>
        {/if}
      </div>

      {#if task.total}
        <div class="bf-track" role="progressbar" aria-valuenow={pct} aria-valuemin="0" aria-valuemax="100">
          <div class="bf-fill" style="transform: scaleX({fill})"></div>
        </div>
      {/if}

      {#if task.stats?.backfill}
        <div class="bf-stats">
          <span class="stat ok">{task.stats.backfill.ok} {okLabel}</span>
          <span class="stat">{task.stats.backfill.skipped} skipped</span>
          {#if task.stats.backfill.errors > 0}
            <span class="stat err">{task.stats.backfill.errors} errors</span>
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
    <div class="bf-idle">
      <i class="lni lni-{icon}" aria-hidden="true"></i>
      <p>Not run yet. Press <strong>Run</strong> to start.</p>
    </div>
  {/if}

  {#if note}
    <p class="bf-note">{note}</p>
  {/if}
</section>

<style>
  /* Button + spinner + callout: the app's shared classes are page-scoped, so a
     standalone component must restate them to stay on-theme. */
  .btn-accent {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.55rem 1rem;
    border: none;
    border-radius: 8px;
    background: var(--accent);
    color: #fff;
    font-family: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition:
      filter 0.12s ease,
      opacity 0.12s ease;
  }
  .btn-accent:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .btn-accent:disabled {
    opacity: 0.45;
    cursor: default;
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

  .callout {
    display: flex;
    align-items: flex-start;
    gap: 0.7rem;
    padding: 0.85rem 1rem;
    border-radius: 10px;
    border: 1px solid transparent;
    font-size: 0.9rem;
  }
  .callout-error {
    background: var(--error-bg);
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
    color: var(--error);
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

  .backfill {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    max-width: 720px;
  }
  .bf-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }
  .bf-heading {
    display: inline-flex;
    align-items: center;
    gap: 0.6rem;
  }
  .bf-heading .lni {
    font-size: 20px;
    color: var(--accent);
  }
  .bf-heading h2 {
    margin: 0;
    font-size: 1.25rem;
  }
  .bf-desc {
    margin: 0;
    color: var(--muted);
    line-height: 1.5;
  }

  .bf-card {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    padding: 1.1rem 1.25rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
  }
  .bf-card.done {
    border-color: color-mix(in srgb, var(--success) 45%, var(--border-soft));
  }
  .bf-card.failed {
    border-color: color-mix(in srgb, var(--error) 45%, var(--border-soft));
  }
  .bf-status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .bf-state {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
    color: var(--text-bright);
  }
  .bf-state .lni-checkmark-circle {
    color: var(--success);
    font-size: 18px;
  }
  .bf-state .lni-xmark-circle {
    color: var(--error);
    font-size: 18px;
  }
  .bf-count {
    font-family: var(--font-mono);
    font-size: 0.85rem;
    color: var(--muted);
  }

  .bf-track {
    height: 8px;
    border-radius: 999px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .bf-fill {
    height: 100%;
    width: 100%;
    transform-origin: left;
    background: var(--accent);
    transition: transform 0.3s ease;
  }

  .bf-stats {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .stat {
    font-size: 0.8rem;
    padding: 0.2rem 0.6rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .stat.ok {
    background: color-mix(in srgb, var(--success) 18%, var(--surface-2));
    color: var(--success);
  }
  .stat.err {
    background: color-mix(in srgb, var(--error) 18%, var(--surface-2));
    color: var(--error);
  }

  .bf-error {
    margin: 0;
    font-size: 0.85rem;
    color: var(--error);
  }
  .bf-when {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted-2);
  }
  .bf-hint {
    margin: 0.1rem 0 0;
    font-size: 0.78rem;
    color: var(--muted-2);
  }

  .bf-idle {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    padding: 2rem 1rem;
    text-align: center;
    color: var(--muted);
    background: var(--surface);
    border: 1px dashed var(--border);
    border-radius: 12px;
  }
  .bf-idle .lni {
    font-size: 28px;
    color: var(--muted-2);
  }
  .bf-idle p {
    margin: 0;
  }

  .bf-note {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted-2);
    line-height: 1.5;
  }

  @media (prefers-reduced-motion: reduce) {
    .bf-fill,
    .spinner {
      transition: none;
      animation: none;
    }
  }
</style>
