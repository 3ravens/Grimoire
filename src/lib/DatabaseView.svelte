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
   * DatabaseView — spreadsheet-style table view for a folder's notes + properties.
   * Shown when the user toggles "Table" on a folder that has property definitions.
   *
   * Props:
   *   folderId   — the folder to display
   *   onOpenNote — callback(noteId) to open a note in the editor
   */
  import { invoke } from '@tauri-apps/api/core';

  let { folderId, onOpenNote = () => {}, onFiltersChange = () => {} } = $props();

  let defs = $state([]);
  let rows = $state([]);
  let loading = $state(false);

  // ── Filter state ───────────────────────────────────────────────────────────
  // filters: { [defId]: { op: string, value: string | string[] } }
  let filters = $state({});
  // Pending edits to filter values before Apply is clicked.
  let pending = $state({});

  // Rows after applying active filters.
  let filteredRows = $derived(applyFilters(rows, filters, defs));

  // ── Load ───────────────────────────────────────────────────────────────────
  $effect(() => {
    if (folderId) {
      loadData(folderId);
    } else {
      defs = [];
      rows = [];
      filters = {};
      pending = {};
    }
  });

  async function loadData(fid) {
    loading = true;
    try {
      const [d, r] = await Promise.all([
        invoke('get_property_defs', { folderId: fid }),
        invoke('list_notes_with_properties', { folderId: fid }),
      ]);
      defs = d;
      rows = r;
      // Restore persisted filters for this folder.
      const saved = localStorage.getItem(`db-filters-${fid}`);
      if (saved) {
        try {
          filters = JSON.parse(saved);
          pending = JSON.parse(saved);
        } catch {
          filters = {};
          pending = {};
        }
      } else {
        filters = {};
        pending = {};
      }
      onFiltersChange(filters);
    } catch {
      defs = [];
      rows = [];
      onFiltersChange({});
    } finally {
      loading = false;
    }
  }

  // ── Filter logic ───────────────────────────────────────────────────────────
  const DEFAULT_OP = {
    text:    'contains',
    number:  '=',
    date:    'on',
    boolean: 'is true',
    select:  'any of',
  };

  const OPS = {
    text:    ['contains', 'equals', 'is empty', 'is not empty'],
    number:  ['=', '≠', '>', '<', '≥', '≤'],
    date:    ['on', 'before', 'after', 'between'],
    boolean: ['is true', 'is false'],
    select:  ['any of', 'none of'],
  };

  function defaultValue(type, op) {
    if (type === 'boolean') return '';
    if (type === 'select')  return [];
    if (type === 'date' && op === 'between') return ['', ''];
    if (op === 'is empty' || op === 'is not empty') return '';
    return '';
  }

  function applyFilters(rows, filters, defs) {
    const active = Object.entries(filters).filter(([, f]) => f && !isFilterEmpty(f));
    if (active.length === 0) return rows;

    return rows.filter(note => {
      return active.every(([defId, f]) => {
        const def = defs.find(d => d.id === Number(defId));
        if (!def) return true;
        const prop = note.properties.find(p => p.def_id === Number(defId));
        const val = prop?.value ?? null;

        if (def.type === 'text') {
          if (f.op === 'is empty')     return val === null || val === '';
          if (f.op === 'is not empty') return val !== null && val !== '';
          if (val === null) return false;
          if (f.op === 'contains') return val.toLowerCase().includes(f.value.toLowerCase());
          if (f.op === 'equals')   return val.toLowerCase() === f.value.toLowerCase();
        }

        if (def.type === 'number') {
          if (val === null || val === '') return false;
          const n = parseFloat(val);
          const v = parseFloat(f.value);
          if (isNaN(n) || isNaN(v)) return false;
          if (f.op === '=')  return n === v;
          if (f.op === '≠')  return n !== v;
          if (f.op === '>')  return n > v;
          if (f.op === '<')  return n < v;
          if (f.op === '≥')  return n >= v;
          if (f.op === '≤')  return n <= v;
        }

        if (def.type === 'date') {
          if (val === null || val === '') return false;
          if (f.op === 'on')     return val === f.value;
          if (f.op === 'before') return val < f.value;
          if (f.op === 'after')  return val > f.value;
          if (f.op === 'between') {
            const [from, to] = Array.isArray(f.value) ? f.value : ['', ''];
            return val >= from && val <= to;
          }
        }

        if (def.type === 'boolean') {
          const b = val === 'true';
          if (f.op === 'is true')  return b === true;
          if (f.op === 'is false') return b === false;
        }

        if (def.type === 'select') {
          const selected = Array.isArray(f.value) ? f.value : [];
          if (selected.length === 0) return true;
          if (f.op === 'any of') return selected.includes(val ?? '');
          if (f.op === 'none of') return !selected.includes(val ?? '');
        }

        return true;
      });
    });
  }

  function isFilterEmpty(f) {
    if (!f || !f.op) return true;
    const { op, value } = f;
    if (op === 'is empty' || op === 'is not empty' || op === 'is true' || op === 'is false') return false;
    if (Array.isArray(value)) return value.every(v => v === '');
    return value === '';
  }

  function activeFilterCount() {
    return Object.values(filters).filter(f => f && !isFilterEmpty(f)).length;
  }

  // ── Filter bar actions ─────────────────────────────────────────────────────
  let addFilterOpen = $state(false);

  function addFilter(def) {
    addFilterOpen = false;
    const op = DEFAULT_OP[def.type] ?? 'contains';
    const entry = { op, value: defaultValue(def.type, op) };
    pending = { ...pending, [def.id]: entry };
  }

  function removeFilter(defId) {
    const p = { ...pending };
    delete p[defId];
    const f = { ...filters };
    delete f[defId];
    filters = f;
    pending = p;
    localStorage.setItem(`db-filters-${folderId}`, JSON.stringify(f));
    onFiltersChange(f);
  }

  function clearFilters() {
    filters = {};
    pending = {};
    localStorage.removeItem(`db-filters-${folderId}`);
    onFiltersChange({});
  }

  function applyPending() {
    filters = { ...pending };
    localStorage.setItem(`db-filters-${folderId}`, JSON.stringify(filters));
    onFiltersChange(filters);
  }

  function onOpChange(defId, op, def) {
    const cur = pending[defId] ?? {};
    pending = { ...pending, [defId]: { op, value: defaultValue(def.type, op) } };
  }

  function onValueChange(defId, value) {
    const cur = pending[defId] ?? {};
    pending = { ...pending, [defId]: { ...cur, value } };
  }

  function onSelectToggle(defId, opt) {
    const cur = pending[defId] ?? {};
    const prev = Array.isArray(cur.value) ? cur.value : [];
    const next = prev.includes(opt) ? prev.filter(v => v !== opt) : [...prev, opt];
    pending = { ...pending, [defId]: { ...cur, value: next } };
  }

  function onDateRangeChange(defId, idx, val) {
    const cur = pending[defId] ?? {};
    const prev = Array.isArray(cur.value) ? [...cur.value] : ['', ''];
    prev[idx] = val;
    pending = { ...pending, [defId]: { ...cur, value: prev } };
  }

  // Defs that don't already have a pending/active filter.
  function availableDefs() {
    return defs.filter(d => !(d.id in pending));
  }

  // ── Template sync ──────────────────────────────────────────────────────────
  let templates = $state([]);
  let syncOpen = $state(false);
  let syncing = $state(false);

  // Load user templates once on mount.
  $effect(() => {
    invoke('list_templates').then(ts => {
      templates = ts.filter(t => !t.builtin);
    }).catch(() => {});
  });

  async function syncFromTemplate(templateId) {
    syncOpen = false;
    syncing = true;
    try {
      await invoke('apply_template_to_folder', { templateId, folderId });
      await loadData(folderId);
    } catch { /* non-fatal */ }
    syncing = false;
  }

  // ── Cell editing (unchanged) ───────────────────────────────────────────────
  function getPropValue(note, defId) {
    const p = note.properties.find(pr => pr.def_id === defId);
    if (!p) return null;
    return p.value ?? null;
  }

  async function updateValue(noteId, defId, value) {
    try {
      await invoke('set_note_property', { noteId, defId, value });
      rows = rows.map(r => {
        if (r.id !== noteId) return r;
        return {
          ...r,
          properties: r.properties.map(p =>
            p.def_id === defId ? { ...p, value } : p
          ),
        };
      });
    } catch { /* non-fatal */ }
  }

  function handleCellBlur(noteId, defId, e) {
    updateValue(noteId, defId, e.currentTarget.value);
  }

  function handleCellKeydown(e) {
    if (e.key === 'Enter') e.currentTarget.blur();
  }

  function handleBoolChange(noteId, defId, e) {
    updateValue(noteId, defId, e.currentTarget.checked ? 'true' : 'false');
  }

  function handleSelectChange(noteId, defId, e) {
    updateValue(noteId, defId, e.currentTarget.value);
  }
