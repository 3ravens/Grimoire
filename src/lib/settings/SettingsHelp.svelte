<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

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
      // Let the app shell update its banner/badge with the fresh result.
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

<h3>Updates</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Check for updates on startup</span>
    <span class="setting-desc">
      Off by default. When enabled, Grimoire asks grimoireapp.dev for the latest version
      number on launch and tells you if a newer build exists. Only a version request is sent —
      no telemetry, identifiers, or vault data — and the check is recorded in the audit log.
      Updates are never downloaded or installed automatically; you choose when to upgrade.
    </span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={updateCheckEnabled}
      onchange={e => setUpdateCheckEnabled(e.currentTarget.checked)}
    />
    <span class="toggle-label">{updateCheckEnabled ? 'On' : 'Off'}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Check now</span>
    <span class="setting-desc">
      Manually check for a newer version once. This sends a single version request to
      grimoireapp.dev regardless of the startup setting above.
    </span>
  </div>
  <div class="setting-actions">
    <button
      class="settings-action-btn"
      onclick={checkForUpdate}
      disabled={updateChecking}
    >
      {updateChecking ? 'Checking…' : 'Check now…'}
    </button>
    {#if updateError}
      <span class="export-err">{updateError}</span>
    {:else if updateResult}
      {#if updateResult.updateAvailable}
        <span class="setting-desc">
          Update available: {updateResult.latest} (you have {updateResult.current}).
        </span>
        <button
          class="settings-action-btn"
          onclick={() => openPublicSite(updateResult.downloadUrl)}
        >
          View download
        </button>
      {:else}
        <span class="setting-desc">You are on the latest version ({updateResult.current}).</span>
      {/if}
    {/if}
  </div>
</div>

<h3>Help</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Replay UI tour</span>
    <span class="setting-desc">
      Walk through a short spotlight tour of the folder panel, note editor, chat sidebar, search, and
      settings. Skippable at any time.
    </span>
  </div>
  <div class="setting-actions">
    <button type="button" class="settings-action-btn" onclick={onReplayTour}>
      Replay UI tour
    </button>
  </div>
</div>

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
