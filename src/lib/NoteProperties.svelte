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
  /**
   * NoteProperties — collapsible properties panel shown between the note title
   * and the content editor. Displays all property definitions for the note's
   * folder and lets the user view/edit values inline.
   *
   * Props:
   *   noteId           — the active note's id
   *   folderId         — the note's folder_id (properties are folder-scoped)
   *   activeTitle      — current note title from the editor (for history compare)
   *   activeContent    — current note body from the editor (for history compare)
   *   onPropertiesLoad — optional callback; receives the loaded properties array
   */
  import { invoke } from '@tauri-apps/api/core';
  import { computeDiff } from './utils/diff.js';
  import DiffView from './DiffView.svelte';

  function parseSelectOptions(raw) {
    try {
      const arr = JSON.parse(raw || '[]');
      return Array.isArray(arr) ? arr : [];
    } catch {
      return [];
    }
  }
  
  let {
    noteId,
    folderId,
    activeTitle = '',
    activeContent = '',
    onPropertiesLoad = () => {},
    onVersionRestore = () => {},
  } = $props();

  let defs = $state([]);
  let propValues = $state([]);
  let open = $state(true);
  let loading = $state(false);
  let historyOpen = $state(false);
  let versionLoading = $state(false);
  let versions = $state([]);
  let selectedVersionId = $state(null);
  let selectedVersion = $state(null);

  const diffHunks = $derived(
    selectedVersion
      ? computeDiff(activeContent ?? '', selectedVersion.content ?? '')
      : [],
  );

  // "Add property" inline form state
  let adding = $state(false);
  let newName = $state('');
  let newType = $state('text');
  let newOptions = $state(''); // comma-separated, for 'select' type

  // Load defs + values on mount. {#key noteId} in the parent ensures this
  // component is always remounted fresh when the note changes, so there is
  // never a stale render frame showing the previous note's properties.
  $effect(() => {
    if (!noteId) return;
    if (folderId) {
      loadProperties(noteId, folderId);
      return;
    }
    defs = [];
    propValues = [];
    onPropertiesLoad([]);
    loadVersions(noteId);
  });

  async function loadProperties(nid, fid) {
    loading = true;
    try {
      const [d, p] = await Promise.all([
        invoke('get_property_defs', { folderId: fid }),
        invoke('get_note_properties', { noteId: nid }),
      ]);
      defs = d;
      propValues = p;
      onPropertiesLoad(p);
      await loadVersions(nid);
    } catch {
      // Non-fatal — just show nothing
    } finally {
      loading = false;
    }
  }

  async function loadVersions(nid) {
    try {
      versions = await invoke('get_note_versions', { noteId: nid });
      if (!versions.some(v => v.id === selectedVersionId)) {
        selectedVersionId = null;
        selectedVersion = null;
      }
    } catch {
      versions = [];
    }
  }

  async function selectVersion(versionId) {
    selectedVersionId = versionId;
    versionLoading = true;
    try {
      const version = await invoke('get_note_version_content', { noteId, versionId });
      selectedVersion = version;
    } catch {
      selectedVersion = null;
    } finally {
      versionLoading = false;
    }
  }

  async function restoreSelectedVersion() {
    if (!selectedVersionId) return;
    const ok = window.confirm('Restore this version? The current note state will be saved as a new revision first.');
    if (!ok) return;
    try {
      const restored = await invoke('restore_note_version', { noteId, versionId: selectedVersionId });
      onVersionRestore(restored);
      await loadVersions(noteId);
      selectedVersionId = null;
      selectedVersion = null;
    } catch {
      // Non-fatal for panel rendering.
    }
  }

  async function setValue(defId, value) {
    try {
      await invoke('set_note_property', { noteId, defId, value });
      // Update local state immediately
      propValues = propValues.map(p => p.def_id === defId ? { ...p, value } : p);
      onPropertiesLoad(propValues);
    } catch {
      // Silently fail — value will reload next time
    }
  }

  async function addProperty() {
    const name = newName.trim();
    if (!name) return;
    try {
      const options = newType === 'select' && newOptions.trim()
        ? JSON.stringify(newOptions.split(',').map(s => s.trim()).filter(Boolean))
        : null;
      // create_property_def returns the new PropertyDef (including its id).
      const def = await invoke('create_property_def', {
        folderId,
        name,
        type: newType,
        options,
      });
      // Seed an empty note_properties row so THIS note immediately shows the
      // new property. Other notes in the folder are not affected.
      await invoke('set_note_property', { noteId, defId: def.id, value: '' });
      newName = '';
      newType = 'text';
      newOptions = '';
      adding = false;
      await loadProperties(noteId, folderId);
    } catch (e) {
      console.error('Failed to create property:', e);
    }
  }

  async function deleteDef(defId) {
    try {
      await invoke('delete_property_def', { id: defId });
      await loadProperties(noteId, folderId);
    } catch (e) {
      console.error('Failed to delete property:', e);
    }
  }

  function handleBooleanChange(defId, checked) {
    setValue(defId, checked ? 'true' : 'false');
  }
