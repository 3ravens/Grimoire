<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';

  let {
    keepInMemory = false, onKeepInMemoryChange = () => {},
  } = $props();

  let chatModel       = $state('llama3.2');
  let embeddingModel  = $state('nomic-embed-text');
  let initialEmbeddingModel = $state('nomic-embed-text'); // model the current index was built with
  let reindexStatus   = $state(''); // '', 'clearing', 'reindexing', 'done', 'error'
  let reindexError    = $state('');
  let reindexProgress = $state({ indexed: 0, processed: 0, total: 0 });
  let chatTemperature = $state(0.8);
  let chatTopP        = $state(0.9);
  let chatTopK        = $state(40);
  let chatRepeatPenalty = $state(1.1);
  let chatNumCtx      = $state(8192);
  let verbosity       = $state('concise');

  onMount(async () => {
    const [model, embed, temp, top_p, top_k, repeat, ctx, verb] = await Promise.all([
      invoke('get_setting', { key: 'chat_model' }),
      invoke('get_setting', { key: 'embedding_model' }),
      invoke('get_setting', { key: 'chat_temperature' }),
      invoke('get_setting', { key: 'chat_top_p' }),
      invoke('get_setting', { key: 'chat_top_k' }),
      invoke('get_setting', { key: 'chat_repeat_penalty' }),
      invoke('get_setting', { key: 'chat_num_ctx' }),
      invoke('get_setting', { key: 'chat_verbosity' }),
    ]);

    if (model)  chatModel       = model;
    if (embed)  { embeddingModel = embed; initialEmbeddingModel = embed; }
    if (temp)   chatTemperature = parseFloat(temp);
    if (top_p)  chatTopP        = parseFloat(top_p);
    if (top_k)  chatTopK        = parseInt(top_k, 10);
    if (repeat) chatRepeatPenalty = parseFloat(repeat);
    if (ctx)    chatNumCtx      = parseInt(ctx, 10);
    if (verb)   verbosity       = verb;
  });

  function save(key, value) {
    invoke('set_setting', { key, value: String(value) }).catch(() => {});
  }

  async function clearAndReindex() {
    reindexStatus = 'clearing';
    reindexError = '';
    reindexProgress = { indexed: 0, processed: 0, total: 0 };
    let unlisten = null;
    try {
      await Promise.all([
        invoke('clear_notes_index'),
        invoke('clear_wiki_index'),
        invoke('clear_scanned_index'),
      ]);
      reindexStatus = 'reindexing';
      unlisten = await listen('reindex:progress', (ev) => {
        reindexProgress = ev.payload;
      });
      await invoke('reindex_all');
      initialEmbeddingModel = embeddingModel;
      reindexStatus = 'done';
    } catch (e) {
      reindexError = String(e);
      reindexStatus = 'error';
    } finally {
      unlisten?.();
    }
  }
</script>

<h3>LLM</h3>
<p class="settings-notice">
  Model changes take effect on the next chat. Models are installed and managed through Ollama.
</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Chat model</span>
    <span class="setting-desc">The model used for chat responses and note improvements.</span>
  </div>
  <select bind:value={chatModel} onchange={() => save('chat_model', chatModel)}>
    <option value="llama3.2">llama3.2 · general (default)</option>
    <option value="phi3">phi3 · lightweight</option>
    <option value="gemma2:2b">gemma2:2b · lightweight</option>
    <option value="mistral">mistral · general</option>
    <option value="codellama">codellama · programming</option>
    <option value="llama3:70b">llama3:70b · high quality (GPU)</option>
  </select>
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
    <option value="nomic-embed-text">nomic-embed-text (default, ~270 MB)</option>
    <option value="mxbai-embed-large">mxbai-embed-large · higher quality</option>
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
      <p class="reindex-msg">Re-indexing notes with {embeddingModel}… {reindexProgress.processed}/{reindexProgress.total} ({pct}%)</p>
      <div class="reindex-bar-track"><div class="reindex-bar-fill" style="width: {pct}%"></div></div>
    {:else if reindexStatus === 'done'}
      <p class="reindex-msg reindex-done">Notes re-indexed. Wikipedia and file scanner indexes were cleared — re-index them from their respective settings panels.</p>
    {:else if reindexStatus === 'error'}
      <p class="reindex-msg reindex-error">Re-index failed: {reindexError}</p>
      <button class="settings-action-btn reindex-btn" onclick={clearAndReindex}>Retry</button>
    {/if}
  </div>
{/if}

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
