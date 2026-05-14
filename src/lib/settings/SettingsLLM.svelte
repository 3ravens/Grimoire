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

  function fmtAppError(e) {
    const msg = e?.message ?? String(e);
    if (e?.kind === 'OllamaUnavailable') return `${msg} — Make sure Ollama is running: ollama serve`;
    return msg;
  }

  /**
   * @param {{ value: string, installedFull?: string | null }} opt
   */
  async function uninstallSettingsChatModel(opt) {
    if (!opt.installedFull || uninstallBusy) return;
    if (
      !confirm(
        `Remove "${opt.installedFull}" from Ollama? This deletes the local copy; you can pull it again later.`,
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
      chatModelSelectError = fmtAppError(e);
    } finally {
      uninstallBusy = null;
    }
  }

  async function commitSettingsChatModel(next) {
    const t = String(next).trim();
    if (!t || chatModelSelectBusy || uninstallBusy) return;
    if (isPullInFlight()) return;
    if (isEmbeddingModelId(t)) {
      chatModelSelectError =
        'That model is for embeddings (semantic search), not chat. Pick a chat model here, or change the embedding model above.';
      chatModelSelectUi = chatModel;
      return;
    }
    if (t === chatModel) {
      chatModelSelectUi = chatModel;
      return;
    }
    chatModelSelectBusy = true;
    chatModelSelectError = '';
    try {
      const installed = await checkChatModelInstalled(t);
      const hwWarn = assessChatModelHardware(t, hardwareReport);

      if (!installed) {
        chatModelSelectUi = chatModel;
        modelDownloadModal = {
          model: t,
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
        chatModelSelectUi = t;
        modelDownloadModal = {
          model: t,
          phase: 'confirm',
          confirmKind: 'installedRisk',
          hardwareWarning: hwWarn,
          statusLine: '',
          progress: null,
          errorMessage: '',
        };
        return;
      }

      chatModel = t;
      chatModelSelectUi = t;
      await saveChatModelSetting(t);
      await refreshExtraInstalledModels();
    } catch (e) {
      chatModelSelectError = fmtAppError(e);
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
        throw new Error('Model still not reported as installed after pull.');
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
        errorMessage: fmtAppError(e),
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
      if (e?.kind === 'OllamaUnavailable') {
        reindexError = `${msg} — Make sure Ollama is running: ollama serve`;
      } else if (e?.kind === 'EmbeddingFailed') {
        reindexError = `${msg} — Check that your embedding model is pulled (ollama pull <model>)`;
      } else {
        reindexError = msg;
      }
      reindexError += ' Partial progress may be saved; use Resume on the home banner or Retry here.';
      reindexStatus = 'error';
    } finally {
      unlisten?.();
    }
  }
</script>

<h3>LLM</h3>
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
<p class="settings-notice">
  Model changes take effect on the next chat. Models are installed and managed through Ollama.
</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Chat model</span>
    <span class="setting-desc">The model used for chat responses and note improvements.</span>
  </div>
  <ChatModelCombobox
    variant="settings"
    selected={chatModelSelectUi}
    options={chatModelSelectOptions}
    disabled={chatModelSelectBusy || isPullInFlight() || !!uninstallBusy}
    ariaLabel="Chat model"
    onOpenChange={(o) => {
      if (o) void refreshExtraInstalledModels();
    }}
    onSelect={(v) => commitSettingsChatModel(v)}
    onUninstall={uninstallSettingsChatModel}
    uninstallBusyKey={uninstallBusy}
  />
</div>
<p class="model-stats-hint" title={selectedChatModelStats.statsDetail}>
  <strong>{chatModelSelectUi}</strong> — {selectedChatModelStats.statsShort} (typical Ollama defaults; exact tag/quant may differ)
</p>
{#if chatModelSelectError}
  <p class="settings-notice" style="color: var(--danger); margin-top: -6px;">{chatModelSelectError}</p>
{/if}

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Custom chat model</span>
    <span class="setting-desc">
      Any Ollama chat model id not listed above (exact name or tag). Apply checks installation, can download the model, and saves it as your chat model. Installed models also show in the chat panel picker.
    </span>
  </div>
  <div class="llm-custom-model-actions">
    <input
      type="text"
      class="llm-custom-model-input"
      bind:value={customChatModelInput}
      placeholder="e.g. mixtral:latest"
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
    >Apply</button>
  </div>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Keep model in memory</span>
    <span class="setting-desc">
      Keeps the chat model loaded at all times. Eliminates cold-start delay but
      holds ~4–8 GB of RAM continuously.
    </span>
  </div>
  <label class="toggle">
    <input type="checkbox" checked={keepInMemory} onchange={(e) => onKeepInMemoryChange(e.currentTarget.checked)} />
    <span class="toggle-label">{keepInMemory ? 'On' : 'Off'}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Embedding model</span>
    <span class="setting-desc">
      Used to index notes for semantic search. Changing this invalidates the
      current index — a full re-index will be required.
    </span>
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
      <p class="reindex-msg">Embedding model changed. The existing index was built with <strong>{initialEmbeddingModel}</strong> and must be cleared and rebuilt before search works correctly.</p>
      <button class="settings-action-btn reindex-btn" onclick={clearAndReindex}>Clear index &amp; re-index all notes</button>
    {:else if reindexStatus === 'clearing'}
      <p class="reindex-msg">Clearing all indexes…</p>
    {:else if reindexStatus === 'reindexing'}
      {@const pct = reindexProgress.total > 0 ? Math.round((reindexProgress.processed / reindexProgress.total) * 100) : 0}
      {@const sk = reindexProgress.permanently_skipped ?? 0}
      {@const verb = reindexRunIsResume ? 'Resuming' : 'Re-indexing'}
      <p class="reindex-msg">
        {verb} notes with {embeddingModel}… {reindexProgress.processed}/{reindexProgress.total} ({pct}%){sk > 0 ? ` · ${sk} skipped after retries` : ''}
      </p>
      {#if reindexProgress.embeddingChunks}
        <p class="reindex-msg">
          Embedding “{reindexProgress.embeddingChunks.note_title}”: {reindexProgress.embeddingChunks.done}/{reindexProgress.embeddingChunks.total} chunks
        </p>
      {/if}
      <div class="reindex-bar-track"><div class="reindex-bar-fill" style="width: {pct}%"></div></div>
    {:else if reindexStatus === 'done'}
      <p class="reindex-msg reindex-done">
        {reindexSummaryText || 'Notes re-indexed.'}{' '}
        Wikipedia and file scanner indexes were cleared — re-index them from their respective settings panels.
      </p>
    {:else if reindexStatus === 'error'}
      <p class="reindex-msg reindex-error">Re-index failed: {reindexError}</p>
      <button class="settings-action-btn reindex-btn" onclick={clearAndReindex}>Retry</button>
    {/if}
  </div>
{/if}

<h4 class="settings-subsection">Reliability</h4>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Background retries</span>
    <span class="setting-desc">
      Retry transient failures (embedding / vector store) up to this many times before skipping, for Wikipedia indexing,
      file scanner, and note re-index. 0 means no retry.
    </span>
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

<h4 class="settings-subsection">Inference parameters</h4>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Temperature</span>
    <span class="setting-desc">Controls randomness. Higher values produce more creative, less predictable responses.</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatTemperature} min="0" max="2" step="0.05" onchange={() => save('chat_temperature', chatTemperature)} />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Verbosity</span>
    <span class="setting-desc">Controls how detailed the model's responses are.</span>
  </div>
  <select bind:value={verbosity} onchange={() => save('chat_verbosity', verbosity)}>
    <option value="concise">Concise (default)</option>
    <option value="thorough">Thorough</option>
    <option value="caveman">Caveman</option>
  </select>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Top P</span>
    <span class="setting-desc">Nucleus sampling threshold. Lower values make output more focused.</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatTopP} min="0" max="1" step="0.05" onchange={() => save('chat_top_p', chatTopP)} />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Top K</span>
    <span class="setting-desc">Limits the next token selection to the K most likely candidates. 0 disables it.</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatTopK} min="0" max="200" step="1" onchange={() => save('chat_top_k', chatTopK)} />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Repeat penalty</span>
    <span class="setting-desc">Penalises tokens that have already appeared. Higher values reduce repetition.</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatRepeatPenalty} min="0.5" max="2" step="0.05" onchange={() => save('chat_repeat_penalty', chatRepeatPenalty)} />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Context window</span>
    <span class="setting-desc">Maximum tokens the model can see at once. Higher values use more RAM.</span>
  </div>
  <input type="number" class="setting-num" bind:value={chatNumCtx} min="512" max="131072" step="512" onchange={() => save('chat_num_ctx', chatNumCtx)} />
</div>
