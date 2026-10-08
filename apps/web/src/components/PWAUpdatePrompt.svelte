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
        <button class="btn-primary" on:click={handleUpdate}>
          Update
        </button>
        <button class="btn-secondary" on:click={handleDismiss}>
          Later
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .pwa-update-prompt {
    position: fixed;
    bottom: 20px;
    right: 20px;
    z-index: 1000;
    animation: slideIn 0.3s ease-in-out;
  }

  .pwa-update-content {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px;
    box-shadow: var(--shadow);
    max-width: 320px;
  }

  h3 {
    margin: 0 0 8px 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-bright);
  }

  p {
    margin: 0 0 16px 0;
    font-size: 14px;
    color: var(--muted);
  }

  .pwa-update-actions {
    display: flex;
    gap: 8px;
  }

  button {
    flex: 1;
    padding: 8px 12px;
    border: none;
    border-radius: 4px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .btn-primary {
    background: var(--accent);
    color: var(--on-accent);
  }

  .btn-primary:hover {
    background: var(--accent-strong);
  }

  .btn-secondary {
    background: var(--surface);
    color: var(--text-bright);
  }

  .btn-secondary:hover {
    background: var(--surface-2);
  }

  @keyframes slideIn {
    from {
      opacity: 0;
      transform: translateY(20px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (max-width: 480px) {
    .pwa-update-prompt {
      bottom: 10px;
      right: 10px;
      left: 10px;
    }

    .pwa-update-content {
      max-width: none;
    }
  }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    button { min-height: 44px; min-width: 44px; }
    .pwa-update-prompt { bottom: auto; top: calc(var(--app-top, 0px) + 1rem); left: 1rem; right: 1rem; max-height: calc(var(--app-height, 100dvh) - 2rem); overflow-y: auto; } .pwa-update-content { margin-left: auto; margin-right: auto; } button { min-height: 44px; }
  }
</style>
