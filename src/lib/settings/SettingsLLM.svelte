<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, getContext } from 'svelte';
  import { CURATED_CHAT_MODELS, DEFAULT_CHAT_MODEL, isExtraInstalledModel, statsForAnyModelId, isEmbeddingModelId, CURATED_EMBEDDING_MODELS } from '../constants/chatModels.js';
  import { assessChatModelHardware } from '../utils/chatModelHardware.js';
  import { firstInstalledFullName } from '../utils/ollamaModelMatch.js';
  import {
    checkChatModelInstalled,
    saveChatModelSetting,
    pullChatModel,
    isPullInFlight,
    deleteOllamaModel,
  } from '../services/chatModelSelection.js';
  import ModelDownloadModal from '../ModelDownloadModal.svelte';
  import ChatModelCombobox from '../ChatModelCombobox.svelte';
  import { t, tParts } from '../i18n/t.js';
  import { formatAppError } from '../i18n/formatAppError.js';

  const settings = getContext('settings');
  const hardwareReport = $derived(settings.hardwareReport ?? null);

  let {
    keepInMemory = false, onKeepInMemoryChange = () => {},
  } = $props();

  let chatModel       = $state(DEFAULT_CHAT_MODEL);
  /** Mirrors the chat model picker; reverts on cancel or failed check. */
  let chatModelSelectUi = $state(DEFAULT_CHAT_MODEL);
  let extraInstalledModels = $state([]);
  let chatModelSelectBusy = $state(false);
  let chatModelSelectError = $state('');
  /** `value` of the row currently being removed from Ollama, if any. */
  let uninstallBusy = $state(/** @type {string | null} */ (null));
  let customChatModelInput = $state('');
  /** @type {null | { model: string, phase: 'confirm' | 'pulling' | 'error', confirmKind?: 'downloadMissing' | 'installedRisk', hardwareWarning?: { level: string, lines: string[] } | null, statusLine: string, progress: { completed: number, total: number } | null, errorMessage: string }} */
  let modelDownloadModal = $state(null);

  const chatModelSelectOptions = $derived.by(() => {
    const inst = extraInstalledModels;
    const rows = CURATED_CHAT_MODELS.map((p) => ({
      value: p.value,
      label: p.label,
      installedFull: firstInstalledFullName(p.value, inst),
      ...statsForAnyModelId(p.value),
    }));
    const seen = new Set(rows.map((r) => r.value));
    const extras = [];
    for (const n of inst) {
      if (!isExtraInstalledModel(n)) continue;
      if (isEmbeddingModelId(n)) continue;
      if (seen.has(n)) continue;
      seen.add(n);
      extras.push({ value: n, label: n, installedFull: n, ...statsForAnyModelId(n) });
    }
    extras.sort((a, b) => a.value.localeCompare(b.value));
    rows.push(...extras);
    if (chatModel && !seen.has(chatModel)) {
      rows.push({
        value: chatModel,
        label: chatModel,
        installedFull: firstInstalledFullName(chatModel, inst),
        ...statsForAnyModelId(chatModel),
      });
    }
    return rows;
  });

  const selectedChatModelStats = $derived(statsForAnyModelId(chatModelSelectUi));

  async function refreshExtraInstalledModels() {
    try {
      const list = await invoke('list_ollama_installed_models');
      extraInstalledModels = Array.isArray(list) ? list : [];
    } catch {
      extraInstalledModels = [];
    }
  }

  /**
   * @param {{ value: string, installedFull?: string | null }} opt
   */
  async function uninstallSettingsChatModel(opt) {
    if (!opt.installedFull || uninstallBusy) return;
    if (
      !confirm(
        t('settings.llm.uninstallConfirm', { name: opt.installedFull }),
      )
    ) {
      return;
    }
    uninstallBusy = opt.value;
    chatModelSelectError = '';
    try {
      await deleteOllamaModel(opt.value);
      if (!(await checkChatModelInstalled(chatModel))) {
        await saveChatModelSetting(DEFAULT_CHAT_MODEL);
        chatModel = DEFAULT_CHAT_MODEL;
        chatModelSelectUi = DEFAULT_CHAT_MODEL;
      }
      await refreshExtraInstalledModels();
    } catch (e) {
      chatModelSelectError = formatAppError(e);
    } finally {
      uninstallBusy = null;
    }
  }

  async function commitSettingsChatModel(next) {
    const modelId = String(next).trim();
    if (!modelId || chatModelSelectBusy || uninstallBusy) return;
    if (isPullInFlight()) return;
    if (isEmbeddingModelId(modelId)) {
      chatModelSelectError = t('settings.llm.embeddingModelError');
      chatModelSelectUi = chatModel;
      return;
    }
    if (modelId === chatModel) {
      chatModelSelectUi = chatModel;
      return;
    }
    chatModelSelectBusy = true;
    chatModelSelectError = '';
    try {
      const installed = await checkChatModelInstalled(modelId);
      const hwWarn = assessChatModelHardware(modelId, hardwareReport);

      if (!installed) {
        chatModelSelectUi = chatModel;
        modelDownloadModal = {
          model: modelId,
          phase: 'confirm',
          confirmKind: 'downloadMissing',
          hardwareWarning: hwWarn.level === 'ok' ? null : hwWarn,
          statusLine: '',
          progress: null,
          errorMessage: '',
        };
        return;
      }

      if (hwWarn.level !== 'ok') {
        chatModelSelectUi = modelId;
        modelDownloadModal = {
          model: modelId,
          phase: 'confirm',
          confirmKind: 'installedRisk',
          hardwareWarning: hwWarn,
          statusLine: '',
          progress: null,
          errorMessage: '',
        };
        return;
      }

      chatModel = modelId;
      chatModelSelectUi = modelId;
      await saveChatModelSetting(modelId);
      await refreshExtraInstalledModels();
    } catch (e) {
      chatModelSelectError = formatAppError(e);
      chatModelSelectUi = chatModel;
    } finally {
      chatModelSelectBusy = false;
    }
  }

  async function onModelDownloadConfirm() {
    const m = modelDownloadModal;
    if (!m || m.phase !== 'confirm') return;

    if (m.confirmKind === 'installedRisk') {
      const name = m.model;
      chatModel = name;
      chatModelSelectUi = name;
      await saveChatModelSetting(name);
      await refreshExtraInstalledModels();
      modelDownloadModal = null;
      return;
    }

    const name = m.model;
    const hwW = m.hardwareWarning ?? null;
    modelDownloadModal = {
      model: name,
      phase: 'pulling',
      confirmKind: 'downloadMissing',
      hardwareWarning: hwW,
      statusLine: '',
      progress: null,
      errorMessage: '',
    };
    try {
      let pullProgress = null;
      await pullChatModel(name, (payload) => {
        const status = typeof payload?.status === 'string' ? payload.status : '';
        const completed = typeof payload?.completed === 'number' ? payload.completed : null;
        const total = typeof payload?.total === 'number' ? payload.total : null;
        if (completed != null && total != null && total > 0) {
          pullProgress = { completed, total };
        }
        const line = status || JSON.stringify(payload);
        modelDownloadModal = {
          model: name,
          phase: 'pulling',
          confirmKind: 'downloadMissing',
          hardwareWarning: hwW,
          statusLine: line,
          progress: pullProgress,
          errorMessage: '',
        };
      });
      const ok = await checkChatModelInstalled(name);
      if (!ok) {
        throw new Error(t('errors.modelNotInstalledAfterPull'));
      }
      chatModel = name;
      chatModelSelectUi = name;
      await saveChatModelSetting(name);
      await refreshExtraInstalledModels();
      modelDownloadModal = null;
    } catch (e) {
      modelDownloadModal = {
        model: name,
        phase: 'error',
        confirmKind: 'downloadMissing',
        hardwareWarning: null,
        statusLine: '',
        progress: null,
        errorMessage: formatAppError(e),
      };
    }
  }

  function closeModelDownloadModal() {
    if (modelDownloadModal?.confirmKind === 'installedRisk') {
      chatModelSelectUi = chatModel;
    }
    modelDownloadModal = null;
  }
  let embeddingModel  = $state('nomic-embed-text');
  let initialEmbeddingModel = $state('nomic-embed-text'); // model the current index was built with
  let reindexStatus   = $state(''); // '', 'clearing', 'reindexing', 'done', 'error'
  let reindexError    = $state('');
  let reindexSummaryText = $state('');
  let reindexProgress = $state({
    indexed: 0,
    processed: 0,
    total: 0,
    permanently_skipped: 0,
    phase: null,
    embeddingChunks: null,
  });
  /** True once progress events indicate we continued a checkpointed run. */
  let reindexRunIsResume = $state(false);
  /** Retries for transient embed / vector-store failures in background tasks (0–10). */
  let backgroundMaxRetries = $state(2);
  let chatTemperature = $state(0.8);
  let chatTopP        = $state(0.9);
  let chatTopK        = $state(40);
  let chatRepeatPenalty = $state(1.1);
  let chatNumCtx      = $state(8192);
  let verbosity       = $state('concise');

  onMount(async () => {
    const [model, embed, temp, top_p, top_k, repeat, ctx, verb, maxRetries] = await Promise.all([
      invoke('get_setting', { key: 'chat_model' }),
      invoke('get_setting', { key: 'embedding_model' }),
      invoke('get_setting', { key: 'chat_temperature' }),
      invoke('get_setting', { key: 'chat_top_p' }),
      invoke('get_setting', { key: 'chat_top_k' }),
      invoke('get_setting', { key: 'chat_repeat_penalty' }),
      invoke('get_setting', { key: 'chat_num_ctx' }),
      invoke('get_setting', { key: 'chat_verbosity' }),
      invoke('get_setting', { key: 'background_max_retries' }),
    ]);

    if (model) {
      chatModel = model;
      chatModelSelectUi = model;
    }
    if (embed)  { embeddingModel = embed; initialEmbeddingModel = embed; }
    if (temp)   chatTemperature = parseFloat(temp);
    if (top_p)  chatTopP        = parseFloat(top_p);
    if (top_k)  chatTopK        = parseInt(top_k, 10);
    if (repeat) chatRepeatPenalty = parseFloat(repeat);
    if (ctx)    chatNumCtx      = parseInt(ctx, 10);
    if (verb)   verbosity       = verb;
    if (maxRetries !== null && maxRetries !== undefined && maxRetries !== '') {
      const n = parseInt(String(maxRetries), 10);
      if (!Number.isNaN(n)) backgroundMaxRetries = Math.min(10, Math.max(0, n));
    }
    await refreshExtraInstalledModels();
  });

  function save(key, value) {
    invoke('set_setting', { key, value: String(value) }).catch(() => {});
  }

  async function clearAndReindex() {
    reindexStatus = 'clearing';
    reindexError = '';
    reindexSummaryText = '';
    reindexRunIsResume = false;
    reindexProgress = {
      indexed: 0,
      processed: 0,
      total: 0,
      permanently_skipped: 0,
      phase: null,
      embeddingChunks: null,
    };
    let unlisten = null;
    try {
      await Promise.all([
        invoke('clear_notes_index'),
        invoke('clear_wiki_index'),
        invoke('clear_scanned_index'),
      ]);
      reindexStatus = 'reindexing';
      unlisten = await listen('reindex:progress', (ev) => {
        const pl = ev.payload;
        if (pl?.resuming) reindexRunIsResume = true;
        reindexProgress = {
          indexed: pl.indexed ?? 0,
          processed: pl.processed ?? 0,
          total: pl.total ?? 0,
          permanently_skipped: pl.permanently_skipped ?? 0,
          phase: pl.phase ?? null,
          embeddingChunks: pl.embedding_chunks ?? null,
        };
      });
      reindexSummaryText = await invoke('reindex_all', { forceRestart: true });
      initialEmbeddingModel = embeddingModel;
      reindexStatus = 'done';
    } catch (e) {
      const msg = e?.message ?? String(e);
      if (e?.kind === 'OllamaUnavailable' || e?.kind === 'EmbeddingFailed') {
        reindexError = formatAppError(e);
      } else {
        reindexError = msg;
      }
      reindexError += t('settings.llm.reindexPartialHint');
      reindexStatus = 'error';
    } finally {
      unlisten?.();
    }
  }
