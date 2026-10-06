<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getContext, onMount } from 'svelte';
  import { focusTrap } from './utils/focusTrap.js';
  import { wizardCuratedChatModels } from './utils/wizardChatModels.js';
  import { CURATED_EMBEDDING_MODELS } from './constants/chatModels.js';
  import {
    checkChatModelInstalled,
    saveChatModelSetting,
    pullChatModel,
    isPullInFlight,
  } from './services/chatModelSelection.js';
  import ModelDownloadModal from './ModelDownloadModal.svelte';
  import grimoireLogo from '../assets/brand/grimoire-logo.png';
  import { t, tParts } from './i18n/t.js';

  /** @type {{ onCompleted: () => void }} */
  let { onCompleted } = $props();

  /** @type {{ showError: (e: unknown) => void }} */
  const err = getContext('err');
  /** @type {{ settingsPendingSection?: string | null } | undefined} */
  const ui = getContext('ui');

  const MS_STARTER = 0;
  const MS_DEPS = 1;
  const MS_HW = 2;
  const MS_MODELS = 3;
  const MS_WIKI = 4;

  let mainStep = $state(MS_STARTER);

  /** @type {'empty' | 'pkm' | 'bullet_journal' | 'para'} */
  let starterPack = $state('empty');

  let ollamaOk = $state(/** @type {boolean | null} */ (null));
  let ollamaCheckBusy = $state(false);

  /** @type {Record<string, unknown> | null} */
  let hardwareReport = $state(null);
  let hwBusy = $state(false);

  let chatPick = $state('llama3.2');
  let customModel = $state('');
  let useCustomModel = $state(false);
  const defaultEmbed = CURATED_EMBEDDING_MODELS[0]?.value ?? 'nomic-embed-text';
  let embedModel = $state(defaultEmbed);
  let embedInstalled = $state(false);
  let chatInstalled = $state(false);
  let chatInstalledRequest = 0;
  let embedPullBusy = $state(false);

  let wikipediaEnable = $state(false);
  let openWikiSettings = $state(false);

  let finishBusy = $state(false);
  let skipAi = $state(false);
  let wizardStatus = $state('');

  /** @type {null | { model: string, phase: 'confirm' | 'pulling' | 'error', confirmKind?: string, hardwareWarning?: unknown, statusLine: string, progress: { completed: number, total: number } | null, errorMessage: string }} */
  let dlModal = $state(null);
  let unsubPull = /** @type {null | (() => void)} */ (null);

  const starterOptions = (
    /** @type {const} */ (['empty', 'pkm', 'bullet_journal', 'para'])
  ).map((id) => ({
    id,
    labelKey: `wizard.starter.options.${id}.label`,
    hintKey: `wizard.starter.options.${id}.hint`,
  }));

  const curatedForWizard = $derived.by(() => {
    const cap = /** @type {any} */ (hardwareReport)?.capability;
    return wizardCuratedChatModels(typeof cap === 'string' ? cap : undefined);
  });

  const showAmdDriverHint = $derived.by(() => {
    const gpus = /** @type {any[]} */ (hardwareReport)?.gpus;
    if (!Array.isArray(gpus)) return false;
    return gpus.some((g) => {
      const n = String(g?.name ?? '').toLowerCase();
      return n.includes('amd') || n.includes('radeon');
    });
  });

  const selectedChatModel = $derived(
    String(useCustomModel ? customModel.trim() : chatPick).trim(),
  );

  const skipDepsStep = $derived(ollamaOk === true);
  const skipModelsStep = $derived(
    ollamaOk === true && embedInstalled && chatInstalled,
  );
  /** Deps step: advance only when Ollama is up or user chose notes-only setup. */
  const depsCanAdvance = $derived(skipAi || ollamaOk === true);

  $effect(() => {
    const rows = curatedForWizard;
    if (!rows.length) return;
    if (!rows.some((r) => r.value === chatPick)) {
      chatPick = rows[0].value;
    }
  });

  onMount(() => {
    void runPreflight();
    return () => {
      unsubPull?.();
    };
  });

  async function checkOllama() {
    ollamaCheckBusy = true;
    try {
      await invoke('list_ollama_installed_models');
      ollamaOk = true;
      await Promise.all([refreshEmbedInstalled(), refreshChatInstalled()]);
    } catch {
      ollamaOk = false;
    } finally {
      ollamaCheckBusy = false;
    }
  }

  async function loadHardware() {
    hwBusy = true;
    try {
      const hw = await invoke('get_hardware_info');
      hardwareReport = /** @type {Record<string, unknown>} */ (hw);
    } catch {
      hardwareReport = null;
    } finally {
      hwBusy = false;
    }
  }

  async function refreshEmbedInstalled() {
    try {
      embedInstalled = await invoke('ollama_model_installed', { model: embedModel });
    } catch {
      embedInstalled = false;
    }
  }

  async function refreshChatInstalled() {
    const model = selectedChatModel;
    const requestId = ++chatInstalledRequest;
    if (!model) {
      if (requestId === chatInstalledRequest) {
        chatInstalled = false;
      }
      return;
    }
    try {
      const installed = await checkChatModelInstalled(model);
      if (requestId === chatInstalledRequest && model === selectedChatModel) {
        chatInstalled = installed;
      }
    } catch {
      if (requestId === chatInstalledRequest && model === selectedChatModel) {
        chatInstalled = false;
      }
    }
  }

  async function runPreflight() {
    await checkOllama();
    if (ollamaOk !== true) return;
    await Promise.all([refreshEmbedInstalled(), refreshChatInstalled()]);
  }

  $effect(() => {
    if (mainStep === MS_HW && !hardwareReport && !hwBusy) {
      void loadHardware();
    }
    if (ollamaOk === true) {
      embedModel;
      selectedChatModel;
      void refreshEmbedInstalled();
      void refreshChatInstalled();
    }
  });

  function advanceFromStarter() {
    if (!skipDepsStep) return MS_DEPS;
    if (!skipModelsStep) return MS_HW;
    return MS_WIKI;
  }

  function advanceFromDeps() {
    if (!skipModelsStep) return MS_HW;
    return MS_WIKI;
  }

  function advanceFromHw() {
    if (skipAi || skipModelsStep) return MS_WIKI;
    return MS_MODELS;
  }

  function retreatFromWiki() {
    if (skipAi || skipModelsStep) return MS_HW;
    if (!skipDepsStep) return MS_HW;
    return MS_STARTER;
  }

  function retreatFromModels() {
    if (!skipDepsStep) return MS_HW;
    return MS_STARTER;
  }

  function retreatFromHw() {
    if (!skipDepsStep) return MS_DEPS;
    return MS_STARTER;
  }

  function stepBack() {
    if (mainStep === MS_WIKI) {
      mainStep = retreatFromWiki();
      return;
    }
    if (mainStep === MS_MODELS) {
      mainStep = retreatFromModels();
      return;
    }
    if (mainStep === MS_HW) {
      mainStep = retreatFromHw();
      return;
    }
    if (mainStep === MS_DEPS) {
      mainStep = MS_STARTER;
    }
  }

  async function stepNext() {
    if (mainStep === MS_STARTER) {
      if (ollamaOk === null) await checkOllama();
      mainStep = advanceFromStarter();
      return;
    }
    if (mainStep === MS_DEPS) {
      if (!depsCanAdvance) return;
      mainStep = advanceFromDeps();
      return;
    }
    if (mainStep === MS_HW) {
      mainStep = advanceFromHw();
      return;
    }
    if (mainStep < MS_WIKI) mainStep += 1;
  }

  function openOllamaDownload() {
    invoke('open_external_url', { url: 'https://ollama.com/download' }).catch((e) =>
      err?.showError?.(e),
    );
  }

  function openAmdDrivers() {
    invoke('open_external_url', { url: 'https://www.amd.com/en/support' }).catch((e) =>
      err?.showError?.(e),
    );
  }

  async function startChatPull() {
    const model = String(useCustomModel ? customModel.trim() : chatPick).trim();
    if (!model || isPullInFlight()) return;
    const installed = await checkChatModelInstalled(model);
    if (installed) {
      await saveChatModelSetting(model);
      chatInstalled = true;
      return;
    }
    dlModal = {
      model,
      phase: 'confirm',
      confirmKind: 'downloadMissing',
      hardwareWarning: null,
      statusLine: '',
      progress: null,
      errorMessage: '',
    };
  }

  async function onModalDownload() {
    if (!dlModal) return;
    const model = dlModal.model;
    dlModal = {
      ...dlModal,
      phase: 'pulling',
      statusLine: t('wizard.download.starting'),
      progress: null,
      errorMessage: '',
    };
    unsubPull?.();
    unsubPull = await listen('ollama:pull_progress', (ev) => {
      const p = /** @type {any} */ (ev.payload);
      if (!dlModal || dlModal.phase !== 'pulling') return;
      const status = p?.status ?? '';
      const completed = Number(p?.completed ?? 0);
      const total = Number(p?.total ?? 0);
      dlModal = {
        ...dlModal,
        statusLine: status || t('wizard.download.downloading'),
        progress: total > 0 ? { completed, total } : dlModal.progress,
      };
    });
    try {
      await pullChatModel(model);
      await saveChatModelSetting(model);
      chatInstalled = true;
      dlModal = null;
      unsubPull?.();
      unsubPull = null;
    } catch (e) {
      dlModal = {
        ...(dlModal ?? { model, phase: 'error', statusLine: '', progress: null, errorMessage: '' }),
        phase: 'error',
        errorMessage: e?.message ?? String(e),
      };
      unsubPull?.();
      unsubPull = null;
    }
  }

  async function pullEmbed() {
    if (embedPullBusy || isPullInFlight()) return;
    embedPullBusy = true;
    try {
      await pullChatModel(embedModel);
      embedInstalled = await invoke('ollama_model_installed', { model: embedModel });
    } catch (e) {
      err?.showError?.(e);
    } finally {
      embedPullBusy = false;
    }
  }

  function continueWithoutAi() {
    skipAi = true;
    wizardStatus = t('wizard.deps.skipAiStatus');
    if (mainStep === MS_DEPS || mainStep === MS_MODELS) {
      mainStep = MS_HW;
    }
  }

  async function finishWizard() {
    if (finishBusy) return;
    finishBusy = true;
    try {
      let chat = null;
      let embed = null;
      if (!skipAi) {
        chat = selectedChatModel;
        if (chat) {
          const ok = await checkChatModelInstalled(chat);
          if (!ok) {
            err?.showError?.(t('wizard.errors.chatModelRequired'));
            finishBusy = false;
            return;
          }
          await saveChatModelSetting(chat);
        }
        if (!(await checkChatModelInstalled(embedModel))) {
          err?.showError?.(t('wizard.errors.embedRequired'));
          finishBusy = false;
          return;
        }
        embed = embedModel;
      }

      const res = await invoke('wizard_finish', {
        starterPackId: starterPack,
        wikipediaEnabled: wikipediaEnable,
        openWikipediaSettingsAfter: openWikiSettings,
        chatModel: chat,
        embeddingModel: embed,
        aiSkipped: skipAi,
      });
      const o = /** @type {{ openWikipediaSettings?: boolean }} */ (res);
      if (o?.openWikipediaSettings) {
        if (ui) ui.settingsPendingSection = 'wikipedia';
      }
      onCompleted();
    } catch (e) {
      err?.showError?.(e);
    } finally {
      finishBusy = false;
    }
  }

  let stepTitle = $derived.by(() => {
    if (mainStep === MS_STARTER) return t('wizard.titles.starter');
    if (mainStep === MS_DEPS) return t('wizard.titles.deps');
    if (mainStep === MS_HW) return t('wizard.titles.hw');
    if (mainStep === MS_MODELS) return t('wizard.titles.models');
    if (mainStep === MS_WIKI) return t('wizard.titles.wiki');
    return t('wizard.titles.fallback');
  });

  const wizardStepNumber = $derived.by(() => {
    const order = [MS_STARTER, MS_DEPS, MS_HW, MS_MODELS, MS_WIKI].filter((s) => {
      if (s === MS_DEPS && skipDepsStep) return false;
      if (s === MS_MODELS && (skipAi || skipModelsStep)) return false;
      return true;
    });
    const idx = order.indexOf(mainStep);
    return idx >= 0 ? { current: idx + 1, total: order.length } : null;
  });

  $effect(() => {
    stepTitle;
    mainStep;
    if (mainStep === MS_DEPS && ollamaCheckBusy) {
      wizardStatus = t('wizard.deps.checking');
    } else if (mainStep === MS_HW && hwBusy) {
      wizardStatus = t('wizard.hw.scanning');
    } else if (mainStep !== MS_DEPS && mainStep !== MS_HW && !skipAi) {
      wizardStatus = '';
    }
  });
