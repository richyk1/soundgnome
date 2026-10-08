<script lang="ts">
  import { onMount } from 'svelte';

  // `beforeinstallprompt` isn't in the DOM lib; minimal shape for what we use.
  interface BeforeInstallPromptEvent extends Event {
    prompt: () => Promise<void>;
    userChoice: Promise<{ outcome: string }>;
  }

  let deferred: BeforeInstallPromptEvent | null = $state(null);
  let installed = $state(false);
  let isIOS = $state(false);
  let showIosHint = $state(false);

  function isStandalone(): boolean {
    // navigator.standalone is iOS-only and not in the TS lib.
    const nav = navigator as Navigator & { standalone?: boolean };
    return (
      window.matchMedia?.('(display-mode: standalone)').matches === true || nav.standalone === true
    );
  }

  onMount(() => {
    if (isStandalone()) {
      installed = true;
      return;
    }
    isIOS = /iphone|ipad|ipod/i.test(navigator.userAgent);

    const onPrompt = (e: Event) => {
      e.preventDefault();
      // Browser install event; no standard TS type for it.
      deferred = e as BeforeInstallPromptEvent;
    };
    const onInstalled = () => {
      installed = true;
      deferred = null;
    };
    window.addEventListener('beforeinstallprompt', onPrompt);
    window.addEventListener('appinstalled', onInstalled);
    return () => {
      window.removeEventListener('beforeinstallprompt', onPrompt);
      window.removeEventListener('appinstalled', onInstalled);
    };
  });

  async function install() {
    if (deferred) {
      await deferred.prompt();
      try {
        await deferred.userChoice;
      } catch {
        /* dismissed */
      }
      deferred = null;
    } else if (isIOS) {
      showIosHint = !showIosHint;
    }
  }
</script>

{#if !installed && (deferred || isIOS)}
  <button class="btn-ghost install-btn" onclick={install}>
    <i class="pxi pxi-download" aria-hidden="true"></i>Install app
  </button>
  {#if showIosHint}
    <p class="ios-hint">
      Tap the <strong>Share</strong> icon in Safari, then <strong>Add to Home Screen</strong>.
    </p>
  {/if}
{/if}

<style>
  /* Secondary hairline control, left-aligned like the sidebar nav rows. */
  .install-btn {
    justify-content: flex-start;
    gap: 10px;
    width: 100%;
    padding: 0 10px;
    margin-bottom: 8px;
    text-align: left;
  }
  .ios-hint {
    margin: 0 0 8px;
    padding: 0 10px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }
  .ios-hint strong {
    font-weight: 600;
    color: var(--text);
  }
</style>
