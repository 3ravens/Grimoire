<!-- Copyright (C) 2026 Wim Palland

This file is part of Grimoire.

Grimoire is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

Grimoire is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with Grimoire. If not, see <https://www.gnu.org/licenses/>. -->

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
   */
  let {
    title = 'Are you sure?',
    message = '',
    confirmLabel = 'Delete',
    onConfirm,
    onCancel,
  } = $props();

  function handleKeydown(e) {
    if (e.key === 'Escape') onCancel();
    // Enter is handled natively by whichever button has focus — no backdrop handler needed.
  }

  // Focus the confirm (destructive) button by default so Enter triggers deletion.
  let confirmBtn = $state(null);
  $effect(() => {
    if (confirmBtn) confirmBtn.focus();
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
      <button class="btn-cancel" onclick={onCancel}>Cancel</button>
    </div>
  </div>
</div>

<style>
  @import './styles/confirm-modal.css';
</style>
