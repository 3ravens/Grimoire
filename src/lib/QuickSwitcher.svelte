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
  import { focusTrap } from './utils/focusTrap.js';

  /**
   * @type {{
   *   onSelect: (note: object) => void,
   *   onSelectNewTab: (note: object) => void,
   *   onClose: () => void,
   * }}
   */
  let { onSelect, onSelectNewTab, onClose } = $props();

  let query = $state('');
  let allNotes = $state([]);
  let selectedIndex = $state(0);
  let inputEl = $state(null);

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return allNotes.slice(0, 20);
    return allNotes
      .filter(n => !n.locked && n.title.toLowerCase().includes(q))
      .slice(0, 20);
  });

  $effect(() => {
    invoke('list_notes', { all: true })
      .then(notes => { allNotes = notes; })
      .catch(() => {});
  });

  // Reset selected index whenever filter results change.
  $effect(() => {
    // reference filtered to track it
    filtered;
    selectedIndex = 0;
  });

  $effect(() => {
    if (inputEl) inputEl.focus();
  });

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (filtered[selectedIndex]) {
        if ((e.ctrlKey || e.metaKey) && onSelectNewTab) {
          onSelectNewTab(filtered[selectedIndex]);
        } else {
          onSelect(filtered[selectedIndex]);
        }
        onClose();
      }
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="qs-backdrop" onclick={onClose} role="dialog" aria-modal="true" aria-label="Quick Switcher" tabindex="-1">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="qs-panel" use:focusTrap onclick={(e) => e.stopPropagation()}>
    <input
      bind:this={inputEl}
      bind:value={query}
      class="qs-input"
      placeholder="Search notes…"
      autocomplete="off"
      spellcheck="false"
      onkeydown={handleKeydown}
      aria-label="Search notes"
      aria-autocomplete="list"
      aria-controls="qs-results"
    />
    <ul class="qs-results" id="qs-results" role="listbox">
      {#each filtered as note, i (note.id)}
        <li
          class="qs-item"
          class:selected={i === selectedIndex}
          role="option"
          aria-selected={i === selectedIndex}
        >
          <button
            class="qs-item-btn"
            onclick={(e) => { (e.ctrlKey || e.metaKey) && onSelectNewTab ? onSelectNewTab(note) : onSelect(note); onClose(); }}
            onmouseenter={() => { selectedIndex = i; }}
          >{note.title}</button>
        </li>
      {:else}
        <li class="qs-empty">No matching notes</li>
      {/each}
    </ul>
  </div>
</div>

<style>
  @import './styles/quick-switcher.css';
</style>
