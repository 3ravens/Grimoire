<script>
  import { focusTrap } from './utils/focusTrap.js';
  /**
   * ConfirmModal — a centered confirmation dialog replacing browser confirm().
   *
   * Props:
   *   title        — heading text (e.g. "Delete note")
   *   message      — body text (e.g. "Are you sure you want to delete…")
   *   confirmLabel — label for the confirm button (default: "Delete")
   *   onConfirm    — called when the user confirms
   *   onCancel     — called when the user cancels or presses Escape
   *   dangerousDefaultFocus — when true, focus confirm on open (Enter deletes). Default false (Cancel focused).
   */
  let {
    title = 'Are you sure?',
    message = '',
    confirmLabel = 'Delete',
    onConfirm,
    onCancel,
    dangerousDefaultFocus = false,
  } = $props();

  function handleKeydown(e) {
    if (e.key === 'Escape') onCancel();
    // Enter is handled natively by whichever button has focus — no backdrop handler needed.
  }

  // Focus cancel by default so Enter does not confirm destructive actions.
  let confirmBtn = $state(null);
  let cancelBtn = $state(null);
  $effect(() => {
    if (dangerousDefaultFocus && confirmBtn) confirmBtn.focus();
    else if (!dangerousDefaultFocus && cancelBtn) cancelBtn.focus();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onCancel} onkeydown={handleKeydown} role="dialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby={message ? 'confirm-msg' : undefined} tabindex="-1">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" use:focusTrap onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <h2 id="confirm-title" class="modal-title">{title}</h2>
      <button class="close-btn" onclick={onCancel} aria-label="Close">✕</button>
    </div>

    {#if message}
      <p id="confirm-msg" class="modal-message">{message}</p>
    {/if}

    <div class="modal-actions">
      <button class="btn-confirm" bind:this={confirmBtn} onclick={onConfirm}>{confirmLabel}</button>
      <button class="btn-cancel" bind:this={cancelBtn} onclick={onCancel}>Cancel</button>
    </div>
  </div>
</div>

<style>
  @import './styles/confirm-modal.css';
</style>
