<script>
  import { getContext } from 'svelte';
  import { t } from './i18n/t.js';
  import { displayTabLabel } from './i18n/displayTabLabel.js';
  import { TAB_MIME, isTabDrag } from './utils/tabReorder.js';

  const ts = getContext('ts');
  const fs = getContext('fs');

  const { onActivate, onClose, onRename, onNew } = $props();

  const tabs           = $derived(ts.tabs);
  const activeTabId    = $derived(ts.activeTabId);
  const renameRequestId = $derived(ts.externalRenameTabId);

  // Local state for the inline rename input.
  let editingTabId = $state(null);
  let renameValue  = $state('');

  /** @type {string | null} */
  let dragTabId = $state(null);
  /** @type {string | null} */
  let dropTargetId = $state(null);
  let dropPlaceAfter = $state(false);
  let reorderLiveMessage = $state('');

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

  function tabLabel(tab) {
    const folderName =
      tab.folderId != null
        ? fs.folders.find((f) => f.id === tab.folderId)?.name
        : undefined;
    return displayTabLabel(tab, { folderName });
  }

  function clearDropState() {
    dragTabId = null;
    dropTargetId = null;
    dropPlaceAfter = false;
  }

  /**
   * @param {DragEvent} e
   * @param {string} tabId
   */
  function onTabDragStart(e, tabId) {
    if (editingTabId != null) {
      e.preventDefault();
      return;
    }
    const origin = /** @type {Element | null} */ (
      e.target instanceof Element ? e.target : null
    );
    if (origin?.closest('.tab-close, .tab-rename')) {
      e.preventDefault();
      return;
    }
    e.dataTransfer?.setData(TAB_MIME, tabId);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
    dragTabId = tabId;
  }

  /**
   * @param {DragEvent} e
   * @param {string} targetId
   */
  function onTabDragOver(e, targetId) {
    if (!isTabDrag(e.dataTransfer) || dragTabId == null || dragTabId === targetId) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    const el = /** @type {HTMLElement | null} */ (
      e.currentTarget instanceof HTMLElement ? e.currentTarget : null
    );
    if (!el) return;
    const rect = el.getBoundingClientRect();
    dropTargetId = targetId;
    dropPlaceAfter = e.clientX > rect.left + rect.width / 2;
  }

  /** @param {DragEvent} e */
  function onTabDragLeave(e) {
    const el = /** @type {HTMLElement | null} */ (
      e.currentTarget instanceof HTMLElement ? e.currentTarget : null
    );
    const related = /** @type {Node | null} */ (e.relatedTarget);
    if (el && related && el.contains(related)) return;
    if (dropTargetId != null) {
      dropTargetId = null;
      dropPlaceAfter = false;
    }
  }

  /**
   * @param {DragEvent} e
   * @param {string} targetId
   */
  function onTabDrop(e, targetId) {
    e.preventDefault();
    e.stopPropagation();
    const fromId = dragTabId ?? e.dataTransfer?.getData(TAB_MIME);
    const placeAfter = dropPlaceAfter;
    clearDropState();
    if (!fromId || fromId === targetId || editingTabId != null) return;
    const changed = ts.reorderTab(fromId, targetId, placeAfter);
    if (!changed) return;
    const moved = ts.tabs.find((t) => t.id === fromId);
    const idx = ts.tabs.findIndex((t) => t.id === fromId);
    if (moved && idx >= 0) {
      reorderLiveMessage = t('tabs.reorderedAnnounce', {
        label: tabLabel(moved),
        position: idx + 1,
      });
    }
  }

  function handleTabListKeydown(e) {
    const tabBtn = /** @type {HTMLElement | null} */ (
      e.target instanceof Element ? e.target.closest('[role="tab"]') : null
    );
    if (!tabBtn) return;

    const tabId = tabBtn.closest('[data-tab-id]')?.getAttribute('data-tab-id');
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
  <div class="sr-only" aria-live="polite" aria-atomic="true">{reorderLiveMessage}</div>
  <!-- tab-list uses display:contents so flex layout is unaffected -->
  <div class="tab-list" role="tablist" aria-label={t('tabs.openTabsAria')} aria-orientation="horizontal" tabindex="-1" onkeydown={handleTabListKeydown}>
    {#each tabs as tab (tab.id)}
      <div
        class="tab"
        class:active={tab.id === activeTabId}
        class:tab-dragging={dragTabId === tab.id}
        class:tab-drop-before={dropTargetId === tab.id && !dropPlaceAfter}
        class:tab-drop-after={dropTargetId === tab.id && dropPlaceAfter}
        role="presentation"
        data-tab-id={tab.id}
        draggable={editingTabId == null}
        ondragstart={(e) => onTabDragStart(e, tab.id)}
        ondragover={(e) => onTabDragOver(e, tab.id)}
        ondragleave={onTabDragLeave}
        ondrop={(e) => onTabDrop(e, tab.id)}
        ondragend={clearDropState}
        onmousedown={(e) => { if (e.button === 1) { e.preventDefault(); onClose(tab.id); } }}
      >
        <button
          class="tab-label"
          role="tab"
          aria-selected={tab.id === activeTabId}
          tabindex={tab.id === activeTabId ? 0 : -1}
          onclick={() => onActivate(tab.id)}
          ondblclick={() => startRename(tab.id)}
          title={tabLabel(tab)}
        >
          {#if editingTabId === tab.id}
            <input
              class="tab-rename"
              bind:value={renameValue}
              onblur={commitRename}
              onkeydown={handleRenameKeydown}
              onclick={(e) => e.stopPropagation()}
              aria-label={t('tabs.renameAria')}
            />
          {:else}
            {tabLabel(tab)}
          {/if}
        </button>
        <button
          class="tab-close"
          onclick={(e) => { e.stopPropagation(); onClose(tab.id); }}
          title={t('tabs.closeTitle')}
          aria-label={t('tabs.closeAria', { label: tabLabel(tab) })}
          tabindex="-1"
        >✕</button>
      </div>
    {/each}
  </div>

  <!-- + button sits immediately after the last tab -->
  <button class="tab-new" onclick={onNew} title={t('tabs.newTabShortcut')} aria-label={t('tabs.newTabAria')}>+</button>

  <!-- Remaining space is drag region -->
  <div class="tabbar-fill" data-tauri-drag-region></div>
</div>
