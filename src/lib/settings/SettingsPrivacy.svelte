<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, onDestroy } from 'svelte';
  import AuditLog from '../AuditLog.svelte';
  import ConfirmModal from '../ConfirmModal.svelte';
  import { t, tp, tParts } from '../i18n/t.js';

  let { onOpenPrivacyPolicy = () => {} } = $props();

  let auditEnabled = $state(true);
  let logFileAccess = $state(true);

  /** 0 = retain indefinitely (default). */
  let retentionDays = $state(0);
  let previewCount = $state(0);
  let showPruneConfirm = $state(false);

  let retentionDebounce;

  const pruneModalMessage = $derived.by(() => {
    const count = previewCount;
    const days = retentionDays;
    const entryWord = count === 1 ? t('settings.privacy.entryWordOne') : t('settings.privacy.entryWordOther');
    const dayWord = days === 1 ? t('settings.privacy.dayWordOne') : t('settings.privacy.dayWordOther');
    return t('settings.privacy.pruneModalMessage', { count, entryWord, days, dayWord });
  });

  async function refreshPreview() {
    if (retentionDays <= 0) {
      previewCount = 0;
      return;
    }
    try {
      previewCount = await invoke('preview_audit_retention_prune', { days: retentionDays });
    } catch {
      previewCount = 0;
    }
  }

  function scheduleRetentionSave() {
    clearTimeout(retentionDebounce);
    retentionDebounce = setTimeout(async () => {
      const v = Math.max(0, Math.floor(Number(retentionDays)) || 0);
      retentionDays = v;
      await invoke('set_setting', { key: 'audit_retention_days', value: String(v) }).catch(() => {});
      await refreshPreview();
    }, 300);
  }

  function onRetentionInput(e) {
    const raw = /** @type {HTMLInputElement} */ (e.target).value;
    retentionDays = raw === '' ? 0 : Math.max(0, parseInt(raw, 10) || 0);
    scheduleRetentionSave();
  }

  async function confirmPrune() {
    try {
      await invoke('prune_audit_log', { days: retentionDays });
      showPruneConfirm = false;
      await refreshPreview();
      window.dispatchEvent(new CustomEvent('grimoire:audit-pruned'));
    } catch (e) {
      alert(t('settings.privacy.pruneFailed', { msg: e?.message ?? e }));
      showPruneConfirm = false;
    }
  }

  onDestroy(() => {
    clearTimeout(retentionDebounce);
  });

  onMount(async () => {
    const [a, l, r] = await Promise.all([
      invoke('get_setting', { key: 'audit_enabled' }),
      invoke('get_setting', { key: 'log_file_access' }),
      invoke('get_setting', { key: 'audit_retention_days' }).catch(() => '0'),
    ]).catch(() => [null, null, '0']);
    if (a !== null && a !== '') auditEnabled = a === 'true';
    if (l !== null && l !== '') logFileAccess = l === 'true';
    const rd = r !== null && r !== '' ? parseInt(String(r), 10) : 0;
    retentionDays = Number.isFinite(rd) && rd >= 0 ? rd : 0;
    await refreshPreview();
  });

  function save(key, value) {
    invoke('set_setting', { key, value: String(value) }).catch(() => {});
  }
</script>

<h3>{t('settings.privacy.title')}</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.privacy.localOnly')}</span>
    <span class="setting-desc">{t('settings.privacy.localOnlyDesc')}</span>
  </div>
  <label class="toggle toggle-locked">
    <input type="checkbox" checked disabled />
    <span class="toggle-label">{t('settings.privacy.alwaysOn')}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.privacy.privacyPolicy')}</span>
    <span class="setting-desc">{t('settings.privacy.privacyPolicyDesc')}</span>
  </div>
  <div class="setting-actions">
    <button type="button" class="settings-action-btn" onclick={onOpenPrivacyPolicy}>
      {t('settings.privacy.openPrivacyPolicy')}
    </button>
  </div>
</div>

