<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import AuditLog from '../AuditLog.svelte';

  let auditEnabled = $state(true);
  let logFileAccess = $state(true);

  onMount(async () => {
    const [a, l] = await Promise.all([
      invoke('get_setting', { key: 'audit_enabled' }),
      invoke('get_setting', { key: 'log_file_access' }),
    ]).catch(() => [null, null]);
    if (a !== null && a !== '') auditEnabled = a === 'true';
    if (l !== null && l !== '') logFileAccess = l === 'true';
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
