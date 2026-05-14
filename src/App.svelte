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
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { openUrl } from "@tauri-apps/plugin-opener";
    import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
    import { listen } from "@tauri-apps/api/event";
    import { onMount, tick, untrack, setContext } from "svelte";
    import ActivityBar from "./lib/ActivityBar.svelte";
    import Calendar from "./lib/Calendar.svelte";
    import Chat from "./lib/Chat.svelte";
    import Graph from "./lib/Graph.svelte";
    import Kanban from "./lib/Kanban.svelte";
    import WikipediaReader from "./lib/WikipediaReader.svelte";
    import WikipediaSearchModal from "./lib/WikipediaSearchModal.svelte";
    import TabBar from "./lib/TabBar.svelte";
    import LockScreen from "./lib/LockScreen.svelte";
    import PasswordModal from "./lib/PasswordModal.svelte";
    import TemplateModal from "./lib/TemplateModal.svelte";
    import DatabaseView from "./lib/DatabaseView.svelte";
    import Settings from "./lib/Settings.svelte";
    import Search from "./lib/Search.svelte";
    import ConfirmModal from "./lib/ConfirmModal.svelte";
    import QuickSwitcher from "./lib/QuickSwitcher.svelte";
    import ContextMenu from "./lib/ContextMenu.svelte";
    import FolderSidebar from "./lib/FolderSidebar.svelte";
    import NoteList from "./lib/NoteList.svelte";
    import NoteEditor from "./lib/NoteEditor.svelte";
    import { createSettings } from "./lib/stores/settings.svelte.js";
    import { createPanelLayout } from "./lib/stores/panelLayout.svelte.js";
    import { createBookmarks } from "./lib/stores/bookmarks.svelte.js";
    import { createTemplates } from "./lib/stores/templates.svelte.js";
    import { createVaultService } from "./lib/services/vaultService.svelte.js";
    import { createFolderService } from "./lib/services/folderService.svelte.js";
    import { createNoteService } from "./lib/services/noteService.svelte.js";
    import { createTabService } from "./lib/services/tabService.svelte.js";
    import { createImproveService } from "./lib/services/improveService.svelte.js";
    import { createUiService } from "./lib/services/uiService.svelte.js";
    import { createErrorService } from "./lib/services/errorService.svelte.js";
    import { createContextMenuService } from "./lib/services/contextMenuService.svelte.js";
    import { folderSubtreeIds } from "./lib/utils/folderTree.js";
    import { createKeyboardService } from "./lib/services/keyboardService.svelte.js";

    const appWindow = getCurrentWindow();

    // ── Stores ─────────────────────────────────────────────────────────────────
    const settings = createSettings();
    const layout = createPanelLayout();
    const bm = createBookmarks();
    const tmpl = createTemplates();
    const err = createErrorService();
    /** @type {{ rootFolderId: number, affectedFolderIds: number[], processed: number, total: number, embeddingChunks: { done: number, total: number, note_title: string } | null } | null} */
    let folderUnlockReindex = $state(null);

    const fs = createFolderService({
        onError: err.showError,
        onFolderUnlocked: (folderId) => {
            invoke("start_folder_unlock_reindex", { rootFolderId: folderId }).catch(
                () => {},
            );
        },
    });
    const ns = createNoteService({ onError: err.showError });
    const ts = createTabService({ onError: err.showError });
    const is = createImproveService({
        onError: err.showError,
        getEditorContent: () => ns.editorContent,
        setEditorContent: (v) => {
            ns.editorContent = v;
        },
        markDirty: () => ns.markDirty(),
        getActiveNote: () => ns.activeNote,
    });
    const ui = createUiService();
    const vault = createVaultService({ onError: err.showError, ns, ts, fs });

    let vaultReindexBannerDismissed = $state(false);
    /** @type {{ nextPos: number, total: number, indexedOk: number, embeddingModel: string } | null} */
    let vaultReindexBanner = $state(null);
    let prevVaultReindexIncomplete = $state(false);

    async function refreshVaultReindexBanner() {
        if (vault.vaultLocked) {
            vaultReindexBanner = null;
            return;
        }
        try {
            const s = await invoke("vault_reindex_status");
            if (s.incomplete && !prevVaultReindexIncomplete) {
                vaultReindexBannerDismissed = false;
            }
            prevVaultReindexIncomplete = Boolean(s.incomplete);
            if (s.incomplete && !vaultReindexBannerDismissed) {
                vaultReindexBanner = {
                    nextPos: s.next_pos,
                    total: s.total,
                    indexedOk: s.indexed_ok,
                    embeddingModel: s.embedding_model,
                };
            } else {
                vaultReindexBanner = null;
            }
        } catch {
            vaultReindexBanner = null;
        }
    }

    function dismissVaultReindexBanner() {
        vaultReindexBannerDismissed = true;
        vaultReindexBanner = null;
    }

    async function resumeVaultReindexFromBanner() {
        vaultReindexBannerDismissed = false;
        const result = await ns.reindexAll();
        if (result?.msg) err.showError(`✓ ${result.msg}.`);
        await refreshVaultReindexBanner();
    }

    /** Drop session keys for a folder subtree and refresh lists (password stays set). */
    async function lockFolderSession(id) {
        const subtree = folderSubtreeIds(fs.folders, id);
        const fid = ns.activeNote?.folder_id;
        if (fid != null && subtree.has(fid)) {
            ns.clearActiveNote();
        }
        await fs.lockFolderSession(id);
        await tick();
        await loadNotes();
    }

    // Context menu service — needs coordinator callbacks, so created after them.
    const ctx = createContextMenuService({
        ns,
        ts,
        fs,
        bm,
        tmpl,
        is,
        settings,
        closeTab,
        closeOtherTabs,
        startTabRenameExternal,
        openNoteInNewTab,
        deleteNote,
        deleteFolder,
        selectFolder,
        openKanbanTab,
        startNoteInline,
        sendSelectionToChat,
        loadNotes,
        lockFolderSession,
        onError: err.showError,
    });

    // Keyboard shortcut handler.
    const kbd = createKeyboardService({
        ns,
        ts,
        fs,
        vault,
        layout,
        ui,
        tmpl,
        activateTab,
        closeTab,
        saveNote,
        newTab,
        startNoteInline,
        lockVault,
        sendSelectionToChat,
        deleteNote,
    });

    // ── Core state ─────────────────────────────────────────────────────────────

    // notes, activeNote, editorTitle, editorContent, isDirty, indexState, editorTextareaEl,
    // noteTags, noteLinks, noteBacklinks, unlinkedMentions, tagFilter, allTags,
    // noteDeletePending, isSeeding, isReindexing all live in noteService (ns).

    // ── Tab state ─────────────────────────────────────────────────────────────────
    // tabs, activeTabId, activeTab, makeTabId, chatInsert, searchOpen, tableViewOpen,
    // activeViewFilters, externalRenameTabId all live in tabService (ts).

    // improveState, refineState and all improve/refine functions live in improveService (is).

    // Search panel
    // searchOpen lives in tabService (ts).

    // isSeeding, isReindexing live in noteService (ns).

    // Tags and links, tagFilter, allTags live in noteService (ns).

    // Inline-rename state and folder expand state live in folderService (fs).

    // Settings overlay — state lives in uiService (ui).

    // Database / table view
    // tableViewOpen lives in tabService (ts).
    // folderHasProperties lives in folderService (fs)
    // activeViewFilters lives in tabService (ts).

    // Password / lock state (vault state in vaultService, folder state in folderService)
    // noteDeletePending lives in noteService (ns)
    // folderDeletePending, folderUnlockTarget, folderPwModal, unlockedFolderIds live in folderService (fs)

    // Context menu — lives in contextMenuService (ctx).

    // Tab rename signal for TabBar
    // externalRenameTabId lives in tabService (ts).

    // Note drag state — lives in folderService (fs).

    // Prevent "not allowed" drag cursor in Tauri/WebView2.
    $effect(() => {
        const allow = (e) => e.preventDefault();
        document.addEventListener("dragover", allow);
        return () => document.removeEventListener("dragover", allow);
    });

    // Persist tab state to localStorage — handled by ts.setupPersistence() above.

    async function closeOtherTabs(keepId) {
        await ts.closeOtherTabs(
            keepId,
            ns.isDirty ? () => saveNote() : null,
            (note) => (note ? openNote(note) : ns.clearActiveNote()),
        );
    }

    function startTabRenameExternal(id) {
        ts.startTabRenameExternal(id);
    }

    function sendSelectionToChat() {
        let text = "";
        if (ns.editorTextareaEl) {
            const { selectionStart, selectionEnd, value } = ns.editorTextareaEl;
            text = value.slice(selectionStart, selectionEnd).trim();
        }
        if (!text && ns.activeNote) text = ns.activeNote.title;
        if (!text) return;
        layout.chatOpen = true;
        ts.chatInsert = { text, seq: (ts.chatInsert?.seq ?? 0) + 1 };
    }

    function insertIntoActiveNote(text) {
        if (!ns.editorTextareaEl || !ns.activeNote) return;
        const { selectionStart, value } = ns.editorTextareaEl;
        ns.editorContent =
            value.slice(0, selectionStart) +
            "\n\n" +
            text +
            "\n\n" +
            value.slice(selectionStart);
        ns.markDirty();
    }

    // ── Context ────────────────────────────────────────────────────────────────────
    setContext("ns", ns);
    setContext("ts", ts);
    setContext("is", is);
    setContext("fs", fs);
    setContext("vault", vault);
    setContext("settings", settings);
    setContext("layout", layout);
    setContext("bm", bm);
    setContext("tmpl", tmpl);
    setContext("ui", ui);
    setContext("err", err);
    setContext("ctx", ctx);

    // ── Error banner — errorMsg and showError live in errorService (err).

    // ── Data loading ────────────────────────────────────────────────────────────────

    async function loadFolders() {
        return fs.loadFolders();
    }

    async function loadNotes() {
        return ns.loadNotes(fs.selectedFolderId, ns.tagFilter);
    }

    async function loadAllTags() {
        return ns.loadAllTags();
    }

    // Context menu listener — managed via $effect so Svelte handles
    // cleanup automatically, including during HMR.
    $effect(() => ctx.setup());

    onMount(() => {
        let cancelled = false;
        /** @type {(() => void)[]} */
        let unsubs = [];

        (async () => {
            try {
                unsubs.push(
                    await listen("folder_unlock_index:progress", (ev) => {
                        const p = /** @type {any} */ (ev.payload);
                        folderUnlockReindex = {
                            rootFolderId: p.root_folder_id,
                            affectedFolderIds: p.affected_folder_ids ?? [],
                            processed: p.processed ?? 0,
                            total: p.total ?? 0,
                            embeddingChunks: p.embedding_chunks ?? null,
                        };
                    }),
                );
                unsubs.push(
                    await listen("folder_unlock_index:done", () => {
                        folderUnlockReindex = null;
                    }),
                );
                unsubs.push(
                    await listen("folder_unlock_index:error", (ev) => {
                        const p = /** @type {any} */ (ev.payload);
                        err.showError(
                            p?.message ??
                                "Semantic indexing failed for unlocked folder.",
                        );
                        folderUnlockReindex = null;
                    }),
                );
            } catch {
                /* ignore */
            }
            if (cancelled) unsubs.forEach((u) => u());
        })();

        const onNoteImported = () => loadNotes();
        window.addEventListener("grimoire:note-imported", onNoteImported);

        const onVaultDataChanged = () => {
            void loadFolders();
            void loadNotes();
            loadAllTags();
            tmpl.loadTemplates();
            bm.loadBookmarks();
        };
        window.addEventListener(
            "grimoire:vault-data-changed",
            onVaultDataChanged,
        );

        const onNavigateNote = async (e) => {
            const noteId = /** @type {CustomEvent} */ (e).detail?.noteId;
            if (!noteId) return;
            try {
                const note = await invoke("get_note", { id: noteId });
                ui.settingsOpen = false;
                navigateToNote(note);
            } catch {
                /* ignore */
            }
        };
        window.addEventListener("grimoire:navigate-note", onNavigateNote);

        (async () => {
            ts.setupPersistence();
            await vault.checkLockState();

            if (!vault.vaultLocked) {
                await Promise.all([loadFolders(), loadNotes(), restoreTabs()]);
                if (ts.tabs.length === 0) newTab();
                loadAllTags();
                tmpl.loadTemplates();
                bm.loadBookmarks();

                invoke("get_hardware_info")
                    .then((hw) => {
                        settings.hwCapability = hw.capability;
                        settings.llmForceEnabled = hw.llmForceEnabled;
                        settings.hardwareReport = hw;
                    })
                    .catch(() => {});

                invoke("get_setting", { key: "wikipedia_enabled" })
                    .then((v) => {
                        settings.wikipediaEnabled = v === "true";
                    })
                    .catch(() => {});

                void refreshVaultReindexBanner();
            }

            await tick();
            await getCurrentWindow().show();
            window.__GRIMOIRE_PERF_READY__ = true;
        })();

        return () => {
            cancelled = true;
            unsubs.forEach((u) => u());
            window.removeEventListener("grimoire:note-imported", onNoteImported);
            window.removeEventListener(
                "grimoire:vault-data-changed",
                onVaultDataChanged,
            );
            window.removeEventListener("grimoire:navigate-note", onNavigateNote);
        };
    });

    // ── Folder actions ──────────────────────────────────────────────────────────────

    async function startFolderInline() {
        return fs.startFolderInline();
    }

    async function confirmInlineRename() {
        // Delegate the DB call to folderService; handle cross-service
        // state updates (editor title, tab labels) here for notes.
        const result = await fs.confirmInlineRename();
        if (!result || result.type === "folder") return;

        // Note rename — update UI state across services.
        const { id, name } = result;
        await loadNotes();
        if (ns.activeNote?.id === id) {
            ns.editorTitle = name;
            ns.activeNote = { ...ns.activeNote, title: name };
            ts.tabs = ts.tabs.map((t) =>
                t.noteId === id ? { ...t, label: name } : t,
            );
        }
    }

    async function revealFolder(folderId) {
        if (!layout.foldersOpen) layout.foldersOpen = true;
        return fs.revealFolder(folderId, true);
    }

    function deleteFolder(id) {
        return fs.deleteFolder(id);
    }

    async function confirmDeleteFolder() {
        const result = await fs.confirmDeleteFolder();
        if (result === "refresh_notes") await loadNotes();
    }

    async function selectFolder(id) {
        if (ns.isDirty) await saveNote();
        fs.selectedFolderId = id;
        ns.tagFilter = null;
        ns.activeNote = null;
        ts.tableViewOpen = false;
        await loadNotes();
        fs.loadFolderPropertyDefs(id);
    }

    // ── Note actions ─────────────────────────────────────────────────────────────────

    async function handleImportNote() {
        const selected = await openFileDialog({
            directory: false,
            multiple: false,
            filters: [
                { name: "Supported files", extensions: ["txt", "md", "pdf"] },
            ],
        }).catch(() => null);
        if (!selected) return;
        const filePath = Array.isArray(selected) ? selected[0] : selected;
        const folderId =
            fs.selectedFolderId === "all"
                ? null
                : (fs.selectedFolderId ?? null);
        try {
            const note = await invoke("import_file_as_note", {
                filePath,
                folderId,
            });
            await loadNotes();
            navigateToNote(note);
            // Vector-index the note, then register it in the file scanner.
            // Use .finally() so add_scanned_path always runs even if index_note fails
            // (e.g. large PDFs with many chunks can time out or exceed Ollama capacity).
            // Both operations use the embed model so they must be sequential, not concurrent.
            invoke("index_note", {
                noteId: note.id,
                title: note.title,
                content: note.content,
            })
                .catch(() => {})
                .finally(() => {
                    invoke("add_scanned_path", {
                        path: filePath,
                        kind: "file",
                    }).catch(() => {});
                });
        } catch (e) {
            err.showError(e);
        }
    }

    async function startNoteInline(templateId = -1) {
        layout.notesOpen = true;
        const folderId =
            fs.selectedFolderId === "all"
                ? null
                : (fs.selectedFolderId ?? null);
        try {
            const note = await invoke("create_note", {
                title: "Untitled",
                folderId,
            });
            await loadNotes();
            if (folderId && templateId > 0) {
                try {
                    const defs = await invoke("apply_template_to_note", {
                        noteId: note.id,
                        folderId,
                        templateId,
                    });
                    fs.folderHasProperties = defs.length > 0;
                } catch {
                    /* non-fatal */
                }
            }
            navigateToNote(note);
            const template = tmpl.templates.find((t) => t.id === templateId);
            const templateContent = template?.content ?? "";
            if (templateContent) {
                ns.editorContent = templateContent;
                ns.isDirty = true;
                invoke("save_note_with_version", {
                    id: note.id,
                    title: "Untitled",
                    content: templateContent,
                }).catch(() => {});
            }
            ns.indexState = "indexing";
            invoke("index_note", {
                noteId: note.id,
                title: "Untitled",
                content: templateContent,
            })
                .then(() => {
                    ns.indexState = "idle";
                })
                .catch(() => {
                    ns.indexState = "error";
                });
            fs.inlineRenaming = {
                id: note.id,
                type: "note",
                value: "Untitled",
            };
        } catch (e) {
            err.showError(e);
        }
    }

    function openNote(note) {
        if (ns.isDirty && ns.activeNote && ns.activeNote.id !== note.id)
            saveNote();
        enhanceStateCancelIfDiff();
        ts.searchOpen = false;
        ns.openNote(note);
    }

    async function navigateToNote(note) {
        await ts.navigateToNote(note, ns.isDirty ? () => saveNote() : null);
        openNote(note);
    }

    async function openNoteInNewTab(note) {
        await ts.openNoteInNewTab(note, ns.isDirty ? () => saveNote() : null);
        openNote(note);
    }

    async function newTab() {
        await ts.newTab();
        ns.clearActiveNote();
    }

    async function closeNote() {
        if (is.improveState.status !== "idle") enhanceStateCancelIfDiff();
        if (ns.isDirty) await saveNote();
        ts.closeNoteInTab();
        ns.clearActiveNote();
    }

    function enhanceStateCancelIfDiff() {
        is.cancelIfActive();
    }

    async function activateTab(id) {
        if (ts.activeTabId === id) return;
        enhanceStateCancelIfDiff();
        await ts.activateTab(
            id,
            ns.isDirty ? () => saveNote() : null,
            (note) => (note ? openNote(note) : ns.clearActiveNote()),
        );
    }

    async function closeTab(id) {
        await ts.closeTab(
            id,
            () => (ns.isDirty ? saveNote() : Promise.resolve()),
            (note) => (note ? openNote(note) : ns.clearActiveNote()),
            async () => {
                await ts.newTab();
                ns.clearActiveNote();
            },
        );
    }

    function openGraphTab() {
        if (ns.isDirty) saveNote();
        ts.openGraphTab();
        ns.clearActiveNote();
    }

    function openCalendarTab() {
        if (ns.isDirty) saveNote();
        ts.openCalendarTab();
        ns.clearActiveNote();
    }

    function openKanbanTab(folderId, folderName) {
        if (ns.isDirty) saveNote();
        fs.selectedFolderId = folderId;
        ns.tagFilter = null;
        ts.tableViewOpen = false;
        loadNotes();
        fs.loadFolderPropertyDefs(folderId);
        ts.openKanbanTab(folderId, folderName);
        ns.clearActiveNote();
    }

    function openWikipediaArticle(bundleId, articlePath, title) {
        if (ns.isDirty) saveNote();
        ts.openWikipediaArticle(bundleId, articlePath, title);
        ns.clearActiveNote();
    }

    function updateWikipediaTab(bundleId, articlePath, title) {
        ts.updateWikipediaTab(bundleId, articlePath, title);
    }

    function openChatTab() {
        if (ns.isDirty) saveNote();
        ts.openChatTab();
        ns.clearActiveNote();
    }

    function renameTab(id, label) {
        ts.renameTab(id, label);
    }

    async function createDailyNote() {
        try {
            const note = await invoke("create_daily_note", {
                dateFormat: settings.dailyNoteFormat,
            });
            await loadNotes();
            openNoteInNewTab(note);
        } catch (e) {
            err.showError(e);
        }
    }

    async function restoreTabs() {
        await ts.restoreTabs((note) => openNote(note));
    }

    async function saveNote() {
        return ns.saveNote(() => loadNotes());
    }

    async function handleVersionRestore(restoredNote) {
        ns.applyRestoredNote(restoredNote);
        await loadNotes();
    }

    async function convertMention(mention) {
        return ns.convertMention(mention);
    }

    function deleteNote(id) {
        return ns.deleteNote(id);
    }

    async function confirmDeleteNote() {
        const result = await ns.confirmDeleteNote(
            async (id) => {
                const tab = ts.tabs.find(
                    (t) => t.type === "note" && t.noteId === id,
                );
                if (tab) await closeTab(tab.id);
            },
            () => bm.loadBookmarks(),
        );
        if (result === "refresh_notes") await loadNotes();
    }

    async function moveNote(noteId, targetFolderId) {
        const result = await ns.moveNote(noteId, targetFolderId);
        if (result === "refresh_notes") await loadNotes();
    }

    async function openNoteById(id) {
        return ns.openNoteById(id, navigateToNote);
    }

    async function filterByTag(tag) {
        ns.tagFilter = tag;
        fs.selectedFolderId = null;
        await loadNotes();
    }

    // ── Lock / unlock ───────────────────────────────────────────────────────────────

    async function onVaultUnlocked() {
        await vault.onVaultUnlocked(async () => {
            await loadFolders();
            await loadNotes();
            loadAllTags();
            await restoreTabs();
            if (ts.tabs.length === 0) newTab();
            invoke("reindex_all").catch(() => {});
            await refreshVaultReindexBanner();
        });
    }

    async function lockVault() {
        await vault.lockVault();
    }

    async function handleVaultPwSubmit(password) {
        return vault.handleVaultPwSubmit(password);
    }

    async function handleFolderUnlockSafe(password) {
        const ok = await fs.handleFolderUnlockSafe(password);
        if (ok) {
            await tick();
            await loadNotes();
        }
        return ok;
    }

    async function handleFolderPwSubmit(password) {
        if (!fs.folderPwModal) return true;
        const folderId = fs.folderPwModal.folderId;
        if (
            fs.folderPwModal.mode === "set" &&
            ns.activeNote?.folder_id === folderId
        ) {
            ns.clearActiveNote();
        }
        const ok = await fs.handleFolderPwSubmit(password);
        if (ok) await loadNotes();
        return ok;
    }

    // ── Keyboard shortcuts — handled by keyboardService (kbd).

    async function seedNotes() {
        const result = await ns.seedNotes();
        if (result?.count != null) {
            err.showError(`✓ Seeded ${result.count} notes and indexed them.`);
        }
    }

    async function reindexAll() {
        const result = await ns.reindexAll();
        if (result?.msg) err.showError(`✓ ${result.msg}.`);
        await refreshVaultReindexBanner();
    }
