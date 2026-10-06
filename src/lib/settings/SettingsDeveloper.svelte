<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { t, tParts } from '../i18n/t.js';

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
      filters: [{ name: t('settings.developer.zimDialogFilter'), extensions: ['zim'] }],
      title: t('settings.developer.zimDialogTitle'),
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
      benchError = t('settings.developer.noZimSelected');
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
          benchError = t('settings.developer.maxEntriesInvalid');
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
      benchCopyHint = t('settings.developer.copied');
      setTimeout(() => { benchCopyHint = ''; }, 2500);
    } catch {
      benchCopyHint = t('settings.developer.copyFailed');
      setTimeout(() => { benchCopyHint = ''; }, 2500);
    }
  }

  function benchNum(v) {
    if (v === null || v === undefined) return t('settings.shared.emDash');
    const n = typeof v === 'bigint' ? Number(v) : Number(v);
    return Number.isFinite(n) ? n.toLocaleString(undefined, { maximumFractionDigits: 2 }) : String(v);
  }
  // ── Test Data Generator ──────────────────────────────────────────────
  let tdNoteCount = $state('120');
  let tdFolderCount = $state('8');
  let tdSeed = $state('');
  let tdDailyNotes = $state(true);
  let tdEmbed = $state(false);
  let tdRunning = $state(false);
  let tdCleanRunning = $state(false);
  let tdCleanError = $state('');
  const tdLocked = $derived(tdRunning || tdCleanRunning);
  let tdError = $state('');
  let tdSummary = $state(null);
  /** @type {{ phase: string, message: string, current?: number, total?: number } | null} */
  let tdProgress = $state(null);

  async function runTestDataGenerator() {
    tdRunning = true;
    tdError = '';
    tdSummary = null;
    tdProgress = null;
    /** @type {(() => void) | null} */
    let unlistenProgress = null;
    try {
      unlistenProgress = await listen('test_data:progress', (ev) => {
        tdProgress = /** @type {any} */ (ev.payload);
      });
      const nc = Number.parseInt(tdNoteCount.trim()) || 120;
      const fc = Number.parseInt(tdFolderCount.trim()) || 8;
      const seed = tdSeed.trim() ? Number.parseInt(tdSeed.trim()) : null;
      const summary = await invoke('generate_test_data', {
        noteCount: nc,
        folderCount: fc,
        seed: seed,
        includeDailyNotes: tdDailyNotes,
        embed: tdEmbed,
      });
      tdSummary = summary;
      window.dispatchEvent(new CustomEvent('grimoire:vault-data-changed'));
    } catch (e) {
      tdError = e?.message ?? String(e);
    } finally {
      if (unlistenProgress) unlistenProgress();
      tdProgress = null;
      tdRunning = false;
    }
  }

  async function runCleanDatabase() {
    tdCleanError = '';
    const ok = confirm(t('settings.developer.cleanConfirm'));
    if (!ok) return;

    tdCleanRunning = true;
    try {
      await invoke('clean_developer_database');
      window.location.reload();
    } catch (e) {
      tdCleanError = e?.message ?? String(e);
    } finally {
      tdCleanRunning = false;
    }
  }

</script>

<h3>{t('settings.developer.title')}</h3>
<p class="settings-notice">{t('settings.developer.devOnlyNotice')}</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.nativeContextMenu')}</span>
    <span class="setting-desc">{t('settings.developer.nativeContextMenuDesc')}</span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={devNativeContextMenu}
      onchange={(e) => onDevNativeContextMenuChange(e.currentTarget.checked)}
    />
    <span class="toggle-label">{devNativeContextMenu ? t('common.on') : t('common.off')}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.wikiPerfLogs')}</span>
    <span class="setting-desc">{t('settings.developer.wikiPerfLogsDesc')}</span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={wikiPerfLogging}
      onchange={(e) => saveWikiPerfLogging(e.currentTarget.checked)}
    />
    <span class="toggle-label">{wikiPerfLogging ? t('common.on') : t('common.off')}</span>
  </label>
</div>

<!-- ── Phase 0: ZIM parsing PoC ─────────────────────────────────────────── -->
<h4 class="section-subhead">{t('settings.developer.zimPocTitle')}</h4>
<p class="settings-notice">{t('settings.developer.zimPocNotice')}</p>

<div class="setting-row zim-path-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.zimPath')}</span>
    <span class="setting-desc">{t('settings.developer.zimPathDesc')}</span>
  </div>
  <div class="zim-path-controls">
    <input
      class="text-input"
      type="text"
      placeholder={t('settings.developer.zimPlaceholder')}
      bind:value={zimPath}
    />
    <button type="button" class="btn btn-outline" onclick={browseZimPath}>{t('common.browse')}</button>
  </div>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.runPoc')}</span>
    <span class="setting-desc">{t('settings.developer.runPocDesc')}</span>
  </div>
  <button
    class="btn"
    onclick={runZimPoC}
    disabled={zimStatus === 'running' || !zimPath.trim()}
  >
    {zimStatus === 'running' ? t('settings.developer.parsing') : t('settings.developer.testZim')}
  </button>
