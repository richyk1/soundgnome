<script lang="ts">
  import { onMount } from 'svelte'
  import { onPWAUpdate, refreshPWA, isPWAAvailable } from '../lib/pwa'

  let showUpdatePrompt = false
  let unsubscribe: (() => void) | undefined

  onMount(() => {
    if (!isPWAAvailable()) {
      return
    }

    // Listen for PWA updates
    unsubscribe = onPWAUpdate(() => {
      showUpdatePrompt = true
    })

    return () => {
      unsubscribe?.()
    }
  })

  function handleUpdate() {
    refreshPWA()
    showUpdatePrompt = false
  }

  function handleDismiss() {
    showUpdatePrompt = false
  }
</script>

{#if showUpdatePrompt}
  <div class="pwa-update-prompt">
    <div class="pwa-update-content">
      <h3>Update available</h3>
      <p>A new version of Soundgnome is available.</p>
      <div class="pwa-update-actions">
        <button class="btn-secondary" on:click={handleDismiss}>
          Later
        </button>
        <button class="btn-primary" on:click={handleUpdate}>
          <i class="pxi pxi-refresh" aria-hidden="true"></i>Update
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Toast: a float panel above the phone dock, bottom-right on desktop. */
  .pwa-update-prompt {
    position: fixed;
    right: calc(var(--safe-right) + 24px);
    bottom: var(--app-bottom-clearance);
    z-index: 1000;
    width: min(340px, calc(100vw - 48px));
    animation: toast-in var(--motion-normal) var(--ease-out);
  }

  .pwa-update-content {
    padding: 16px;
    background: var(--float);
    border: 1px solid var(--float-border);
    border-radius: var(--radius-card);
    box-shadow: var(--float-shadow);
  }

  h3 {
    margin: 0 0 4px;
    font-size: 15px;
    line-height: 1.35;
    color: var(--text-bright);
  }

  p {
    margin: 0 0 14px;
    font-size: 14px;
    line-height: 1.45;
    color: var(--muted);
  }

  .pwa-update-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  @keyframes toast-in {
    from { opacity: 0; translate: 0 8px; }
    to { opacity: 1; translate: 0 0; }
  }

  @media (prefers-reduced-motion: reduce) {
    .pwa-update-prompt { animation: none; }
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .pwa-update-prompt {
      left: calc(var(--safe-left) + 16px);
      right: calc(var(--safe-right) + 16px);
      width: auto;
    }
    .pwa-update-actions button { flex: 1; }
  }
</style>
