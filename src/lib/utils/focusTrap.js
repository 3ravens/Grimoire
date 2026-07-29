/**
 * focusTrap — Svelte action that traps Tab/Shift+Tab focus within a container.
 *
 * Usage:
 *   <div use:focusTrap>…</div>
 *
 * On mount:
 *   - Saves the element that currently has focus (the trigger).
 *   - Moves focus to the first focusable element inside the container.
 * While active:
 *   - Tab cycles forward through focusable elements, wrapping at the end.
 *   - Shift+Tab cycles backward, wrapping at the start.
 *   - Focus cannot leave the container via keyboard.
 * On destroy:
 *   - Restores focus to the element that had it when the action was mounted.
 */

const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  'details > summary',
].join(', ');

/**
 * @param {HTMLElement} node
 * @returns {{ destroy(): void }}
 */
export function focusTrap(node) {
  const trigger = /** @type {HTMLElement | null} */ (document.activeElement);

  /** @returns {HTMLElement[]} */
  function getFocusable() {
    return /** @type {HTMLElement[]} */ (Array.from(node.querySelectorAll(FOCUSABLE)).filter(
      (el) => !el.closest('[hidden]') && getComputedStyle(el).display !== 'none'
    ));
  }

  // Move initial focus to the first focusable element.
  const first = getFocusable()[0];
  if (first) first.focus();

  /**
   * @param {KeyboardEvent} e
   */
  function handleKeydown(e) {
    if (e.key !== 'Tab') return;

    const focusable = getFocusable();
    if (focusable.length === 0) {
      e.preventDefault();
      return;
    }

    const firstEl = focusable[0];
    const lastEl = focusable[focusable.length - 1];

    if (e.shiftKey) {
      // Shift+Tab: if focus is on the first element, wrap to last.
      if (document.activeElement === firstEl) {
        e.preventDefault();
        lastEl.focus();
      }
    } else {
      // Tab: if focus is on the last element, wrap to first.
      if (document.activeElement === lastEl) {
        e.preventDefault();
        firstEl.focus();
      }
    }
  }

  node.addEventListener('keydown', handleKeydown);

  return {
    destroy() {
      node.removeEventListener('keydown', handleKeydown);
      // Restore focus to the element that triggered the modal.
      if (trigger && typeof trigger.focus === 'function') {
        trigger.focus();
      }
    },
  };
}
