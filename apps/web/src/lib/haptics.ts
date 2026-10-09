// A light tap of haptic feedback for every button press made by touch.
// Android exposes the Vibration API. iOS Safari does not, but toggling a native
// switch control plays the system selection haptic (Safari 18+), so a hidden
// <input type="checkbox" switch> is clicked from inside the user's own gesture.
// The OS "System Haptics" setting still decides whether anything is felt.

const PRESSABLE = 'button:not(:disabled), [role="button"]:not([aria-disabled="true"]), a[href], summary';

let label: HTMLLabelElement | undefined;

/** One light haptic tick. Must run inside a user gesture (click or pointerup) to be felt on iOS. */
export function haptic(from?: Element | null): void {
  if ('vibrate' in navigator) {
    navigator.vibrate(8);
    return;
  }
  if (!label) return;
  // A modal <dialog> makes everything outside it inert, so the switch rides along.
  (from?.closest('dialog[open]') ?? document.body).append(label);
  // Clicking the label must not steal focus from dialogs or fields.
  const focused = document.activeElement as HTMLElement | null;
  label.click();
  if (focused && document.activeElement !== focused) focused.focus({ preventScroll: true });
}

export function installHaptics(): () => void {
  const switchLabel = document.createElement('label');
  switchLabel.setAttribute('aria-hidden', 'true');
  switchLabel.style.cssText = 'position:fixed;left:-100px;top:0;width:1px;height:1px;overflow:hidden;opacity:0;pointer-events:none';
  const input = document.createElement('input');
  input.type = 'checkbox';
  input.setAttribute('switch', '');
  input.tabIndex = -1;
  switchLabel.append(input);
  document.body.append(switchLabel);
  label = switchLabel;

  let lastPointer = '';
  const onPointerDown = (event: PointerEvent) => { lastPointer = event.pointerType; };

  function onClick(event: MouseEvent) {
    const target = event.target as Element | null;
    if (lastPointer !== 'touch' || !target || switchLabel.contains(target) || !target.closest(PRESSABLE)) return;
    haptic(target);
  }

  document.addEventListener('pointerdown', onPointerDown, { capture: true, passive: true });
  document.addEventListener('click', onClick, { capture: true });
  return () => {
    document.removeEventListener('pointerdown', onPointerDown, { capture: true });
    document.removeEventListener('click', onClick, { capture: true });
    switchLabel.remove();
    label = undefined;
  };
}