</script>

<h3>{t('settings.llm.title')}</h3>
{#if modelDownloadModal}
  <ModelDownloadModal
    model={modelDownloadModal.model}
    phase={modelDownloadModal.phase}
    confirmKind={modelDownloadModal.confirmKind ?? 'downloadMissing'}
    hardwareWarning={modelDownloadModal.hardwareWarning ?? null}
    statusLine={modelDownloadModal.statusLine}
    progress={modelDownloadModal.progress}
    errorMessage={modelDownloadModal.errorMessage}
    onDownload={onModelDownloadConfirm}
    onCancel={closeModelDownloadModal}
  />
{/if}
<p class="settings-notice">{t('settings.llm.notice')}</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.chatModel')}</span>
    <span class="setting-desc">{t('settings.llm.chatModelDesc')}</span>
  </div>
  <ChatModelCombobox
    variant="settings"
    selected={chatModelSelectUi}
    options={chatModelSelectOptions}
    disabled={chatModelSelectBusy || isPullInFlight() || !!uninstallBusy}
    ariaLabel={t('settings.llm.chatModelAria')}
    onOpenChange={(o) => {
      if (o) void refreshExtraInstalledModels();
    }}
    onSelect={(v) => commitSettingsChatModel(v)}
    onUninstall={uninstallSettingsChatModel}
    uninstallBusyKey={uninstallBusy}
  />
</div>
<p class="model-stats-hint" title={selectedChatModelStats.statsDetail}>
  <strong>{chatModelSelectUi}</strong> {t('settings.llm.modelStatsSuffix', { stats: selectedChatModelStats.statsShort })}
</p>
{#if chatModelSelectError}
  <p class="settings-notice" style="color: var(--danger); margin-top: -6px;">{chatModelSelectError}</p>
{/if}

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.customChatModel')}</span>
    <span class="setting-desc">{t('settings.llm.customChatModelDesc')}</span>
  </div>
  <div class="llm-custom-model-actions">
    <input
      type="text"
      class="llm-custom-model-input"
      bind:value={customChatModelInput}
      placeholder={t('settings.llm.customModelPlaceholder')}
      disabled={chatModelSelectBusy || isPullInFlight()}
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          commitSettingsChatModel(customChatModelInput);
        }
      }}
    />
    <button
      type="button"
      class="llm-custom-model-apply"
      disabled={chatModelSelectBusy || isPullInFlight() || !customChatModelInput.trim()}
      onclick={() => commitSettingsChatModel(customChatModelInput)}
    >{t('common.apply')}</button>
  </div>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.keepInMemory')}</span>
    <span class="setting-desc">{t('settings.llm.keepInMemoryDesc')}</span>
  </div>
  <label class="toggle">
    <input type="checkbox" checked={keepInMemory} onchange={(e) => onKeepInMemoryChange(e.currentTarget.checked)} />
    <span class="toggle-label">{keepInMemory ? t('common.on') : t('common.off')}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.embeddingModel')}</span>
    <span class="setting-desc">{t('settings.llm.embeddingModelDesc')}</span>
  </div>
  <select bind:value={embeddingModel} onchange={() => save('embedding_model', embeddingModel)}>
    {#each CURATED_EMBEDDING_MODELS as em (em.value)}
      <option value={em.value}>{em.label}</option>
    {/each}
  </select>
</div>

{#if embeddingModel !== initialEmbeddingModel || reindexStatus !== ''}
  <div class="reindex-warning">
    {#if reindexStatus === ''}
      <p class="reindex-msg">
        {#each tParts('settings.llm.embedChanged', { model: initialEmbeddingModel }) as part}
          {#if part.type === 'text'}{part.value}{:else if part.name === 'model'}<strong>{initialEmbeddingModel}</strong>{/if}
        {/each}
      </p>
      <button class="settings-action-btn reindex-btn" onclick={clearAndReindex}>{t('settings.llm.clearAndReindex')}</button>
    {:else if reindexStatus === 'clearing'}
      <p class="reindex-msg">{t('settings.llm.clearingIndexes')}</p>
    {:else if reindexStatus === 'reindexing'}
      {@const pct = reindexProgress.total > 0 ? Math.round((reindexProgress.processed / reindexProgress.total) * 100) : 0}
      {@const sk = reindexProgress.permanently_skipped ?? 0}
      {@const verb = reindexRunIsResume ? t('settings.llm.resuming') : t('settings.llm.reindexing')}
      <p class="reindex-msg">
        {t('settings.llm.reindexProgress', {
          verb,
          model: embeddingModel,
          processed: reindexProgress.processed,
          total: reindexProgress.total,
          pct,
          skippedPart: sk > 0 ? t('settings.llm.skippedAfterRetries', { count: sk }) : '',
        })}
      </p>
      {#if reindexProgress.embeddingChunks}
        <p class="reindex-msg">
          {t('settings.llm.embeddingChunks', {
            title: reindexProgress.embeddingChunks.note_title,
            done: reindexProgress.embeddingChunks.done,
            total: reindexProgress.embeddingChunks.total,
          })}
        </p>
      {/if}
      <div class="reindex-bar-track"><div class="reindex-bar-fill" style="width: {pct}%"></div></div>
    {:else if reindexStatus === 'done'}
      <p class="reindex-msg reindex-done">
        {reindexSummaryText || t('settings.llm.reindexDoneDefault')}{' '}
        {t('settings.llm.reindexDoneSuffix')}
      </p>
    {:else if reindexStatus === 'error'}
      <p class="reindex-msg reindex-error">{t('settings.llm.reindexFailed', { error: reindexError })}</p>
      <button class="settings-action-btn reindex-btn" onclick={clearAndReindex}>{t('settings.shared.retry')}</button>
    {/if}
  </div>
{/if}

<h4 class="settings-subsection">{t('settings.llm.reliability')}</h4>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.backgroundRetries')}</span>
    <span class="setting-desc">{t('settings.llm.backgroundRetriesDesc')}</span>
  </div>
  <input
    type="number"
    class="setting-num"
    bind:value={backgroundMaxRetries}
    min="0"
    max="10"
    step="1"
    onchange={() => save('background_max_retries', Math.min(10, Math.max(0, parseInt(String(backgroundMaxRetries), 10) || 0)))}
  />
</div>

<h4 class="settings-subsection">{t('settings.llm.inferenceParams')}</h4>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.temperature')}</span>
    <span class="setting-desc">{t('settings.llm.temperatureDesc')}</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatTemperature} min="0" max="2" step="0.05" onchange={() => save('chat_temperature', chatTemperature)} />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.verbosity')}</span>
    <span class="setting-desc">{t('settings.llm.verbosityDesc')}</span>
  </div>
  <select bind:value={verbosity} onchange={() => save('chat_verbosity', verbosity)}>
    <option value="concise">{t('settings.llm.verbosityConcise')}</option>
    <option value="thorough">{t('settings.llm.verbosityThorough')}</option>
    <option value="caveman">{t('settings.llm.verbosityCaveman')}</option>
  </select>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.topP')}</span>
    <span class="setting-desc">{t('settings.llm.topPDesc')}</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatTopP} min="0" max="1" step="0.05" onchange={() => save('chat_top_p', chatTopP)} />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.topK')}</span>
    <span class="setting-desc">{t('settings.llm.topKDesc')}</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatTopK} min="0" max="200" step="1" onchange={() => save('chat_top_k', chatTopK)} />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.repeatPenalty')}</span>
    <span class="setting-desc">{t('settings.llm.repeatPenaltyDesc')}</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatRepeatPenalty} min="0.5" max="2" step="0.05" onchange={() => save('chat_repeat_penalty', chatRepeatPenalty)} />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.llm.contextWindow')}</span>
    <span class="setting-desc">{t('settings.llm.contextWindowDesc')}</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatNumCtx} min="512" max="131072" step="512" onchange={() => save('chat_num_ctx', chatNumCtx)} />
</div>
