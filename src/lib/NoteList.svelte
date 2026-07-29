<script>
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
    <button class="clear-filter-btn" onclick={clearTagFilter} title="Clear tag filter">✕</button>
  {/if}
  <select class="sort-select" bind:value={noteSort} title="Sort notes" aria-label="Sort notes">
    <option value="modified">Modified</option>
    <option value="created">Created</option>
    <option value="name">Name</option>
  </select>
  {#if !tagFilter && selectedFolderId && selectedFolderId !== 'all'}
    <button
      class="panel-view-btn"
      class:active={tableViewOpen}
      aria-pressed={tableViewOpen}
      title="Table view"
      aria-label="Table view"
      onclick={onTableViewToggle}
    >Table</button>
    <button
      class="panel-view-btn"
      title="Kanban view"
      aria-label="Board view"
      onclick={() => onOpenKanbanTab?.(selectedFolderId, folders.find(f => f.id === selectedFolderId)?.name ?? '')}
    >Board</button>
  {/if}
</div>

{#if showFolderUnlockProgress}
  <p class="folder-unlock-index-status" role="status">
    {#if showFolderUnlockProgress.embeddingChunks}
      Embedding “{showFolderUnlockProgress.embeddingChunks.note_title}”… {showFolderUnlockProgress.embeddingChunks.done}/{showFolderUnlockProgress.embeddingChunks.total} chunks · notes {showFolderUnlockProgress.processed}/{showFolderUnlockProgress.total}
    {:else}
      Indexing notes for AI… {showFolderUnlockProgress.processed}/{showFolderUnlockProgress.total}
    {/if}
  </p>
{/if}

<ul role="listbox" aria-label="Notes">
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
        <span class="row-btn note-title note-locked"><span class="lock-icon"><LockClosedIcon /></span>(locked)</span>
      {:else if inlineRenaming?.id === note.id && inlineRenaming?.type === 'note'}
        <input
          class="inline-rename"
          use:autofocus
          bind:value={inlineRenaming.value}
          onkeydown={(e) => { if (e.key === 'Enter' || e.key === 'Escape') { e.preventDefault(); onConfirmInlineRename?.(); } }}
          onblur={() => onConfirmInlineRename?.()}
        />
      {:else}
        <span class="drag-handle" title="Drag to move" aria-hidden="true">⠇</span>
        <span class="row-btn note-title">{note.title}</span>
        <button
          class="icon-btn danger"
          type="button"
          onclick={(e) => {
            e.stopPropagation();
            e.preventDefault();
            onDeleteNote?.(note.id);
          }}
          title="Delete note"
          aria-label="Delete note {note.title}"
        >✕</button>
      {/if}
    </li>
  {:else}
    <li class="empty" role="status">No notes here</li>
  {/each}
</ul>

<button class="seed-btn" onclick={onReindexAll} disabled={isReindexing}>
  {#if isReindexing}
    {#if reindexProgress && reindexProgress.total > 0}
      {#if reindexProgress.embeddingChunks}
        Embedding “{reindexProgress.embeddingChunks.note_title}”… {reindexProgress.embeddingChunks.done}/{reindexProgress.embeddingChunks.total} chunks · notes {reindexProgress.processed}/{reindexProgress.total}
      {:else}
        Indexing… {reindexProgress.processed}/{reindexProgress.total}
      {/if}
    {:else}
      Indexing…
    {/if}
  {:else}
    Re-index all notes
  {/if}
</button>
