<script>
  import { t, tParts } from './i18n/t.js';
  import { getContext } from 'svelte';
  import { autofocus } from './utils/autofocus.js';
  import { sortNotesWithFolderPins } from './utils/sortNotesWithFolderPins.js';
  import { FOLDER_PIN_MIME, isFolderPinDrag } from './utils/folderPinDrag.js';
  import LockClosedIcon from './icons/LockClosedIcon.svelte';
  import FolderPinIcon from './icons/FolderPinIcon.svelte';

  const ns = getContext('ns');
  const fs = getContext('fs');
  const ts = getContext('ts');
  const folderPins = getContext('folderPins');

  // Data props replaced by context aliases.
  const notes            = $derived(ns.notes);
  const activeNote       = $derived(ns.activeNote);
  const tagFilter        = $derived(ns.tagFilter);
  const isReindexing     = $derived(ns.isReindexing);
  const reindexProgress  = $derived(ns.reindexProgress);
  const folders          = $derived(fs.folders);
  const selectedFolderId = $derived(fs.selectedFolderId);
  const inlineRenaming   = $derived(fs.inlineRenaming);
  const tableViewOpen    = $derived(ts.tableViewOpen);
  const pinOrder         = $derived(folderPins?.orderedIds ?? []);
  const pinnedNoteIds    = $derived(folderPins?.pinnedNoteIds ?? new Set());

  let {
    folderUnlockReindex = null,
    onOpenNote,
    onOpenNoteInNewTab,
    onDeleteNote,
    onConfirmInlineRename,
    onOpenKanbanTab,
    onSaveNote,
    onReindexAll,
    onTableViewToggle,
    onNoteDragStart,
    onNoteDragEnd,
  } = $props();

  async function clearTagFilter() {
    ns.tagFilter = null;
    await ns.loadNotes(fs.selectedFolderId, null);
    if (typeof fs.selectedFolderId === 'number') {
      await folderPins?.load(fs.selectedFolderId);
    } else {
      folderPins?.clear();
    }
  }

  let noteSort = $state('modified');
  /** @type {number | null} */
  let pinDragId = $state(null);
  /** @type {number | null} */
  let pinDropTargetId = $state(null);
  let pinLiveMessage = $state('');

  const canUseFolderPins = $derived(
    !tagFilter && typeof selectedFolderId === 'number',
  );

  const sortedNotes = $derived.by(() =>
    sortNotesWithFolderPins(
      notes,
      noteSort,
      canUseFolderPins ? pinOrder : [],
    ),
  );

  const showFolderUnlockProgress = $derived.by(() => {
    const u = folderUnlockReindex;
    if (!u || tagFilter) return null;
    const sid = selectedFolderId;
    if (typeof sid !== 'number') return null;
    if (!u.affectedFolderIds?.includes(sid)) return null;
    if (!u.total) return null;
    return u;
  });

  /**
   * @param {DragEvent} e
   * @param {number} noteId
   */
  function onPinGripDragStart(e, noteId) {
    e.stopPropagation();
    pinDragId = noteId;
    e.dataTransfer?.setData(FOLDER_PIN_MIME, String(noteId));
    e.dataTransfer?.setData('text/plain', '');
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }

  function onPinGripDragEnd() {
    pinDragId = null;
    pinDropTargetId = null;
  }

  /**
   * @param {DragEvent} e
   * @param {number} targetNoteId
   */
  function onPinRowDragOver(e, targetNoteId) {
    if (!isFolderPinDrag(e.dataTransfer) || pinDragId == null) return;
    if (!pinnedNoteIds.has(targetNoteId)) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    pinDropTargetId = targetNoteId;
  }

  /**
   * @param {DragEvent} e
   * @param {number} targetNoteId
   */
  async function onPinRowDrop(e, targetNoteId) {
    e.preventDefault();
    e.stopPropagation();
    const fromId = pinDragId;
    pinDragId = null;
    pinDropTargetId = null;
    if (
      fromId == null ||
      !canUseFolderPins ||
      !pinnedNoteIds.has(fromId) ||
      !pinnedNoteIds.has(targetNoteId) ||
      fromId === targetNoteId
    ) {
      return;
    }
    const next = [...pinOrder];
    const fromIdx = next.indexOf(fromId);
    const toIdx = next.indexOf(targetNoteId);
    if (fromIdx < 0 || toIdx < 0) return;
    next.splice(fromIdx, 1);
    next.splice(toIdx, 0, fromId);
    try {
      await folderPins.reorder(selectedFolderId, next);
      const note = notes.find((n) => n.id === fromId);
      pinLiveMessage = t('notes.pinMovedAnnounce', {
        title: note?.title ?? '',
        position: toIdx + 1,
      });
    } catch (err) {
      // store onError already notified; snap-back via unchanged order
    }
  }
</script>

