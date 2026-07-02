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
   * Kanban — board view for a folder's notes grouped by a select property.
   *
   * Props:
   *   folderId   — the folder whose notes and property defs to display
   *   onOpenNote — callback(noteId) to open a note in the editor
   */
  import { tick, getContext } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  let { folderId, onOpenNote = () => {}, onDeleteNote = () => {} } = $props();

  const ns = getContext('ns');

  // ── Data ───────────────────────────────────────────────────────────────────

  let defs     = $state([]);
  let notes    = $state([]);
  let loading  = $state(false);
  let errorMsg = $state('');

  const selectDefs = $derived(defs.filter(d => d.type === 'select'));

  // ── Persisted toolbar state ────────────────────────────────────────────────

  let groupByDefId  = $state(null);       // id of the select def to group by
  let visibleDefIds = $state(new Set());  // ids of other defs to show on cards

  // Per-column note ordering: { [colKey]: noteId[] }
  // Allows manual reordering within a column without touching the DB.
  let columnOrders = $state({});

  // ── Load ───────────────────────────────────────────────────────────────────

  /** Last folder we ran a full `loadData` for (vs. lightweight `refreshNotes`). */
  let loadedFolderId = $state(/** @type {number | null} */ (null));

  $effect(() => {
    if (folderId == null) {
      loadedFolderId = null;
      defs = [];
      notes = [];
      return;
    }
    if (loadedFolderId !== folderId) {
      loadedFolderId = folderId;
      loadData(folderId);
      return;
    }
    // Same folder: sidebar note list changed (e.g. delete) — refresh board rows.
    ns.notes;
    void refreshNotes(folderId);
  });

  async function loadData(fid) {
    loading  = true;
    errorMsg = '';
    try {
      const [d, n] = await Promise.all([
        invoke('get_property_defs',          { folderId: fid }),
        invoke('list_notes_with_properties', { folderId: fid }),
      ]);
      defs  = d;
      notes = n;

      // Restore group-by.
      const savedGroup = localStorage.getItem(`grimoire:kanban:${fid}:groupBy`);
      const selDefs = d.filter(def => def.type === 'select');
      if (savedGroup && selDefs.find(def => String(def.id) === savedGroup)) {
        groupByDefId = Number(savedGroup);
      } else if (selDefs.length > 0) {
        groupByDefId = selDefs[0].id;
      } else {
        groupByDefId = null;
      }

      // Restore visible fields.
      const savedVisible = localStorage.getItem(`grimoire:kanban:${fid}:visibleDefs`);
      if (savedVisible) {
        try { visibleDefIds = new Set(JSON.parse(savedVisible)); } catch { visibleDefIds = new Set(); }
      } else {
        visibleDefIds = new Set();
      }

      // Restore column orders.
      const savedOrders = localStorage.getItem(`grimoire:kanban:${fid}:orders`);
      if (savedOrders) {
        try { columnOrders = JSON.parse(savedOrders); } catch { columnOrders = {}; }
      } else {
        columnOrders = {};
      }
    } catch (e) {
      errorMsg = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  async function refreshNotes(fid) {
    try {
      const n = await invoke('list_notes_with_properties', { folderId: fid });
      if (fid !== folderId) return;
      notes = n;
    } catch (e) {
      if (fid === folderId) errorMsg = e?.message ?? String(e);
    }
  }

  // ── Toolbar helpers ────────────────────────────────────────────────────────

  function setGroupBy(defId) {
    groupByDefId = defId;
    localStorage.setItem(`grimoire:kanban:${folderId}:groupBy`, String(defId));
    if (visibleDefIds.has(defId)) {
      visibleDefIds = new Set([...visibleDefIds].filter(id => id !== defId));
      persistVisible();
    }
  }

  function toggleVisible(defId) {
    const next = new Set(visibleDefIds);
    if (next.has(defId)) { next.delete(defId); } else { next.add(defId); }
    visibleDefIds = next;
    persistVisible();
  }

  function persistVisible() {
    localStorage.setItem(`grimoire:kanban:${folderId}:visibleDefs`, JSON.stringify([...visibleDefIds]));
  }

  function persistOrders() {
    localStorage.setItem(`grimoire:kanban:${folderId}:orders`, JSON.stringify(columnOrders));
  }

  // ── Column derivation ──────────────────────────────────────────────────────

  const columns = $derived.by(() => {
    if (!groupByDefId) return [];
    const def = defs.find(d => d.id === groupByDefId);
    if (!def) return [];

    let options = [];
    try { options = JSON.parse(def.options ?? '[]'); } catch { options = []; }

    /** @type {Map<string, any[]>} */
    const groups = new Map();
    groups.set('__unset__', []);
    for (const opt of options) groups.set(opt, []);

    for (const note of notes) {
      const prop = note.properties.find(p => p.def_id === groupByDefId);
      const val  = prop?.value ?? '';
      const key  = val.trim() === '' ? '__unset__' : val;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(note);
    }

    // Apply stored manual order. Notes not in the stored list go to the end.
    function applyOrder(colKey, colNotes) {
      const order = columnOrders[colKey];
      if (!order || order.length === 0) return colNotes;
      const known = new Set(order);
      return [
        ...order.filter(id => colNotes.find(n => n.id === id)).map(id => colNotes.find(n => n.id === id)),
        ...colNotes.filter(n => !known.has(n.id)),
      ];
    }

    return [
      { key: '__unset__', label: 'Unset', notes: applyOrder('__unset__', groups.get('__unset__') ?? []) },
      ...options.map(opt => ({ key: opt, label: opt, notes: applyOrder(opt, groups.get(opt) ?? []) })),
    ];
  });

  // ── Drag and drop ──────────────────────────────────────────────────────────

  let dragSourceCol   = $state(null);  // column key the drag started from
  let dragOverCol     = $state(null);  // column key hovered (zone-level, no card)
  let dragOverCardId  = $state(null);  // note id of the card being hovered
  let dragInsertAbove = $state(true);  // insert above (true) or below (false) the hovered card

  function onCardDragStart(e, noteId, colKey) {
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('kanban-note-id', String(noteId));
    dragSourceCol = colKey;
  }

  function onCardDragOver(e, noteId) {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'move';
    dragOverCardId = noteId;
    dragOverCol    = null;
    const rect = e.currentTarget.getBoundingClientRect();
    dragInsertAbove = (e.clientY - rect.top) < rect.height / 2;
  }

  function onCardDragLeave(e) {
    if (!e.currentTarget.contains(/** @type {Node} */ (e.relatedTarget))) {
      dragOverCardId = null;
    }
  }

  async function onCardDrop(e, targetNoteId, colKey) {
    e.preventDefault();
    e.stopPropagation();
    const noteId    = Number(e.dataTransfer.getData('kanban-note-id'));
    const above     = dragInsertAbove;
    dragOverCardId  = null;
    dragOverCol     = null;
    if (!noteId || !groupByDefId) return;

    if (dragSourceCol === colKey) {
      reorderWithinColumn(colKey, noteId, targetNoteId, above);
    } else {
      await moveToColumn(noteId, colKey);
    }
    dragSourceCol = null;
  }

  function onColDragOver(e, colKey) {
    if (dragOverCardId != null) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    dragOverCol = colKey;
  }

  function onColDragLeave(e, colKey) {
    if (!e.currentTarget.contains(/** @type {Node} */ (e.relatedTarget))) {
      if (dragOverCol === colKey) dragOverCol = null;
    }
  }

  async function onColDrop(e, colKey) {
    e.preventDefault();
    const noteId = Number(e.dataTransfer.getData('kanban-note-id'));
    dragOverCol    = null;
    dragOverCardId = null;
    if (!noteId || !groupByDefId) return;
    if (dragSourceCol !== colKey) {
      await moveToColumn(noteId, colKey);
    }
    dragSourceCol = null;
  }

  function onBoardDragEnd() {
    dragSourceCol  = null;
    dragOverCol    = null;
    dragOverCardId = null;
  }

  async function moveToColumn(noteId, colKey) {
    const newValue = colKey === '__unset__' ? '' : colKey;
    try {
      await invoke('set_note_property', { noteId, defId: groupByDefId, value: newValue });
      await refreshNotes(folderId);
    } catch (e) {
      errorMsg = e?.message ?? String(e);
    }
  }

  function reorderWithinColumn(colKey, draggedId, targetId, insertAbove) {
    const col = columns.find(c => c.key === colKey);
    if (!col) return;
    const currentOrder = col.notes.map(n => n.id);
    const filtered = currentOrder.filter(id => id !== draggedId);
    let toIdx = filtered.indexOf(targetId);
    if (toIdx === -1) return;
    if (!insertAbove) toIdx += 1;
    filtered.splice(toIdx, 0, draggedId);
    columnOrders = { ...columnOrders, [colKey]: filtered };
    persistOrders();
  }

  // ── Keyboard move ────────────────────────────────────────────────────────

  let movingNoteId     = $state(null);   // note currently being keyboard-moved
  let moveAnnouncement = $state('');     // text announced to screen readers

  async function handleCardKeydown(e, note, col) {
    if (movingNoteId === note.id) {
      if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
        e.preventDefault();
        const colIdx = columns.findIndex(c => c.key === col.key);
        const newIdx = e.key === 'ArrowLeft' ? colIdx - 1 : colIdx + 1;
        if (newIdx < 0 || newIdx >= columns.length) {
          moveAnnouncement = `Already at the ${e.key === 'ArrowLeft' ? 'first' : 'last'} column.`;
          return;
        }
        const newCol = columns[newIdx];
        if (!groupByDefId) return;
        const newValue = newCol.key === '__unset__' ? '' : newCol.key;
        try {
          await invoke('set_note_property', { noteId: note.id, defId: groupByDefId, value: newValue });
          await refreshNotes(folderId);
          moveAnnouncement = `Moved to ${newCol.label}. Press arrow keys to move or reorder, Enter to confirm, Escape to cancel.`;
          await tick();
          /** @type {HTMLElement | null} */ (document.querySelector(`[data-note-id="${note.id}"] .kanban-card-title`))?.focus();
        } catch (err) {
          errorMsg = String(err);
        }
      } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault();
        const noteIds = col.notes.map(n => n.id);
        const idx = noteIds.indexOf(note.id);
        if (idx === -1) return;
        const targetIdx = e.key === 'ArrowUp' ? idx - 1 : idx + 1;
        if (targetIdx < 0 || targetIdx >= noteIds.length) {
          moveAnnouncement = `Already at the ${e.key === 'ArrowUp' ? 'top' : 'bottom'} of ${col.label}.`;
          return;
        }
        const targetId = noteIds[targetIdx];
        reorderWithinColumn(col.key, note.id, targetId, e.key === 'ArrowUp');
        moveAnnouncement = `Reordered within ${col.label}. Press arrow keys to move or reorder, Enter to confirm, Escape to cancel.`;
        await tick();
        /** @type {HTMLElement | null} */ (document.querySelector(`[data-note-id="${note.id}"] .kanban-card-title`))?.focus();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        movingNoteId = null;
        moveAnnouncement = 'Note placed.';
      } else if (e.key === 'Escape') {
        e.preventDefault();
        movingNoteId = null;
        moveAnnouncement = 'Move cancelled.';
      }
    } else if (e.key === 'm' || e.key === 'M') {
      e.preventDefault();
      movingNoteId = note.id;
      moveAnnouncement = `Moving "${note.title}" from ${col.label}. Press arrow keys to move between columns or reorder within a column, Enter to confirm, Escape to cancel.`;
    }
  }

  // ── Create note in column (inline) ────────────────────────────────────────

  let creating = $state(null);   // colKey currently being created in
  let creatingTitle = $state(''); // title typed in the inline input
  let creatingInput = $state(null); // input element ref

  function startCreating(colKey) {
    creating = colKey;
    creatingTitle = '';
    tick().then(() => creatingInput?.focus());
  }

  function cancelCreating() {
    creating = null;
    creatingTitle = '';
  }

  async function submitCreating(colKey) {
    const title = creatingTitle.trim();
    if (!title) { cancelCreating(); return; }
    creating = null;
    creatingTitle = '';
    try {
      const note = await invoke('create_note', { title, folderId });
      if (colKey !== '__unset__' && groupByDefId) {
        await invoke('set_note_property', { noteId: note.id, defId: groupByDefId, value: colKey });
      }
      try {
        await invoke('index_note', {
          noteId: note.id,
          title: note.title,
          content: note.content ?? '',
        });
      } catch {
        /* non-fatal — note exists; user can re-index */
      }
      await refreshNotes(folderId);
    } catch (e) {
      errorMsg = e?.message ?? String(e);
    }
  }

  function onInputKeydown(e, colKey) {
    if (e.key === 'Enter') {
      e.preventDefault();
      submitCreating(colKey);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancelCreating();
    }
  }
