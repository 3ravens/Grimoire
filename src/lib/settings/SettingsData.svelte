<script>
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';

  let exportStatus = $state('');

  async function runExport() {
    exportStatus = 'running';
    try {
      const dir = await openDialog({ directory: true, multiple: false, title: 'Export notes to…' });
      if (!dir) { exportStatus = ''; return; }
      const count = await invoke('export_notes', { destDir: dir });
      exportStatus = `done:${count}`;
    } catch (e) {
      exportStatus = `error:${e?.message ?? e}`;
    }
  }
</script>

<h3>Data</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Export notes to Markdown</span>
    <span class="setting-desc">
      Saves all unlocked notes as <code>.md</code> files in a folder you choose.
      Folder structure is preserved as subdirectories. Locked notes are skipped.
    </span>
  </div>
  <div class="setting-actions">
    <button class="settings-action-btn" onclick={runExport} disabled={exportStatus === 'running'}>
      {exportStatus === 'running' ? 'Exporting…' : 'Export'}
    </button>
    {#if exportStatus.startsWith('done:')}
      <span class="export-ok">✓ {exportStatus.slice(5)} notes exported</span>
    {:else if exportStatus.startsWith('error:')}
      <span class="export-err">{exportStatus.slice(6)}</span>
    {/if}
  </div>
</div>
