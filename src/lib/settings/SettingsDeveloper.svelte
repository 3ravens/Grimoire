<script>
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
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

  async function browseZimPath() {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      filters: [{ name: 'Kiwix / ZIM', extensions: ['zim'] }],
      title: 'Select a Wikipedia .zim file',
    }).catch(() => null);
    if (selected === null || selected === undefined) return;
    zimPath = Array.isArray(selected) ? selected[0] : selected;
    zimError = '';
    benchError = '';
  }

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

  // ── Wikipedia indexing benchmark (read + parse + embed, no DB writes) ─────
  let benchMaxEntries = $state('');
  let benchStatus = $state('idle'); // idle | running | done | error
  let benchResult = $state(null);
  let benchError = $state('');
  let benchCopyHint = $state('');

  async function runWikiIndexBenchmark() {
    const path = zimPath.trim();
    if (!path) {
      benchError =
        'No ZIM file selected. Click Browse… next to the path field (above), or paste the full path to your .zim file.';
      benchStatus = 'error';
      benchResult = null;
      return;
    }
    benchStatus = 'running';
    benchResult = null;
    benchError = '';
    benchCopyHint = '';
    try {
      const trimmed = benchMaxEntries.trim();
      let maxEntries = undefined;
      if (trimmed !== '') {
        const n = Number.parseInt(trimmed, 10);
        if (!Number.isFinite(n) || n < 1) {
          benchError = 'Max entries must be a positive integer.';
          benchStatus = 'error';
          return;
        }
        maxEntries = n;
      }
      const payload = { zimPath: path };
      if (maxEntries !== undefined) payload.maxEntries = maxEntries;
      const result = await invoke('benchmark_wikipedia_indexing', payload);
      benchResult = result;
      benchStatus = 'done';
    } catch (e) {
      benchError = e?.message ?? String(e);
      benchStatus = 'error';
    }
  }

  async function copyBenchJson() {
    if (!benchResult) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(benchResult, null, 2));
      benchCopyHint = 'Copied to clipboard.';
      setTimeout(() => { benchCopyHint = ''; }, 2500);
    } catch {
      benchCopyHint = 'Copy failed.';
      setTimeout(() => { benchCopyHint = ''; }, 2500);
    }
  }

  function benchNum(v) {
    if (v === null || v === undefined) return '—';
    const n = typeof v === 'bigint' ? Number(v) : Number(v);
    return Number.isFinite(n) ? n.toLocaleString(undefined, { maximumFractionDigits: 2 }) : String(v);
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

<div class="setting-row zim-path-row">
  <div class="setting-label">
    <span class="setting-name">ZIM file path</span>
    <span class="setting-desc">
      Absolute path on disk. Use Browse… to choose a bundle — otherwise the indexing benchmark stays idle until this is filled.
    </span>
  </div>
  <div class="zim-path-controls">
    <input
      class="text-input"
      type="text"
      placeholder="C:\path\to\bundle.zim"
      bind:value={zimPath}
    />
    <button type="button" class="btn btn-outline" onclick={browseZimPath}>Browse…</button>
  </div>
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

<!-- ── Wikipedia indexing performance benchmark ─────────────────────────── -->
<h4 class="section-subhead">Wikipedia — indexing benchmark</h4>
<p class="settings-notice">
  Uses the same <strong>ZIM file path</strong> as above (set it with <strong>Browse…</strong> or paste).
  Runs read → parse → embed (Ollama) for a bounded prefix of the archive — does not write SQLite or LanceDB.
  Save JSON and compare with <code>scripts/compare_wiki_benchmark.py</code>.
</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Max ZIM entries</span>
    <span class="setting-desc">
      Cap how many sequential ZIM entry indices to walk (default 20,000 if empty). Lower = faster smoke test.
    </span>
  </div>
  <input
    class="text-input bench-max-input"
    type="text"
    inputmode="numeric"
    placeholder="20000"
    bind:value={benchMaxEntries}
    disabled={benchStatus === 'running'}
  />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Run benchmark</span>
    <span class="setting-desc">
      Ensure Ollama is running and your embedding model matches Settings → LLM.
    </span>
  </div>
  <button
    class="btn"
    onclick={runWikiIndexBenchmark}
    disabled={benchStatus === 'running'}
  >
    {benchStatus === 'running' ? 'Benchmarking…' : 'Run indexing benchmark'}
  </button>
</div>

{#if benchStatus === 'error'}
  <p class="settings-notice error-text">{benchError}</p>
{/if}

{#if benchCopyHint}
  <p class="settings-notice bench-copy-hint">{benchCopyHint}</p>
{/if}

{#if benchStatus === 'done' && benchResult}
  <div class="zim-result bench-result-block">
    <div class="zim-stats bench-stats-grid">
      <span>Model: <strong>{benchResult.model}</strong></span>
      <span>Total in ZIM: <strong>{benchNum(benchResult.total_entries_in_zim)}</strong></span>
      <span>Benchmark window: <strong>{benchNum(benchResult.benchmark_entries)}</strong> entries</span>
      <span>Scanned: <strong>{benchNum(benchResult.scanned_entries)}</strong></span>
      <span>Accepted articles: <strong>{benchNum(benchResult.accepted_articles)}</strong></span>
      <span>Embedded: <strong>{benchNum(benchResult.embedded_articles)}</strong></span>
      <span>Windows: <strong>{benchNum(benchResult.windows)}</strong></span>
      <span>Total time: <strong>{benchNum(benchResult.total_ms)}</strong> ms</span>
      <span>Read: <strong>{benchNum(benchResult.read_ms)}</strong> ms</span>
      <span>Parse: <strong>{benchNum(benchResult.parse_ms)}</strong> ms</span>
      <span>Embed: <strong>{benchNum(benchResult.embed_ms)}</strong> ms</span>
      <span>Entries/s: <strong>{benchNum(benchResult.entries_per_sec)}</strong></span>
      <span>Accepted/s: <strong>{benchNum(benchResult.accepted_per_sec)}</strong></span>
      <span>Embedded/s: <strong>{benchNum(benchResult.embedded_per_sec)}</strong></span>
    </div>
    <div class="bench-actions">
      <button type="button" class="btn btn-secondary" onclick={copyBenchJson}>Copy result JSON</button>
    </div>
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

  .zim-path-controls {
    display: flex;
    flex: 1;
    min-width: 0;
    gap: 0.5rem;
    align-items: center;
  }

  .zim-path-controls .text-input {
    flex: 1;
    min-width: 0;
  }

  .btn-outline {
    flex-shrink: 0;
    padding: 0.35rem 0.65rem;
    font-size: 0.8rem;
    white-space: nowrap;
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

  .bench-max-input {
    max-width: 8rem;
  }

  .bench-result-block {
    margin-top: 0.5rem;
  }

  .bench-stats-grid {
    flex-direction: column;
    align-items: flex-start;
    gap: 0.35rem;
  }

  .bench-actions {
    margin-top: 0.75rem;
  }

  .btn-secondary {
    font-size: 0.85rem;
  }

  .bench-copy-hint {
    margin-top: 0.35rem;
    color: var(--text-muted);
  }
</style>