<div class="panel-header">
  <h2>
    {#if tagFilter}#{tagFilter}
    {:else if selectedFolderId === 'all'}All Notes
    {:else if selectedFolderId === null}Unfiled
    {:else}{folders.find(f => f.id === selectedFolderId)?.name ?? ''}
    {/if}
  </h2>
  {#if tagFilter}
    <button class="clear-filter-btn" onclick={clearTagFilter} title={t('notes.clearTagFilter')}>✕</button>
  {/if}
  <select class="sort-select" bind:value={noteSort} title={t('notes.sortNotes')} aria-label={t('notes.sortNotes')}>
    <option value="modified">{t('notes.sortModified')}</option>
    <option value="created">{t('notes.sortCreated')}</option>
    <option value="name">{t('notes.sortName')}</option>
  </select>
  {#if !tagFilter && selectedFolderId && selectedFolderId !== 'all'}
    <button
      class="panel-view-btn"
      class:active={tableViewOpen}
      aria-pressed={tableViewOpen}
      title={t('notes.tableView')}
      aria-label={t('notes.tableView')}
      onclick={onTableViewToggle}
    >{t('notes.table')}</button>
    <button
      class="panel-view-btn"
      title={t('notes.boardView')}
      aria-label={t('notes.boardView')}
      onclick={() => onOpenKanbanTab?.(selectedFolderId, folders.find(f => f.id === selectedFolderId)?.name ?? '')}
    >{t('notes.board')}</button>
  {/if}
</div>

{#if showFolderUnlockProgress}
  <p class="folder-unlock-index-status" role="status">
    {#if showFolderUnlockProgress.embeddingChunks}
      {t('notes.embeddingNoteChunks', {
        title: showFolderUnlockProgress.embeddingChunks.note_title,
        done: showFolderUnlockProgress.embeddingChunks.done,
        total: showFolderUnlockProgress.embeddingChunks.total,
        processed: showFolderUnlockProgress.processed,
        noteTotal: showFolderUnlockProgress.total,
      })}
    {:else}
      {t('notes.indexingNotesForAi', {
        processed: showFolderUnlockProgress.processed,
        total: showFolderUnlockProgress.total,
      })}
    {/if}
  </p>
{/if}

<div class="sr-only" aria-live="polite" aria-atomic="true">{pinLiveMessage}</div>

<ul role="listbox" aria-label={t('notes.notesListAria')}>
  {#each sortedNotes as note (note.id)}
    {@const isPinned = canUseFolderPins && pinnedNoteIds.has(note.id)}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_no_noninteractive_tabindex -->
    <li
      role="option"
      aria-selected={activeNote?.id === note.id}
      class:active={activeNote?.id === note.id}
      class:locked-row={note.locked}
      class:folder-pinned={isPinned}
      class:pin-drop-target={pinDropTargetId === note.id}
      data-note-id={note.id}
      data-note-list-row=""
      draggable={!note.locked}
      ondragstart={(e) => {
        if (note.locked) return;
        if (isFolderPinDrag(e.dataTransfer)) return;
        onNoteDragStart?.(e, note);
      }}
      ondragend={onNoteDragEnd}
      ondragover={(e) => isPinned && onPinRowDragOver(e, note.id)}
      ondrop={(e) => isPinned && onPinRowDrop(e, note.id)}
      tabindex="0"
      onclick={(e) => { if (note.locked) return; if (e.ctrlKey) onOpenNoteInNewTab?.(note); else onOpenNote?.(note); }}
      onkeydown={(e) => { if (e.key === 'Enter' && !note.locked) { onOpenNote?.(note); } }}
    >
      {#if note.locked}
        <span class="row-btn note-title note-locked"><span class="lock-icon"><LockClosedIcon /></span>{t('notes.lockedShort')}</span>
      {:else if inlineRenaming?.id === note.id && inlineRenaming?.type === 'note'}
        <input
          class="inline-rename"
          use:autofocus
          bind:value={inlineRenaming.value}
          onkeydown={(e) => { if (e.key === 'Enter' || e.key === 'Escape') { e.preventDefault(); onConfirmInlineRename?.(); } }}
          onblur={() => onConfirmInlineRename?.()}
        />
      {:else}
        {#if isPinned}
          <span
            class="pin-reorder-grip"
            title={t('notes.reorderPin')}
            role="presentation"
            draggable="true"
            ondragstart={(e) => onPinGripDragStart(e, note.id)}
            ondragend={onPinGripDragEnd}
          >⠿</span>
        {/if}
        <span class="drag-handle" title={t('notes.dragToMove')} aria-hidden="true">⠇</span>
        <span class="row-btn note-title">{note.title}</span>
        {#if isPinned}
          <span class="folder-pin-marker" title={t('notes.pinnedInFolder')} aria-hidden="true">
            <FolderPinIcon />
          </span>
        {/if}
        <button
          class="icon-btn danger"
          type="button"
          onclick={(e) => {
            e.stopPropagation();
            e.preventDefault();
            onDeleteNote?.(note.id);
          }}
          title={t('notes.deleteNote')}
          aria-label={t('notes.deleteNoteAria', { title: note.title })}
        >✕</button>
      {/if}
    </li>
  {:else}
    <li class="empty" role="status">
      {#each tParts('notes.emptyFolder', { newNote: t('notes.newNoteStrong') }) as part}{#if part.type === 'slot' && part.name === 'newNote'}<strong>{part.value}</strong>{:else}{part.value}{/if}{/each}
    </li>
  {/each}
</ul>

<button class="seed-btn" onclick={onReindexAll} disabled={isReindexing}>
  {#if isReindexing}
    {#if reindexProgress && reindexProgress.total > 0}
      {#if reindexProgress.embeddingChunks}
        {t('notes.embeddingNoteChunks', {
          title: reindexProgress.embeddingChunks.note_title,
          done: reindexProgress.embeddingChunks.done,
          total: reindexProgress.embeddingChunks.total,
          processed: reindexProgress.processed,
          noteTotal: reindexProgress.total,
        })}
      {:else}
        {t('notes.indexingProgress', {
          processed: reindexProgress.processed,
          total: reindexProgress.total,
        })}
      {/if}
    {:else}
      {t('notes.indexing')}
    {/if}
  {:else}
    {t('notes.reindexAllNotes')}
  {/if}
</button>
