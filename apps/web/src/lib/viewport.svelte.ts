import { MediaQuery } from 'svelte/reactivity';

/** Phone layout, matching the CSS breakpoint used across the app. Reactive via `phone.current`. */
export const phone = new MediaQuery('(max-width: 860px), (hover: none) and (pointer: coarse)');