</script>

{#if folderId && (defs.length > 0 || !loading)}
<div class="note-properties">
  <button class="props-toggle" aria-expanded={open} onclick={() => (open = !open)}>
    <span class="props-toggle-icon">{open ? '˅' : '›'}</span>
    <span class="props-toggle-label">Properties</span>
    {#if !open && defs.length > 0}
      <span class="props-count">{defs.length}</span>
    {/if}
  </button>

  {#if open}
    <div class="props-grid">
      {#each propValues as prop (prop.def_id)}
        <div class="prop-row">
          <span class="prop-name">{prop.name}</span>
          <div class="prop-value">
            {#if prop.type === 'text'}
              <input
                type="text"
                value={prop.value}
                onblur={(e) => setValue(prop.def_id, e.currentTarget.value)}
                onkeydown={(e) => { if (e.key === 'Enter') e.currentTarget.blur(); }}
                class="prop-input"
                aria-label={prop.name}
                placeholder="—"
              />
            {:else if prop.type === 'number'}
              <input
                type="number"
                value={prop.value}
                onblur={(e) => setValue(prop.def_id, e.currentTarget.value)}
                onkeydown={(e) => { if (e.key === 'Enter') e.currentTarget.blur(); }}
                class="prop-input"
                placeholder="—"
              />
            {:else if prop.type === 'date'}
              <input
                type="date"
                value={prop.value}
                onchange={(e) => setValue(prop.def_id, e.currentTarget.value)}
                class="prop-input"
                aria-label={prop.name}
              />
            {:else if prop.type === 'boolean'}
              <label class="prop-checkbox" for="prop-{prop.def_id}">
                <input
                  id="prop-{prop.def_id}"
                  type="checkbox"
                  checked={prop.value === 'true'}
                  onchange={(e) => handleBooleanChange(prop.def_id, e.currentTarget.checked)}
                  aria-label={prop.name}
                />
              </label>
            {:else if prop.type === 'select'}
              <select
                class="prop-input"
                value={prop.value}
                aria-label={prop.name}
                onchange={(e) => setValue(prop.def_id, e.currentTarget.value)}
              >
                <option value="">—</option>
                {#each parseSelectOptions(prop.options) as opt}
                  <option value={opt}>{opt}</option>
                {/each}
              </select>
            {/if}
          </div>
          <button class="prop-delete icon-btn danger" onclick={() => deleteDef(prop.def_id)} title="Remove property" aria-label="Remove property {prop.name}">✕</button>
        </div>
      {/each}
    </div>

    {#if adding}
      <div class="prop-add-form">
        <input
          class="prop-input"
          bind:value={newName}
          placeholder="Property name"
          aria-label="New property name"
          onkeydown={(e) => { if (e.key === 'Enter') addProperty(); if (e.key === 'Escape') adding = false; }}
        />
        <select class="prop-input" bind:value={newType} aria-label="Property type">
          <option value="text">Text</option>
          <option value="number">Number</option>
          <option value="date">Date</option>
          <option value="boolean">Checkbox</option>
          <option value="select">Select</option>
        </select>
        {#if newType === 'select'}
          <input
            class="prop-input"
            bind:value={newOptions}
            placeholder="Options (comma-separated)"
          />
        {/if}
        <button class="prop-add-btn" onclick={addProperty}>Add</button>
        <button class="prop-cancel-btn" onclick={() => (adding = false)}>Cancel</button>
      </div>
    {:else}
      <button class="prop-add-trigger" onclick={() => (adding = true)}>+ Add property</button>
    {/if}
  {/if}
</div>
{/if}

<div class="note-properties">
  <button class="props-toggle" aria-expanded={historyOpen} onclick={() => (historyOpen = !historyOpen)}>
    <span class="props-toggle-icon">{historyOpen ? '˅' : '›'}</span>
    <span class="props-toggle-label">History</span>
    {#if !historyOpen && versions.length > 0}
      <span class="props-count">{versions.length}</span>
    {/if}
  </button>

  {#if historyOpen}
    <div class="history-layout">
      <div class="history-list">
        {#if versions.length === 0}
          <div class="history-empty">No saved revisions yet.</div>
        {:else}
          {#each versions as version (version.id)}
            <button
              type="button"
              class="history-version-btn"
              class:active={version.id === selectedVersionId}
              onclick={() => selectVersion(version.id)}
            >
              <span class="history-version-time">{new Date(version.created_at * 1000).toLocaleString()}</span>
              {#if version.preview_title || version.preview_body}
                <span class="history-version-preview">
                  {#if version.preview_title}<span class="history-preview-title">{version.preview_title}</span>{/if}
                  {#if version.preview_title && version.preview_body}<span class="history-preview-sep"> — </span>{/if}
                  {#if version.preview_body}<span class="history-preview-body">{version.preview_body}</span>{/if}
                </span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>

      <div class="history-preview">
        {#if versionLoading}
          <div class="history-empty">Loading revision…</div>
        {:else if selectedVersion}
          <div class="history-actions">
            <button type="button" class="prop-add-btn" onclick={restoreSelectedVersion}>Restore selected version</button>
          </div>
          <div class="history-title-section" role="region" aria-label="Title comparison">
            {#if (selectedVersion.title ?? '') === (activeTitle ?? '')}
              <p class="history-title-same">Title unchanged.</p>
            {:else}
              <div class="history-title-grid">
                <span class="history-title-label">Revision</span>
                <span class="history-title-label">Current</span>
                <span class="history-title-cell history-title-rev">{selectedVersion.title ?? ''}</span>
                <span class="history-title-cell history-title-cur">{activeTitle ?? ''}</span>
              </div>
            {/if}
          </div>
          <div class="history-diff-wrap">
            <DiffView hunks={diffHunks} readonly sideBySide headerTitle="Body" />
          </div>
        {:else}
          <div class="history-empty">Select a revision to compare and restore.</div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .history-layout { display: grid; grid-template-columns: 240px 1fr; gap: 12px; margin-top: 8px; }
  .history-list { display: flex; flex-direction: column; gap: 6px; max-height: 220px; overflow: auto; }
  .history-version-btn {
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: flex-start;
  }
  .history-version-btn.active { outline: 1px solid var(--accent, #999); }
  .history-version-time { font-weight: 500; }
  .history-version-preview { font-size: 12px; opacity: 0.85; line-height: 1.3; word-break: break-word; }
  .history-preview-body { opacity: 0.9; }
  .history-preview { min-height: 120px; }
  .history-diff-wrap { margin-top: 8px; max-height: min(420px, 50vh); overflow: auto; }
  .history-empty { opacity: 0.75; font-size: 13px; }
  .history-actions { display: flex; justify-content: flex-end; }
  .history-title-section { margin-top: 8px; margin-bottom: 4px; }
  .history-title-same {
    margin: 0;
    font-size: 12px;
    opacity: 0.75;
  }
  .history-title-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px 10px;
    font-size: 13px;
  }
  .history-title-label {
    font: 11px/1.2 var(--sans, system-ui);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.85;
    border-bottom: 1px solid var(--border, #444);
    padding-bottom: 4px;
  }
  .history-title-cell {
    word-break: break-word;
    padding: 4px 0;
    font-weight: 500;
    color: var(--text-h, inherit);
  }
</style>
