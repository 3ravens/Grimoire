<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let {
    devNativeContextMenu = false,
    onDevNativeContextMenuChange = () => {},
  } = $props();

  let wikiPerfLogging = $state(false);

  onMount(async () => {
    const raw = await invoke('get_setting', { key: 'wiki_perf_logging' }).catch(() => '');
    wikiPerfLogging = raw === 'true';
  });

  function saveWikiPerfLogging(enabled) {
    wikiPerfLogging = enabled;
    invoke('set_setting', { key: 'wiki_perf_logging', value: enabled ? 'true' : 'false' }).catch(() => {});
  }

  // ── ZIM parsing PoC ───────────────────────────────────────────────────────
  let zimPath    = $state('');
  let zimStatus  = $state('idle'); // idle | running | done | error
  let zimResult  = $state(null);
  let zimError   = $state('');

  async function runZimPoC() {
    if (!zimPath.trim()) return;
    zimStatus = 'running';
    zimResult = null;
    zimError  = '';
    try {
      const result = await invoke('test_zim_parse', { zimPath: zimPath.trim() });
      zimResult = result;
      zimStatus = 'done';
    } catch (e) {
      zimError  = e?.message ?? String(e);
      zimStatus = 'error';
    }
  }
</script>

<h3>Developer</h3>
<p class="settings-notice">These settings are only visible in dev builds.</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Use native context menu</span>
    <span class="setting-desc">
      Disables the custom context menu and restores the native WebView2 menu,
      which includes Inspect Element. Useful for debugging layout and styles.
    </span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={devNativeContextMenu}
      onchange={(e) => onDevNativeContextMenuChange(e.currentTarget.checked)}
    />
    <span class="toggle-label">{devNativeContextMenu ? 'On' : 'Off'}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Wikipedia perf logs</span>
    <span class="setting-desc">
      Emits periodic <code>[wiki_index_perf]</code> timing summaries while indexing.
      Disable to keep logs clean. Output appears in the dev terminal and is also
      written to a dedicated <code>wiki-index-perf.log</code> file.
    </span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={wikiPerfLogging}
      onchange={(e) => saveWikiPerfLogging(e.currentTarget.checked)}
    />
    <span class="toggle-label">{wikiPerfLogging ? 'On' : 'Off'}</span>
  </label>
</div>

<!-- ── Phase 0: ZIM parsing PoC ─────────────────────────────────────────── -->
<h4 class="section-subhead">Wikipedia — ZIM parsing PoC</h4>
<p class="settings-notice">
  Paste the absolute path to a Kiwix .zim file, then click Test. The command
  reads up to 500 articles and returns counts + 5 content previews so you can
  judge whether the <code>zim</code> crate is usable for this bundle.
</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">ZIM file path</span>
    <span class="setting-desc">Absolute path on disk, e.g. C:\Downloads\wikipedia_en_mathematics_maxi.zim</span>
  </div>
  <input
    class="text-input"
    type="text"
    placeholder="C:\path\to\bundle.zim"
    bind:value={zimPath}
  />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Run PoC</span>
    <span class="setting-desc">Opens the ZIM, iterates up to 500 articles, reports counts and sample content.</span>
  </div>
  <button
    class="btn"
    onclick={runZimPoC}
    disabled={zimStatus === 'running' || !zimPath.trim()}
  >
    {zimStatus === 'running' ? 'Parsing…' : 'Test ZIM'}
  </button>
</div>

{#if zimStatus === 'error'}
  <p class="settings-notice error-text">{zimError}</p>
{/if}

{#if zimStatus === 'done' && zimResult}
  <div class="zim-result">
    <div class="zim-stats">
      <span>Total entries: <strong>{zimResult.total_entries}</strong></span>
      <span>Articles: <strong>{zimResult.article_count}</strong></span>
      <span>Redirects: <strong>{zimResult.redirect_count}</strong></span>
      <span>Other namespaces: <strong>{zimResult.other_namespace}</strong></span>
      <span>Compression: <strong>{zimResult.compression}</strong></span>
    </div>
    {#each zimResult.samples as sample, i}
      <div class="zim-sample">
        <p class="zim-sample-title">#{i + 1} — {sample.title} <span class="zim-url">({sample.url})</span></p>
        <pre class="zim-preview">{sample.content_preview}</pre>
      </div>
    {/each}
  </div>
{/if}

<style>
  .section-subhead {
    margin: 1.5rem 0 0.25rem;
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .text-input {
    flex: 1;
    min-width: 0;
    padding: 0.3rem 0.5rem;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    font-family: var(--mono);
    font-size: 0.8rem;
  }

  .error-text {
    color: var(--danger);
  }

  .zim-result {
    margin-top: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .zim-stats {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    font-size: 0.85rem;
    padding: 0.5rem 0.75rem;
    background: var(--bg-hover);
    border-radius: 4px;
  }

  .zim-sample {
    padding: 0.5rem 0.75rem;
    background: var(--bg-hover);
    border-radius: 4px;
    border-left: 2px solid var(--accent);
  }

  .zim-sample-title {
    margin: 0 0 0.25rem;
    font-size: 0.85rem;
    font-weight: 600;
  }

  .zim-url {
    font-weight: 400;
    color: var(--text-muted);
    font-family: var(--mono);
    font-size: 0.75rem;
  }

  .zim-preview {
    margin: 0;
    font-size: 0.75rem;
    font-family: var(--mono);
    white-space: pre-wrap;
    word-break: break-all;
    color: var(--text-muted);
    max-height: 8rem;
    overflow-y: auto;
  }
</style>
