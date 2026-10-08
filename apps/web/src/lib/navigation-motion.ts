import { tick } from 'svelte';

let latestRequest = 0;
let activeTransition: ViewTransition | undefined;
let cleanupFallback: (() => void) | undefined;
let fallbackVariant = false;

function animateFallback() {
  const panel = document.querySelector<HTMLElement>('.content-panel');
  if (!panel) return;

  // Alternate names to restart on rapid navigation without forcing a layout.
  fallbackVariant = !fallbackVariant;
  const className = fallbackVariant ? 'navigation-enter-a' : 'navigation-enter-b';
  panel.classList.add(className);

  function cleanup() {
    panel?.classList.remove(className);
    panel?.removeEventListener('animationend', onEnd);
    panel?.removeEventListener('animationcancel', onEnd);
    if (cleanupFallback === cleanup) cleanupFallback = undefined;
  }
  function onEnd(event: AnimationEvent) {
    if (event.target === panel) cleanup();
  }
  panel.addEventListener('animationend', onEnd);
  panel.addEventListener('animationcancel', onEnd);
  cleanupFallback = cleanup;
}

// Route mutations belong in update, not in a later fetch completion. A skipped
// native transition still invokes its callback, so guard that callback too.
export async function runNavigation(update: () => void | Promise<void>): Promise<void> {
  const request = ++latestRequest;
  const apply = async () => {
    if (request !== latestRequest) return;
    await update();
    await tick();
  };

  try {
    activeTransition?.skipTransition();
    activeTransition = undefined;
    cleanupFallback?.();

    if (typeof document === 'undefined'
      || window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      await apply();
      return;
    }

    if (typeof document.startViewTransition === 'function') {
      const root = document.documentElement;
      // Names the page area for this transition only; see App.svelte.
      root.classList.add('page-nav');
      let transition: ViewTransition;
      try {
        transition = document.startViewTransition(apply);
      } catch {
        // A browser may expose the API while being unable to start a snapshot.
        root.classList.remove('page-nav');
        await apply();
        if (request === latestRequest) animateFallback();
        return;
      }
      activeTransition = transition;
      // Skipping rejects ready, but doesn't cancel the DOM update. Handle every
      // native promise without letting an older completion clear a newer one.
      void transition.ready.catch(() => {});
      const clear = () => {
        if (activeTransition !== transition) return;
        activeTransition = undefined;
        root.classList.remove('page-nav');
      };
      void transition.finished.then(clear, clear);
      await transition.updateCallbackDone;
      return;
    }

    await apply();
    if (request === latestRequest) animateFallback();
  } catch (error) {
    console.error('Navigation update failed', error);
  }
}