<h3>{t('settings.privacy.auditLogTitle')}</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.privacy.enableAuditLog')}</span>
    <span class="setting-desc">{t('settings.privacy.enableAuditLogDesc')}</span>
  </div>
  <label class="toggle">
    <input type="checkbox" checked={auditEnabled} onchange={e => { auditEnabled = e.currentTarget.checked; save('audit_enabled', auditEnabled); }} />
    <span class="toggle-label">{auditEnabled ? t('common.on') : t('common.off')}</span>
  </label>
</div>

<h4>{t('settings.privacy.fileScannerHeading')}</h4>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.privacy.logFileAccess')}</span>
    <span class="setting-desc">{t('settings.privacy.logFileAccessDesc')}</span>
  </div>
  <label class="toggle" class:toggle-locked={!auditEnabled}>
    <input type="checkbox" checked={logFileAccess} disabled={!auditEnabled} onchange={e => { logFileAccess = e.currentTarget.checked; save('log_file_access', logFileAccess); }} />
    <span class="toggle-label">{logFileAccess ? t('common.on') : t('common.off')}</span>
  </label>
</div>

{#if auditEnabled}
  <h4>{t('settings.privacy.retentionHeading')}</h4>

  <div class="setting-row retention-row">
    <div class="setting-label">
      <span class="setting-name">{t('settings.privacy.retentionLabel')}</span>
      <span class="setting-desc">
        {#each tParts('settings.privacy.retentionDesc') as part}
          {#if part.type === 'text'}{part.value}{:else if part.name === 'zero'}<strong>0</strong>{/if}
        {/each}
      </span>
    </div>
    <div class="retention-controls">
      <input
        class="retention-input"
        type="number"
        min="0"
        step="1"
        value={retentionDays}
        oninput={onRetentionInput}
        aria-label={t('settings.privacy.retentionAria')}
      />
      <span class="retention-suffix">{t('settings.shared.days')}</span>
    </div>
  </div>

  {#if retentionDays > 0 && previewCount > 0}
    <p class="retention-preview" role="status" aria-live="polite">{tp('settings.privacy.retentionPreview', previewCount, { count: previewCount.toLocaleString() })}</p>
  {/if}

  <div class="retention-actions">
    <button
      type="button"
      class="prune-btn"
      disabled={retentionDays <= 0 || previewCount === 0}
      onclick={() => (showPruneConfirm = true)}
    >
      {t('settings.privacy.pruneNow')}
    </button>
  </div>

  <AuditLog />
{:else}
  <p class="audit-disabled-note">{t('settings.privacy.auditDisabledNote')}</p>
{/if}

{#if showPruneConfirm}
  <ConfirmModal
    title={t('settings.privacy.pruneModalTitle')}
    message={pruneModalMessage}
    confirmLabel={t('common.delete')}
    onConfirm={confirmPrune}
    onCancel={() => (showPruneConfirm = false)}
  />
{/if}

<style>
  h4 {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text);
    opacity: 0.6;
    margin: 16px 0 6px;
  }

  .audit-disabled-note {
    font: 13px var(--sans);
    color: var(--text);
    opacity: 0.5;
    margin-top: 12px;
  }

  .retention-row {
    align-items: flex-start;
  }

  .retention-controls {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .retention-input {
    width: 72px;
    height: 28px;
    padding: 0 8px;
    font: 13px var(--sans);
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  .retention-suffix {
    font: 13px var(--sans);
    color: var(--text-muted);
    white-space: nowrap;
  }

  .retention-preview {
    margin: 4px 0 0;
    font: 12px var(--sans);
    color: var(--text-muted);
  }

  .retention-actions {
    margin: 8px 0 4px;
  }

  .prune-btn {
    height: 28px;
    padding: 0 12px;
    font: 13px var(--sans);
    color: var(--danger);
    background: var(--bg3);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
  }

  .prune-btn:hover:not(:disabled) {
    border-color: var(--danger);
  }

  .prune-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
