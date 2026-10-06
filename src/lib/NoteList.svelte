<script>
  import { t, tParts } from './i18n/t.js';
  import { getContext } from 'svelte';
  import { autofocus } from './utils/autofocus.js';
  import LockClosedIcon from './icons/LockClosedIcon.svelte';

  const ns = getContext('ns');
  const fs = getContext('fs');
  const ts = getContext('ts');

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
  }

  let noteSort = $state('modified');

  const sortedNotes = $derived.by(() => {
    const arr = [...notes];
    if (noteSort === 'name')    arr.sort((a, b) => a.title.localeCompare(b.title));
    else if (noteSort === 'created') arr.sort((a, b) => b.created_at - a.created_at);
    else arr.sort((a, b) => b.updated_at - a.updated_at);
    return arr;
  });

  const showFolderUnlockProgress = $derived.by(() => {
    const u = folderUnlockReindex;
    if (!u || tagFilter) return null;
    const sid = selectedFolderId;
    if (typeof sid !== 'number') return null;
    if (!u.affectedFolderIds?.includes(sid)) return null;
    if (!u.total) return null;
    return u;
  });
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

<ul role="listbox" aria-label={t('notes.notesListAria')}>
  {#each sortedNotes as note (note.id)}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_no_noninteractive_tabindex -->
    <li
      role="option"
      aria-selected={activeNote?.id === note.id}
      class:active={activeNote?.id === note.id}
      class:locked-row={note.locked}
      data-note-id={note.id}
      draggable={!note.locked}
      ondragstart={(e) => !note.locked && onNoteDragStart?.(e, note)}
      ondragend={onNoteDragEnd}
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
        <span class="drag-handle" title={t('notes.dragToMove')} aria-hidden="true">⠇</span>
        <span class="row-btn note-title">{note.title}</span>
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
