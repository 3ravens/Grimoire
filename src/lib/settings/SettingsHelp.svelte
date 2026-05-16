<script>
  import { invoke } from '@tauri-apps/api/core';

  let bugStatus = $state('');
  let siteLinkError = $state('');

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

<h3>Help</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Report a bug</span>
    <span class="setting-desc">
      Opens the bug report page on grimoireapp.dev in your browser. Your app version, operating
      system, CPU architecture, and bundle name are appended to the URL so the form can
      pre-fill them. No note content, logs, or other vault data are sent.
    </span>
  </div>
  <div class="setting-actions">
    <button
      class="settings-action-btn"
      onclick={reportBug}
      disabled={bugStatus === 'opening'}
    >
      {bugStatus === 'opening' ? 'Opening…' : 'Report a bug…'}
    </button>
    {#if bugStatus.startsWith('error:')}
      <span class="export-err">{bugStatus.slice(6)}</span>
    {/if}
  </div>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Documentation and website</span>
    <span class="setting-desc">
      Documentation lives on a separate site. The main site has release notes and general
      information.
    </span>
  </div>
  <div class="setting-actions">
    <button class="settings-action-btn" onclick={() => openPublicSite('https://docs.grimoireapp.dev')}>
      Documentation
    </button>
    <button class="settings-action-btn" onclick={() => openPublicSite('https://grimoireapp.dev')}>
      grimoireapp.dev
    </button>
    {#if siteLinkError}
      <span class="export-err">{siteLinkError}</span>
    {/if}
  </div>
</div>
