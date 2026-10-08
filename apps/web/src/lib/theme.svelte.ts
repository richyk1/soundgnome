export type Theme = 'light' | 'dark';

// Seeded by the pre-paint script in index.html; observeTheme keeps it current.
export const theme = $state<{ current: Theme }>({
  current: document.documentElement.dataset.theme === 'light' ? 'light' : 'dark',
});

export function observeTheme(): () => void {
  const root = document.documentElement;
  let timer: number | undefined;

  function update() {
    window.clearTimeout(timer);
    const now = new Date();
    const hour = now.getHours();
    // Local daylight picks the theme: light 07:00–19:00, dark otherwise.
    theme.current = hour >= 7 && hour < 19 ? 'light' : 'dark';
    root.dataset.theme = theme.current;
    const background = getComputedStyle(root).getPropertyValue('--bg').trim();
    document.querySelector('meta[name="theme-color"]')?.setAttribute('content', background);
    document.querySelector('meta[name="apple-mobile-web-app-status-bar-style"]')
      ?.setAttribute('content', theme.current === 'light' ? 'default' : 'black-translucent');

    const next = new Date(now);
    if (hour < 7) {
      next.setHours(7, 0, 0, 0);
    } else if (hour < 19) {
      next.setHours(19, 0, 0, 0);
    } else {
      next.setDate(next.getDate() + 1);
      next.setHours(7, 0, 0, 0);
    }
    timer = window.setTimeout(update, next.getTime() - now.getTime() + 1);
  }

  function onVisibility() {
    if (document.visibilityState === 'visible') update();
  }

  update();
  window.addEventListener('focus', update);
  document.addEventListener('visibilitychange', onVisibility);
  return () => {
    window.clearTimeout(timer);
    window.removeEventListener('focus', update);
    document.removeEventListener('visibilitychange', onVisibility);
  };
}
