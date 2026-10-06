<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { onMount, onDestroy } from 'svelte';
  import { t, tp, tParts } from '../i18n/t.js';

  // ── State ────────────────────────────────────────────────────────────────

  // ScannedPath shape: { id, path, kind, added_at, last_scanned_at, enabled, file_count, error_msg, exclude_patterns }
  let paths = $state([]);

  /** Global newline-separated globs (settings key `file_scanner_global_excludes`). */
  let globalExcludes = $state('');
  let showGlobalExcludes = $state(false);

  /** `path_id` → { root_missing, missing_files } */
  let staleById = $state({});

  /** Which path row has the per-path excludes editor open (id or null). */
  let showExcludesFor = $state(null);

  /** Draft text while editing excludes per path (keyed by id). */
  let excludeDraft = $state({});

  // progress[path_id] — scanning state including chunk-level embedding progress (large files, CSV).
  let progress = $state({});

  /** First meaningful embedding progress per path — used for ETA (same idea as Wikipedia indexing). */
  let embedStarts = $state({});

  // id of the path currently being imported as a note (null if none)
  let importingNoteId = $state(null);

  // lastImportedNoteId[path_id] = note id — set after a successful import so we can show "View note"
  let lastImportedNoteId = $state({});

  // Global bulk re-index status for all scanned paths.
  let rescanningAll = $state(false);
  let rescanAllStatus = $state('');

  let unlisten = null;

  // ── Lifecycle ────────────────────────────────────────────────────────────

  onMount(async () => {
    await loadPaths();
    await loadGlobalExcludes();
    await loadStaleSummary();

    unlisten = await listen('filescanner:progress', (ev) => {
      const payload = ev.payload;
      const path_id = payload.path_id;
      const prev = progress[path_id] ?? {};

      const next = {
        ...prev,
        scanned: payload.scanned ?? 0,
        skipped: payload.skipped ?? 0,
        total: payload.total ?? 0,
        visited:
          payload.visited !== undefined && payload.visited !== null ? payload.visited : prev.visited ?? 0,
        done: !!payload.done,
        error: payload.error ?? null,
      };

      if (payload.phase !== undefined && payload.phase !== null) next.phase = payload.phase;
      if (payload.chunks_embedded !== undefined && payload.chunks_embedded !== null) {
        next.chunks_embedded = payload.chunks_embedded;
      }
      if (payload.chunks_total !== undefined && payload.chunks_total !== null) {
        next.chunks_total = payload.chunks_total;
      }
      if (payload.current_file !== undefined) next.current_file = payload.current_file;
      if (payload.permanently_skipped !== undefined && payload.permanently_skipped !== null) {
        next.permanently_skipped = payload.permanently_skipped;
      }
      if (payload.permanently_skipped_chunks !== undefined && payload.permanently_skipped_chunks !== null) {
        next.permanently_skipped_chunks = payload.permanently_skipped_chunks;
      }

      const ct = payload.chunks_total ?? next.chunks_total ?? 0;
      if (ct === 0) {
        const es = { ...embedStarts };
        delete es[path_id];
        embedStarts = es;
      }

      if (
        ct > 0 &&
        (payload.chunks_embedded ?? 0) > 0 &&
        !embedStarts[path_id]
      ) {
        embedStarts = {
          ...embedStarts,
          [path_id]: { time: Date.now(), at: payload.chunks_embedded ?? 0 },
        };
      }

      progress = { ...progress, [path_id]: next };

      if (payload.done) {
        const es = { ...embedStarts };
        delete es[path_id];
        embedStarts = es;
        loadPaths();
        loadStaleSummary();
      }
    });
  });

  onDestroy(() => {
    unlisten?.();
  });

  // ── Helpers ───────────────────────────────────────────────────────────────

  async function loadPaths() {
    paths = await invoke('get_scanned_paths').catch(() => []);
  }

  async function loadGlobalExcludes() {
    globalExcludes =
      (await invoke('get_setting', { key: 'file_scanner_global_excludes' }).catch(() => '')) || '';
  }

  async function saveGlobalExcludes() {
    try {
      await invoke('set_setting', { key: 'file_scanner_global_excludes', value: globalExcludes });
    } catch (e) {
      alert(t('settings.fileScanner.saveGlobalExcludesFailed', { msg: e?.message ?? e }));
    }
  }

  async function loadStaleSummary() {
    const rows = await invoke('get_scanned_path_stale_summary').catch(() => []);
    const m = {};
    for (const r of rows) {
      m[r.path_id] = { root_missing: !!r.root_missing, missing_files: r.missing_files ?? 0 };
    }
    staleById = m;
  }

  function toggleExcludesEditor(p) {
    if (showExcludesFor === p.id) {
      showExcludesFor = null;
    } else {
      excludeDraft = { ...excludeDraft, [p.id]: p.exclude_patterns ?? '' };
      showExcludesFor = p.id;
    }
  }

  async function savePathExcludes(id) {
    const patterns = excludeDraft[id] ?? '';
    try {
      await invoke('update_scanned_path_excludes', { id, patterns });
      showExcludesFor = null;
      await loadPaths();
    } catch (e) {
      alert(t('settings.fileScanner.saveExcludesFailed', { msg: e?.message ?? e }));
    }
  }

  async function clearStaleFiles(id) {
    try {
      const n = await invoke('clear_stale_scanned_files', { id });
      await loadPaths();
      await loadStaleSummary();
      if (n > 0) {
        alert(tp('settings.fileScanner.cleanupRemoved', n, { count: n }));
      }
    } catch (e) {
      alert(t('settings.fileScanner.cleanupFailed', { msg: e?.message ?? e }));
    }
  }

  async function addFile() {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      filters: [
        {
          name: t('settings.fileScanner.dialogSupportedFiles'),
          extensions: [
            'txt',
            'md',
            'pdf',
            'csv',
            'html',
            'htm',
            'docx',
            'odt',
            'log',
          ],
        },
      ],
    });
    if (!selected) return;
    const filePath = Array.isArray(selected) ? selected[0] : selected;
    try {
      const row = await invoke('add_scanned_path', { path: filePath, kind: 'file' });
      paths = [row, ...paths];
    } catch (e) {
      alert(e?.message ?? String(e));
    }
  }

  async function addFolder() {
    const selected = await openDialog({ directory: true, multiple: false });
    if (!selected) return;
    const folderPath = Array.isArray(selected) ? selected[0] : selected;
    try {
      const row = await invoke('add_scanned_path', { path: folderPath, kind: 'folder' });
      paths = [row, ...paths];
    } catch (e) {
      alert(t('settings.fileScanner.addFolderFailed', { msg: e?.message ?? e }));
    }
  }

  async function removePath(id) {
    try {
      await invoke('remove_scanned_path', { id });
      paths = paths.filter(p => p.id !== id);
      // Clean up any in-progress state for this path.
      const next = { ...progress };
      delete next[id];
      progress = next;
      await loadStaleSummary();
    } catch (e) {
      console.error('[remove_scanned_path]', e);
      alert(t('settings.fileScanner.removePathFailed', { msg: e?.message ?? e }));
    }
  }

  async function togglePath(id, enabled) {
    try {
      await invoke('toggle_scanned_path', { id, enabled });
      paths = paths.map(p => p.id === id ? { ...p, enabled } : p);
    } catch (e) {
      alert(t('settings.fileScanner.toggleFailed', { msg: e?.message ?? e }));
    }
  }

  async function rescan(id) {
    // Clear stale progress for this path before rescanning.
    progress = {
      ...progress,
      [id]: {
        scanned: 0,
        skipped: 0,
        visited: 0,
        total: 0,
        done: false,
        error: null,
        phase: null,
        chunks_embedded: 0,
        chunks_total: 0,
        current_file: null,
      },
    };
    try {
      await invoke('rescan_path', { id });
    } catch (e) {
      alert(t('settings.fileScanner.rescanFailed', { msg: e?.message ?? e }));
    }
  }

  async function stopScan(id) {
    try {
      await invoke('cancel_scanned_path_index', { id });
      const existing = progress[id] ?? { scanned: 0, total: 0, done: true, error: null };
      const es = { ...embedStarts };
      delete es[id];
      embedStarts = es;
      progress = { ...progress, [id]: { ...existing, done: true, error: null } };
      await loadPaths();
    } catch (e) {
      alert(t('settings.fileScanner.stopFailed', { msg: e?.message ?? e }));
    }
  }

  async function rescanAllPaths() {
    if (!paths.length) {
      return;
    }
    if (paths.some((p) => isScanning(p.id))) {
      return;
    }

    rescanningAll = true;
    rescanAllStatus = '';

    const ids = paths.map((p) => p.id);
    for (const id of ids) {
      progress = {
        ...progress,
        [id]: {
          scanned: 0,
          skipped: 0,
          visited: 0,
          total: 0,
          done: false,
          error: null,
          phase: null,
          chunks_embedded: 0,
          chunks_total: 0,
          current_file: null,
        },
      };
    }

    const starts = await Promise.all(
      ids.map((id) => invoke('rescan_path', { id }).then(() => null).catch((e) => e?.message ?? String(e)))
    );
    const failed = starts.filter(Boolean).length;
    const started = ids.length - failed;

    if (failed > 0) {
      rescanAllStatus = t('settings.fileScanner.rescanAllPartial', { started, total: ids.length, failed });
    } else {
      rescanAllStatus = tp('settings.fileScanner.rescanAllStarted', started, { count: started });
    }

    rescanningAll = false;
  }

  async function importAsNote(p) {
    importingNoteId = p.id;
    try {
      const note = await invoke('import_file_as_note', { filePath: p.path, folderId: null });
      lastImportedNoteId = { ...lastImportedNoteId, [p.id]: note.id };
      // Signal the main app to refresh its note list and offer navigation.
      window.dispatchEvent(new CustomEvent('grimoire:note-imported', { detail: { noteId: note.id } }));
    } catch (e) {
      alert(t('settings.fileScanner.importFailed', { msg: e?.message ?? e }));
    } finally {
      importingNoteId = null;
    }
  }

  function formatDate(timestamp) {
    if (!timestamp) return t('settings.fileScanner.neverScanned');
    return new Date(timestamp * 1000).toLocaleString();
  }

  /** Format a duration in seconds as a human-readable string (matches Wikipedia indexing). */
  function fmtEta(secs) {
    if (!isFinite(secs) || secs < 0) return null;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${s}s`;
    return `${s}s`;
  }

  /**
   * Composite progress: completed files use `scanned - 1`, current file adds chunk fraction when known.
   * Avoids showing ~100% for “file 1 of 1” during a long embedding run.
   */
  function progressFraction(id) {
    const p = progress[id];
    if (!p || p.total === 0) return 0;
    const ct = p.chunks_total ?? 0;
    const ce = p.chunks_embedded ?? 0;
    const scanned = p.scanned ?? 0;
    if (ct > 0) {
      const base = Math.max(0, scanned - 1);
      return Math.min(1, (base + ce / ct) / p.total);
    }
    return Math.min(1, scanned / p.total);
  }

  function scanPhaseDetail(id) {
    const p = progress[id];
    if (!p || p.error) return '';
    const phase = p.phase ?? '';
    const name = p.current_file ? ` · ${p.current_file}` : '';
    if (phase === 'storing') return t('settings.fileScanner.storing', { name });
    const ct = p.chunks_total ?? 0;
    const ce = p.chunks_embedded ?? 0;
    if (ct > 0) {
      return t('settings.fileScanner.embeddingChunks', {
        done: ce.toLocaleString(),
        total: ct.toLocaleString(),
        name,
      });
    }
    if (phase === 'reading') return t('settings.fileScanner.reading', { name });
    if (phase === 'cleanup') return t('settings.fileScanner.cleanup');
    if (phase === 'starting') return t('settings.fileScanner.starting');
    if (phase === 'walking') {
      const v = p.visited ?? 0;
      const total = p.total ?? 0;
      return t('settings.fileScanner.walking', { visited: v, total });
    }
    if (phase === 'embedding') return t('settings.fileScanner.embedding', { name });
    return '';
  }

  function embedEtaSuffix(id) {
    const p = progress[id];
    const start = embedStarts[id];
    if (!p || !start || p.done || p.error) return '';
    const ct = p.chunks_total ?? 0;
    const ce = p.chunks_embedded ?? 0;
    if (ct <= 0 || ce <= start.at || ce >= ct) return '';
    const elapsedSec = (Date.now() - start.time) / 1000;
    if (elapsedSec < 4) return '';
    const rate = (ce - start.at) / elapsedSec;
    const remainingSec = (ct - ce) / rate;
    if (!isFinite(remainingSec) || remainingSec < 5) return '';
    const formatted = fmtEta(remainingSec);
    return formatted ? t('settings.fileScanner.embedEta', { eta: formatted }) : '';
  }

  function isScanning(id) {
    const p = progress[id];
    return p && !p.done;
  }
</script>

<div class="file-scanner-settings">
  <div class="fs-header">
    <div class="fs-header-text">
      <h2>{t('settings.fileScanner.title')}</h2>
      <p class="fs-description">{t('settings.fileScanner.description')}</p>
    </div>
    <div class="fs-add-buttons">
      <button
        class="fs-add-btn"
        onclick={rescanAllPaths}
        disabled={rescanningAll || paths.some((p) => isScanning(p.id)) || paths.length === 0}
        title={t('settings.fileScanner.reindexAllTitle')}
      >
        {t('settings.fileScanner.reindexAll')}
      </button>
      <button class="fs-add-btn" onclick={addFile}>{t('settings.fileScanner.addFile')}</button>
      <button class="fs-add-btn" onclick={addFolder}>{t('settings.fileScanner.addFolder')}</button>
    </div>
  </div>

  <div class="fs-global-excludes">
    <button
      type="button"
      class="fs-collapse-toggle"
      onclick={() => (showGlobalExcludes = !showGlobalExcludes)}
      aria-expanded={showGlobalExcludes}
      title={showGlobalExcludes ? t('settings.fileScanner.hideGlobalExcludes') : t('settings.fileScanner.showGlobalExcludes')}
    >
      {showGlobalExcludes ? t('settings.fileScanner.hideGlobalExcludes') : t('settings.fileScanner.showGlobalExcludes')}
    </button>

    {#if showGlobalExcludes}
      <h3 class="fs-subheading">{t('settings.fileScanner.globalExcludesTitle')}</h3>
      <p class="fs-hint">
        {#each tParts('settings.fileScanner.globalExcludesHint') as part}
          {#if part.type === 'text'}{part.value}{:else if part.name === 'rescan'}<strong>{t('settings.fileScanner.rescan')}</strong>{/if}
        {/each}
      </p>
      <textarea class="fs-exclude-textarea" bind:value={globalExcludes} rows="4" spellcheck="false"></textarea>
      <button type="button" class="fs-add-btn" onclick={saveGlobalExcludes}>{t('settings.fileScanner.saveGlobalExcludes')}</button>
    {/if}
  </div>

  {#if rescanAllStatus}
    <p class="fs-bulk-status">{rescanAllStatus}</p>
  {/if}

  {#if paths.length === 0}
    <div class="fs-empty">{t('settings.fileScanner.empty')}</div>
  {:else}
    <div class="fs-list">
      {#each paths as p (p.id)}
        {@const prog = progress[p.id]}
        {@const scanning = isScanning(p.id)}
        {@const stale = staleById[p.id]}

        <div class="fs-row" class:disabled={!p.enabled}>
          <div class="fs-row-main">
            <span class="fs-kind-badge">{p.kind === 'folder' ? t('settings.fileScanner.folderKind') : t('settings.fileScanner.fileKind')}</span>
            <span class="fs-path" title={p.path}>{p.path}</span>
          </div>

          <div class="fs-row-meta">
            {#if stale?.root_missing}
              <span class="fs-stale-badge fs-stale-root" title={t('settings.fileScanner.pathMissingTitle')}>
                {t('settings.fileScanner.pathMissing')}
              </span>
            {:else if stale && stale.missing_files > 0}
              <span class="fs-stale-badge" title={t('settings.fileScanner.staleFilesTitle')}>
                {t('settings.fileScanner.staleMissing', { count: stale.missing_files })}
              </span>
            {/if}
            {#if p.error_msg && !scanning}
              <span class="fs-error" title={p.error_msg}>{t('common.error')}</span>
            {/if}
            <span class="fs-file-count">{tp('settings.fileScanner.fileCount', p.file_count, { count: p.file_count })}</span>
            <span class="fs-scanned-at" title={t('settings.fileScanner.lastScannedTitle')}>{formatDate(p.last_scanned_at)}</span>
          </div>

          {#if stale?.root_missing}
            <p class="fs-stale-root-msg">
              {#each tParts('settings.fileScanner.staleRootMsg') as part}
                {#if part.type === 'text'}{part.value}{:else if part.name === 'remove'}<strong>{t('settings.shared.remove')}</strong>{/if}
              {/each}
            </p>
          {/if}

          {#if scanning}
            <div class="fs-progress-stack">
              <div class="fs-progress-row">
                <div class="fs-progress-bar">
                  <div class="fs-progress-fill" style="width: {Math.round(progressFraction(p.id) * 100)}%"></div>
                </div>
                <span class="fs-progress-label">
                  {#if prog?.error}
                    {prog.error}
                  {:else}
                    {@const s = prog?.skipped ?? 0}
                    {t('settings.fileScanner.progressFile', {
                      visited: prog?.visited ?? 0,
                      total: prog?.total ?? 0,
                      unchangedPart: s > 0 ? t('settings.fileScanner.unchangedPart', { count: s }) : '',
                    })}
                  {/if}
                </span>
              </div>
              {#if !prog?.error}
                {@const detail = scanPhaseDetail(p.id)}
                {@const eta = embedEtaSuffix(p.id)}
                {#if detail || eta}
                  <div class="fs-progress-detail">
                    {detail}{eta}
                  </div>
                {/if}
              {/if}
            </div>
          {/if}

          {#if prog?.done && ((prog?.permanently_skipped ?? 0) > 0 || (prog?.permanently_skipped_chunks ?? 0) > 0)}
            <p class="fs-hint fs-skip-summary">
              {t('settings.fileScanner.skipSummary', {
                files: prog.permanently_skipped,
                chunks: prog.permanently_skipped_chunks,
              })}
            </p>
          {/if}

          <div class="fs-row-actions">
            <label class="fs-toggle" title={p.enabled ? t('settings.fileScanner.toggleDisableTitle') : t('settings.fileScanner.toggleEnableTitle')}>
              <input
                type="checkbox"
                checked={p.enabled}
                onchange={(e) => togglePath(p.id, /** @type {HTMLInputElement} */ (e.target).checked)}
              />
              {p.enabled ? t('settings.fileScanner.enabled') : t('settings.fileScanner.disabled')}
            </label>
            {#if p.kind === 'file'}
              {#if lastImportedNoteId[p.id]}
                <button
                  class="fs-action-btn fs-view-btn"
                  onclick={() => window.dispatchEvent(new CustomEvent('grimoire:navigate-note', { detail: { noteId: lastImportedNoteId[p.id] } }))}
                  title={t('settings.fileScanner.viewNoteTitle')}
                >
                  {t('settings.fileScanner.viewNote')}
                </button>
              {/if}
              <button
                class="fs-action-btn"
                onclick={() => importAsNote(p)}
                disabled={scanning || importingNoteId === p.id}
                title={t('settings.fileScanner.turnIntoNoteTitle')}
              >
                {importingNoteId === p.id ? t('settings.fileScanner.importing') : t('settings.fileScanner.turnIntoNote')}
              </button>
            {/if}
            <button
              class="fs-action-btn"
              onclick={() => stopScan(p.id)}
              disabled={!scanning}
              title={t('settings.fileScanner.stopTitle')}
            >
              {t('settings.shared.stop')}
            </button>
            <button
              class="fs-action-btn"
              onclick={() => rescan(p.id)}
              disabled={scanning}
              title={t('settings.fileScanner.rescanTitle')}
            >
              {t('settings.fileScanner.rescan')}
            </button>
            <button
              type="button"
              class="fs-action-btn"
              onclick={() => clearStaleFiles(p.id)}
              disabled={scanning || stale?.root_missing || !stale || stale.missing_files === 0}
              title={t('settings.fileScanner.cleanUpTitle')}
            >
              {t('settings.fileScanner.cleanUp')}
            </button>
            <button
              type="button"
              class="fs-action-btn"
              onclick={() => toggleExcludesEditor(p)}
              title={t('settings.fileScanner.excludesTitle')}
            >
              {showExcludesFor === p.id ? t('settings.fileScanner.hideExcludes') : t('settings.fileScanner.excludes')}
            </button>
            <button
              class="fs-action-btn fs-remove-btn"
              class:fs-remove-emphasis={stale?.root_missing}
              onclick={() => removePath(p.id)}
              disabled={scanning}
              title={t('settings.fileScanner.removeTitle')}
            >
              {t('settings.shared.remove')}
            </button>
          </div>

          {#if showExcludesFor === p.id}
            <div class="fs-excludes-panel">
              <p class="fs-hint">{t('settings.fileScanner.excludesPanelHint')}</p>
              <textarea
                class="fs-exclude-textarea"
                rows="4"
                spellcheck="false"
                value={excludeDraft[p.id] ?? ''}
                oninput={(e) => {
                  excludeDraft = { ...excludeDraft, [p.id]: /** @type {HTMLTextAreaElement} */ (e.target).value };
                }}
              ></textarea>
              <div class="fs-excludes-actions">
                <button type="button" class="fs-add-btn" onclick={() => savePathExcludes(p.id)}>{t('settings.fileScanner.saveExcludes')}</button>
                <button type="button" class="fs-action-btn" onclick={() => { showExcludesFor = null; }}>{t('common.cancel')}</button>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  @import '../styles/settings-file-scanner.css';
</style>
