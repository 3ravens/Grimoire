<script>
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { t, tp } from '../i18n/t.js';

  let exportStatus = $state('');

  async function runExport() {
    exportStatus = 'running';
    try {
      const dir = await openDialog({ directory: true, multiple: false, title: t('settings.data.exportDialogTitle') });
      if (!dir) { exportStatus = ''; return; }
      const count = await invoke('export_notes', { destDir: dir });
      exportStatus = `done:${count}`;
    } catch (e) {
      exportStatus = `error:${e?.message ?? e}`;
    }
  }
</script>

<h3>{t('settings.data.title')}</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.data.exportMarkdown')}</span>
    <span class="setting-desc">{t('settings.data.exportMarkdownDesc')}</span>
  </div>
  <div class="setting-actions">
    <button class="settings-action-btn" onclick={runExport} disabled={exportStatus === 'running'}>
      {exportStatus === 'running' ? t('settings.data.exporting') : t('common.export')}
    </button>
    {#if exportStatus.startsWith('done:')}
      <span class="export-ok" role="status" aria-live="polite">{tp('settings.data.exportedCount', Number(exportStatus.slice(5)), { count: exportStatus.slice(5) })}</span>
    {:else if exportStatus.startsWith('error:')}
      <span class="export-err" role="status" aria-live="polite">{exportStatus.slice(6)}</span>
    {/if}
  </div>
</div>
