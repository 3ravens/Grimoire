<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { onMount, onDestroy } from 'svelte';

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
      alert(`Could not save global excludes: ${e?.message ?? e}`);
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
      alert(`Could not save excludes: ${e?.message ?? e}`);
    }
  }

  async function clearStaleFiles(id) {
    try {
      const n = await invoke('clear_stale_scanned_files', { id });
      await loadPaths();
      await loadStaleSummary();
      if (n > 0) {
        alert(`Removed ${n} missing file${n === 1 ? '' : 's'} from the index.`);
      }
    } catch (e) {
      alert(`Clean up failed: ${e?.message ?? e}`);
    }
  }

  async function addFile() {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      filters: [
        {
          name: 'Supported files',
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
      alert(`Could not add folder: ${e?.message ?? e}`);
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
      alert(`Could not remove path: ${e?.message ?? e}`);
    }
  }

  async function togglePath(id, enabled) {
    try {
      await invoke('toggle_scanned_path', { id, enabled });
      paths = paths.map(p => p.id === id ? { ...p, enabled } : p);
    } catch (e) {
      alert(`Could not toggle path: ${e?.message ?? e}`);
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
      alert(`Could not rescan path: ${e?.message ?? e}`);
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
      alert(`Could not stop scan: ${e?.message ?? e}`);
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
      rescanAllStatus = `Started ${started} of ${ids.length} paths. ${failed} failed to start.`;
    } else {
      rescanAllStatus = `Started re-indexing ${started} path${started === 1 ? '' : 's'}. Progress appears per row.`;
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
      alert(`Could not import file as note: ${e?.message ?? e}`);
    } finally {
      importingNoteId = null;
    }
  }

  function formatDate(timestamp) {
    if (!timestamp) return 'Never';
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
    if (phase === 'storing') return `Saving to index${name}`;
    const ct = p.chunks_total ?? 0;
    const ce = p.chunks_embedded ?? 0;
    if (ct > 0) {
      return `Embedding ${ce.toLocaleString()} / ${ct.toLocaleString()} chunks${name}`;
    }
    if (phase === 'reading') return `Reading${name}`;
    if (phase === 'storing') return `Saving to index${name}`;
    if (phase === 'cleanup') return 'Removing stale index entries…';
    if (phase === 'starting') return 'Starting…';
    if (phase === 'walking') {
      const v = p.visited ?? 0;
      const t = p.total ?? 0;
      return `Finding files… (${v} / ${t})`;
    }
    if (phase === 'embedding') return `Embedding…${name}`;
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
    return formatted ? ` · ~${formatted} left` : '';
  }

  function isScanning(id) {
    const p = progress[id];
    return p && !p.done;
  }
</script>

<div class="file-scanner-settings">
  <div class="fs-header">
    <div class="fs-header-text">
      <h2>File Scanner</h2>
      <p class="fs-description">
        Add files or folders from outside your vault as context sources.
        Indexed content is searched alongside your notes when chatting.
        Supports <code>.txt</code>, <code>.md</code>, <code>.pdf</code>, <code>.csv</code>, <code>.html</code>,
        <code>.docx</code>, <code>.odt</code>, <code>.log</code>, and more.
      </p>
    </div>
    <div class="fs-add-buttons">
      <button
        class="fs-add-btn"
        onclick={rescanAllPaths}
        disabled={rescanningAll || paths.some((p) => isScanning(p.id)) || paths.length === 0}
        title="Re-index every path currently in File Scanner"
      >
        Re-index all
      </button>
      <button class="fs-add-btn" onclick={addFile}>Add file</button>
      <button class="fs-add-btn" onclick={addFolder}>Add folder</button>
    </div>
  </div>

  <div class="fs-global-excludes">
    <button
      type="button"
      class="fs-collapse-toggle"
      onclick={() => (showGlobalExcludes = !showGlobalExcludes)}
      aria-expanded={showGlobalExcludes}
      title="Show or hide global exclude patterns"
    >
      {showGlobalExcludes ? 'Hide' : 'Show'} global exclude patterns
    </button>

    {#if showGlobalExcludes}
      <h3 class="fs-subheading">Global exclude patterns</h3>
      <p class="fs-hint">
        Newline-separated globs (merged with each path). Filename-only patterns match at any depth.
        Use forward slashes. Example: <code>node_modules</code>, <code>*.tmp</code>, <code>draft.txt</code>.
        Save here, then use <strong>Rescan</strong> on each path to apply.
      </p>
      <textarea class="fs-exclude-textarea" bind:value={globalExcludes} rows="4" spellcheck="false"></textarea>
      <button type="button" class="fs-add-btn" onclick={saveGlobalExcludes}>Save global excludes</button>
    {/if}
  </div>

  {#if rescanAllStatus}
    <p class="fs-bulk-status">{rescanAllStatus}</p>
  {/if}

  {#if paths.length === 0}
    <div class="fs-empty">
      No paths added yet. Add a file or folder to make it available as chat context.
    </div>
  {:else}
    <div class="fs-list">
      {#each paths as p (p.id)}
        {@const prog = progress[p.id]}
        {@const scanning = isScanning(p.id)}
        {@const stale = staleById[p.id]}

        <div class="fs-row" class:disabled={!p.enabled}>
          <div class="fs-row-main">
            <span class="fs-kind-badge">{p.kind === 'folder' ? 'Folder' : 'File'}</span>
            <span class="fs-path" title={p.path}>{p.path}</span>
          </div>

          <div class="fs-row-meta">
            {#if stale?.root_missing}
              <span class="fs-stale-badge fs-stale-root" title="The scanned path no longer exists on disk">
                Path missing on disk
              </span>
            {:else if stale && stale.missing_files > 0}
              <span class="fs-stale-badge" title="Some indexed files were deleted or moved">
                Stale: {stale.missing_files} missing
              </span>
            {/if}
            {#if p.error_msg && !scanning}
              <span class="fs-error" title={p.error_msg}>Error</span>
            {/if}
            <span class="fs-file-count">{p.file_count} file{p.file_count !== 1 ? 's' : ''}</span>
            <span class="fs-scanned-at" title="Last scanned">{formatDate(p.last_scanned_at)}</span>
          </div>

          {#if stale?.root_missing}
            <p class="fs-stale-root-msg">
              This index entry points at a path that is gone. Use <strong>Remove</strong> to delete the entry
              and its vectors (no separate clean-up needed).
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
                    File {prog?.visited ?? 0} / {prog?.total ?? 0}{@const s = prog?.skipped ?? 0}{s > 0 ? ` (${s} unchanged)` : ''}
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
              Indexed with {prog.permanently_skipped} skipped file(s), {prog.permanently_skipped_chunks} skipped chunk(s).
            </p>
          {/if}

          <div class="fs-row-actions">
            <label class="fs-toggle" title={p.enabled ? 'Disable (excludes from RAG)' : 'Enable'}>
              <input
                type="checkbox"
                checked={p.enabled}
                onchange={(e) => togglePath(p.id, /** @type {HTMLInputElement} */ (e.target).checked)}
              />
              {p.enabled ? 'Enabled' : 'Disabled'}
            </label>
            {#if p.kind === 'file'}
              {#if lastImportedNoteId[p.id]}
                <button
                  class="fs-action-btn fs-view-btn"
                  onclick={() => window.dispatchEvent(new CustomEvent('grimoire:navigate-note', { detail: { noteId: lastImportedNoteId[p.id] } }))}
                  title="Open the imported note in the editor"
                >
                  View note
                </button>
              {/if}
              <button
                class="fs-action-btn"
                onclick={() => importAsNote(p)}
                disabled={scanning || importingNoteId === p.id}
                title="Copy the file content into a new note in the Unfiled folder"
              >
                {importingNoteId === p.id ? 'Importing…' : 'Turn into note'}
              </button>
            {/if}
            <button
              class="fs-action-btn"
              onclick={() => stopScan(p.id)}
              disabled={!scanning}
              title="Stop the current indexing run for this path"
            >
              Stop
            </button>
            <button
              class="fs-action-btn"
              onclick={() => rescan(p.id)}
              disabled={scanning}
              title="Re-index all files in this path"
            >
              Rescan
            </button>
            <button
              type="button"
              class="fs-action-btn"
              onclick={() => clearStaleFiles(p.id)}
              disabled={scanning || stale?.root_missing || !stale || stale.missing_files === 0}
              title="Remove index rows for files that no longer exist on disk"
            >
              Clean up
            </button>
            <button
              type="button"
              class="fs-action-btn"
              onclick={() => toggleExcludesEditor(p)}
              title="Edit per-path exclude globs (Rescan to apply)"
            >
              {showExcludesFor === p.id ? 'Hide excludes' : 'Excludes'}
            </button>
            <button
              class="fs-action-btn fs-remove-btn"
              class:fs-remove-emphasis={stale?.root_missing}
              onclick={() => removePath(p.id)}
              disabled={scanning}
              title="Remove this path and delete its indexed data"
            >
              Remove
            </button>
          </div>

          {#if showExcludesFor === p.id}
            <div class="fs-excludes-panel">
              <p class="fs-hint">One pattern per line. Merged with global excludes. Save, then Rescan.</p>
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
                <button type="button" class="fs-add-btn" onclick={() => savePathExcludes(p.id)}>Save excludes</button>
                <button type="button" class="fs-action-btn" onclick={() => { showExcludesFor = null; }}>Cancel</button>
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