</script>

<div class="kanban-container">

  <!-- Screen-reader announcement region for keyboard move actions -->
  <div class="sr-only" aria-live="assertive" aria-atomic="true">{moveAnnouncement}</div>

  <!-- ── Toolbar ───────────────────────────────────────────────────────── -->
  <div class="kanban-toolbar">
    {#if selectDefs.length > 0}
      <label class="kanban-toolbar-label" for="kanban-groupby">Group by</label>
      <select
        id="kanban-groupby"
        class="kanban-select"
        value={groupByDefId}
        onchange={(e) => setGroupBy(Number(e.currentTarget.value))}
      >
        {#each selectDefs as def (def.id)}
          <option value={def.id}>{def.name}</option>
        {/each}
      </select>

      {#if defs.filter(d => d.id !== groupByDefId).length > 0}
        <details class="kanban-fields-picker">
          <summary class="kanban-fields-btn" aria-label="Show optional card fields">Show fields</summary>
          <div class="kanban-fields-menu">
            {#each defs.filter(d => d.id !== groupByDefId) as def (def.id)}
              <label class="kanban-fields-row">
                <input
                  type="checkbox"
                  checked={visibleDefIds.has(def.id)}
                  onchange={() => toggleVisible(def.id)}
                />
                {def.name}
              </label>
            {/each}
          </div>
        </details>
      {/if}
    {/if}
  </div>

  <!-- ── Board ─────────────────────────────────────────────────────────── -->
  {#if loading}
    <p class="kanban-status">Loading…</p>
  {:else if errorMsg}
    <p class="kanban-status kanban-error">{errorMsg}</p>
  {:else if selectDefs.length === 0}
    <div class="kanban-empty">
      <p>This folder has no <strong>select</strong>-type properties.</p>
      <p class="kanban-empty-hint">Open the table view for this folder and add a select property (e.g. "Status" with options Todo, In Progress, Done) to use the Kanban board.</p>
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="kanban-board" ondragend={onBoardDragEnd}>
      {#each columns as col (col.key)}
        <div class="kanban-col">
          <div class="kanban-col-header">
            <span class="kanban-col-title">{col.label}</span>
            <span class="kanban-col-count">{col.notes.length}</span>
          </div>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="kanban-col-drop"
            class:drag-over={dragOverCol === col.key && dragOverCardId === null}
            ondragover={(e) => onColDragOver(e, col.key)}
            ondragleave={(e) => onColDragLeave(e, col.key)}
            ondrop={(e) => onColDrop(e, col.key)}
          >
              {#each col.notes as note (note.id)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="kanban-card"
                class:drop-above={dragOverCardId === note.id && dragInsertAbove}
                class:drop-below={dragOverCardId === note.id && !dragInsertAbove}
                class:moving={movingNoteId === note.id}
                data-note-id={note.id}
                draggable={movingNoteId !== note.id}
                ondragstart={(e) => onCardDragStart(e, note.id, col.key)}
                ondragover={(e) => onCardDragOver(e, note.id)}
                ondragleave={onCardDragLeave}
                ondrop={(e) => onCardDrop(e, note.id, col.key)}
              >
                <div class="kanban-card-top">
                  <button
                    class="kanban-card-title"
                    aria-label="{note.title}{movingNoteId === note.id ? '. Moving. Press left or right to change column, Enter to confirm, Escape to cancel.' : '. Press M to move between columns.'}"
                    onclick={() => onOpenNote(note.id)}
                    onkeydown={(e) => handleCardKeydown(e, note, col)}
                  >{note.title}</button>
                  <button
                    class="kanban-card-delete-btn"
                    aria-label="Delete {note.title}"
                    onclick={() => onDeleteNote(note.id)}
                  >✕</button>
                </div>
                {#if visibleDefIds.size > 0}
                  <dl class="kanban-card-props">
                    {#each note.properties.filter(p => visibleDefIds.has(p.def_id) && (p.value ?? '').trim() !== '') as prop}
                      <div class="kanban-card-prop-row">
                        <dt class="kanban-card-prop-name">{prop.name}</dt>
                        <dd class="kanban-card-prop-value">{prop.value}</dd>
                      </div>
                    {/each}
                  </dl>
                {/if}
              </div>
            {:else}
              <p class="kanban-col-empty">No notes</p>
            {/each}

            {#if creating === col.key}
              <input
                class="kanban-inline-input"
                type="text"
                placeholder="Note title…"
                aria-label="New note title"
                bind:value={creatingTitle}
                bind:this={creatingInput}
                onkeydown={(e) => onInputKeydown(e, col.key)}
                onblur={() => { if (creatingTitle.trim() === '') cancelCreating(); else submitCreating(col.key); }}
              />
            {:else}
              <button
                class="kanban-add-btn"
                aria-label="New note in {col.label}"
                onclick={() => startCreating(col.key)}
              >+ New</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

</div>

