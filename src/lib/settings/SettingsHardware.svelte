<script>
  import { invoke } from '@tauri-apps/api/core';
  import { t } from '../i18n/t.js';

  let {
    llmEnabled = true,
    onHardwareChange = () => {},
  } = $props();

  let hw            = $state(null);
  let hwLoading     = $state(false);
  let hwError       = $state('');
  let runningModels = $state([]);

  function fmtMb(mb) {
    if (mb == null) return t('settings.shared.emDash');
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`;
  }

  function pct(used, total) {
    if (!total) return 0;
    return Math.min(100, Math.round((used / total) * 100));
  }

  function capabilityLabel(cap) {
    if (cap === 'full')          return t('settings.hardware.capabilityFull');
    if (cap === 'embeddingOnly') return t('settings.hardware.capabilityEmbeddingOnly');
    return t('settings.hardware.capabilityInsufficient');
  }

  function indexingTierLabel(tier) {
    if (tier === 'high') return t('settings.hardware.tierHigh');
    if (tier === 'mid') return t('settings.hardware.tierMid');
    return t('settings.hardware.tierLow');
  }

  async function refreshHardware() {
    hw = null;
    hwLoading = true;
    try {
      [hw, runningModels] = await Promise.all([
        invoke('get_hardware_info'),
        invoke('get_running_models'),
      ]);
      hwError = '';
    } catch (e) {
      hwError = e?.message ?? String(e);
    } finally {
      hwLoading = false;
    }
  }

  async function handleForceToggle(e) {
    const val = e.currentTarget.checked;
    await invoke('set_setting', { key: 'llm_force_enabled', value: String(val) });
    if (val) {
      await invoke('set_setting', { key: 'wizard_ai_skipped', value: 'false' });
    }
    hw = { ...hw, llmForceEnabled: val };
    onHardwareChange(hw.capability, val);
  }

  $effect(() => {
    if (hw === null && !hwLoading) {
      hwLoading = true;
      Promise.all([
        invoke('get_hardware_info'),
        invoke('get_running_models'),
      ])
        .then(([r, models]) => { hw = r; runningModels = models; hwError = ''; })
        .catch(e => { hwError = e?.message ?? String(e); })
        .finally(() => { hwLoading = false; });
    }

    const id = setInterval(() => {
      Promise.all([
        invoke('get_hardware_info'),
        invoke('get_running_models'),
      ])
        .then(([r, models]) => { hw = r; runningModels = models; })
        .catch(() => {});
    }, 5000);
    return () => clearInterval(id);
  });
</script>

<h3>{t('settings.hardware.title')}</h3>

{#if hwLoading}
  <p class="settings-notice">{t('settings.hardware.detecting')}</p>
{:else if hwError}
  <p class="settings-notice hw-error">{hwError}</p>
  <button class="settings-action-btn" onclick={refreshHardware}>{t('settings.shared.retry')}</button>
{:else if hw}
  <div class="hw-capability-row">
    <span class="hw-badge hw-badge-{hw.capability}">{capabilityLabel(hw.capability)}</span>
    <button class="settings-action-btn" onclick={refreshHardware}>{t('common.refresh')}</button>
  </div>

  <div class="hw-card">
    <div class="hw-card-title">{t('settings.hardware.backgroundIndexing')}</div>
    <div class="hw-row">
      <span class="hw-label">{t('settings.hardware.throughputTier')}</span>
      <span class="hw-value">{indexingTierLabel(hw.indexingThroughputTier)}</span>
    </div>
    <p class="setting-desc hw-indexing-summary">{hw.indexingThroughputSummary}</p>
  </div>

  <div class="hw-card">
    <div class="hw-card-title">{t('settings.hardware.cpu')}</div>
    <div class="hw-row">
      <span class="hw-label">{t('settings.hardware.model')}</span>
      <span class="hw-value">{hw.cpuName}</span>
    </div>
    <div class="hw-row">
      <span class="hw-label">{t('settings.hardware.cores')}</span>
      <span class="hw-value">{hw.cpuCores}</span>
    </div>
  </div>

  <div class="hw-card">
    <div class="hw-card-title">{t('settings.hardware.memory')}</div>
    <div class="hw-row">
      <span class="hw-label">{t('settings.hardware.usedInclCache')}</span>
      <span class="hw-value">{fmtMb(hw.ramUsedMb)} / {fmtMb(hw.ramTotalMb)}</span>
    </div>
    <div class="hw-bar"><div class="hw-bar-fill" style="width: {pct(hw.ramUsedMb, hw.ramTotalMb)}%"></div></div>
    <div class="hw-row">
      <span class="hw-label">{t('settings.hardware.grimoire')}</span>
      <span class="hw-value">{fmtMb(hw.ramGrimoireMb)}</span>
    </div>
  </div>

  {#if hw.gpus.length === 0}
    <div class="hw-card">
      <div class="hw-card-title">{t('settings.hardware.gpu')}</div>
      <p class="hw-empty">{t('settings.hardware.noGpu')}</p>
    </div>
  {:else}
    {#each hw.gpus as gpu}
      <div class="hw-card">
        <div class="hw-card-header">
          <span class="hw-card-title">{gpu.name}</span>
          {#if gpu.isUnifiedMemory}
            <span class="hw-tag">{t('settings.hardware.unifiedMemory')}</span>
          {/if}
        </div>
        {#if gpu.vramTotalMb != null}
          <div class="hw-row">
            <span class="hw-label">{t('settings.shared.vram')}</span>
            <span class="hw-value">{gpu.vramUsedMb != null ? `${fmtMb(gpu.vramUsedMb)} / ` : ''}{fmtMb(gpu.vramTotalMb)}</span>
          </div>
          <div class="hw-bar">
            {#if gpu.vramUsedMb != null}
              <div class="hw-bar-fill" style="width: {pct(gpu.vramUsedMb, gpu.vramTotalMb)}%"></div>
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  {/if}

  {#if hw.capability !== 'full'}
    <div class="setting-row">
      <div class="setting-label">
        <span class="setting-name">{t('settings.hardware.forceEnableLlm')}</span>
        <span class="setting-desc">{t('settings.hardware.forceEnableLlmDesc')}</span>
      </div>
      <label class="toggle">
        <input type="checkbox" checked={hw.llmForceEnabled} onchange={handleForceToggle} />
        <span class="toggle-label">{hw.llmForceEnabled ? t('common.on') : t('common.off')}</span>
      </label>
    </div>
  {/if}

  <div class="hw-card">
    <div class="hw-card-title">{t('settings.hardware.runningModels')}</div>
    {#if runningModels.length === 0}
      <p class="hw-empty">{t('settings.hardware.noModelsLoaded')}</p>
    {:else}
      {#each runningModels as m}
        <div class="hw-row">
          <span class="hw-label hw-model-name">{m.name}</span>
          <span class="hw-value">
            {#if m.vramMb != null}{fmtMb(m.vramMb)} {t('settings.shared.vram')} &nbsp;{/if}{#if m.pinned}<span class="hw-tag hw-tag-pinned">{t('settings.shared.pinned')}</span>{/if}
          </span>
        </div>
      {/each}
    {/if}
  </div>
{/if}
