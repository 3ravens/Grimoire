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

  // ── Props ────────────────────────────────────────────────────────────────
  /** @type {{ folders: Array<{id: number, name: string}>, open: boolean, onSelectNote: (id: number) => void }} */
  let { folders = [], open = false, onSelectNote } = $props();

  // ── State ────────────────────────────────────────────────────────────────
  let query = $state('');
  let results = $state([]);
  let loading = $state(false);         // FTS in-flight
  let semanticLoading = $state(false); // semantic refinement in-flight
  let errorMsg = $state('');

  // Reciprocal Rank Fusion constant
  const RRF_K = 60;
  function rrfScore(rank) { return 1 / (RRF_K + rank + 1); }

  // Input element ref for auto-focus
  let inputEl = $state(null);

  // ── Derived ──────────────────────────────────────────────────────────────
  /** Map folder id → name for quick lookup when rendering results. */
  let folderMap = $derived(
    Object.fromEntries(folders.map(f => [f.id, f.name]))
  );

  // ── Search logic ─────────────────────────────────────────────────────────

  let debounceTimer = null;

  function onInput() {
    clearTimeout(debounceTimer);
    errorMsg = '';
    if (!query.trim()) {
      results = [];
      loading = false;
      return;
    }
    loading = true;
    debounceTimer = setTimeout(runSearch, 200);
  }

  async function runSearch() {
    const current = query.trim();
    if (!current) return;

    // ── Phase 1: instant FTS ─────────────────────────────────────────────
    loading = true;
    semanticLoading = false;
    results = [];
    try {
      const ftsRes = await invoke('fts_search', { query: current, limit: 12 });
      if (query.trim() !== current) return; // stale
      results = ftsRes.map((r, i) => ({
        note_id: r.note_id,
        title: r.title,
        folder_id: r.folder_id,
        snippet: r.snippet,
        excerpt: null,
        matched_by: 'fts',
        score: rrfScore(i),
      }));
      loading = false;
    } catch (e) {
      if (query.trim() === current) { errorMsg = e?.message ?? String(e); loading = false; }
      return;
    }

    // ── Phase 2: semantic refinement (background) ────────────────────────
    semanticLoading = true;
    try {
      const semanticRes = await invoke('search_notes', { query: current });
      if (query.trim() !== current) return; // stale

      // JS-side RRF merge: promote notes that match both sources.
      const merged = new Map();
      for (const r of results) merged.set(r.note_id, { ...r });
      for (const [rank, m] of semanticRes.entries()) {
        const score = rrfScore(rank);
        if (merged.has(m.note_id)) {
          const entry = merged.get(m.note_id);
          entry.score += score;
          entry.matched_by = 'both';
          if (!entry.excerpt && m.excerpts.length > 0) entry.excerpt = m.excerpts[0];
        } else {
          merged.set(m.note_id, {
            note_id: m.note_id,
            title: m.title,
            folder_id: null,
            snippet: null,
            excerpt: m.excerpts[0] ?? null,
            matched_by: 'semantic',
            score,
          });
        }
      }
      results = [...merged.values()]
        .sort((a, b) => b.score - a.score)
        .slice(0, 12);
      semanticLoading = false;
    } catch (_e) {
      // Semantic failed silently — keep FTS results intact.
      if (query.trim() === current) semanticLoading = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      query = '';
      results = [];
    }
  }

  // Focus the input whenever the panel becomes visible.
  $effect(() => {
    if (open && inputEl) inputEl.focus();
  });
</script>

<div class="search-panel">
  <div class="search-bar">
    <svg class="search-icon" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
      <circle cx="8.5" cy="8.5" r="5.5"/>
      <line x1="12.5" y1="12.5" x2="17" y2="17"/>
    </svg>
    <input
      bind:this={inputEl}
      bind:value={query}
      oninput={onInput}
      onkeydown={handleKeydown}
      class="search-input"
      type="search"
      placeholder="Search notes…"
      aria-label="Search notes"
      autocomplete="off"
      spellcheck="false"
    />
    {#if query}
      <button class="clear-btn" onclick={() => { query = ''; results = []; }} aria-label="Clear search">✕</button>
    {/if}
  </div>

  <div class="search-body" role="region" aria-label="Search results" aria-live="polite" aria-atomic="true" aria-busy={loading}>
    {#if loading}
      <span class="status-msg">Searching…</span>
    {:else if errorMsg}
      <span class="status-msg error">{errorMsg}</span>
    {:else if query.trim() && results.length === 0}
      <span class="status-msg">No results for <em>{query}</em></span>
    {:else if !query.trim()}
      <span class="status-msg hint">Type to search your notes</span>
    {:else}
      {#if semanticLoading}
        <span class="status-msg refining">Refining with semantic search…</span>
      {/if}
      <ul class="result-list">
        {#each results as result (result.note_id)}
          <li>
            <button class="result-row" onclick={() => onSelectNote(result.note_id)}>
              <div class="result-header">
                <span class="result-title">{result.title}</span>
                <span class="match-badge" class:badge-both={result.matched_by === 'both'}>
                  {result.matched_by === 'both' ? 'FTS + Semantic' : result.matched_by === 'fts' ? 'FTS' : 'Semantic'}
                </span>
              </div>
              {#if result.folder_id && folderMap[result.folder_id]}
                <span class="result-folder">{folderMap[result.folder_id]}</span>
              {/if}
              {#if result.snippet}
                <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                <!-- snippet comes from SQLite's FTS5 snippet() — the only HTML it can
                     contain is <b>…</b> tags wrapping matched terms. No user content
                     can inject arbitrary HTML here because FTS5 escapes all other text. -->
                <p class="result-excerpt">{@html result.snippet}</p>
              {:else if result.excerpt}
                <p class="result-excerpt">{result.excerpt}</p>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  @import './styles/search.css';
</style>
