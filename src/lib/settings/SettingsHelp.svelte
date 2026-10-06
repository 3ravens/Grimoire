<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { t } from '../i18n/t.js';

  /** @type {{ onReplayTour?: () => void }} */
  let { onReplayTour = () => {} } = $props();

  let bugStatus = $state('');
  let siteLinkError = $state('');

  // ── Updates (opt-in, notify-only) ──────────────────────────────────────────
  let updateCheckEnabled = $state(false);
  let updateChecking = $state(false);
  /** @type {null | { enabled: boolean, current: string, latest: string | null, updateAvailable: boolean, downloadUrl: string }} */
  let updateResult = $state(null);
  let updateError = $state('');

  onMount(async () => {
    try {
      const v = await invoke('get_setting', { key: 'update_check_enabled' });
      updateCheckEnabled = v === 'true';
    } catch {
      updateCheckEnabled = false;
    }
  });

  function setUpdateCheckEnabled(enabled) {
    updateCheckEnabled = enabled;
    updateError = '';
    invoke('set_setting', { key: 'update_check_enabled', value: String(enabled) }).catch(() => {});
    if (!enabled) {
      updateResult = null;
    } else {
      checkForUpdate();
    }
  }

  async function checkForUpdate() {
    updateChecking = true;
    updateError = '';
    try {
      updateResult = await invoke('check_for_update');
      window.dispatchEvent(
        new CustomEvent('grimoire:update-check', { detail: updateResult })
      );
    } catch (e) {
      updateError = e?.message ?? String(e);
    } finally {
      updateChecking = false;
    }
  }

  async function reportBug() {
    bugStatus = 'opening';
    try {
      await invoke('open_bug_report');
      bugStatus = '';
    } catch (e) {
      bugStatus = `error:${e?.message ?? e}`;
    }
  }

  async function openPublicSite(url) {
    siteLinkError = '';
    try {
      await invoke('open_external_url', { url });
    } catch (e) {
      siteLinkError = e?.message ?? String(e);
    }
  }
</script>

<h3>{t('settings.help.updatesTitle')}</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.help.checkOnStartup')}</span>
    <span class="setting-desc">{t('settings.help.checkOnStartupDesc')}</span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={updateCheckEnabled}
      onchange={e => setUpdateCheckEnabled(e.currentTarget.checked)}
    />
    <span class="toggle-label">{updateCheckEnabled ? t('common.on') : t('common.off')}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.help.checkNow')}</span>
    <span class="setting-desc">{t('settings.help.checkNowDesc')}</span>
  </div>
  <div class="setting-actions">
    <button
      class="settings-action-btn"
      onclick={checkForUpdate}
      disabled={updateChecking}
    >
      {updateChecking ? t('settings.help.checking') : t('settings.help.checkNowBtn')}
    </button>
    {#if updateError}
      <span class="export-err">{updateError}</span>
    {:else if updateResult}
      {#if updateResult.updateAvailable}
        <span class="setting-desc">
          {t('settings.help.updateAvailable', { latest: updateResult.latest, current: updateResult.current })}
        </span>
        <button
          class="settings-action-btn"
          onclick={() => openPublicSite(updateResult.downloadUrl)}
        >
          {t('settings.help.viewDownload')}
        </button>
      {:else}
        <span class="setting-desc">{t('settings.help.onLatest', { current: updateResult.current })}</span>
      {/if}
    {/if}
  </div>
</div>

<h3>{t('settings.help.helpTitle')}</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.help.replayTour')}</span>
    <span class="setting-desc">{t('settings.help.replayTourDesc')}</span>
  </div>
  <div class="setting-actions">
    <button type="button" class="settings-action-btn" onclick={onReplayTour}>
      {t('settings.help.replayTourBtn')}
    </button>
  </div>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.help.reportBug')}</span>
    <span class="setting-desc">{t('settings.help.reportBugDesc')}</span>
  </div>
  <div class="setting-actions">
    <button
      class="settings-action-btn"
      onclick={reportBug}
      disabled={bugStatus === 'opening'}
    >
      {bugStatus === 'opening' ? t('settings.help.opening') : t('settings.help.reportBugBtn')}
    </button>
    {#if bugStatus.startsWith('error:')}
      <span class="export-err">{bugStatus.slice(6)}</span>
    {/if}
  </div>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.help.docsAndSite')}</span>
    <span class="setting-desc">{t('settings.help.docsAndSiteDesc')}</span>
  </div>
  <div class="setting-actions">
    <button class="settings-action-btn" onclick={() => openPublicSite('https://docs.grimoireapp.dev')}>
      {t('settings.help.documentation')}
    </button>
    <button class="settings-action-btn" onclick={() => openPublicSite('https://grimoireapp.dev')}>
      {t('settings.help.mainSite')}
    </button>
    {#if siteLinkError}
      <span class="export-err">{siteLinkError}</span>
    {/if}
  </div>
</div>
