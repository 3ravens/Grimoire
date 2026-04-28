<!-- Copyright (C) 2026 Wim Palland — see App.svelte for license header. -->
<script>
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import { autofocus } from './utils/autofocus.js';
  import { buildFolderTree, isFolderDescendantOrSelf } from './utils/folderTree.js';

  let {
    // Data
    folders = [],
    notes = [],
    bookmarks = [],
    bookmarkedNoteIds,
    allTags = [],
    templates = [],
    selectedFolderId = null,
    tagFilter = null,
    unlockedFolderIds,
    isDragging = false,

    // Bindable state shared with parent
    inlineRenaming = $bindable(null),
    folderExpanded = $bindable({}),

    // Events
    onSelectFolder,
    onCreateNote,
    onImportNote,
    onCreateFolder,
    onDeleteFolder,
    onOpenNoteById,
    onFilterByTag,
    onClearTagFilter,
    onOpenNoteInNewTab,
    onRemoveBookmark,
    onRequestFolderUnlock,
    onSetFolderPassword,
    onNewTemplate,
    onEditTemplate,
    onDeleteTemplate,
    onConfirmInlineRename,
    onMoveNote,
    onMoveFolder,
  } = $props();

  // ── Local state ────────────────────────────────────────────────────────────

  let bookmarksOpen  = $state(true);
  let tagsOpen       = $state(false);
  let tagSearch      = $state('');
  let templatesOpen  = $state(false);
  let dragOverFolderId = $state(null);
  let draggingFolderId = $state(null);

  const TAG_LIMIT = 3;
  const folderTree = $derived(buildFolderTree(folders));
  const visibleTags = $derived(
    tagSearch.trim()
      ? allTags.filter(t => t.name.includes(tagSearch.trim().toLowerCase().replace(/^#/, '')))
      : allTags.slice(0, TAG_LIMIT)
  );

  // ── Folder expand/collapse ────────────────────────────────────────────────

  function toggleFolder(id) {
    folderExpanded = { ...folderExpanded, [id]: !(folderExpanded[id] ?? true) };
  }

  function expandAll() {
    const expanded = {};
    for (const f of folders) expanded[f.id] = true;
    folderExpanded = expanded;
  }

  function collapseAll() {
    const collapsed = {};
    for (const f of folders) collapsed[f.id] = false;
    folderExpanded = collapsed;
  }

  // ── Keyboard navigation ──────────────────────────────────────────────────

  function getFolderTreeButtons() {
    return /** @type {HTMLElement[]} */ (
      Array.from(document.querySelectorAll('.folder-list .folder-row .row-btn:not([disabled])'))
    );
  }

  async function handleFolderTreeKeydown(e) {
    if (!['ArrowDown', 'ArrowUp', 'ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(e.key)) return;
    e.preventDefault();

    const btns   = getFolderTreeButtons();
    const focused = /** @type {HTMLElement | null} */ (document.activeElement);
    const idx    = focused ? btns.indexOf(focused) : -1;

    if (e.key === 'ArrowDown') {
      btns[Math.min(idx + 1, btns.length - 1)]?.focus();
    } else if (e.key === 'ArrowUp') {
      btns[Math.max(idx - 1, 0)]?.focus();
    } else if (e.key === 'ArrowRight') {
      const row = focused?.closest('.folder-row');
      const expandBtn = /** @type {HTMLElement | null} */ (
        row?.querySelector('.folder-expand-btn[aria-expanded="false"]')
      );
      if (expandBtn) {
        expandBtn.click();
        await tick();
        const freshBtns = getFolderTreeButtons();
        freshBtns[Math.min(idx + 1, freshBtns.length - 1)]?.focus();
      }
    } else if (e.key === 'ArrowLeft') {
      const row = focused?.closest('.folder-row');
      const expandBtn = /** @type {HTMLElement | null} */ (
        row?.querySelector('.folder-expand-btn[aria-expanded="true"]')
      );
      if (expandBtn) {
        expandBtn.click();
      } else {
        const parentLi  = focused?.closest('.folder-children')?.parentElement;
        const parentBtn = /** @type {HTMLElement | null} */ (
          parentLi?.querySelector(':scope > .folder-row .row-btn')
        );
        if (parentBtn) parentBtn.focus();
      }
    } else if (e.key === 'Home') {
      btns[0]?.focus();
    } else if (e.key === 'End') {
      btns[btns.length - 1]?.focus();
    }
  }

  // ── Folder drag-to-reparent ──────────────────────────────────────────────

  function onFolderRowDragStart(e, folderId) {
    e.stopPropagation();
    e.dataTransfer.setData('folder-id', String(folderId));
    e.dataTransfer.effectAllowed = 'move';
    draggingFolderId = folderId;
  }

  function onFolderRowDragEnd() {
    draggingFolderId = null;
    dragOverFolderId = null;
  }

  function onFolderDropZoneDragOver(e, targetFolderId) {
    e.stopPropagation();
    if (e.dataTransfer.types.includes('folder-id')) {
      if (draggingFolderId && isFolderDescendantOrSelf(folders, targetFolderId, draggingFolderId)) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
    } else {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
    }
    dragOverFolderId = targetFolderId;
  }

  async function onFolderDropZoneDrop(e, targetFolderId) {
    e.stopPropagation();
    e.preventDefault();
    dragOverFolderId = null;
    if (e.dataTransfer.types.includes('folder-id')) {
      const movingId = Number(e.dataTransfer.getData('folder-id'));
      if (!movingId || isFolderDescendantOrSelf(folders, targetFolderId, movingId)) return;
      try {
        await invoke('move_folder', { id: movingId, newParentId: targetFolderId });
        folderExpanded = { ...folderExpanded, [targetFolderId]: true };
        onMoveFolder?.();
      } catch { /* non-fatal */ }
    } else {
      const noteId = Number(e.dataTransfer.getData('text/plain'));
      if (noteId) onMoveNote?.(noteId, targetFolderId);
    }
  }
</script>

<div class="panel-header">
  <h2>Folders</h2>
  <span class="panel-header-actions">
    <button class="icon-btn" onclick={expandAll} title="Expand all">
      <svg width="14" height="14" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="3,2.5 7.5,7 12,2.5"/>
        <polyline points="3,7.5 7.5,12 12,7.5"/>
      </svg>
    </button>
    <button class="icon-btn" onclick={collapseAll} title="Collapse all">
      <svg width="14" height="14" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="3,7 7.5,2.5 12,7"/>
        <polyline points="3,12 7.5,7.5 12,12"/>
      </svg>
    </button>
    <button class="icon-btn" data-action="create-note-btn" onclick={() => onCreateNote?.()} title="New note (right-click to pick template)">
      <svg width="14" height="14" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M11.5 1.5 L13.5 3.5 L5 12 L2 12.5 L2.5 9.5 Z"/>
        <line x1="9.5" y1="3.5" x2="11.5" y2="5.5"/>
      </svg>
    </button>
    <button class="icon-btn" onclick={() => onImportNote?.()} title="Import file as note (.txt, .md, or .pdf)">
      <svg width="14" height="14" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M2 4A1 1 0 0 1 3 3H8L10 5H12A1 1 0 0 1 13 6V11A1 1 0 0 1 12 12H3A1 1 0 0 1 2 11V4Z"/>
        <line x1="7.5" y1="7" x2="7.5" y2="10.5"/>
        <polyline points="5.5,8.5 7.5,10.5 9.5,8.5"/>
      </svg>
    </button>
    <button class="icon-btn" onclick={() => onCreateFolder?.()} title="New folder">
      <svg width="14" height="14" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M1 4A1 1 0 0 1 2 3H5L6.5 5H12A1 1 0 0 1 13 6V11A1 1 0 0 1 12 12H2A1 1 0 0 1 1 11V4Z"/>
        <line x1="9.5" y1="7.5" x2="9.5" y2="10.5"/>
        <line x1="8" y1="9" x2="11" y2="9"/>
      </svg>
    </button>
  </span>
</div>

<!-- Bookmarks -->
{#if bookmarks.length > 0}
  <section class="bookmarks-section">
    <div class="sidebar-section-label">
      <span>Bookmarks</span>
      <button class="collapse-btn" onclick={() => (bookmarksOpen = !bookmarksOpen)} title={bookmarksOpen ? 'Collapse' : 'Expand'}>
        {bookmarksOpen ? '˅' : '›'}
      </button>
    </div>
    {#if bookmarksOpen}
      <ul class="bookmark-list">
        {#each bookmarks as bm (bm.note_id)}
          <li class="bookmark-row" data-note-id={bm.note_id}>
            <button class="bookmark-name" onclick={() => onOpenNoteById?.(bm.note_id)} title={bm.title}>
              {bm.title}
            </button>
            <button class="bookmark-remove icon-btn" onclick={() => onRemoveBookmark?.(bm.note_id)} title="Remove bookmark">✕</button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<!-- Folder tree -->
<ul class="folder-list" role="tree" aria-label="Folders" onkeydown={handleFolderTreeKeydown}>
  <li class:active={selectedFolderId === 'all'} data-folder-id="all" role="treeitem" aria-selected={selectedFolderId === 'all'}>
    <div class="folder-row">
      <span class="folder-expand-spacer"></span>
      <button class="row-btn" onclick={() => onSelectFolder?.('all')}>All Notes</button>
    </div>
  </li>
  <li
    class:active={selectedFolderId === null}
    class:drag-over={dragOverFolderId === 'unfiled'}
    class:drag-active={isDragging || !!draggingFolderId}
    data-folder-id="unfiled"
    role="treeitem"
    aria-selected={selectedFolderId === null}
    ondragover={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; dragOverFolderId = 'unfiled'; }}
    ondragleave={(e) => { if (dragOverFolderId === 'unfiled' && !e.currentTarget.contains(/** @type {Node} */ (e.relatedTarget))) dragOverFolderId = null; }}
    ondrop={(e) => {
      e.preventDefault(); dragOverFolderId = null;
      if (e.dataTransfer.types.includes('folder-id')) {
        const fid = Number(e.dataTransfer.getData('folder-id'));
        if (fid) invoke('move_folder', { id: fid, newParentId: null }).then(() => onMoveFolder?.()).catch(() => {});
      } else {
        const noteId = Number(e.dataTransfer.getData('text/plain'));
        if (noteId) onMoveNote?.(noteId, null);
      }
    }}
  >
    <div class="folder-row">
      <span class="folder-expand-spacer"></span>
      <button class="row-btn" onclick={() => onSelectFolder?.(null)}>Unfiled</button>
    </div>
  </li>

  {#snippet renderFolder(node)}
    {@const folder = node.folder}
    {@const isExpanded = folderExpanded[folder.id] ?? true}
    <li
      class:active={selectedFolderId === folder.id}
      class:locked-row={folder.locked}
      class:drag-over={dragOverFolderId === folder.id}
      class:drag-active={(isDragging || !!draggingFolderId) && !folder.locked}
      data-folder-id={folder.id}
      role="treeitem"
      aria-selected={selectedFolderId === folder.id}
      aria-expanded={node.children.length > 0 ? (folderExpanded[folder.id] ?? true) : undefined}
      draggable={!folder.locked}
      ondragstart={(e) => !folder.locked && onFolderRowDragStart(e, folder.id)}
      ondragend={onFolderRowDragEnd}
      ondragover={(e) => !folder.locked && onFolderDropZoneDragOver(e, folder.id)}
      ondragleave={(e) => { if (dragOverFolderId === folder.id && !e.currentTarget.contains(/** @type {Node} */ (e.relatedTarget))) dragOverFolderId = null; }}
      ondrop={(e) => !folder.locked && onFolderDropZoneDrop(e, folder.id)}
    >
      <div class="folder-row">
        {#if node.children.length > 0}
          <button class="folder-expand-btn" onclick={() => toggleFolder(folder.id)} title={isExpanded ? 'Collapse' : 'Expand'} aria-expanded={isExpanded} aria-label={isExpanded ? `Collapse ${folder.name}` : `Expand ${folder.name}`}>
            {isExpanded ? '▾' : '▸'}
          </button>
        {:else}
          <span class="folder-expand-spacer"></span>
        {/if}

        {#if folder.locked}
          <button class="row-btn folder-name" onclick={() => onRequestFolderUnlock?.(folder)}>
            <span class="lock-icon">🔒</span>{folder.name === '<locked>' ? '(locked folder)' : folder.name}
          </button>
        {:else if inlineRenaming?.id === folder.id && inlineRenaming?.type === 'folder'}
          <input
            class="inline-rename"
            use:autofocus
            bind:value={inlineRenaming.value}
            onkeydown={(e) => { if (e.key === 'Enter' || e.key === 'Escape') { e.preventDefault(); onConfirmInlineRename?.(); } }}
            onblur={() => onConfirmInlineRename?.()}
          />
        {:else}
          {@const folderCount = notes.filter(n => n.folder_id === folder.id).length}
          <button class="row-btn folder-name" onclick={() => onSelectFolder?.(folder.id)}>{folder.name}</button>
          {#if folderCount > 0}<span class="folder-count">{folderCount}</span>{/if}
          {#if unlockedFolderIds?.has(folder.id)}
            <button class="icon-btn" title="Remove folder password"
              onclick={() => onSetFolderPassword?.(folder.id, 'remove')}>
              <svg width="13" height="13" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <rect x="2" y="7" width="11" height="7" rx="1"/>
                <path d="M5 7V4.5a2.5 2.5 0 0 1 5 0"/>
              </svg>
            </button>
          {:else}
            <button class="icon-btn" title="Set folder password"
              onclick={() => onSetFolderPassword?.(folder.id, 'set')}>
              <svg width="13" height="13" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <rect x="2" y="7" width="11" height="7" rx="1"/>
                <path d="M5 7V4.5a2.5 2.5 0 0 1 5 0V7"/>
              </svg>
            </button>
          {/if}
        {/if}
        <button class="icon-btn danger" onclick={() => onDeleteFolder?.(folder.id)} title="Delete folder" aria-label="Delete folder {folder.name}">✕</button>
      </div>

      {#if isExpanded && node.children.length > 0}
        <ul class="folder-children" role="group">
          {#each node.children as child}
            {@render renderFolder(child)}
          {/each}
        </ul>
      {/if}
    </li>
  {/snippet}

  {#each folderTree as node}
    {@render renderFolder(node)}
  {/each}
</ul>

<!-- Tags section -->
{#if allTags.length > 0}
  <div class="sidebar-section-label tags-header">
    <span>Tags</span>
    <button class="collapse-btn" onclick={() => (tagsOpen = !tagsOpen)} title={tagsOpen ? 'Collapse' : 'Expand'} aria-expanded={tagsOpen} aria-label={tagsOpen ? 'Collapse tags' : 'Expand tags'}>
      {tagsOpen ? '˅' : '›'}
    </button>
  </div>
  {#if tagsOpen}
    <div class="tag-search-row">
      <input class="tag-search-input" bind:value={tagSearch} placeholder="Search tags…" />
      {#if tagSearch}
        <button class="clear-filter-btn" onclick={() => (tagSearch = '')} title="Clear">✕</button>
      {/if}
    </div>
    <ul class="tag-list">
      {#each visibleTags as tag}
        <li class:active={tagFilter === tag.name}>
          <button class="row-btn" onclick={() => onFilterByTag?.(tag.name)}>#{tag.name}</button>
          <span class="tag-count">{tag.count}</span>
        </li>
      {:else}
        <li class="empty">No matches</li>
      {/each}
    </ul>
    {#if !tagSearch && allTags.length > TAG_LIMIT}
      <p class="tag-overflow">{allTags.length - TAG_LIMIT} more — search to find them</p>
    {/if}
  {/if}
{/if}

<!-- Templates section -->
<div class="sidebar-section-label">
  <span>Templates</span>
  <button class="collapse-btn" onclick={() => (templatesOpen = !templatesOpen)} title={templatesOpen ? 'Collapse' : 'Expand'} aria-expanded={templatesOpen} aria-label={templatesOpen ? 'Collapse templates' : 'Expand templates'}>
    {templatesOpen ? '˅' : '›'}
  </button>
</div>
{#if templatesOpen}
  <ul class="template-list">
    {#each templates as t (t.id)}
      <li>
        <span class="template-name">{t.name}</span>
        {#if !t.builtin}
          <button class="icon-btn" onclick={() => onEditTemplate?.(t)} title="Edit template" aria-label="Edit template {t.name}">✎</button>
          <button class="icon-btn danger" onclick={() => onDeleteTemplate?.(t.id)} title="Delete template" aria-label="Delete template {t.name}">✕</button>
        {/if}
      </li>
    {/each}
  </ul>
  <button class="vault-btn" onclick={() => onNewTemplate?.()}>+ New template</button>
{/if}
