<script>
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import ConfirmModal from './ConfirmModal.svelte';
  import { t, tp } from './i18n/t.js';

  // ---------------------------------------------------------------------------
  // State
  // ---------------------------------------------------------------------------

  const PAGE_SIZE = 25;

  let page        = $state(1);
  let totalCount  = $state(0);
  let entries     = $state([]);
  let filter      = $state('all');
  let searchInput = $state('');
  let searchQuery = $state('');  // debounced version of searchInput
  let loading     = $state(false);
  let showClear   = $state(false);
  /** Last export status message */
  let exportStatus = $state('');

  let debounceTimer;

  /** Monotonic id so stale `load()` responses cannot overwrite newer UI state. */
  let loadGeneration = 0;

  const totalPages = $derived(Math.max(1, Math.ceil(totalCount / PAGE_SIZE)));

  // ---------------------------------------------------------------------------
  // Data fetching
  // ---------------------------------------------------------------------------

  async function load() {
    const myGen = ++loadGeneration;
    loading = true;
    try {
      const [rows, count] = await Promise.all([
        invoke('get_audit_log', {
          page,
          pageSize: PAGE_SIZE,
          actionFilter: filter,
          search: searchQuery || null,
        }),
        invoke('get_audit_log_count', {
          actionFilter: filter,
          search: searchQuery || null,
        }),
      ]);
      if (myGen !== loadGeneration) return;
      entries    = rows;
      totalCount = count;
    } catch (e) {
      if (myGen !== loadGeneration) return;
      console.error('Failed to load audit log:', e);
    } finally {
      if (myGen === loadGeneration) loading = false;
    }
  }

  // Reload whenever filter, searchQuery, or page changes.
  $effect(() => {
    filter; searchQuery; page;
    load();
  });

  // Poll every 5 seconds so newly-written entries appear without user interaction.
  $effect(() => {
    const interval = setInterval(load, 5000);
    return () => clearInterval(interval);
  });

  onMount(() => {
    const onPruned = () => load();
    window.addEventListener('grimoire:audit-pruned', onPruned);
    return () => window.removeEventListener('grimoire:audit-pruned', onPruned);
  });

  // Debounce the raw search input by 300 ms.
  function onSearchInput(e) {
    searchInput = e.target.value;
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      page        = 1;
      searchQuery = searchInput.trim();
    }, 300);
  }

  function onFilterChange(e) {
    filter = e.target.value;
    page   = 1;
  }

  // ---------------------------------------------------------------------------
  // Clear log
  // ---------------------------------------------------------------------------

  async function confirmClear() {
    await invoke('clear_audit_log');
    showClear = false;
    page      = 1;
    load();
  }

  async function exportAudit(format) {
    exportStatus = '';
    try {
      const dateStr = new Date().toISOString().slice(0, 10);
      const ext = format === 'csv' ? 'csv' : 'json';
      const path = await save({
        title: t('views.auditExportDialogTitle'),
        defaultPath: `grimoire-audit-${dateStr}.${ext}`,
        filters: [
          format === 'csv'
            ? { name: t('views.auditFilterCsv'), extensions: ['csv'] }
            : { name: t('views.auditFilterJson'), extensions: ['json'] },
        ],
      });
      if (!path) return;
      const result = await invoke('export_audit_log', {
        format,
        actionFilter: filter,
        search: searchQuery || null,
        destPath: path,
      });
      const skipped = result?.skipped_locked ?? result?.skippedLocked ?? 0;
      const exported = result?.exported ?? 0;
      exportStatus =
        skipped > 0
          ? (exported === 1
            ? t('views.auditExportSuccess', { exported, skipped })
            : t('views.auditExportSuccessPlural', { exported, skipped }))
          : (exported === 1
            ? t('views.auditExportSuccessSimple', { exported })
            : t('views.auditExportSuccessSimplePlural', { exported }));
    } catch (e) {
      exportStatus = t('views.auditExportFailed', { msg: e?.message ?? e });
    }
  }

  // ---------------------------------------------------------------------------
  // Formatting helpers
  // ---------------------------------------------------------------------------

  function formatTimestamp(unix) {
    return new Date(unix * 1000).toLocaleString(undefined, {
      year:   'numeric',
      month:  'short',
      day:    '2-digit',
      hour:   '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  }

  // Map action string to a display label and category class.
  function meta(action) {
    const labels = {
      note_open: t('views.auditActionNoteOpen'),
      note_create: t('views.auditActionNoteCreate'),
      note_update: t('views.auditActionNoteUpdate'),
      note_delete: t('views.auditActionNoteDelete'),
      note_export: t('views.auditActionNoteExport'),
      folder_create: t('views.auditActionFolderCreate'),
      folder_rename: t('views.auditActionFolderRename'),
      folder_delete: t('views.auditActionFolderDelete'),
      search_fts: t('views.auditActionSearchFts'),
      search_semantic: t('views.auditActionSearchSemantic'),
      search_combined: t('views.auditActionSearchCombined'),
      llm_chat: t('views.auditActionLlmChat'),
      llm_improve: t('views.auditActionLlmImprove'),
      file_scan: t('views.auditActionFileScan'),
      file_import: t('views.auditActionFileImport'),
      wikipedia_read: t('views.auditActionWikipedia'),
    };
    const cats = {
      note_open: 'note', note_create: 'note', note_update: 'note', note_delete: 'note', note_export: 'note',
      folder_create: 'folder', folder_rename: 'folder', folder_delete: 'folder',
      search_fts: 'search', search_semantic: 'search', search_combined: 'search',
      llm_chat: 'llm', llm_improve: 'llm',
      file_scan: 'file', file_import: 'file',
      wikipedia_read: 'wiki',
    };
    return { label: labels[action] ?? action, cat: cats[action] ?? 'other' };
  }
</script>

<div class="audit-log">
  <!-- Controls -->
  <div class="audit-controls">
    <select class="filter-select" value={filter} onchange={onFilterChange} aria-label={t('views.auditFilterAria')}>
      <option value="all">{t('views.auditAllActions')}</option>
      <option value="notes">{t('views.auditNotes')}</option>
      <option value="folders">{t('views.auditFolders')}</option>
      <option value="search">{t('views.auditSearch')}</option>
      <option value="llm">{t('views.auditLlm')}</option>
      <option value="file_scanner">{t('views.auditFileScanner')}</option>
      <option value="wikipedia">{t('views.auditWikipedia')}</option>
    </select>

    <input
      class="search-input"
      type="search"
      placeholder={t('views.auditSearchPlaceholder')}
      value={searchInput}
      oninput={onSearchInput}
      aria-label={t('views.auditSearchAria')}
    />

    <button
      type="button"
      class="export-btn"
      onclick={() => exportAudit('csv')}
      disabled={loading || totalCount === 0}
      title={t('views.auditExportTitle')}
    >
      {t('views.auditExportCsv')}
    </button>
    <button
      type="button"
      class="export-btn"
      onclick={() => exportAudit('json')}
      disabled={loading || totalCount === 0}
      title={t('views.auditExportJsonTitle')}
    >
      {t('views.auditExportJson')}
    </button>
    <button class="clear-btn" onclick={() => (showClear = true)} disabled={totalCount === 0}>
      {t('views.auditClear')}
    </button>
  </div>

  {#if exportStatus}
    <p class="export-status" role="status" aria-live="polite">{exportStatus}</p>
  {/if}

  <!-- Table -->
  {#if loading && entries.length === 0}
    <p class="empty-state" role="status" aria-live="polite">{t('views.auditLoading')}</p>
  {:else if entries.length === 0}
    <p class="empty-state">{searchQuery || filter !== 'all' ? t('views.auditEmptyFiltered') : t('views.auditEmptyAll')}</p>
  {:else}
    <div class="audit-table-wrap">
      <table class="audit-table">
        <thead>
          <tr>
            <th>{t('views.auditColTime')}</th>
            <th>{t('views.auditColAction')}</th>
            <th>{t('views.auditColResource')}</th>
            <th>{t('views.auditColDetail')}</th>
          </tr>
        </thead>
        <tbody>
          {#each entries as entry (entry.id)}
            {@const m = meta(entry.action)}
            <tr>
              <td class="col-time">{formatTimestamp(entry.created_at)}</td>
              <td class="col-action">
                <span class="action-badge cat-{m.cat}">{m.label}</span>
              </td>
              <td class="col-resource">{entry.resource_name ?? t('views.emDash')}</td>
              <td class="col-detail">{entry.detail ?? ''}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <!-- Pagination -->
    {#if totalPages > 1}
      <div class="pagination">
        <button onclick={() => { page = Math.max(1, page - 1); }} disabled={page <= 1}>{t('views.auditPrev')}</button>
        <span class="page-info">{t('views.auditPageOf', { page, total: totalPages })}</span>
        <button onclick={() => { page = Math.min(totalPages, page + 1); }} disabled={page >= totalPages}>{t('views.auditNext')}</button>
      </div>
    {/if}
  {/if}
</div>

{#if showClear}
  <ConfirmModal
    title={t('views.auditClearTitle')}
    message={t('views.auditClearMessage', { count: totalCount })}
    confirmLabel={t('views.auditClearConfirm')}
    onConfirm={confirmClear}
    onCancel={() => (showClear = false)}
  />
{/if}

<style>
  .audit-log {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 12px;
  }

  /* Controls row */
  .audit-controls {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .filter-select,
  .search-input {
    height: 28px;
    padding: 0 8px;
    font: 13px var(--sans);
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    outline: none;
  }

  .filter-select:focus,
  .search-input:focus {
    border-color: var(--accent);
  }

  .search-input {
    flex: 1;
    min-width: 0;
  }

  .export-btn {
    height: 28px;
    padding: 0 10px;
    font: 13px var(--sans);
    color: var(--text);
    background: var(--bg3);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
  }

  .export-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text-h);
  }

  .export-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .export-status {
    margin: 0;
    font: 12px var(--sans);
    color: var(--text-muted);
  }

  .clear-btn {
    height: 28px;
    padding: 0 10px;
    font: 13px var(--sans);
    color: var(--danger);
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
  }

  .clear-btn:hover:not(:disabled) {
    border-color: var(--danger);
    background: rgba(192, 57, 43, 0.06);
  }

  .clear-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  /* Table */
  .audit-table-wrap {
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  .audit-table {
    width: 100%;
    border-collapse: collapse;
    font: 13px var(--sans);
  }

  .audit-table th {
    padding: 6px 10px;
    text-align: left;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text);
    background: var(--bg2);
    border-bottom: 1px solid var(--border);
  }

  .audit-table td {
    padding: 5px 10px;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
    color: var(--text);
  }

  .audit-table tbody tr:last-child td {
    border-bottom: none;
  }

  .audit-table tbody tr:hover td {
    background: var(--bg3);
  }

  .col-time {
    white-space: nowrap;
    font: 12px var(--mono);
    color: var(--text);
    opacity: 0.75;
    width: 170px;
  }

  .col-action {
    width: 130px;
    white-space: nowrap;
  }

  .col-resource {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .col-detail {
    font: 12px var(--mono);
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.8;
  }

  /* Action badges */
  .action-badge {
    display: inline-block;
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 11px;
    font-weight: 500;
    border: 1px solid transparent;
  }

  .cat-note   { color: var(--accent);  border-color: var(--accent);  background: var(--accent-bg); }
  .cat-folder { color: var(--text-h);  border-color: var(--border);  background: var(--bg3); }
  .cat-search { color: var(--text-h);  border-color: var(--border);  background: var(--bg3); }
  .cat-llm    { color: var(--danger);  border-color: var(--danger);  background: rgba(192, 57, 43, 0.06); }
  .cat-file   { color: var(--text-h);  border-color: var(--border);  background: var(--bg3); }
  .cat-wiki   { color: var(--text-h);  border-color: var(--border);  background: var(--bg3); }
  .cat-other  { color: var(--text);    border-color: var(--border);  background: var(--bg3); }

  /* Empty state */
  .empty-state {
    font: 13px var(--sans);
    color: var(--text);
    opacity: 0.6;
    padding: 16px 0;
    text-align: center;
  }

  /* Pagination */
  .pagination {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-content: center;
    padding: 4px 0;
  }

  .pagination button {
    height: 26px;
    padding: 0 10px;
    font: 12px var(--sans);
    color: var(--text);
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
  }

  .pagination button:hover:not(:disabled) {
    background: var(--bg3);
  }

  .pagination button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .page-info {
    font: 12px var(--sans);
    color: var(--text);
    opacity: 0.7;
  }
</style>
