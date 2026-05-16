<!-- Copyright (C) 2026 Wim Palland
     Part of Grimoire — GPL-3.0 or later. -->

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

  /** @type {{ onCompleted: () => void }} */
  let { onCompleted } = $props();

  /** @type {{ showError: (e: unknown) => void }} */
  const err = getContext('err');
  /** @type {{ settingsPendingSection?: string | null } | undefined} */
  const ui = getContext('ui');

  const MS_TOUR = 0;
  const MS_STARTER = 1;
  const MS_DEPS = 2;
  const MS_HW = 3;
  const MS_MODELS = 4;
  const MS_WIKI = 5;

  let mainStep = $state(MS_TOUR);
  let tourSlide = $state(0);

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
  let embedPullBusy = $state(false);

  let wikipediaEnable = $state(false);
  let openWikiSettings = $state(false);

  let finishBusy = $state(false);

  /** @type {null | { model: string, phase: 'confirm' | 'pulling' | 'error', confirmKind?: string, hardwareWarning?: unknown, statusLine: string, progress: { completed: number, total: number } | null, errorMessage: string }} */
  let dlModal = $state(null);
  let unsubPull = /** @type {null | (() => void)} */ (null);

  const tourSlides = [
    {
      title: 'Notes and folders',
      body: 'Write Markdown notes, organise them in the folder tree, and use tags plus `[[wiki-links]]` between ideas.',
    },
    {
      title: 'Chat with your vault',
      body: 'The chat sidebar uses a local Ollama model. Grimoire never sends your notes to the cloud — retrieval and inference stay on this machine.',
    },
    {
      title: 'Search',
      body: 'Use Search (Ctrl+F) for full-text and semantic search across unlocked notes.',
    },
    {
      title: 'Settings',
      body: 'All models, privacy tools, and optional sources like Wikipedia are configured in Settings — the same place you can change anything later.',
    },
  ];

  const starterOptions = [
    { id: 'empty', label: 'Empty workspace', hint: 'No folders or starter notes — just a blank vault.' },
    { id: 'pkm', label: 'Knowledge builders (PKM)', hint: 'Inbox, fleeting, literature, permanent, and maps-of-content folders plus a welcome note.' },
    { id: 'bullet_journal', label: 'Bullet journal', hint: 'Collections, future log, and monthly folders with a short setup note.' },
    { id: 'para', label: 'PARA method', hint: 'Projects, Areas, Resources, and Archives — each with a short README note.' },
  ];

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

  $effect(() => {
    const rows = curatedForWizard;
    if (!rows.length) return;
    if (!rows.some((r) => r.value === chatPick)) {
      chatPick = rows[0].value;
    }
  });

  onMount(() => {
    return () => {
      unsubPull?.();
    };
  });

  async function checkOllama() {
    ollamaCheckBusy = true;
    try {
      await invoke('list_ollama_installed_models');
      ollamaOk = true;
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

  $effect(() => {
    if (mainStep === MS_HW && !hardwareReport && !hwBusy) {
      void loadHardware();
    }
    if (mainStep === MS_DEPS && ollamaOk === null && !ollamaCheckBusy) {
      void checkOllama();
    }
    if (mainStep === MS_MODELS) {
      void refreshEmbedInstalled();
    }
  });

  function tourNext() {
    if (tourSlide < tourSlides.length - 1) {
      tourSlide += 1;
    } else {
      mainStep = MS_STARTER;
    }
  }

  function tourBack() {
    if (tourSlide > 0) tourSlide -= 1;
  }

  function skipTour() {
    mainStep = MS_STARTER;
  }

  function stepNext() {
    if (mainStep < MS_WIKI) mainStep += 1;
  }

  function stepBack() {
    if (mainStep > MS_STARTER) {
      mainStep -= 1;
      return;
    }
    if (mainStep === MS_STARTER) {
      mainStep = MS_TOUR;
      tourSlide = tourSlides.length - 1;
    }
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
    dlModal = { ...dlModal, phase: 'pulling', statusLine: 'Starting…', progress: null, errorMessage: '' };
    unsubPull?.();
    unsubPull = await listen('ollama:pull_progress', (ev) => {
      const p = /** @type {any} */ (ev.payload);
      if (!dlModal || dlModal.phase !== 'pulling') return;
      const status = p?.status ?? '';
      const completed = Number(p?.completed ?? 0);
      const total = Number(p?.total ?? 0);
      dlModal = {
        ...dlModal,
        statusLine: status || 'Downloading…',
        progress: total > 0 ? { completed, total } : dlModal.progress,
      };
    });
    try {
      await pullChatModel(model);
      await saveChatModelSetting(model);
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

  async function finishWizard() {
    if (finishBusy) return;
    finishBusy = true;
    try {
      const chat = String(useCustomModel ? customModel.trim() : chatPick).trim();
      if (chat) {
        const ok = await checkChatModelInstalled(chat);
        if (!ok) {
          err?.showError?.(
            'Pull or pick an installed chat model before finishing, or clear the custom id.',
          );
          finishBusy = false;
          return;
        }
        await saveChatModelSetting(chat);
      }
      if (!(await checkChatModelInstalled(embedModel))) {
        err?.showError?.('Pull the embedding model before finishing (required for semantic search).');
        finishBusy = false;
        return;
      }

      const res = await invoke('wizard_finish', {
        starterPackId: starterPack,
        wikipediaEnabled: wikipediaEnable,
        openWikipediaSettingsAfter: openWikiSettings,
        chatModel: chat || null,
        embeddingModel: embedModel,
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
    if (mainStep === MS_TOUR) return 'Welcome to Grimoire';
    if (mainStep === MS_STARTER) return 'Workspace starter';
    if (mainStep === MS_DEPS) return 'Local AI runtime';
    if (mainStep === MS_HW) return 'Your hardware';
    if (mainStep === MS_MODELS) return 'Models';
    if (mainStep === MS_WIKI) return 'Wikipedia (optional)';
    return 'Setup';
  });
</script>

<div class="wiz-screen" use:focusTrap role="dialog" aria-modal="true" aria-labelledby="wiz-title">
  <div class="wiz-card">
    <h1 id="wiz-title" class="wiz-h1">{stepTitle}</h1>
    <p class="wiz-privacy">
      Grimoire is local-first: nothing here phones home. Network use is only what you explicitly start (for
      example pulling an Ollama model or downloading Wikipedia later).
    </p>

    {#if mainStep === MS_TOUR}
      <div class="wiz-tour">
        <h2 class="wiz-h2">{tourSlides[tourSlide]?.title}</h2>
        <p class="wiz-body">{tourSlides[tourSlide]?.body}</p>
      </div>
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={skipTour}>Skip tour</button>
        <div class="wiz-spacer"></div>
        {#if tourSlide > 0}
          <button type="button" class="wiz-btn secondary" onclick={tourBack}>Back</button>
        {/if}
        <button type="button" class="wiz-btn primary" onclick={tourNext}>
          {tourSlide < tourSlides.length - 1 ? 'Next' : 'Continue'}
        </button>
      </div>
    {:else if mainStep === MS_STARTER}
      <p class="wiz-body">Pick a starting layout. You can change folders and notes freely afterwards.</p>
      <div class="wiz-options" role="radiogroup" aria-label="Starter workspace">
        {#each starterOptions as o}
          <label class="wiz-opt" class:selected={starterPack === o.id}>
            <input type="radio" name="starter" value={o.id} bind:group={starterPack} />
            <span class="wiz-opt-title">{o.label}</span>
            <span class="wiz-opt-hint">{o.hint}</span>
          </label>
        {/each}
      </div>
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>Back</button>
        <button type="button" class="wiz-btn primary" onclick={stepNext}>Next</button>
      </div>
    {:else if mainStep === MS_DEPS}
      <p class="wiz-body">
        Grimoire uses <strong>Ollama</strong> on your machine for chat and embeddings. Install it, start
        <code class="wiz-code">ollama serve</code>, then re-check.
      </p>
      {#if ollamaOk === true}
        <p class="wiz-ok" role="status">Ollama is reachable.</p>
      {:else if ollamaOk === false}
        <p class="wiz-warn" role="alert">Could not reach Ollama on this computer.</p>
      {/if}
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={openOllamaDownload}>Open Ollama download</button>
        <button type="button" class="wiz-btn secondary" onclick={checkOllama} disabled={ollamaCheckBusy}
          >{ollamaCheckBusy ? 'Checking…' : 'Check again'}</button
        >
      </div>
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>Back</button>
        <button type="button" class="wiz-btn primary" onclick={stepNext}>Next</button>
      </div>
    {:else if mainStep === MS_HW}
      {#if hwBusy}
        <p class="wiz-body">Scanning hardware…</p>
      {:else if hardwareReport}
        <ul class="wiz-list">
          <li><strong>CPU:</strong> {String(hardwareReport.cpuName ?? '')}</li>
          <li>
            <strong>RAM:</strong>
            {Math.round(Number(hardwareReport.ramTotalMb ?? 0) / 1024)} GB total (Grimoire uses this for indexing
            speed hints)
          </li>
          <li><strong>LLM tier:</strong> {String(hardwareReport.capability ?? '')}</li>
          {#each (hardwareReport.gpus ?? []) as g}
            <li><strong>GPU:</strong> {String(g?.name ?? '')}</li>
          {/each}
        </ul>
        {#if showAmdDriverHint}
          <p class="wiz-note">
            AMD GPUs often need an up-to-date graphics driver (Vulkan) for smooth local inference.
            <button type="button" class="wiz-link" onclick={openAmdDrivers}>AMD driver support</button>
          </p>
        {/if}
      {:else}
        <p class="wiz-body">Hardware details unavailable — you can review them later under Settings → Hardware.</p>
      {/if}
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>Back</button>
        <button type="button" class="wiz-btn primary" onclick={stepNext}>Next</button>
      </div>
    {:else if mainStep === MS_MODELS}
      <p class="wiz-body">
        Third-party models are community weights — use them at your own risk. Grimoire does not vet model behaviour.
      </p>
      <label class="wiz-check">
        <input type="checkbox" bind:checked={useCustomModel} />
        Use custom Ollama model id
      </label>
      {#if useCustomModel}
        <input class="wiz-input" placeholder="e.g. mistral:7b-instruct" bind:value={customModel} />
      {:else}
        <div class="wiz-options" role="radiogroup" aria-label="Chat model">
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
        <button type="button" class="wiz-btn secondary" onclick={startChatPull}>Pull / save chat model</button>
      </div>
      <hr class="wiz-hr" />
      <p class="wiz-body">
        <strong>Embedding model</strong> ({embedModel}) powers semantic search. It must be installed in Ollama.
      </p>
      {#if embedInstalled}
        <p class="wiz-ok" role="status">Embedding model is installed.</p>
      {:else}
        <p class="wiz-warn" role="status">Embedding model not installed yet.</p>
      {/if}
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={pullEmbed} disabled={embedPullBusy}>
          {embedPullBusy ? 'Pulling…' : `Pull ${embedModel}`}
        </button>
        <button type="button" class="wiz-btn secondary" onclick={refreshEmbedInstalled}>Refresh status</button>
      </div>
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>Back</button>
        <button type="button" class="wiz-btn primary" onclick={stepNext}>Next</button>
      </div>
    {:else if mainStep === MS_WIKI}
      <p class="wiz-body">
        Wikipedia is fully offline after download. Bundles can be large — configure downloads later in Settings if you
        prefer.
      </p>
      <label class="wiz-check">
        <input type="checkbox" bind:checked={wikipediaEnable} />
        Enable Wikipedia in the app (you can download a language bundle from Settings → Wikipedia)
      </label>
      <label class="wiz-check">
        <input type="checkbox" bind:checked={openWikiSettings} disabled={!wikipediaEnable} />
        Open Settings on Wikipedia after setup
      </label>
      <div class="wiz-row">
        <button type="button" class="wiz-btn secondary" onclick={stepBack}>Back</button>
        <button type="button" class="wiz-btn primary" onclick={finishWizard} disabled={finishBusy}>
          {finishBusy ? 'Finishing…' : 'Finish setup'}
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
  .wiz-tour {
    min-height: 7rem;
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
  .wiz-ok {
    color: #8fd19e;
    margin: 0.25rem 0 0.75rem;
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
</style>
