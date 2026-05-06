<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import AuditLog from '../AuditLog.svelte';
  import ConfirmModal from '../ConfirmModal.svelte';

  let auditEnabled = $state(true);
  let logFileAccess = $state(true);

  /** 0 = retain indefinitely (default). */
  let retentionDays = $state(0);
  let previewCount = $state(0);
  let showPruneConfirm = $state(false);

  let retentionDebounce;

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
      alert(`Prune failed: ${e?.message ?? e}`);
      showPruneConfirm = false;
    }
  }

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

<h3>Privacy</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Local only</span>
    <span class="setting-desc">No data ever leaves this machine. Cannot be disabled.</span>
  </div>
  <label class="toggle toggle-locked">
    <input type="checkbox" checked disabled />
    <span class="toggle-label">Always on</span>
  </label>
</div>

<h3>Audit Log</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Enable audit log</span>
    <span class="setting-desc">
      Records every privacy-sensitive action — note opens, searches, LLM queries,
      exports — to a local log stored on this machine. Never transmitted.
    </span>
  </div>
  <label class="toggle">
    <input type="checkbox" checked={auditEnabled} onchange={e => { auditEnabled = e.currentTarget.checked; save('audit_enabled', auditEnabled); }} />
    <span class="toggle-label">{auditEnabled ? 'On' : 'Off'}</span>
  </label>
</div>

<h4>File Scanner</h4>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Log file access</span>
    <span class="setting-desc">
      Include file scanner reads in the audit log. Only active when the audit log is enabled.
    </span>
  </div>
  <label class="toggle" class:toggle-locked={!auditEnabled}>
    <input type="checkbox" checked={logFileAccess} disabled={!auditEnabled} onchange={e => { logFileAccess = e.currentTarget.checked; save('log_file_access', logFileAccess); }} />
    <span class="toggle-label">{logFileAccess ? 'On' : 'Off'}</span>
  </label>
</div>

{#if auditEnabled}
  <h4>Retention</h4>

  <div class="setting-row retention-row">
    <div class="setting-label">
      <span class="setting-name">Auto-delete entries older than</span>
      <span class="setting-desc">
        Number of days to keep (based on each entry’s timestamp). <strong>0</strong> means retain indefinitely
        (default). Old rows are removed automatically when the app starts, and you can prune immediately with the
        button below.
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
        aria-label="Audit log retention in days"
      />
      <span class="retention-suffix">days</span>
    </div>
  </div>

  {#if retentionDays > 0 && previewCount > 0}
    <p class="retention-preview">~{previewCount.toLocaleString()} entr{previewCount === 1 ? 'y' : 'ies'} would be removed.</p>
  {/if}

  <div class="retention-actions">
    <button
      type="button"
      class="prune-btn"
      disabled={retentionDays <= 0 || previewCount === 0}
      onclick={() => (showPruneConfirm = true)}
    >
      Prune now
    </button>
  </div>

  <AuditLog />
{:else}
  <p class="audit-disabled-note">Enable the audit log above to view and manage entries.</p>
{/if}

{#if showPruneConfirm}
  <ConfirmModal
    title="Prune audit log"
    message="Permanently delete {previewCount} entr{previewCount === 1 ? 'y' : 'ies'} older than {retentionDays} day{retentionDays === 1 ? '' : 's'}. This cannot be undone."
    confirmLabel="Delete"
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
