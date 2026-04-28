<!-- Copyright (C) 2026 Wim Palland

This file is part of Grimoire.

Grimoire is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

Grimoire is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with Grimoire. If not, see <https://www.gnu.org/licenses/>. -->

<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { onMount, onDestroy } from 'svelte';

  // ── State ────────────────────────────────────────────────────────────────

  // ScannedPath shape: { id, path, kind, added_at, last_scanned_at, enabled, file_count, error_msg }
  let paths = $state([]);

  // progress[path_id] = { scanned: number, total: number, done: boolean, error: string|null }
  let progress = $state({});

  // id of the path currently being imported as a note (null if none)
  let importingNoteId = $state(null);

  // lastImportedNoteId[path_id] = note id — set after a successful import so we can show "View note"
  let lastImportedNoteId = $state({});

  let unlisten = null;

  // ── Lifecycle ────────────────────────────────────────────────────────────

  onMount(async () => {
    await loadPaths();

    unlisten = await listen('filescanner:progress', (ev) => {
      const { path_id, scanned, total, done, error } = ev.payload;
      progress = {
        ...progress,
        [path_id]: { scanned, total, done, error: error ?? null },
      };
      if (done) {
        // Refresh the row so file_count / last_scanned_at update.
        loadPaths();
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

  async function addFile() {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      filters: [{ name: 'Supported files', extensions: ['txt', 'md', 'pdf'] }],
    });
    if (!selected) return;
    const filePath = Array.isArray(selected) ? selected[0] : selected;
    try {
      const row = await invoke('add_scanned_path', { path: filePath, kind: 'file' });
      paths = [row, ...paths];
    } catch (e) {
      alert(String(e));
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
      alert(`Could not add folder: ${e}`);
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
    } catch (e) {
      alert(`Could not remove path: ${e}`);
    }
  }

  async function togglePath(id, enabled) {
    try {
      await invoke('toggle_scanned_path', { id, enabled });
      paths = paths.map(p => p.id === id ? { ...p, enabled } : p);
    } catch (e) {
      alert(`Could not toggle path: ${e}`);
    }
  }

  async function rescan(id) {
    // Clear stale progress for this path before rescanning.
    progress = { ...progress, [id]: { scanned: 0, total: 0, done: false, error: null } };
    try {
      await invoke('rescan_path', { id });
    } catch (e) {
      alert(`Could not rescan path: ${e}`);
    }
  }

  async function importAsNote(p) {
    importingNoteId = p.id;
    try {
      const note = await invoke('import_file_as_note', { filePath: p.path, folderId: null });
      lastImportedNoteId = { ...lastImportedNoteId, [p.id]: note.id };
      // Signal the main app to refresh its note list and offer navigation.
      window.dispatchEvent(new CustomEvent('grimoire:note-imported', { detail: { noteId: note.id } }));
    } catch (e) {
      alert(`Could not import file as note: ${e}`);
    } finally {
      importingNoteId = null;
    }
  }

  function formatDate(timestamp) {
    if (!timestamp) return 'Never';
    return new Date(timestamp * 1000).toLocaleString();
  }

  function progressFraction(id) {
    const p = progress[id];
    if (!p || p.total === 0) return 0;
    return p.scanned / p.total;
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
        Supports <code>.txt</code>, <code>.md</code>, and <code>.pdf</code> files.
      </p>
    </div>
    <div class="fs-add-buttons">
      <button class="fs-add-btn" onclick={addFile}>Add file</button>
      <button class="fs-add-btn" onclick={addFolder}>Add folder</button>
    </div>
  </div>

  {#if paths.length === 0}
    <div class="fs-empty">
      No paths added yet. Add a file or folder to make it available as chat context.
    </div>
  {:else}
    <div class="fs-list">
      {#each paths as p (p.id)}
        {@const prog = progress[p.id]}
        {@const scanning = isScanning(p.id)}

        <div class="fs-row" class:disabled={!p.enabled}>
          <div class="fs-row-main">
            <span class="fs-kind-badge">{p.kind === 'folder' ? 'Folder' : 'File'}</span>
            <span class="fs-path" title={p.path}>{p.path}</span>
          </div>

          <div class="fs-row-meta">
            {#if p.error_msg && !scanning}
              <span class="fs-error" title={p.error_msg}>Error</span>
            {/if}
            <span class="fs-file-count">{p.file_count} file{p.file_count !== 1 ? 's' : ''}</span>
            <span class="fs-scanned-at" title="Last scanned">{formatDate(p.last_scanned_at)}</span>
          </div>

          {#if scanning}
            <div class="fs-progress-row">
              <div class="fs-progress-bar">
                <div class="fs-progress-fill" style="width: {Math.round(progressFraction(p.id) * 100)}%"></div>
              </div>
              <span class="fs-progress-label">
                {#if prog?.error}
                  {prog.error}
                {:else}
                  Scanning {prog?.scanned ?? 0} / {prog?.total ?? 0}
                {/if}
              </span>
            </div>
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
              onclick={() => rescan(p.id)}
              disabled={scanning}
              title="Re-index all files in this path"
            >
              Rescan
            </button>
            <button
              class="fs-action-btn fs-remove-btn"
              onclick={() => removePath(p.id)}
              disabled={scanning}
              title="Remove this path and delete its indexed data"
            >
              Remove
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  @import '../styles/settings-file-scanner.css';
</style>
