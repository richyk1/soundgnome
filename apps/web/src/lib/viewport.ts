export function observeViewport(): () => void {
  const viewport = window.visualViewport;
  const root = document.documentElement;
  let frame = 0;

  function update() {
    frame = 0;
    const height = viewport?.height ?? window.innerHeight;
    const editable = !!document.activeElement?.matches('input:not([type="range"]):not([type="checkbox"]):not([type="radio"]), textarea, select, [contenteditable="true"]');
    const compactEditing = (editable || root.hasAttribute('data-compact-editing'))
      && height * (viewport?.scale ?? 1) < window.innerHeight - 150;
    root.toggleAttribute('data-editing', editable);
    root.toggleAttribute('data-compact-editing', compactEditing);
    // Let pinch zoom magnify the layout rather than reflowing it at every zoom step.
    if (viewport && viewport.scale !== 1) return;
    const top = viewport?.offsetTop ?? 0;
    root.style.setProperty('--app-height', `${height}px`);
    root.style.setProperty('--app-top', `${top}px`);
  }

  function schedule() {
    if (!frame) frame = requestAnimationFrame(update);
  }

  update();
  viewport?.addEventListener('resize', schedule);
  viewport?.addEventListener('scroll', schedule);
  window.addEventListener('resize', schedule);
  window.addEventListener('pageshow', schedule);
  document.addEventListener('focusin', schedule);
  document.addEventListener('focusout', schedule);

  return () => {
    cancelAnimationFrame(frame);
    viewport?.removeEventListener('resize', schedule);
    viewport?.removeEventListener('scroll', schedule);
    window.removeEventListener('resize', schedule);
    window.removeEventListener('pageshow', schedule);
    document.removeEventListener('focusin', schedule);
    document.removeEventListener('focusout', schedule);
    root.removeAttribute('data-editing');
    root.removeAttribute('data-compact-editing');
  };
}