</script>

<div class="wiz-screen" use:focusTrap role="dialog" aria-modal="true" aria-labelledby="wiz-title">
  <div class="wiz-card">
    {#if wizardStepNumber}
      <p class="wiz-step-count" id="wiz-step-count">
        {t('wizard.stepOf', {
          current: wizardStepNumber.current,
          total: wizardStepNumber.total,
        })}
      </p>
    {/if}
    <div class="sr-only" aria-live="polite" aria-atomic="true">{wizardStatus || stepTitle}</div>
    {#if mainStep === MS_STARTER}
      <img class="wiz-logo" src={grimoireLogo} alt="" width="72" height="50" />
    {/if}
    <h1 id="wiz-title" class="wiz-h1">{stepTitle}</h1>
    <p class="wiz-privacy">{t('wizard.privacy')}</p>

    {#if mainStep === MS_STARTER}
      <p class="wiz-body">{t('wizard.starter.body')}</p>
      <div class="wiz-options" role="radiogroup" aria-label={t('wizard.starter.radiogroup')}>
        {#each starterOptions as o}
          <label class="wiz-opt" class:selected={starterPack === o.id}>
            <input type="radio" name="starter" value={o.id} bind:group={starterPack} />
            <span class="wiz-opt-title">{t(o.labelKey)}</span>
            <span class="wiz-opt-hint">{t(o.hintKey)}</span>
          </label>
        {/each}
      </div>
      <div class="wiz-row">
        <button type="button" class="wiz-btn primary" onclick={stepNext}>{t('common.next')}</button>
      </div>
    {:else if mainStep === MS_DEPS}
      {#if ollamaOk === null && ollamaCheckBusy}
        <p class="wiz-body">{t('wizard.deps.checking')}</p>
      {:else if ollamaOk === true}
        <p class="wiz-body" role="status">{t('wizard.deps.ollamaRunning')}</p>
      {:else}
        <p class="wiz-body">
          {#each tParts('wizard.deps.noOllamaIntro') as part}
            {#if part.type === 'text'}
              {part.value}
            {:else if part.name === 'not'}
              <strong>{t('wizard.deps.notWord')}</strong>
            {/if}
          {/each}
        </p>
        <ol class="wiz-steps">
          <li>
            {#each tParts('wizard.deps.stepInstall') as part}
              {#if part.type === 'text'}
                {part.value}
              {:else if part.name === 'installOllama'}
                <strong>{t('wizard.deps.stepInstallTitle')}</strong>
              {:else if part.name === 'downloadLink'}
                <button type="button" class="wiz-link" onclick={openOllamaDownload}
                  >{t('wizard.deps.downloadLinkLabel')}</button
                >
              {/if}
            {/each}
          </li>
          <li>
            {#each tParts('wizard.deps.stepServe') as part}
              {#if part.type === 'text'}
                {part.value}
              {:else if part.name === 'startService'}
                <strong>{t('wizard.deps.stepServeTitle')}</strong>
              {:else if part.name === 'serveCmd'}
                <code class="wiz-code">{t('wizard.deps.serveCmd')}</code>
              {/if}
            {/each}
          </li>
          <li>
            {#each tParts('wizard.deps.stepCheck') as part}
              {#if part.type === 'text'}
                {part.value}
              {:else if part.name === 'checkConnection'}
                <strong>{t('wizard.deps.stepCheckTitle')}</strong>
              {/if}
            {/each}
          </li>
        </ol>
        {#if ollamaOk === false}
          <p class="wiz-warn" role="alert">{t('wizard.deps.unreachable')}</p>
        {/if}
        <div class="wiz-row">
          <button type="button" class="wiz-btn secondary" onclick={openOllamaDownload}
            >{t('wizard.deps.openDownload')}</button
          >
          <button type="button" class="wiz-btn secondary" onclick={checkOllama} disabled={ollamaCheckBusy}
            >{ollamaCheckBusy ? t('wizard.deps.checkingShort') : t('wizard.deps.checkAgain')}</button
          >
        </div>
      {/if}
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={continueWithoutAi}
          >{t('wizard.deps.continueWithoutAi')}</button
        >
      </div>
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>{t('common.back')}</button>
        <button
          type="button"
          class="wiz-btn primary"
          onclick={stepNext}
          disabled={!depsCanAdvance}
          title={depsCanAdvance ? '' : t('wizard.deps.nextDisabledTitle')}
          >{t('common.next')}</button
        >
      </div>
    {:else if mainStep === MS_HW}
      {#if hwBusy}
        <p class="wiz-body">{t('wizard.hw.scanning')}</p>
      {:else if hardwareReport}
        <ul class="wiz-list">
          <li><strong>{t('wizard.hw.cpu')}</strong> {String(hardwareReport.cpuName ?? '')}</li>
          <li>
            <strong>{t('wizard.hw.ram')}</strong>
            {t('wizard.hw.ramDetail', {
              gb: Math.round(Number(hardwareReport.ramTotalMb ?? 0) / 1024),
            })}
          </li>
          <li><strong>{t('wizard.hw.llmTier')}</strong> {String(hardwareReport.capability ?? '')}</li>
          {#each (hardwareReport.gpus ?? []) as g}
            <li><strong>{t('wizard.hw.gpu')}</strong> {String(g?.name ?? '')}</li>
          {/each}
        </ul>
        {#if showAmdDriverHint}
          <p class="wiz-note">
            {t('wizard.hw.amdHint')}
            <button type="button" class="wiz-link" onclick={openAmdDrivers}
              >{t('wizard.hw.amdDriversLink')}</button
            >
          </p>
        {/if}
        {#if String(hardwareReport?.capability ?? '') !== 'full'}
          <p class="wiz-note">{t('wizard.hw.limitedHardware')}</p>
        {/if}
      {:else}
        <p class="wiz-body">{t('wizard.hw.unavailable')}</p>
      {/if}
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>{t('common.back')}</button>
        <button type="button" class="wiz-btn primary" onclick={stepNext}>{t('common.next')}</button>
      </div>
    {:else if mainStep === MS_MODELS}
      {#if !chatInstalled || !embedInstalled}
        <p class="wiz-body">{t('wizard.models.disclaimer')}</p>
      {/if}
      {#if !chatInstalled}
      <label class="wiz-check">
        <input type="checkbox" bind:checked={useCustomModel} />
        {t('wizard.models.useCustomId')}
      </label>
      {#if useCustomModel}
        <input
          class="wiz-input"
          aria-label={t('wizard.models.customIdAria')}
          placeholder={t('wizard.models.customIdPlaceholder')}
          bind:value={customModel}
        />
      {:else}
        <div class="wiz-options" role="radiogroup" aria-label={t('wizard.models.chatRadiogroup')}>
          {#each curatedForWizard as m}
            <label class="wiz-opt" class:selected={chatPick === m.value}>
              <input type="radio" name="chat" value={m.value} bind:group={chatPick} />
              <span class="wiz-opt-title">{m.label}</span>
              <span class="wiz-opt-hint">{m.statsShort}</span>
            </label>
          {/each}
        </div>
      {/if}
      <div class="wiz-model-actions">
        <button type="button" class="wiz-btn secondary" onclick={startChatPull}
          >{t('wizard.models.pullChat')}</button
        >
      </div>
      {/if}
      {#if !chatInstalled && !embedInstalled}
        <hr class="wiz-hr" />
      {/if}
      {#if !embedInstalled}
      <p class="wiz-body">
        {#each tParts('wizard.models.embedIntro', { model: embedModel }) as part}
          {#if part.type === 'text'}
            {part.value}
          {:else if part.name === 'embedLabel'}
            <strong>{t('wizard.models.embedLabel')}</strong>
          {:else if part.name === 'model'}
            {part.value}
          {/if}
        {/each}
      </p>
        <div class="wiz-row">
          <button type="button" class="wiz-btn secondary" onclick={pullEmbed} disabled={embedPullBusy}>
            {embedPullBusy
              ? t('wizard.models.pulling')
              : t('wizard.models.pullModel', { model: embedModel })}
          </button>
        </div>
      {/if}
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>{t('common.back')}</button>
        <button type="button" class="wiz-btn primary" onclick={stepNext}>{t('common.next')}</button>
      </div>
    {:else if mainStep === MS_WIKI}
      <p class="wiz-body">{t('wizard.wiki.body')}</p>
      <label class="wiz-check">
        <input type="checkbox" bind:checked={wikipediaEnable} />
        {t('wizard.wiki.enable')}
      </label>
      <label class="wiz-check">
        <input type="checkbox" bind:checked={openWikiSettings} disabled={!wikipediaEnable} />
        {t('wizard.wiki.openSettingsAfter')}
      </label>
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>{t('common.back')}</button>
        <button type="button" class="wiz-btn primary" onclick={finishWizard} disabled={finishBusy}>
          {finishBusy ? t('wizard.finish.finishing') : t('wizard.finish.finishSetup')}
        </button>
      </div>
    {/if}
  </div>
</div>

{#if dlModal}
  <ModelDownloadModal
    model={dlModal.model}
    phase={dlModal.phase}
    confirmKind={/** @type {'downloadMissing'} */ (dlModal.confirmKind ?? 'downloadMissing')}
    hardwareWarning={dlModal.hardwareWarning ?? null}
    statusLine={dlModal.statusLine}
    progress={dlModal.progress}
    errorMessage={dlModal.errorMessage}
    onDownload={onModalDownload}
    onCancel={() => {
      dlModal = null;
      unsubPull?.();
      unsubPull = null;
    }}
  />
{/if}

<style>
  .wiz-screen {
    position: fixed;
    inset: 0;
    z-index: 9500;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.55);
    padding: 1rem;
  }
  .wiz-card {
    width: min(560px, 100%);
    max-height: min(90vh, 720px);
    overflow: auto;
    background: var(--bg-elevated, #1e1a16);
    color: var(--text-primary, #f0e6d8);
    border: 1px solid var(--border-subtle, #444);
    border-radius: 10px;
    padding: 1.25rem 1.5rem;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
  }
  .wiz-h1 {
    font-size: 1.35rem;
    margin: 0 0 0.5rem;
  }
  .wiz-logo {
    display: block;
    width: 72px;
    max-width: 100%;
    height: auto;
    margin: 0 auto 0.75rem;
  }
  .wiz-step-count {
    font-size: 0.8rem;
    opacity: 0.75;
    margin: 0 0 0.35rem;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  .wiz-h2 {
    font-size: 1.1rem;
    margin: 0 0 0.5rem;
  }
  .wiz-privacy {
    font-size: 0.85rem;
    opacity: 0.85;
    margin: 0 0 1rem;
    line-height: 1.45;
  }
  .wiz-body {
    line-height: 1.5;
    margin: 0 0 1rem;
  }
  .wiz-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
    margin-top: 1rem;
  }
  .wiz-spacer {
    flex: 1;
  }
  .wiz-btn {
    padding: 0.45rem 0.85rem;
    border-radius: 6px;
    border: 1px solid var(--border-subtle, #555);
    background: transparent;
    color: inherit;
    cursor: pointer;
    font: inherit;
  }
  .wiz-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .wiz-btn.primary {
    background: var(--accent, #a52a2a);
    border-color: var(--accent, #a52a2a);
    color: #fff;
  }
  .wiz-btn.secondary:hover {
    background: rgba(255, 255, 255, 0.06);
  }
  .wiz-options {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .wiz-opt {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.25rem 0.6rem;
    padding: 0.6rem 0.75rem;
    border: 1px solid var(--border-subtle, #444);
    border-radius: 8px;
    cursor: pointer;
  }
  .wiz-opt.selected {
    border-color: var(--accent, #a52a2a);
    background: rgba(165, 42, 42, 0.12);
  }
  .wiz-opt input {
    grid-row: 1 / span 2;
    margin-top: 0.2rem;
  }
  .wiz-opt-title {
    font-weight: 600;
  }
  .wiz-opt-hint {
    grid-column: 2;
    font-size: 0.85rem;
    opacity: 0.85;
  }
  .wiz-check {
    display: flex;
    gap: 0.5rem;
    align-items: flex-start;
    margin: 0.5rem 0;
    cursor: pointer;
    line-height: 1.4;
  }
  .wiz-input {
    width: 100%;
    padding: 0.45rem 0.6rem;
    border-radius: 6px;
    border: 1px solid var(--border-subtle, #555);
    background: var(--bg-input, #111);
    color: inherit;
    margin-bottom: 0.75rem;
    font: inherit;
  }
  .wiz-code {
    font-family: ui-monospace, monospace;
    font-size: 0.9em;
  }
  .wiz-warn {
    color: #f0c674;
    margin: 0.25rem 0 0.75rem;
  }
  .wiz-list {
    margin: 0 0 1rem 1rem;
    line-height: 1.5;
  }
  .wiz-note {
    font-size: 0.9rem;
    opacity: 0.9;
  }
  .wiz-link {
    background: none;
    border: none;
    color: var(--accent, #c96);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
    padding: 0;
  }
  .wiz-hr {
    border: none;
    border-top: 1px solid var(--border-subtle, #444);
    margin: 1rem 0;
  }
  .wiz-model-actions {
    margin: 0.5rem 0 0.75rem;
  }
  .wiz-steps {
    margin: 0 0 1rem 1.1rem;
    padding: 0;
    line-height: 1.55;
  }
  .wiz-steps li {
    margin-bottom: 0.65rem;
  }
  .wiz-steps li:last-child {
    margin-bottom: 0;
  }
</style>
