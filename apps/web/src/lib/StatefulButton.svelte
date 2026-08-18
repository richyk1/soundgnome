<script lang="ts">
  import { onDestroy } from 'svelte';
  import { fly } from 'svelte/transition';

  type Status = 'idle' | 'loading' | 'success' | 'error';

  interface Props {
    /** Async work to run on click. The button owns the loading/success/error state. */
    action: () => Promise<void>;
    label?: string;
    /** Lineicon suffix shown before the label in the idle state. */
    icon?: string;
    variant?: 'primary' | 'ghost' | 'danger';
    size?: 'md' | 'sm';
    disabled?: boolean;
    title?: string;
    /** Reports the failure reason (or null to clear) so the caller can show it inline. */
    onError?: (message: string | null) => void;
    onSuccess?: () => void;
  }

  let {
    action,
    label,
    icon,
    variant = 'primary',
    size = 'md',
    disabled = false,
    title,
    onError,
    onSuccess,
  }: Props = $props();

  let status: Status = $state('idle');
  let timer: number | null = null;

  const reduce =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  async function run() {
    if (status === 'loading' || disabled) return;
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    status = 'loading';
    onError?.(null);
    try {
      await action();
      status = 'success';
      onSuccess?.();
      timer = setTimeout(() => (status = 'idle'), 1500);
    } catch (e: unknown) {
      status = 'error';
      const msg = e instanceof Error ? e.message : String(e);
      // Drop the internal "custom error:" prefix from domain errors.
      onError?.(msg.replace(/^custom error:\s*/i, ''));
      timer = setTimeout(() => (status = 'idle'), 2400);
    }
  }

  onDestroy(() => {
    if (timer !== null) clearTimeout(timer);
  });
</script>

<button
  type="button"
  class="sbtn sbtn-{variant} sbtn-{size}"
  class:is-loading={status === 'loading'}
  class:is-success={status === 'success'}
  class:is-error={status === 'error'}
  {title}
  disabled={disabled || status === 'loading'}
  aria-busy={status === 'loading'}
  aria-label={label ?? title}
  onclick={run}
>
  {#key status}
    <span class="sbtn-in" in:fly={{ y: 5, duration: reduce ? 0 : 150 }}>
      {#if status === 'loading'}
        <span class="sbtn-spin" aria-hidden="true"></span>
      {:else if status === 'success'}
        <i class="lni lni-check" aria-hidden="true"></i>
      {:else if status === 'error'}
        <i class="lni lni-xmark" aria-hidden="true"></i>
      {:else}
        {#if icon}<i class="lni lni-{icon}" aria-hidden="true"></i>{/if}
        {#if label}<span>{label}</span>{/if}
      {/if}
    </span>
  {/key}
</button>

<style>
  .sbtn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: 8px;
    font: inherit;
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
    cursor: pointer;
    overflow: hidden;
    transition:
      background 0.15s ease,
      border-color 0.15s ease,
      color 0.15s ease,
      transform 0.08s ease;
  }
  .sbtn-md {
    min-width: 90px;
    height: 34px;
    padding: 0 0.9rem;
    font-size: 0.85rem;
  }
  .sbtn-sm {
    min-width: 74px;
    height: 30px;
    padding: 0 0.75rem;
    font-size: 0.8rem;
  }
  .sbtn:active:not(:disabled) {
    transform: scale(0.97);
  }
  .sbtn:disabled {
    cursor: default;
  }

  .sbtn-in {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }
  .sbtn-in .lni {
    font-size: 15px;
  }

  /* Variants (idle) */
  .sbtn-primary {
    background: var(--accent);
    color: #fff;
  }
  .sbtn-primary:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .sbtn-ghost {
    background: transparent;
    color: var(--muted);
    border-color: var(--border);
  }
  .sbtn-ghost:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--text);
  }
  .sbtn-danger {
    background: transparent;
    color: var(--muted);
    border-color: var(--border);
  }
  .sbtn-danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--error) 12%, transparent);
    color: var(--error);
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
  }

  /* Status overrides (apply across variants for a clear signal) */
  .sbtn.is-success {
    background: var(--success);
    color: #fff;
    border-color: transparent;
  }
  .sbtn.is-error {
    background: var(--error);
    color: #fff;
    border-color: transparent;
    animation: sbtn-shake 0.42s ease;
  }

  .sbtn-spin {
    width: 14px;
    height: 14px;
    border: 2px solid color-mix(in srgb, currentColor 30%, transparent);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: sbtn-rot 0.7s linear infinite;
  }

  @keyframes sbtn-rot {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes sbtn-shake {
    10%,
    90% {
      transform: translateX(-1px);
    }
    20%,
    80% {
      transform: translateX(2px);
    }
    30%,
    50%,
    70% {
      transform: translateX(-3px);
    }
    40%,
    60% {
      transform: translateX(3px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .sbtn.is-error {
      animation: none;
    }
    .sbtn:active:not(:disabled) {
      transform: none;
    }
    .sbtn-spin {
      animation-duration: 1.3s;
    }
  }
</style>
