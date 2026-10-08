// A light tap of haptic feedback for every button press made by touch.
// Android exposes the Vibration API. iOS Safari does not, but toggling a native
// switch control plays the system selection haptic (Safari 18+), so a hidden
// <input type="checkbox" switch> is clicked from inside the user's own gesture.
// The OS "System Haptics" setting still decides whether anything is felt.

const PRESSABLE = 'button:not(:disabled), [role="button"]:not([aria-disabled="true"]), a[href], summary';

export function installHaptics(): () => void {
  const label = document.createElement('label');
  label.setAttribute('aria-hidden', 'true');
  label.style.cssText = 'position:fixed;left:-100px;top:0;width:1px;height:1px;overflow:hidden;opacity:0;pointer-events:none';
  const input = document.createElement('input');
  input.type = 'checkbox';
  input.setAttribute('switch', '');
  input.tabIndex = -1;
  label.append(input);
  document.body.append(label);

  let lastPointer = '';
  const onPointerDown = (event: PointerEvent) => { lastPointer = event.pointerType; };

  function onClick(event: MouseEvent) {
    const target = event.target as Element | null;
    if (lastPointer !== 'touch' || !target || label.contains(target) || !target.closest(PRESSABLE)) return;
    if ('vibrate' in navigator) {
      navigator.vibrate(8);
      return;
    }
    // A modal <dialog> makes everything outside it inert, so the switch rides along.
    (target.closest('dialog[open]') ?? document.body).append(label);
    // Clicking the label must not steal focus from dialogs or fields.
    const focused = document.activeElement as HTMLElement | null;
    label.click();
    if (focused && document.activeElement !== focused) focused.focus({ preventScroll: true });
  }

  document.addEventListener('pointerdown', onPointerDown, { capture: true, passive: true });
  document.addEventListener('click', onClick, { capture: true });
  return () => {
    document.removeEventListener('pointerdown', onPointerDown, { capture: true });
    document.removeEventListener('click', onClick, { capture: true });
    label.remove();
  };
}