</script>

<svelte:window onkeydown={(e) => kbd.handle(e)} />
<svelte:document onmousemove={layout.onDragMove} onmouseup={layout.onDragEnd} />

{#if !vault.lockCheckDone}
    <!-- Blank while we check vault lock state to avoid a flash of content -->
{:else if vault.vaultLocked}
    <LockScreen onUnlocked={onVaultUnlocked} />
{:else}
    {#if err.errorMsg}
        <div class="error-banner" role="alert">{err.errorMsg}</div>
    {/if}

    {#if vaultReindexBanner && !ns.isReindexing}
        <div class="vault-reindex-banner" role="status">
            <span>
                Semantic search re-index is incomplete:
                <strong>{vaultReindexBanner.indexedOk}</strong> notes embedded so far,
                <strong>{vaultReindexBanner.nextPos}</strong> of
                <strong>{vaultReindexBanner.total}</strong> unlockable notes processed.
                Already-embedded notes stay searchable. Resume to continue (model:
                {vaultReindexBanner.embeddingModel}).
            </span>
            <span class="banner-actions">
                <button type="button" class="primary" onclick={resumeVaultReindexFromBanner}>
                    Resume
                </button>
                <button type="button" onclick={dismissVaultReindexBanner}>Dismiss</button>
            </span>
        </div>
    {/if}

    <!-- Password modals (rendered above everything) -->
    {#if fs.folderUnlockTarget}
        <PasswordModal
            title="Locked folder"
            confirmLabel="Unlock"
            onSubmit={handleFolderUnlockSafe}
            onCancel={() => (fs.folderUnlockTarget = null)}
        />
    {/if}

    {#if vault.vaultPwModal === "set"}
        <PasswordModal
            title="Set vault password"
            confirmLabel="Set password"
            warning="If you forget this password, your notes cannot be recovered. There is no reset option."
            requireAck={true}
            onSubmit={handleVaultPwSubmit}
            onCancel={() => (vault.vaultPwModal = null)}
        />
    {:else if vault.vaultPwModal === "change"}
        <PasswordModal
            title="Change vault password"
            confirmLabel="Set new password"
            warning="If you forget this password, your notes cannot be recovered. There is no reset option."
            requireAck={true}
            onSubmit={handleVaultPwSubmit}
            onCancel={() => (vault.vaultPwModal = null)}
        />
    {:else if vault.vaultPwModal === "remove"}
        <PasswordModal
            title="Remove vault password"
            confirmLabel="Remove password"
            onSubmit={handleVaultPwSubmit}
            onCancel={() => (vault.vaultPwModal = null)}
        />
    {/if}

    {#if fs.folderPwModal?.mode === "set"}
        <PasswordModal
            title="Set folder password"
            confirmLabel="Set password"
            warning="If you forget this password, notes in this folder cannot be recovered."
            requireAck={true}
            onSubmit={handleFolderPwSubmit}
            onCancel={() => (fs.folderPwModal = null)}
        />
    {:else if fs.folderPwModal?.mode === "remove"}
        <PasswordModal
            title="Remove folder password"
            confirmLabel="Remove password"
            onSubmit={handleFolderPwSubmit}
            onCancel={() => (fs.folderPwModal = null)}
        />
    {/if}

    {#if tmpl.templateModalOpen}
        <TemplateModal
            onSave={tmpl.saveTemplate}
            onCancel={() => (tmpl.templateModalOpen = false)}
        />
    {:else if tmpl.editingTemplate}
        <TemplateModal
            template={tmpl.editingTemplate}
            onSave={tmpl.updateTemplate}
            onCancel={() => (tmpl.editingTemplate = null)}
        />
    {/if}

    {#if ns.noteDeletePending}
        <ConfirmModal
            title="Delete note"
            message={"Are you sure you want to delete \u201c" +
                ns.noteDeletePending.title +
                "\u201d?"}
            confirmLabel="Delete"
            onConfirm={confirmDeleteNote}
            onCancel={() => (ns.noteDeletePending = null)}
        />
    {/if}

    {#if fs.folderDeletePending}
        <ConfirmModal
            title="Delete folder"
            message={"Are you sure you want to delete \u201c" +
                fs.folderDeletePending.name +
                "\u201d? Notes inside will become unfiled."}
            confirmLabel="Delete"
            onConfirm={confirmDeleteFolder}
            onCancel={() => (fs.folderDeletePending = null)}
        />
    {/if}

    <!-- ── Activity bar ───────────────────────────────────────────────────── -->
    <ActivityBar
        searchActive={ts.searchOpen}
        showLock={vault.vaultHasPassword}
        onSearch={() => (ts.searchOpen = !ts.searchOpen)}
        onGraph={openGraphTab}
        onCalendar={openCalendarTab}
        onDailyNote={createDailyNote}
        onQuickSwitcher={() => (ui.quickSwitcherOpen = true)}
        wikipediaEnabled={settings.wikipediaEnabled}
        onWikipedia={() => (ui.wikiSearchOpen = true)}
        onLock={lockVault}
        onSettings={() => (ui.settingsOpen = true)}
        onHelp={() => openUrl("https://grimoire.app")}
        onForum={() => openUrl("https://grimoire.app/forum")}
    />

    <!-- ── Custom title bar ─────────────────────────────────────────────── -->
    <div class="titlebar">
        <div class="titlebar-left">
            <button
                class="titlebar-btn"
                onclick={() => (layout.foldersOpen = !layout.foldersOpen)}
                title="Toggle folders"
            >
                <svg
                    width="15"
                    height="15"
                    viewBox="0 0 15 15"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.5"
                    stroke-linecap="round"
                >
                    <rect x="1" y="1" width="13" height="13" rx="1" />
                    <line x1="5" y1="1" x2="5" y2="14" />
                </svg>
            </button>
            <button
                class="titlebar-btn"
                onclick={() => (layout.notesOpen = !layout.notesOpen)}
                title="Toggle notes list"
            >
                <svg
                    width="15"
                    height="15"
                    viewBox="0 0 15 15"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.5"
                    stroke-linecap="round"
                >
                    <line x1="4" y1="4" x2="11" y2="4" />
                    <line x1="4" y1="7.5" x2="11" y2="7.5" />
                    <line x1="4" y1="11" x2="9" y2="11" />
                </svg>
            </button>
        </div>

        <!-- Tab strip — replaces the old "Grimoire" drag region -->
        <TabBar
            onActivate={activateTab}
            onClose={closeTab}
            onRename={renameTab}
            onNew={newTab}
        />

        <div class="titlebar-right">
            <button
                class="titlebar-btn"
                class:titlebar-btn-active={layout.focusMode}
                onclick={layout.toggleFocusMode}
                title="Focus mode (F11)"
            >
                <!-- Compress icon when in focus mode, expand icon when normal -->
                {#if layout.focusMode}
                    <svg
                        width="15"
                        height="15"
                        viewBox="0 0 15 15"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <polyline points="14,6 14,1 9,1" />
                        <polyline points="1,9 1,14 6,14" />
                        <line x1="14" y1="1" x2="9" y2="6" />
                        <line x1="1" y1="14" x2="6" y2="9" />
                    </svg>
                {:else}
                    <svg
                        width="15"
                        height="15"
                        viewBox="0 0 15 15"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <polyline points="1,6 1,1 6,1" />
                        <polyline points="9,14 14,14 14,9" />
                        <line x1="1" y1="1" x2="6" y2="6" />
                        <line x1="14" y1="14" x2="9" y2="9" />
                    </svg>
                {/if}
            </button>
            <button
                class="titlebar-btn"
                class:titlebar-btn-active={layout.chatOpen}
                onclick={() => (layout.chatOpen = !layout.chatOpen)}
                title={settings.llmEnabled
                    ? "Toggle chat"
                    : "Chat unavailable — check Hardware settings"}
                disabled={!settings.llmEnabled}
            >
                <svg
                    width="15"
                    height="15"
                    viewBox="0 0 15 15"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                >
                    <path
                        d="M2 2h11a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H5l-3 3V3a1 1 0 0 1 1-1z"
                    />
                </svg>
            </button>
        </div>

        <div class="titlebar-winctl">
            <button
                class="winctl-btn"
                onclick={() => appWindow.minimize()}
                title="Minimise"
                aria-label="Minimise"
            >
                <svg
                    width="11"
                    height="11"
                    viewBox="0 0 11 11"
                    fill="currentColor"
                    ><rect x="0" y="5" width="11" height="1" /></svg
                >
            </button>
            <button
                class="winctl-btn"
                onclick={() => appWindow.toggleMaximize()}
                title="Maximise"
                aria-label="Maximise"
            >
                <svg
                    width="11"
                    height="11"
                    viewBox="0 0 11 11"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1"
                    ><rect x="0.5" y="0.5" width="10" height="10" /></svg
                >
            </button>
            <button
                class="winctl-btn close"
                onclick={() => appWindow.close()}
                title="Close"
                aria-label="Close"
            >
                <svg
                    width="11"
                    height="11"
                    viewBox="0 0 11 11"
                    stroke="currentColor"
                    stroke-width="1.2"
                    stroke-linecap="round"
                >
                    <line x1="1" y1="1" x2="10" y2="10" /><line
                        x1="10"
                        y1="1"
                        x2="1"
                        y2="10"
                    />
                </svg>
            </button>
        </div>
    </div>

    <div class="layout" style:grid-template-columns={layout.gridCols}>
        <!-- Sidebar: Folders -->
        <aside class="sidebar" class:collapsed={!layout.foldersOpen}>
            {#if layout.foldersOpen}
                <FolderSidebar
                    isDragging={fs.isDragging}
                    onSelectFolder={selectFolder}
                    onCreateNote={() => startNoteInline()}
                    onImportNote={() => handleImportNote()}
                    onCreateFolder={() => startFolderInline()}
                    onDeleteFolder={deleteFolder}
                    onOpenNoteById={openNoteById}
                    onOpenNoteInNewTab={openNoteInNewTab}
                    onFilterByTag={filterByTag}
                    onDeleteTemplate={(id) =>
                        tmpl.deleteTemplate(id, err.showError)}
                    onConfirmInlineRename={confirmInlineRename}
                    onMoveNote={moveNote}
                    onMoveFolder={loadFolders}
                    onLockFolderSession={lockFolderSession}
                />
            {:else}
                <button
                    class="collapsed-strip"
                    onclick={() => (layout.foldersOpen = true)}
                    title="Expand folders"
                >
                    <span>Folders</span>
                </button>
            {/if}
        </aside>

        <button
            class="panel-divider folders-divider"
            aria-label="Resize folders panel"
            class:dragging={layout.activeDrag?.panel === "folders"}
            onmousedown={(e) => layout.startDrag("folders", e)}
        ></button>

        <!-- Note list -->
        <div class="note-list" class:collapsed={!layout.notesOpen}>
            {#if layout.notesOpen}
                <NoteList
                    {folderUnlockReindex}
                    onOpenNote={navigateToNote}
                    onOpenNoteInNewTab={openNoteInNewTab}
                    onDeleteNote={deleteNote}
                    onConfirmInlineRename={confirmInlineRename}
                    onOpenKanbanTab={openKanbanTab}
                    onSaveNote={saveNote}
                    onSeedNotes={seedNotes}
                    onReindexAll={reindexAll}
                    onTableViewToggle={() => {
                        if (ns.isDirty) saveNote();
                        const kanban = ts.tabs.find(
                            (t) =>
                                t.type === "kanban" &&
                                t.folderId === fs.selectedFolderId,
                        );
                        if (kanban) {
                            ts.tabs = ts.tabs.filter((t) => t.id !== kanban.id);
                            if (ts.activeTabId === kanban.id) {
                                ns.clearActiveNote();
                            }
                        }
                        ts.tableViewOpen = !ts.tableViewOpen;
                    }}
                    onNoteDragStart={fs.onNoteDragStart}
                    onNoteDragEnd={fs.onNoteDragEnd}
                />
            {:else}
                <button
                    class="collapsed-strip"
                    onclick={() => (layout.notesOpen = true)}
                    title="Expand notes"
                >
                    <span>Notes</span>
                </button>
            {/if}
        </div>

        <button
            class="panel-divider notes-divider"
            aria-label="Resize notes panel"
            class:dragging={layout.activeDrag?.panel === "notes"}
            onmousedown={(e) => layout.startDrag("notes", e)}
        ></button>

        <!-- Editor -->
        <main class="editor">
            <div style="display: {ts.searchOpen ? 'contents' : 'none'};">
                <Search
                    onSelectNote={(id) => {
                        openNoteById(id);
                        ts.searchOpen = false;
                    }}
                />
            </div>
            {#if !ts.searchOpen}
                {#if ts.tableViewOpen && fs.selectedFolderId && fs.selectedFolderId !== "all"}
                    <div class="tab-fullview">
                        <button
                            class="tab-fullview-close"
                            onclick={() => (ts.tableViewOpen = false)}
                            title="Close table">✕ Close</button
                        >
                        {#key tmpl.dbKey}
                            <DatabaseView
                                folderId={fs.selectedFolderId}
                                onOpenNote={(id) => {
                                    ts.tableViewOpen = false;
                                    openNoteById(id);
                                }}
                                onFiltersChange={(f) =>
                                    (ts.activeViewFilters = f)}
                            />
                        {/key}
                    </div>
                {:else if ts.activeTab?.type === "graph"}
                    <div class="tab-fullview">
                        <button
                            class="tab-fullview-close"
                            onclick={() => closeTab(ts.activeTabId)}
                            title="Close graph">✕ Close</button
                        >
                        <Graph
                            onSelectNote={(id) => openNoteById(id)}
                            activeNoteId={ns.activeNote?.id ?? null}
                            theme={settings.theme}
                        />
                    </div>
                {:else if ts.activeTab?.type === "calendar"}
                    <div class="tab-fullview">
                        <button
                            class="tab-fullview-close"
                            onclick={() => closeTab(ts.activeTabId)}
                            title="Close calendar">✕ Close</button
                        >
                        <Calendar
                            onSelectNote={(note) => navigateToNote(note)}
                            onRefresh={() => {
                                loadFolders();
                                loadNotes();
                            }}
                            onSelectFolder={selectFolder}
                            dateFormat={settings.dailyNoteFormat}
                        />
                    </div>
                {:else if ts.activeTab?.type === "kanban"}
                    <div class="tab-fullview">
                        <button
                            class="tab-fullview-close"
                            onclick={() => closeTab(ts.activeTabId)}
                            title="Close kanban">✕ Close</button
                        >
                        <Kanban
                            folderId={ts.activeTab.folderId}
                            onOpenNote={(id) => openNoteById(id)}
                        />
                    </div>
                {:else if ts.activeTab?.type === "wikipedia"}
                    <div class="tab-fullview">
                        <WikipediaReader
                            bundleId={ts.activeTab.bundleId}
                            articlePath={ts.activeTab.articlePath}
                            bundleName={ts.activeTab.label}
                            onArticleNavigate={(bid, apath, title) =>
                                updateWikipediaTab(bid, apath, title)}
                            onOpenArticle={(bid, apath, title) =>
                                openWikipediaArticle(bid, apath, title)}
                            onClose={() => closeTab(ts.activeTabId)}
                        />
                    </div>
                {:else if ts.activeTab?.type === "chat"}
                    <Chat
                        suppressNoteContext={true}
                        onClose={() => closeTab(ts.activeTabId)}
                        onContextMenu={(x, y, items) =>
                            (ctx.ctxMenu = { x, y, items })}
                        onOpenWikipediaArticle={openWikipediaArticle}
                    />
                {:else if ns.activeNote}
                    <NoteEditor
                        onSave={saveNote}
                        onCloseNote={closeNote}
                        onMoveNote={moveNote}
                        onRevealFolder={revealFolder}
                        onOpenKanbanTab={openKanbanTab}
                        onOpenNoteById={openNoteById}
                        onFilterByTag={filterByTag}
                        onConvertMention={convertMention}
                        onVersionRestore={handleVersionRestore}
                        onExportError={err.showError}
                        onOpenTableView={() => {
                            if (ns.isDirty) saveNote();
                            const kanban = ts.tabs.find(
                                (t) =>
                                    t.type === "kanban" &&
                                    t.folderId === ns.activeNote?.folder_id,
                            );
                            if (kanban) {
                                ts.tabs = ts.tabs.filter(
                                    (t) => t.id !== kanban.id,
                                );
                                if (ts.activeTabId === kanban.id) {
                                    ns.clearActiveNote();
                                }
                            }
                            fs.selectedFolderId = ns.activeNote?.folder_id;
                            ns.clearActiveNote();
                            ts.tableViewOpen = true;
                        }}
                    />
                {:else}
                    <div class="empty-editor">Select or create a note</div>
                {/if}
            {/if}
        </main>

        {#if layout.chatOpen && ts.activeTab?.type !== "chat"}
            <button
                class="panel-divider chat-divider"
                aria-label="Resize chat panel"
                class:dragging={layout.activeDrag?.panel === "chat"}
                onmousedown={(e) => layout.startDrag("chat", e)}
            ></button>
            <Chat
                onClose={() => (layout.chatOpen = false)}
                onContextMenu={(x, y, items) => (ctx.ctxMenu = { x, y, items })}
                onInsertIntoNote={ns.activeNote ? insertIntoActiveNote : null}
                onOpenWikipediaArticle={openWikipediaArticle}
            />
        {/if}
    </div>

    {#if ui.quickSwitcherOpen}
        <QuickSwitcher
            onSelect={(note) => navigateToNote(note)}
            onSelectNewTab={(note) => openNoteInNewTab(note)}
            onClose={() => (ui.quickSwitcherOpen = false)}
        />
    {/if}

    {#if ui.wikiSearchOpen}
        <WikipediaSearchModal
            onOpenArticle={(bid, apath, title) => {
                openWikipediaArticle(bid, apath, title);
                ui.wikiSearchOpen = false;
            }}
            onClose={() => (ui.wikiSearchOpen = false)}
        />
    {/if}

    {#if ui.settingsOpen}
        <Settings
            onClose={() => (ui.settingsOpen = false)}
            vaultHasPassword={vault.vaultHasPassword}
            onSetVaultPassword={() => (vault.vaultPwModal = "set")}
            onChangeVaultPassword={() => (vault.vaultPwModal = "change")}
            onRemoveVaultPassword={() => (vault.vaultPwModal = "remove")}
            onLockVault={lockVault}
            keepInMemory={settings.keepModelInMemory}
            onKeepInMemoryChange={(v) => (settings.keepModelInMemory = v)}
            accent={settings.accent}
            onAccentChange={(v) => (settings.accent = v)}
            theme={settings.theme}
            onThemeChange={(v) => (settings.theme = v)}
            dateFormat={settings.dailyNoteFormat}
            onDateFormatChange={(v) => (settings.dailyNoteFormat = v)}
            devNativeContextMenu={settings.devNativeContextMenu}
            onDevNativeContextMenuChange={(v) =>
                (settings.devNativeContextMenu = v)}
            llmEnabled={settings.llmEnabled}
            onHardwareChange={(cap, force) => {
                settings.hwCapability = cap;
                settings.llmForceEnabled = force;
                invoke("get_hardware_info")
                    .then((hw) => {
                        settings.hardwareReport = hw;
                    })
                    .catch(() => {});
            }}
            wikipediaEnabled={settings.wikipediaEnabled}
            onWikipediaEnabledChange={(v) => (settings.wikipediaEnabled = v)}
        />
    {/if}

    {#if ctx.ctxMenu}
        <ContextMenu
            x={ctx.ctxMenu.x}
            y={ctx.ctxMenu.y}
            items={ctx.ctxMenu.items}
            onClose={ctx.close}
        />
    {/if}
{/if}
<!-- end of vault-unlocked block -->
