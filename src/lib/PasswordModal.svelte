<script>
  import { focusTrap } from './utils/focusTrap.js';
  /**
   * PasswordModal — reusable modal for password prompts.
   *
   * Props:
   *   title          — heading text
   *   onSubmit(pw)   — async function; should return true on success, false on wrong password,
   *                    or throw a string error message
   *   onCancel       — called when the user dismisses the modal
   *   confirmLabel   — optional button label (default: "Confirm")
   *   warning        — optional warning text shown above the input (e.g. no-recovery notice)
   *   requireAck     — if true, user must check a checkbox before confirming (for irreversible ops)
   */

  let {
    title,
    onSubmit,
    onCancel,
    confirmLabel = 'Confirm',
    warning = '',
    requireAck = false,
  } = $props();

  let password = $state('');
  let error = $state('');
  let loading = $state(false);
  let acked = $state(false);

  $effect(() => {
    // Focus the input when the modal mounts.
    document.getElementById('pw-modal-input')?.focus();
  });

  async function submit() {
    if (!password) return;
    if (requireAck && !acked) return;
    loading = true;
    error = '';
    try {
      const result = await onSubmit(password);
      if (result === false) {
        error = 'Incorrect password.';
        password = '';
      }
      // On success (true or undefined), the parent dismisses the modal.
    } catch (e) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Enter') {
      e.stopPropagation();
      e.preventDefault();
      submit();
    } else if (e.key === 'Escape') {
      e.stopPropagation();
      onCancel();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" onclick={onCancel} onkeydown={handleKeydown} role="dialog" aria-modal="true" aria-labelledby="pw-modal-title" tabindex="-1">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" use:focusTrap onclick={(e) => e.stopPropagation()}>
    <h2 id="pw-modal-title" class="modal-title">{title}</h2>

    {#if warning}
      <p class="modal-warning">{warning}</p>
    {/if}
    {#if requireAck}
      <label class="modal-ack" for="pw-modal-ack">
        <input id="pw-modal-ack" type="checkbox" bind:checked={acked} />
        I understand
      </label>
    {/if}

    <input
      id="pw-modal-input"
      type="password"
      bind:value={password}
      onkeydown={handleKeydown}
      placeholder="Password…"
      aria-label="Password"
      disabled={loading}
    />

    {#if error}
      <p class="modal-error">{error}</p>
    {/if}

    <div class="modal-actions">
      <button class="modal-cancel" onclick={onCancel} disabled={loading}>Cancel</button>
      <button
        class="modal-confirm"
        onclick={submit}
        disabled={loading || !password || (requireAck && !acked)}
      >
        {loading ? 'Working…' : confirmLabel}
      </button>
    </div>
  </div>
</div>

<style>
  @import './styles/password-modal.css';
</style>
