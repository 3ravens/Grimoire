<script>
  import { invoke } from '@tauri-apps/api/core';

  // Props
  let { onUnlocked } = $props();

  let password = $state('');
  let error = $state('');
  let loading = $state(false);

  async function submit() {
    if (!password) return;
    loading = true;
    error = '';
    try {
      const ok = await invoke('unlock_vault', { password });
      if (ok) {
        await onUnlocked?.();
      } else {
        error = 'Incorrect password.';
        password = '';
      }
    } catch (e) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Enter') submit();
  }

  function focus(el) {
    el.focus();
  }
</script>

<div class="lock-screen">
  <div class="lock-box">
    <h1 class="lock-title">Grimoire</h1>
    <p id="lock-subtitle" class="lock-subtitle">This vault is locked.</p>

    <div class="lock-field">
      <input
        type="password"
        bind:value={password}
        onkeydown={handleKeydown}
        placeholder="Enter password…"
        aria-label="Password"
        aria-describedby="lock-subtitle"
        disabled={loading}
        use:focus
      />
    </div>

    {#if error}
      <p class="lock-error">{error}</p>
    {/if}

    <button onclick={submit} disabled={loading || !password} class="lock-btn">
      {loading ? 'Unlocking…' : 'Unlock'}
    </button>
  </div>
</div>

<style>
  @import './styles/lock.css';
</style>
