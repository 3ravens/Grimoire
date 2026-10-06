<script>
  import { invoke } from '@tauri-apps/api/core';
  import grimoireLogo from '../assets/brand/grimoire-logo.png';
  import { t } from './i18n/t.js';

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
        error = t('lock.incorrect');
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
    <img class="lock-logo" src={grimoireLogo} alt="" width="64" height="45" />
    <h1 class="lock-title">Grimoire</h1>
    <p id="lock-subtitle" class="lock-subtitle">{t('lock.subtitle')}</p>

    <div class="lock-field">
      <input
        type="password"
        bind:value={password}
        onkeydown={handleKeydown}
        placeholder={t('lock.passwordPlaceholder')}
        aria-label={t('lock.passwordAria')}
        aria-describedby="lock-subtitle"
        disabled={loading}
        use:focus
      />
    </div>

    {#if error}
      <p class="lock-error">{error}</p>
    {/if}

    <button onclick={submit} disabled={loading || !password} class="lock-btn">
      {loading ? t('lock.unlocking') : t('lock.unlock')}
    </button>
  </div>
</div>

<style>
  @import './styles/lock.css';
</style>