</script>

<div class="db-view">
  {#if loading}
    <p class="db-loading">Loading…</p>
  {:else}

    <!-- ── Filter bar ──────────────────────────────────────────────────────── -->
    {#if defs.length > 0}
      <div class="db-filter-bar">
        <div class="db-filter-chips">
          {#each Object.entries(pending) as [defIdStr] (defIdStr)}
            {@const defId = Number(defIdStr)}
            {@const def = defs.find(d => d.id === defId)}
            {@const f = pending[defIdStr]}
            {#if def && f}
              <div class="db-filter-chip">
                <span class="db-filter-chip-name">{def.name}</span>

                <!-- Op selector -->
                <select
                  class="db-filter-op"
                  value={f.op}
                  onchange={e => onOpChange(defId, e.currentTarget.value, def)}
                >
                  {#each OPS[def.type] ?? [] as op}
                    <option value={op}>{op}</option>
                  {/each}
                </select>

                <!-- Value input — varies by type and op -->
                {#if def.type === 'boolean' || f.op === 'is empty' || f.op === 'is not empty'}
                  <!-- No value input needed -->
                {:else if def.type === 'select'}
                  <div class="db-filter-select-opts">
                    {#each JSON.parse(def.options || '[]') as opt}
                      {@const checked = Array.isArray(f.value) && f.value.includes(opt)}
                      <label class="db-filter-select-opt">
                        <input
                          type="checkbox"
                          {checked}
                          onchange={() => onSelectToggle(defId, opt)}
                        />
                        {opt}
                      </label>
                    {/each}
                  </div>
                {:else if def.type === 'date' && f.op === 'between'}
                  <input
                    type="date"
                    class="db-filter-input"
                    value={Array.isArray(f.value) ? f.value[0] : ''}
                    oninput={e => onDateRangeChange(defId, 0, e.currentTarget.value)}
                  />
                  <span class="db-filter-between-sep">–</span>
                  <input
                    type="date"
                    class="db-filter-input"
                    value={Array.isArray(f.value) ? f.value[1] : ''}
                    oninput={e => onDateRangeChange(defId, 1, e.currentTarget.value)}
                  />
                {:else if def.type === 'date'}
                  <input
                    type="date"
                    class="db-filter-input"
                    value={typeof f.value === 'string' ? f.value : ''}
                    oninput={e => onValueChange(defId, e.currentTarget.value)}
                  />
                {:else if def.type === 'number'}
                  <input
                    type="number"
                    class="db-filter-input db-filter-input--narrow"
                    value={typeof f.value === 'string' ? f.value : ''}
                    oninput={e => onValueChange(defId, e.currentTarget.value)}
                  />
                {:else}
                  <input
                    type="text"
                    class="db-filter-input"
                    value={typeof f.value === 'string' ? f.value : ''}
                    oninput={e => onValueChange(defId, e.currentTarget.value)}
                    placeholder="value"
                  />
                {/if}

                <button class="db-filter-remove" onclick={() => removeFilter(defId)} title="Remove filter">×</button>
              </div>
            {/if}
          {/each}
        </div>

        <div class="db-filter-actions">
          <!-- Add filter dropdown -->
          <div class="db-filter-add-wrap">
            <button
              class="db-filter-add-btn"
              onclick={() => addFilterOpen = !addFilterOpen}
              disabled={availableDefs().length === 0}
            >+ Filter</button>
            {#if addFilterOpen}
              <div class="db-filter-add-menu">
                {#each availableDefs() as def}
                  <button class="db-filter-add-opt" onclick={() => addFilter(def)}>{def.name}</button>
                {/each}
              </div>
            {/if}
          </div>

          {#if Object.keys(pending).length > 0}
            <button class="db-filter-apply-btn" onclick={applyPending}>Apply</button>
          {/if}
          {#if activeFilterCount() > 0}
            <button class="db-filter-clear-btn" onclick={clearFilters}>Clear all</button>
          {/if}

          {#if activeFilterCount() > 0}
            <span class="db-filter-count">{filteredRows.length} of {rows.length}</span>
          {/if}

          <!-- Sync from template -->
          {#if templates.length > 0}
            <div class="db-filter-add-wrap db-sync-wrap">
              <button
                class="db-filter-add-btn db-sync-btn"
                onclick={() => syncOpen = !syncOpen}
                disabled={syncing}
                title="Apply a template's current properties to all notes in this folder"
              >{syncing ? 'Syncing…' : 'Sync from template'}</button>
              {#if syncOpen}
                <div class="db-filter-add-menu">
                  {#each templates as t}
                    <button class="db-filter-add-opt" onclick={() => syncFromTemplate(t.id)}>{t.name}</button>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    {/if}

    <!-- ── Table ──────────────────────────────────────────────────────────── -->
    {#if rows.length === 0}
      <p class="db-empty">No notes in this folder.</p>
    {:else if filteredRows.length === 0}
      <p class="db-empty">No notes match the active filters.</p>
    {:else}
      <div class="db-table-wrap">
        <table class="db-table">
          <thead>
            <tr>
              <th class="db-th-title">Title</th>
              {#each defs as def (def.id)}
                <th>{def.name}</th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each filteredRows as note (note.id)}
              <tr>
                <td class="db-td-title">
                  <button class="db-note-link" onclick={() => onOpenNote(note.id)}>
                    {note.title}
                  </button>
                </td>
                {#each defs as def (def.id)}
                  {@const val = getPropValue(note, def.id)}
                  <td class="db-td-value">
                    {#if val === null}
                      <span class="db-cell-empty">—</span>
                    {:else if def.type === 'boolean'}
                      <input
                        type="checkbox"
                        checked={val === 'true'}
                        onchange={(e) => handleBoolChange(note.id, def.id, e)}
                      />
                    {:else if def.type === 'select'}
                      <select
                        class="db-cell-input"
                        value={val}
                        onchange={(e) => handleSelectChange(note.id, def.id, e)}
                      >
                        <option value="">—</option>
                        {#each JSON.parse(def.options || '[]') as opt}
                          <option value={opt}>{opt}</option>
                        {/each}
                      </select>
                    {:else if def.type === 'date'}
                      <input
                        type="date"
                        class="db-cell-input"
                        value={val}
                        onchange={(e) => handleCellBlur(note.id, def.id, e)}
                      />
                    {:else if def.type === 'number'}
                      <input
                        type="number"
                        class="db-cell-input"
                        value={val}
                        onblur={(e) => handleCellBlur(note.id, def.id, e)}
                        onkeydown={handleCellKeydown}
                      />
                    {:else}
                      <input
                        type="text"
                        class="db-cell-input"
                        value={val}
                        onblur={(e) => handleCellBlur(note.id, def.id, e)}
                        onkeydown={handleCellKeydown}
                        placeholder="—"
                      />
                    {/if}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</div>