</div>

{#if zimStatus === 'error'}
  <p class="settings-notice error-text">{zimError}</p>
{/if}

{#if zimStatus === 'done' && zimResult}
  <div class="zim-result">
    <div class="zim-stats">
      <span>{t('settings.developer.totalEntries')} <strong>{zimResult.total_entries}</strong></span>
      <span>{t('settings.developer.articles')} <strong>{zimResult.article_count}</strong></span>
      <span>{t('settings.developer.redirects')} <strong>{zimResult.redirect_count}</strong></span>
      <span>{t('settings.developer.otherNamespaces')} <strong>{zimResult.other_namespace}</strong></span>
      <span>{t('settings.developer.compression')} <strong>{zimResult.compression}</strong></span>
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
<h4 class="section-subhead">{t('settings.developer.benchTitle')}</h4>
<p class="settings-notice">
  {#each tParts('settings.developer.benchNotice') as part}
    {#if part.type === 'text'}{part.value}{:else if part.name === 'zimPath'}<strong>{t('settings.developer.zimPath')}</strong>{:else if part.name === 'browse'}<strong>{t('common.browse')}</strong>{/if}
  {/each}
</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.maxZimEntries')}</span>
    <span class="setting-desc">{t('settings.developer.maxZimEntriesDesc')}</span>
  </div>
  <input
    class="text-input bench-max-input"
    type="text"
    inputmode="numeric"
    placeholder={t('settings.developer.maxEntriesPlaceholder')}
    bind:value={benchMaxEntries}
    disabled={benchStatus === 'running'}
  />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.runBenchmark')}</span>
    <span class="setting-desc">{t('settings.developer.runBenchmarkDesc')}</span>
  </div>
  <button
    class="btn"
    onclick={runWikiIndexBenchmark}
    disabled={benchStatus === 'running'}
  >
    {benchStatus === 'running' ? t('settings.developer.benchmarking') : t('settings.developer.runIndexingBenchmark')}
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
      <span>{t('settings.developer.benchModel')} <strong>{benchResult.model}</strong></span>
      <span>{t('settings.developer.benchTotalInZim')} <strong>{benchNum(benchResult.total_entries_in_zim)}</strong></span>
      <span>{t('settings.developer.benchWindow')} <strong>{benchNum(benchResult.benchmark_entries)}</strong>{t('settings.developer.entriesSuffix')}</span>
      <span>{t('settings.developer.benchScanned')} <strong>{benchNum(benchResult.scanned_entries)}</strong></span>
      <span>{t('settings.developer.benchAccepted')} <strong>{benchNum(benchResult.accepted_articles)}</strong></span>
      <span>{t('settings.developer.benchEmbedded')} <strong>{benchNum(benchResult.embedded_articles)}</strong></span>
      <span>{t('settings.developer.benchWindows')} <strong>{benchNum(benchResult.windows)}</strong></span>
      <span>{t('settings.developer.benchTotalTime')} <strong>{benchNum(benchResult.total_ms)}</strong>{t('settings.developer.msSuffix')}</span>
      <span>{t('settings.developer.benchRead')} <strong>{benchNum(benchResult.read_ms)}</strong>{t('settings.developer.msSuffix')}</span>
      <span>{t('settings.developer.benchParse')} <strong>{benchNum(benchResult.parse_ms)}</strong>{t('settings.developer.msSuffix')}</span>
      <span>{t('settings.developer.benchEmbed')} <strong>{benchNum(benchResult.embed_ms)}</strong>{t('settings.developer.msSuffix')}</span>
      <span>{t('settings.developer.benchEntriesPerSec')} <strong>{benchNum(benchResult.entries_per_sec)}</strong></span>
      <span>{t('settings.developer.benchAcceptedPerSec')} <strong>{benchNum(benchResult.accepted_per_sec)}</strong></span>
      <span>{t('settings.developer.benchEmbeddedPerSec')} <strong>{benchNum(benchResult.embedded_per_sec)}</strong></span>
    </div>
    <div class="bench-actions">
      <button type="button" class="btn btn-secondary" onclick={copyBenchJson}>{t('settings.developer.copyResultJson')}</button>
    </div>
  </div>
{/if}


<!-- ── Test Data Generator ───────────────────────────────────── -->
<h4 class="section-subhead">{t('settings.developer.testDataTitle')}</h4>
<p class="settings-notice">{t('settings.developer.testDataNotice')}</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.noteCount')}</span>
    <span class="setting-desc">{t('settings.developer.noteCountDesc')}</span>
  </div>
  <input
    class="text-input bench-max-input"
    type="text"
    inputmode="numeric"
    placeholder="120"
    bind:value={tdNoteCount}
    disabled={tdLocked}
  />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.folderCount')}</span>
    <span class="setting-desc">{t('settings.developer.folderCountDesc')}</span>
  </div>
  <input
    class="text-input bench-max-input"
    type="text"
    inputmode="numeric"
    placeholder="8"
    bind:value={tdFolderCount}
    disabled={tdLocked}
  />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.randomSeed')}</span>
    <span class="setting-desc">{t('settings.developer.randomSeedDesc')}</span>
  </div>
  <input
    class="text-input bench-max-input"
    type="text"
    inputmode="numeric"
    placeholder={t('settings.developer.seedPlaceholder')}
    bind:value={tdSeed}
    disabled={tdLocked}
  />
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.includeDailyNotes')}</span>
    <span class="setting-desc">{t('settings.developer.includeDailyNotesDesc')}</span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={tdDailyNotes}
      onchange={(e) => (tdDailyNotes = e.currentTarget.checked)}
      disabled={tdLocked}
    />
    <span class="toggle-label">{tdDailyNotes ? t('common.yes') : t('common.no')}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.generateEmbeddings')}</span>
    <span class="setting-desc">{t('settings.developer.generateEmbeddingsDesc')}</span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={tdEmbed}
      onchange={(e) => (tdEmbed = e.currentTarget.checked)}
      disabled={tdLocked}
    />
    <span class="toggle-label">{tdEmbed ? t('common.yes') : t('common.no')}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">{t('settings.developer.vaultTools')}</span>
    <span class="setting-desc">{t('settings.developer.vaultToolsDesc')}</span>
  </div>
  <div class="td-btn-row">
    <button
      type="button"
      class="btn"
      onclick={runTestDataGenerator}
      disabled={tdLocked}
    >
      {tdRunning ? t('settings.developer.generating') : t('settings.developer.generateTestData')}
    </button>
    <button
      type="button"
      class="btn btn-secondary"
      onclick={runCleanDatabase}
      disabled={tdLocked}
    >
      {tdCleanRunning ? t('settings.developer.cleaning') : t('settings.developer.cleanDatabase')}
    </button>
  </div>
</div>

{#if tdCleanError}
  <p class="settings-notice error-text">{tdCleanError}</p>
{/if}

{#if tdProgress}
  <div class="td-progress" role="status" aria-live="polite" aria-busy="true">
    <p class="td-progress-label">{tdProgress.message}</p>
    {#if tdProgress.total != null && tdProgress.total > 0 && tdProgress.current != null}
      {@const pct = Math.min(100, Math.round((tdProgress.current / tdProgress.total) * 100))}
      <div class="td-progress-bar-wrap" aria-hidden="true">
        <div class="td-progress-bar" style="width: {pct}%"></div>
      </div>
      <p class="td-progress-meta">{tdProgress.current} / {tdProgress.total} ({pct}%)</p>
    {:else}
      <p class="td-progress-meta phase-tag">{tdProgress.phase}</p>
    {/if}
  </div>
{/if}

{#if tdError}
  <p class="settings-notice error-text">{tdError}</p>
{/if}

{#if tdSummary}
  <div class="zim-result bench-result-block">
    <div class="zim-stats bench-stats-grid">
      <span>{t('settings.developer.summaryNotes')} <strong>{tdSummary.notes}</strong></span>
      <span>{t('settings.developer.summaryFolders')} <strong>{tdSummary.folders}</strong></span>
      <span>{t('settings.developer.summaryTemplates')} <strong>{tdSummary.templates}</strong></span>
      <span>{t('settings.developer.summaryTags')} <strong>{tdSummary.tags}</strong></span>
      <span>{t('settings.developer.summaryLinks')} <strong>{tdSummary.links}</strong></span>
      <span>{t('settings.developer.summaryDailyNotes')} <strong>{tdSummary.daily_notes}</strong></span>
      <span>{t('settings.developer.summaryEmbedded')} <strong>{tdSummary.embedded}</strong></span>
    </div>
    {#if tdSummary.errors && tdSummary.errors.length > 0}
      <div class="zim-sample">
        <p class="zim-sample-title">{t('settings.developer.errorsTitle', { count: tdSummary.errors.length })}</p>
        <pre class="zim-preview">{tdSummary.errors.join('\n')}</pre>
      </div>
    {/if}
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

  .td-btn-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
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

  .td-progress {
    margin-top: 0.65rem;
    padding: 0.5rem 0.75rem;
    background: var(--bg-hover);
    border-radius: 4px;
    border-left: 2px solid var(--accent);
  }

  .td-progress-label {
    margin: 0 0 0.4rem;
    font-size: 0.88rem;
    line-height: 1.35;
  }

  .td-progress-bar-wrap {
    height: 6px;
    background: var(--bg-input);
    border-radius: 3px;
    overflow: hidden;
  }

  .td-progress-bar {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s ease;
  }

  .td-progress-meta {
    margin: 0.35rem 0 0;
    font-size: 0.75rem;
    color: var(--text-muted);
    font-family: var(--mono);
  }

  .td-progress-meta.phase-tag {
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
