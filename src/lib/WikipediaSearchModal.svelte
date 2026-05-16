<!-- Copyright (C) 2026 Wim Palland
This file is part of Grimoire — licensed under GPL-3.0 or later. -->

<script>
  import { invoke } from '@tauri-apps/api/core';
  import { focusTrap } from './utils/focusTrap.js';

  /**
   * @type {{
   *   onOpenArticle: (bundleId: string, articlePath: string, title: string) => void,
   *   onClose: () => void,
   * }}
   */
  let { onOpenArticle, onClose } = $props();

  let query          = $state('');
  let bundles        = $state([]);
  let selectedBundle = $state(null);
  let results        = $state([]);
  let errorMsg       = $state('');
  let searching      = $state(false);
  let selectedIndex  = $state(0);
  let inputEl        = $state(null);
  let debounce       = null;
  /** Ignore stale `suggest_wikipedia_articles` responses. */
  let suggestSeq     = 0;

  // Load installed bundles on mount.
  $effect(() => {
    invoke('list_wikipedia_bundles')
      .then(list => {
        bundles = list;
        if (list.length === 1) selectedBundle = list[0];
      })
      .catch(() => { errorMsg = 'Could not load Wikipedia bundles.'; });
  });

  $effect(() => {
    if (inputEl) inputEl.focus();
  });

  // Reset selection index whenever results change.
  $effect(() => {
    results;
    selectedIndex = 0;
  });

  function onInput() {
    clearTimeout(debounce);
    errorMsg = '';
    const q = query.trim();
    if (!q || !selectedBundle) { results = []; searching = false; return; }
    searching = true;
    const mySeq = ++suggestSeq;
    debounce = setTimeout(async () => {
      try {
        const next = await invoke('suggest_wikipedia_articles', {
          bundleId: selectedBundle.id,
          query: q,
        });
        if (mySeq !== suggestSeq) return;
        results = next;
        if (results.length === 0) errorMsg = 'No results found.';
      } catch (err) {
        if (mySeq !== suggestSeq) return;
        console.error('Wikipedia search failed:', err);
        results = [];
        errorMsg = typeof err === 'string' ? err : 'Search failed.';
      } finally {
        if (mySeq === suggestSeq) searching = false;
      }
    }, 250);
  }

  function selectResult(result) {
    onOpenArticle(selectedBundle.id, result.path, result.title);
    onClose();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, results.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (results[selectedIndex]) selectResult(results[selectedIndex]);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="wsm-backdrop" onclick={onClose} role="dialog" aria-modal="true" aria-label="Search Wikipedia" tabindex="-1">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="wsm-panel" use:focusTrap onclick={(e) => e.stopPropagation()}>

    {#if bundles.length === 0 && !errorMsg}
      <p class="wsm-status">Loading bundles…</p>
    {:else if bundles.length === 0 && errorMsg}
      <p class="wsm-status wsm-error">{errorMsg}</p>
    {:else}
      {#if bundles.length > 1}
        <div class="wsm-bundle-row">
          <label class="wsm-bundle-label" for="wsm-bundle-select">Bundle</label>
          <select
            id="wsm-bundle-select"
            class="wsm-bundle-select"
            value={selectedBundle?.id ?? ''}
            onchange={(e) => {
              const sel = /** @type {HTMLSelectElement} */ (e.target);
              selectedBundle = bundles.find(b => b.id === sel.value) ?? null;
              results = [];
              errorMsg = '';
              onInput();
            }}
          >
            <option value="" disabled>Select a bundle…</option>
            {#each bundles as b}
              <option value={b.id}>{b.title || b.name}</option>
            {/each}
          </select>
        </div>
      {:else if bundles.length === 1}
        <div class="wsm-bundle-hint">Searching: {bundles[0].title || bundles[0].name}</div>
      {/if}

      <input
        bind:this={inputEl}
        bind:value={query}
        class="wsm-input"
        placeholder="Search Wikipedia articles…"
        autocomplete="off"
        spellcheck="false"
        oninput={onInput}
        onkeydown={handleKeydown}
        aria-label="Search Wikipedia articles"
        aria-autocomplete="list"
        aria-controls="wsm-results"
        disabled={!selectedBundle}
      />

      {#if !selectedBundle && bundles.length > 1}
        <p class="wsm-status">Select a bundle above to search.</p>
      {:else if searching}
        <p class="wsm-status">Searching…</p>
      {:else if results.length > 0}
        <ul class="wsm-results" id="wsm-results" role="listbox">
          {#each results as result, i}
            <li
              class="wsm-item"
              class:selected={i === selectedIndex}
              role="option"
              aria-selected={i === selectedIndex}
            >
              <button
                class="wsm-item-btn"
                onclick={() => selectResult(result)}
                onmouseenter={() => { selectedIndex = i; }}
              >{result.title}</button>
            </li>
          {/each}
        </ul>
      {:else if errorMsg}
        <p class="wsm-status">{errorMsg}</p>
      {:else if query.trim()}
        <p class="wsm-status">Type to search…</p>
      {/if}
    {/if}
  </div>
</div>

<style>
  .wsm-backdrop {
    position: fixed;
    inset: 0;
    z-index: 300;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 120px;
  }

  .wsm-panel {
    width: 540px;
    max-width: calc(100vw - 40px);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .wsm-bundle-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px 0;
  }

  .wsm-bundle-label {
    font: 12px var(--sans);
    color: var(--text-muted);
    white-space: nowrap;
  }

  .wsm-bundle-select {
    flex: 1;
    background: var(--bg2, var(--bg));
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    font: 13px var(--sans);
    padding: 4px 8px;
    outline: none;
  }

  .wsm-bundle-hint {
    padding: 8px 16px 0;
    font: 12px var(--sans);
    color: var(--text-muted);
  }

  .wsm-input {
    width: 100%;
    padding: 12px 16px;
    background: var(--bg);
    border: none;
    border-bottom: 1px solid var(--border);
    color: var(--text-h);
    font: 14px var(--sans);
    outline: none;
    box-sizing: border-box;
  }

  .wsm-input::placeholder {
    color: var(--text-muted, var(--text));
    opacity: 0.5;
  }

  .wsm-input:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .wsm-results {
    list-style: none;
    margin: 0;
    padding: 4px 0;
    max-height: 320px;
    overflow-y: auto;
  }

  .wsm-item {
    display: flex;
  }

  .wsm-item-btn {
    flex: 1;
    background: none;
    border: none;
    padding: 8px 16px;
    text-align: left;
    color: var(--text);
    font: 13px var(--sans);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .wsm-item.selected .wsm-item-btn,
  .wsm-item-btn:hover {
    background: var(--accent-bg);
    color: var(--accent);
  }

  .wsm-status {
    padding: 10px 16px;
    margin: 0;
    color: var(--text-muted);
    font: 13px var(--sans);
  }

  .wsm-error {
    color: var(--danger);
  }
</style>
