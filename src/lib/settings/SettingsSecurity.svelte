<script>
  import { t } from '../i18n/t.js';

  let {
    vaultHasPassword = false,
    onSetVaultPassword = () => {},
    onChangeVaultPassword = () => {},
    onRemoveVaultPassword = () => {},
    onLockVault = () => {},
  } = $props();
</script>

<h3>{t('settings.security.title')}</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.security.vaultPassword')}</span>
    <span class="setting-desc">{t('settings.security.vaultPasswordDesc')}</span>
  </div>
  <div class="vault-actions">
    {#if !vaultHasPassword}
      <button class="settings-action-btn" onclick={onSetVaultPassword}>{t('settings.security.setPassword')}</button>
    {:else}
      <button class="settings-action-btn" onclick={onChangeVaultPassword}>{t('settings.security.changePassword')}</button>
      <button class="settings-action-btn" onclick={onRemoveVaultPassword}>{t('settings.security.removePassword')}</button>
      <button class="settings-action-btn danger" onclick={onLockVault}>{t('settings.security.lockVaultNow')}</button>
    {/if}
  </div>
</div>
