<script>
  import { getContext } from 'svelte';

  const ts = getContext('ts');

  const { onActivate, onClose, onRename, onNew } = $props();

  const tabs           = $derived(ts.tabs);
  const activeTabId    = $derived(ts.activeTabId);
  const renameRequestId = $derived(ts.externalRenameTabId);

  // Local state for the inline rename input.
  let editingTabId = $state(null);
  let renameValue  = $state('');

  $effect(() => {
    if (renameRequestId != null) startRename(renameRequestId);
  });

  function startRename(id) {
    const tab = tabs.find(t => t.id === id);
    if (!tab) return;
    editingTabId = id;
    renameValue  = tab.customLabel ?? tab.label;
  }

  function commitRename() {
    if (!editingTabId) return;
    onRename(editingTabId, renameValue.trim() || null);
    editingTabId = null;
  }

  function handleRenameKeydown(e) {
    if (e.key === 'Enter')  { e.preventDefault(); commitRename(); }
    if (e.key === 'Escape') { editingTabId = null; }
  }

  function handleTabListKeydown(e) {
    const tabBtn = /** @type {HTMLElement | null} */ (
      e.target instanceof Element ? e.target.closest('[role="tab"]') : null
    );
    if (!tabBtn) return;

    const tabId = Number(tabBtn.closest('[data-tab-id]')?.getAttribute('data-tab-id'));
    const idx = tabs.findIndex(t => t.id === tabId);
    if (idx === -1) return;

    if (e.key === 'ArrowRight') {
      e.preventDefault();
      const nextIdx = (idx + 1) % tabs.length;
      /** @type {HTMLElement | null} */
      const next = e.currentTarget instanceof Element
        ? e.currentTarget.querySelector(`[data-tab-id="${tabs[nextIdx].id}"] [role="tab"]`)
        : null;
      next?.focus();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      const prevIdx = (idx - 1 + tabs.length) % tabs.length;
      /** @type {HTMLElement | null} */
      const prev = e.currentTarget instanceof Element
        ? e.currentTarget.querySelector(`[data-tab-id="${tabs[prevIdx].id}"] [role="tab"]`)
        : null;
      prev?.focus();
    } else if (e.key === 'Delete') {
      e.preventDefault();
      onClose(tabId);
    }
  }
</script>

<!--
  The container itself carries data-tauri-drag-region so any gap between
  or after the tabs still drags the window.  Individual tab buttons are NOT
  drag regions — click events propagate normally.
-->
<div class="tabbar" data-tauri-drag-region>
  <!-- tab-list uses display:contents so flex layout is unaffected -->
  <div class="tab-list" role="tablist" aria-label="Open tabs" aria-orientation="horizontal" tabindex="-1" onkeydown={handleTabListKeydown}>
    {#each tabs as tab (tab.id)}
      <div
        class="tab"
        class:active={tab.id === activeTabId}
        role="presentation"
        data-tab-id={tab.id}
        onmousedown={(e) => { if (e.button === 1) { e.preventDefault(); onClose(tab.id); } }}
      >
        <button
          class="tab-label"
          role="tab"
          aria-selected={tab.id === activeTabId}
          tabindex={tab.id === activeTabId ? 0 : -1}
          onclick={() => onActivate(tab.id)}
          ondblclick={() => startRename(tab.id)}
          title={tab.customLabel ?? tab.label}
        >
          {#if editingTabId === tab.id}
            <input
              class="tab-rename"
              bind:value={renameValue}
              onblur={commitRename}
              onkeydown={handleRenameKeydown}
              onclick={(e) => e.stopPropagation()}
              aria-label="Rename tab"
            />
          {:else}
            {tab.customLabel ?? tab.label}
          {/if}
        </button>
        <button
          class="tab-close"
          onclick={(e) => { e.stopPropagation(); onClose(tab.id); }}
          title="Close tab"
          aria-label="Close {tab.customLabel ?? tab.label}"
          tabindex="-1"
        >✕</button>
      </div>
    {/each}
  </div>

  <!-- + button sits immediately after the last tab -->
  <button class="tab-new" onclick={onNew} title="New tab (Ctrl+T)" aria-label="New tab">+</button>

  <!-- Remaining space is drag region -->
  <div class="tabbar-fill" data-tauri-drag-region></div>
</div>
