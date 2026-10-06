<script>
    import { invoke } from "@tauri-apps/api/core";
    import { tick, getContext } from "svelte";
    import { autofocus } from "./utils/autofocus.js";
    import {
        buildFolderTree,
        isFolderDescendantOrSelf,
    } from "./utils/folderTree.js";
    import LockClosedIcon from "./icons/LockClosedIcon.svelte";
    import LockRemovePasswordIcon from "./icons/LockRemovePasswordIcon.svelte";
    import PencilIcon from "./icons/PencilIcon.svelte";
    import ImportNoteIcon from "./icons/ImportNoteIcon.svelte";
    import NewFolderIcon from "./icons/NewFolderIcon.svelte";
    import { t, tParts } from "./i18n/t.js";

    const ns = getContext("ns");
    const fs = getContext("fs");
    const bmSvc = getContext("bm");
    const tmpl = getContext("tmpl");

    // Data aliases — read-only, pulled from context.
    const folders = $derived(fs.folders);
    const notes = $derived(ns.notes);
    const allTags = $derived(ns.allTags);
    const tagFilter = $derived(ns.tagFilter);
    const bookmarks = $derived(bmSvc.bookmarks);
    const selectedFolderId = $derived(fs.selectedFolderId);
    const unlockedFolderIds = $derived(fs.unlockedFolderIds);
    const templates = $derived(tmpl.templates);
    // inlineRenaming and folderExpanded are read/written via fs directly.

    let {
        // isDragging comes from folderService (fs) via App.svelte prop.
        isDragging = false,
        // Coordinator callbacks — cross multiple services or involve App-level logic.
        onSelectFolder,
        onCreateNote,
        onImportNote,
        onCreateFolder,
        onDeleteFolder,
        onOpenNoteById,
        onOpenNoteInNewTab,
        onFilterByTag,
        onConfirmInlineRename,
        onDeleteTemplate,
        onMoveNote,
        onMoveFolder,
        onLockFolderSession,
    } = $props();

    // ── Local state ────────────────────────────────────────────────────────────

    let bookmarksOpen = $state(true);
    let tagsOpen = $state(false);
    let tagSearch = $state("");
    let templatesOpen = $state(false);
    let dragOverFolderId = $state(null);
    let draggingFolderId = $state(null);

    const TAG_LIMIT = 3;
    const folderTree = $derived(buildFolderTree(folders));
    const visibleTags = $derived(
        tagSearch.trim()
            ? (() => {
                  const q = tagSearch.trim().toLowerCase().replace(/^#/, "");
                  return allTags.filter((t) =>
                      t.name.trim().toLowerCase().replace(/^#/, "").includes(q),
                  );
              })()
            : allTags.slice(0, TAG_LIMIT),
    );

    // ── Folder expand/collapse ────────────────────────────────────────────────

    function toggleFolder(id) {
        fs.folderExpanded = {
            ...fs.folderExpanded,
            [id]: !(fs.folderExpanded[id] ?? true),
        };
    }

    function expandAll() {
        const expanded = {};
        for (const f of folders) expanded[f.id] = true;
        fs.folderExpanded = expanded;
    }

    function collapseAll() {
        const collapsed = {};
        for (const f of folders) collapsed[f.id] = false;
        fs.folderExpanded = collapsed;
    }

    // ── Keyboard navigation ──────────────────────────────────────────────────

    function getFolderTreeButtons() {
        return /** @type {HTMLElement[]} */ (
            Array.from(
                document.querySelectorAll(
                    ".folder-list .folder-row .row-btn:not([disabled])",
                ),
            )
        );
    }

    async function handleFolderTreeKeydown(e) {
        if (
            ![
                "ArrowDown",
                "ArrowUp",
                "ArrowRight",
                "ArrowLeft",
                "Home",
                "End",
            ].includes(e.key)
        )
            return;
        e.preventDefault();

        const btns = getFolderTreeButtons();
        const focused = /** @type {HTMLElement | null} */ (
            document.activeElement
        );
        const idx = focused ? btns.indexOf(focused) : -1;

        if (e.key === "ArrowDown") {
            btns[Math.min(idx + 1, btns.length - 1)]?.focus();
        } else if (e.key === "ArrowUp") {
            btns[Math.max(idx - 1, 0)]?.focus();
        } else if (e.key === "ArrowRight") {
            const row = focused?.closest(".folder-row");
            const expandBtn = /** @type {HTMLElement | null} */ (
                row?.querySelector('.folder-expand-btn[aria-expanded="false"]')
            );
            if (expandBtn) {
                expandBtn.click();
                await tick();
                const freshBtns = getFolderTreeButtons();
                freshBtns[Math.min(idx + 1, freshBtns.length - 1)]?.focus();
            }
        } else if (e.key === "ArrowLeft") {
            const row = focused?.closest(".folder-row");
            const expandBtn = /** @type {HTMLElement | null} */ (
                row?.querySelector('.folder-expand-btn[aria-expanded="true"]')
            );
            if (expandBtn) {
                expandBtn.click();
            } else {
                const parentLi =
                    focused?.closest(".folder-children")?.parentElement;
                const parentBtn = /** @type {HTMLElement | null} */ (
                    parentLi?.querySelector(":scope > .folder-row .row-btn")
                );
                if (parentBtn) parentBtn.focus();
            }
        } else if (e.key === "Home") {
            btns[0]?.focus();
        } else if (e.key === "End") {
            btns[btns.length - 1]?.focus();
        }
    }

    // ── Folder drag-to-reparent ──────────────────────────────────────────────

    function onFolderRowDragStart(e, folderId) {
        e.stopPropagation();
        e.dataTransfer.setData("folder-id", String(folderId));
        e.dataTransfer.effectAllowed = "move";
        draggingFolderId = folderId;
    }

    function onFolderRowDragEnd() {
        draggingFolderId = null;
        dragOverFolderId = null;
    }

    function onFolderDropZoneDragOver(e, targetFolderId) {
        e.stopPropagation();
        if (e.dataTransfer.types.includes("folder-id")) {
            if (
                draggingFolderId &&
                isFolderDescendantOrSelf(
                    folders,
                    targetFolderId,
                    draggingFolderId,
                )
            )
                return;
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
        } else {
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
        }
        dragOverFolderId = targetFolderId;
    }

    async function onFolderDropZoneDrop(e, targetFolderId) {
        e.stopPropagation();
        e.preventDefault();
        dragOverFolderId = null;
        if (e.dataTransfer.types.includes("folder-id")) {
            const movingId = Number(e.dataTransfer.getData("folder-id"));
            if (
                !movingId ||
                isFolderDescendantOrSelf(folders, targetFolderId, movingId)
            )
                return;
            try {
                await invoke("move_folder", {
                    id: movingId,
                    newParentId: targetFolderId,
                });
                fs.folderExpanded = {
                    ...fs.folderExpanded,
                    [targetFolderId]: true,
                };
                onMoveFolder?.();
            } catch {
                /* non-fatal */
            }
        } else {
            const noteId = Number(e.dataTransfer.getData("text/plain"));
            if (noteId) onMoveNote?.(noteId, targetFolderId);
        }
    }
</script>

<div class="panel-header">
    <h2>{t('folders.title')}</h2>
    <span class="panel-header-actions">
        <button class="icon-btn" onclick={expandAll} title={t('folders.expandAll')}>
            <svg
                width="14"
                height="14"
                viewBox="0 0 15 15"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <polyline points="3,2.5 7.5,7 12,2.5" />
                <polyline points="3,7.5 7.5,12 12,7.5" />
            </svg>
        </button>
        <button class="icon-btn" onclick={collapseAll} title={t('folders.collapseAll')}>
            <svg
                width="14"
                height="14"
                viewBox="0 0 15 15"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <polyline points="3,7 7.5,2.5 12,7" />
                <polyline points="3,12 7.5,7.5 12,12" />
            </svg>
        </button>
        <button
            class="icon-btn"
            data-action="create-note-btn"
            onclick={() => onCreateNote?.()}
            title={t('folders.newNote')}
        >
            <PencilIcon size={14} />
        </button>
        <button
            class="icon-btn"
            onclick={() => onImportNote?.()}
            title={t('folders.importNote')}
        >
            <ImportNoteIcon size={14} />
        </button>
        <button
            class="icon-btn"
            onclick={() => onCreateFolder?.()}
            title={t('folders.newFolder')}
        >
            <NewFolderIcon size={14} />
        </button>
    </span>
</div>

<!-- Bookmarks -->
{#if bookmarks.length > 0}
    <section class="bookmarks-section">
        <div class="sidebar-section-label">
            <span>{t('folders.bookmarks')}</span>
            <button
                class="collapse-btn"
                onclick={() => (bookmarksOpen = !bookmarksOpen)}
                title={bookmarksOpen ? t('folders.collapse') : t('folders.expand')}
            >
                {bookmarksOpen ? "˅" : "›"}
            </button>
        </div>
        {#if bookmarksOpen}
            <ul class="bookmark-list">
                {#each bookmarks as bookmark (bookmark.note_id)}
                    <li class="bookmark-row" data-note-id={bookmark.note_id}>
                        <button
                            class="bookmark-name"
                            onclick={() => onOpenNoteById?.(bookmark.note_id)}
                            title={bookmark.title}
                        >
                            {bookmark.title}
                        </button>
                        <button
                            class="bookmark-remove icon-btn"
                            onclick={() =>
                                bmSvc.removeBookmark(bookmark.note_id)}
                            title={t('folders.removeBookmark')}>✕</button
                        >
                    </li>
                {/each}
            </ul>
        {/if}
    </section>
{/if}

<!-- Folder tree -->
<ul
    class="folder-list"
    role="tree"
    aria-label={t('folders.aria')}
    onkeydown={handleFolderTreeKeydown}
>
    <li
        class:active={selectedFolderId === "all"}
        data-folder-id="all"
        role="treeitem"
        aria-selected={selectedFolderId === "all"}
    >
        <div class="folder-row">
            <span class="folder-expand-spacer"></span>
            <button class="row-btn" onclick={() => onSelectFolder?.("all")}
                >{t('folders.allNotes')}</button
            >
        </div>
    </li>
    <li
        class:active={selectedFolderId === null}
        class:drag-over={dragOverFolderId === "unfiled"}
        class:drag-active={isDragging || !!draggingFolderId}
        data-folder-id="unfiled"
        role="treeitem"
        aria-selected={selectedFolderId === null}
        ondragover={(e) => {
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
            dragOverFolderId = "unfiled";
        }}
        ondragleave={(e) => {
            if (
                dragOverFolderId === "unfiled" &&
                !e.currentTarget.contains(/** @type {Node} */ (e.relatedTarget))
            )
                dragOverFolderId = null;
        }}
        ondrop={(e) => {
            e.preventDefault();
            dragOverFolderId = null;
            if (e.dataTransfer.types.includes("folder-id")) {
                const fid = Number(e.dataTransfer.getData("folder-id"));
                if (fid)
                    invoke("move_folder", { id: fid, newParentId: null })
                        .then(() => onMoveFolder?.())
                        .catch(() => {});
            } else {
                const noteId = Number(e.dataTransfer.getData("text/plain"));
                if (noteId) onMoveNote?.(noteId, null);
            }
        }}
    >
        <div class="folder-row">
            <span class="folder-expand-spacer"></span>
            <button class="row-btn" onclick={() => onSelectFolder?.(null)}
                >{t('folders.unfiled')}</button
            >
        </div>
    </li>

    {#snippet renderFolder(node)}
        {@const folder = node.folder}
        <li
            class:active={selectedFolderId === folder.id}
            class:locked-row={folder.locked}
            class:drag-over={dragOverFolderId === folder.id}
            class:drag-active={(isDragging || !!draggingFolderId) &&
                !folder.locked}
            data-folder-id={folder.id}
            role="treeitem"
            aria-selected={selectedFolderId === folder.id}
            aria-expanded={node.children.length > 0
                ? (fs.folderExpanded[folder.id] ?? true)
                : undefined}
            draggable={!folder.locked}
            ondragstart={(e) =>
                !folder.locked && onFolderRowDragStart(e, folder.id)}
            ondragend={onFolderRowDragEnd}
            ondragover={(e) =>
                !folder.locked && onFolderDropZoneDragOver(e, folder.id)}
            ondragleave={(e) => {
                if (
                    dragOverFolderId === folder.id &&
                    !e.currentTarget.contains(
                        /** @type {Node} */ (e.relatedTarget),
                    )
                )
                    dragOverFolderId = null;
            }}
            ondrop={(e) => !folder.locked && onFolderDropZoneDrop(e, folder.id)}
        >
            <div class="folder-row">
                {#if node.children.length > 0}
                    <button
                        class="folder-expand-btn"
                        onclick={() => toggleFolder(folder.id)}
                        title={(fs.folderExpanded[folder.id] ?? true)
                            ? t('folders.collapse')
                            : t('folders.expand')}
                        aria-expanded={fs.folderExpanded[folder.id] ?? true}
                        aria-label={(fs.folderExpanded[folder.id] ?? true)
                            ? t('folders.collapseFolderAria', { name: folder.name })
                            : t('folders.expandFolderAria', { name: folder.name })}
                    >
                        {(fs.folderExpanded[folder.id] ?? true) ? "▾" : "▸"}
                    </button>
                {:else}
                    <span class="folder-expand-spacer"></span>
                {/if}

                {#if folder.locked}
                    <button
                        class="row-btn folder-name"
                        onclick={() => fs.requestFolderUnlock(folder)}
                    >
                        <span class="lock-icon"><LockClosedIcon /></span
                        >{folder.name === "<locked>"
                            ? t('folders.lockedFolderPlaceholder')
                            : folder.name}
                    </button>
                {:else if fs.inlineRenaming?.id === folder.id && fs.inlineRenaming?.type === "folder"}
                    <input
                        class="inline-rename"
                        use:autofocus
                        bind:value={fs.inlineRenaming.value}
                        onkeydown={(e) => {
                            if (e.key === "Enter" || e.key === "Escape") {
                                e.preventDefault();
                                onConfirmInlineRename?.();
                            }
                        }}
                        onblur={() => onConfirmInlineRename?.()}
                    />
                {:else}
                    {@const folderCount = notes.filter(
                        (n) => n.folder_id === folder.id,
                    ).length}
                    <button
                        class="row-btn folder-name"
                        onclick={() => onSelectFolder?.(folder.id)}
                        >{folder.name}</button
                    >
                    {#if folderCount > 0}<span class="folder-count"
                            >{folderCount}</span
                        >{/if}
                {/if}
                <div class="folder-row-trailing-icons">
                    {#if !folder.locked && !(fs.inlineRenaming?.id === folder.id && fs.inlineRenaming?.type === "folder")}
                        {#if unlockedFolderIds?.has(folder.id)}
                            <button
                                class="icon-btn"
                                title={t('folders.lockSession')}
                                onclick={() => onLockFolderSession?.(folder.id)}
                            >
                                <LockClosedIcon />
                            </button>
                            <button
                                class="icon-btn"
                                title={t('folders.removeFolderPassword')}
                                onclick={() =>
                                    fs.openFolderPwModal(folder.id, "remove")}
                            >
                                <LockRemovePasswordIcon />
                            </button>
                        {:else}
                            <button
                                class="icon-btn"
                                title={t('folders.setFolderPassword')}
                                onclick={() =>
                                    fs.openFolderPwModal(folder.id, "set")}
                            >
                                <LockClosedIcon />
                            </button>
                        {/if}
                        <button
                            class="icon-btn"
                            onclick={() => {
                                fs.inlineRenaming = {
                                    id: folder.id,
                                    type: "folder",
                                    value: folder.name,
                                };
                            }}
                            title={t('folders.renameFolder')}
                            aria-label={t('folders.renameFolderAria', { name: folder.name })}
                        >
                            <PencilIcon size={13} />
                        </button>
                    {/if}
                    <button
                        class="icon-btn danger"
                        onclick={() => onDeleteFolder?.(folder.id)}
                        title={t('folders.deleteFolder')}
                        aria-label={t('folders.deleteFolderAria', { name: folder.name })}>✕</button
                    >
                </div>
            </div>

            {#if (fs.folderExpanded[folder.id] ?? true) && node.children.length > 0}
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
    {:else}
        <li class="folder-empty" role="status">
            {#each tParts('sidebar.noFoldersYet') as part}{#if part.type === 'slot' && part.name === 'newFolder'}<strong>{t('sidebar.newFolder')}</strong>{:else if part.type === 'slot' && part.name === 'newNote'}<strong>{t('sidebar.newNote')}</strong>{:else}{part.value}{/if}{/each}
        </li>
    {/each}
</ul>

<!-- Tags section -->
{#if allTags.length > 0}
    <div class="sidebar-section-label tags-header">
        <span>{t('folders.tags')}</span>
        <button
            class="collapse-btn"
            onclick={() => (tagsOpen = !tagsOpen)}
            title={tagsOpen ? t('folders.collapse') : t('folders.expand')}
            aria-expanded={tagsOpen}
            aria-label={tagsOpen ? t('folders.collapseTags') : t('folders.expandTags')}
        >
            {tagsOpen ? "˅" : "›"}
        </button>
    </div>
    {#if tagsOpen}
        <div class="tag-search-row">
            <input
                class="tag-search-input"
                bind:value={tagSearch}
                placeholder={t('folders.searchTags')}
            />
            {#if tagSearch}
                <button
                    class="clear-filter-btn"
                    onclick={() => (tagSearch = "")}
                    title={t('folders.clearTagSearch')}
                >✕</button>
            {/if}
        </div>
        <ul class="tag-list">
            {#each visibleTags as tag}
                <li class:active={tagFilter === tag.name}>
                    <button
                        class="row-btn"
                        onclick={() => onFilterByTag?.(tag.name)}
                        >#{tag.name}</button
                    >
                    <span class="tag-count">{tag.count}</span>
                </li>
            {:else}
                <li class="empty">{t('folders.noTagMatches')}</li>
            {/each}
        </ul>
        {#if !tagSearch && allTags.length > TAG_LIMIT}
            <p class="tag-overflow">
                {t('folders.tagOverflow', { n: allTags.length - TAG_LIMIT })}
            </p>
        {/if}
    {/if}
{/if}

<!-- Templates section -->
<div class="sidebar-section-label">
    <span>{t('folders.templates')}</span>
    <button
        class="collapse-btn"
        onclick={() => (templatesOpen = !templatesOpen)}
        title={templatesOpen ? t('folders.collapse') : t('folders.expand')}
        aria-expanded={templatesOpen}
        aria-label={templatesOpen ? t('folders.collapseTemplates') : t('folders.expandTemplates')}
    >
        {templatesOpen ? "˅" : "›"}
    </button>
</div>
{#if templatesOpen}
    <ul class="template-list">
        {#each templates as tpl (tpl.id)}
            <li>
                <span class="template-name">{tpl.name}</span>
                {#if !tpl.builtin}
                    <button
                        class="icon-btn"
                        onclick={() => {
                            tmpl.editingTemplate = tpl;
                        }}
                        title={t('folders.editTemplate')}
                        aria-label={t('folders.editTemplateAria', { name: tpl.name })}>✎</button
                    >
                    <button
                        class="icon-btn danger"
                        onclick={() => onDeleteTemplate?.(tpl.id)}
                        title={t('folders.deleteTemplate')}
                        aria-label={t('folders.deleteTemplateAria', { name: tpl.name })}>✕</button
                    >
                {/if}
            </li>
        {/each}
    </ul>
    <button
        class="vault-btn"
        onclick={() => {
            tmpl.templateModalOpen = true;
        }}>+ New template</button
    >
{/if}
