import type { Action } from 'svelte/action';

const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

/**
 * Pops the element when `state` changes after mount (a like, play/pause,
 * shuffle), confirming the toggle. It never plays on first render, so rows
 * scrolling into view stay still.
 */
export const pop: Action<HTMLElement, unknown> = (node, state) => {
  let last = state;
  return {
    update(next) {
      if (Object.is(next, last)) return;
      last = next;
      if (reducedMotion.matches) return;
      node.animate(
        [{ transform: 'scale(1)' }, { transform: 'scale(1.25)', offset: 0.35 }, { transform: 'scale(1)' }],
        { duration: 260, easing: 'cubic-bezier(.16, 1, .3, 1)' },
      );
    },
  };
};
