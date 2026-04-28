<script>
  import { invoke } from '@tauri-apps/api/core';
  import AuditLog from '../AuditLog.svelte';

  let auditEnabled = $state(true);
  let logFileAccess = $state(true);

  $effect(() => {
    invoke('get_setting', { key: 'audit_enabled' })
      .then(v => { if (v !== '') auditEnabled = v === 'true'; })
      .catch(() => {});
    invoke('get_setting', { key: 'log_file_access' })
      .then(v => { if (v !== '') logFileAccess = v === 'true'; })
      .catch(() => {});
  });

  $effect(() => {
    invoke('set_setting', { key: 'audit_enabled', value: String(auditEnabled) }).catch(() => {});
  });

  $effect(() => {
    invoke('set_setting', { key: 'log_file_access', value: String(logFileAccess) }).catch(() => {});
  });
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
    <input type="checkbox" bind:checked={auditEnabled} />
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
    <input type="checkbox" bind:checked={logFileAccess} disabled={!auditEnabled} />
    <span class="toggle-label">{logFileAccess ? 'On' : 'Off'}</span>
  </label>
</div>

{#if auditEnabled}
  <AuditLog />
{:else}
  <p class="audit-disabled-note">Enable the audit log above to view and manage entries.</p>
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
</style>
