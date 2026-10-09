<script lang="ts">
  import { onDestroy } from 'svelte';
  import { fly } from 'svelte/transition';

  type Status = 'idle' | 'loading' | 'success' | 'error';

  interface Props {
    /** Async work to run on click. The button owns the loading/success/error state. */
    action: () => Promise<void>;
    label?: string;
    /** Pixel icon name (the `pxi-NAME` suffix) shown before the label in the idle state. */
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
  class="sbtn sbtn-{size} {variant === 'primary' ? 'btn-primary' : variant === 'danger' ? 'btn-danger' : 'btn-secondary'}"
  class:btn-sm={size === 'sm'}
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
    <span class="sbtn-in" in:fly={{ y: 4, duration: reduce ? 0 : 150 }}>
      {#if status === 'loading'}
        <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>
      {:else if status === 'success'}
        <i class="pxi pxi-check" aria-hidden="true"></i>
      {:else if status === 'error'}
        <i class="pxi pxi-close" aria-hidden="true"></i>
      {:else}
        {#if icon}<i class="pxi pxi-{icon}" aria-hidden="true"></i>{/if}
        {#if label}<span>{label}</span>{/if}
      {/if}
    </span>
  {/key}
</button>

<style>
  /* The look comes from the global .btn-primary / .btn-danger / .btn-secondary
     (+ .btn-sm); this only keeps the width steady while the content swaps and
     paints the transient status. */
  .sbtn {
    overflow: hidden;
    line-height: 1;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      border-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out);
  }
  .sbtn-md { min-width: 90px; }
  .sbtn-sm { min-width: 74px; }

  .sbtn-in {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .sbtn-sm .sbtn-in { gap: 6px; }

  /* Loading is busy, not unavailable: keep full strength while disabled. */
  .sbtn.is-loading:disabled {
    opacity: 1;
    cursor: progress;
  }

  /* Status overrides apply across variants for a clear signal. */
  .sbtn.is-success,
  .sbtn.is-success:hover:not(:disabled) {
    background: var(--success);
    border-color: transparent;
    color: var(--on-success);
  }
  .sbtn.is-error,
  .sbtn.is-error:hover:not(:disabled) {
    background: var(--error);
    border-color: transparent;
    color: var(--on-error);
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .sbtn { min-height: 44px; }
  }
</style>
