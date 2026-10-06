<script>
    import { getContext, tick } from "svelte";
    import NoteBodyTextarea from "./NoteBodyTextarea.svelte";
    import NoteProperties from "./NoteProperties.svelte";
    import DiffView from "./DiffView.svelte";
    import ImprovePopover from "./ImprovePopover.svelte";
    import { renderTransclusionMarkdownToHtml } from "./utils/transclusion.js";
    import {
        exportNoteHtml,
        exportNoteMarkdown,
        exportNotePdfPrint,
    } from "./utils/noteExportActions.js";
    import { applyEditorTab } from "./utils/editorIndent.js";
    import {
        applyChecklistToggle,
        applyListEnter,
        applyListTab,
        toggleChecklistAtIndex,
    } from "./utils/editorLists.js";
    import { editorReadableStats } from "./utils/readableText.js";
    import { t, tp } from "./i18n/t.js";

    const ns = getContext("ns");
    const ts = getContext("ts");
    const is = getContext("is");
    const fs = getContext("fs");
    const settings = getContext("settings");

    const llmImproveDisabled = $derived(!settings.llmEnabled);
    const improveTooltip = $derived(
        llmImproveDisabled
            ? t("notes.improveDisabled")
            : t("notes.suggestImprovements"),
    );

    // ── Composed callbacks (still provided by the coordinator) ────────────────
    let {
        onSave,
        onCloseNote,
        onMoveNote,
        onRevealFolder,
        onOpenKanbanTab,
        onOpenNoteById,
        onFilterByTag,
        onConvertMention,
        onOpenTableView,
        onVersionRestore,
        onExportError = () => {},
    } = $props();

    /** @type {HTMLDetailsElement | null} */
    let exportDetailsEl = $state(null);
    let exportMenuOpen = $state(false);
    let exportFocusPos = $state(0);

    function closeExportMenu() {
        if (exportDetailsEl) exportDetailsEl.open = false;
        exportMenuOpen = false;
    }

    async function handleExportMenuToggle(e) {
        exportMenuOpen = exportDetailsEl?.open ?? false;
        if (exportMenuOpen) {
            exportFocusPos = 0;
            await tick();
            exportDetailsEl?.querySelector('[role="menuitem"]')?.focus();
        }
    }

    async function handleExportMenuKeydown(e) {
        const items = Array.from(
            exportDetailsEl?.querySelectorAll('[role="menuitem"]') ?? [],
        );
        if (items.length === 0) return;

        if (e.key === "ArrowDown") {
            e.preventDefault();
            exportFocusPos = (exportFocusPos + 1) % items.length;
            await tick();
            /** @type {HTMLElement} */ (items[exportFocusPos])?.focus();
        } else if (e.key === "ArrowUp") {
            e.preventDefault();
            exportFocusPos = (exportFocusPos - 1 + items.length) % items.length;
            await tick();
            /** @type {HTMLElement} */ (items[exportFocusPos])?.focus();
        } else if (e.key === "Escape") {
            e.preventDefault();
            closeExportMenu();
            exportDetailsEl?.querySelector("summary")?.focus();
        }
    }

    // ── Local derived / state ─────────────────────────────────────────────────
    const activeTab = $derived(
        ts.tabs.find((t) => t.id === ts.activeTabId) ?? null,
    );
    const readableStats = $derived(
        editorReadableStats({
            locked: ns.activeNote?.locked,
            body: ns.editorContent,
            wpm: settings.readingWpm,
        }),
    );
    const wordCount = $derived(readableStats?.wordCount ?? 0);
    const readingTime = $derived(readableStats?.readingMinutes ?? 0);
    const wordCountLabel = $derived(
        readableStats
            ? tp("editor.wordCountLabel", wordCount, { readingTime })
            : "",
    );

    let propertiesReady = $state(!ns.activeNote?.folder_id);
    let loadedNoteId = ns.activeNote?.id ?? null;

    // Reset propertiesReady when the active note changes (by id, not object replacement on save).
    $effect(() => {
        const note = ns.activeNote;
        if (!note) return;
        if (note.id === loadedNoteId) return;
        loadedNoteId = note.id;
        propertiesReady = !note.folder_id;
    });

    function handlePropertiesLoad(defs) {
        propertiesReady = true;
        fs.folderHasProperties = defs.length > 0;
    }

    /** Populated in read mode via {@link renderTransclusionMarkdownToHtml}. */
    let readModeHtml = $state("");

    $effect(() => {
        const read = activeTab?.readMode;
        const idle = is.improveState.status === "idle";
        const content = ns.editorContent;
        /** Reactive dependency — bumps after save so embedded notes refresh. */
        ns.transclusionRefresh;
        const rootId = ns.activeNote?.id;

        if (!read || !idle) {
            readModeHtml = "";
            return;
        }

        let cancelled = false;
        renderTransclusionMarkdownToHtml(content ?? "", {
            rootNoteId: rootId,
            interactiveChecklists: !ns.activeNote?.locked,
        }).then((html) => {
            if (!cancelled) readModeHtml = html;
        });
        return () => {
            cancelled = true;
        };
    });

    function handleReadModeChecklistChange(e) {
        if (ns.activeNote?.locked) return;
        const target = /** @type {EventTarget | null} */ (e.target);
        if (!(target instanceof HTMLInputElement)) return;
        if (target.type !== "checkbox") return;
        const raw = target.getAttribute("data-checklist-index");
        if (raw == null) return;
        const index = Number(raw);
        if (!Number.isInteger(index)) return;

        const next = toggleChecklistAtIndex(ns.editorContent ?? "", index);
        if (!next) return;
        ns.editorContent = next.value;
        ns.markDirty();
        onSave?.();
    }

    // ── Editor keydown (lists, Tab indent, wiki-link brackets) ────────────────
    function applyEditorEdit(el, next) {
        ns.editorContent = next.value;
        // Sync the DOM immediately so caret restore is not racing the bind effect.
        el.value = next.value;
        ns.markDirty();
        el.selectionStart = next.selectionStart;
        el.selectionEnd = next.selectionEnd;
    }

    function isEnterKey(e) {
        return (
            e.key === "Enter" ||
            e.code === "Enter" ||
            e.code === "NumpadEnter"
        );
    }

    function handleEditorKeydown(e) {
        const el = /** @type {HTMLTextAreaElement} */ (e.currentTarget);
        const { selectionStart: start, selectionEnd: end, value } = el;

        // Ctrl/Cmd+Enter: toggle checklist on the current line.
        // Always preventDefault so the browser does not insert a newline.
        if (
            isEnterKey(e) &&
            (e.ctrlKey || e.metaKey) &&
            !e.shiftKey &&
            !e.altKey
        ) {
            e.preventDefault();
            e.stopPropagation();
            const next = applyChecklistToggle(value, start, end);
            if (next) applyEditorEdit(el, next);
            return;
        }

        // Enter: continue / exit / split Markdown list items.
        if (
            isEnterKey(e) &&
            !e.ctrlKey &&
            !e.metaKey &&
            !e.altKey &&
            !e.shiftKey
        ) {
            const listNext = applyListEnter(value, start, end);
            if (listNext) {
                e.preventDefault();
                applyEditorEdit(el, listNext);
            }
            return;
        }

        // Tab / Shift+Tab: nest list lines, otherwise soft-indent.
        if (e.key === "Tab" && !e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            const listNext = applyListTab(value, start, end, {
                shiftKey: e.shiftKey,
            });
            const next =
                listNext ??
                applyEditorTab(value, start, end, { shiftKey: e.shiftKey });
            applyEditorEdit(el, next);
            return;
        }

        if (e.key !== "[") return;
        const prevChar = value[start - 1];
        e.preventDefault();
        if (prevChar === "[") {
            const before = value.slice(0, start - 1);
            const after = value.slice(end + (value[end] === "]" ? 1 : 0));
            const cursor = before.length + 2;
            ns.editorContent = before + "[[]]" + after;
            ns.markDirty();
            requestAnimationFrame(() => {
                el.selectionStart = cursor;
                el.selectionEnd = cursor;
            });
        } else {
            const before = value.slice(0, start);
            const after = value.slice(end);
            const cursor = before.length + 1;
            ns.editorContent = before + "[]" + after;
            ns.markDirty();
            requestAnimationFrame(() => {
                el.selectionStart = cursor;
                el.selectionEnd = cursor;
            });
        }
    }
</script>

<div class="editor-toolbar">
    <input
        class="title-input"
        bind:value={ns.editorTitle}
        oninput={ns.markDirty}
        placeholder={t("notes.noteTitlePlaceholder")}
        aria-label={t("notes.noteTitleAria")}
    />
    <div class="toolbar-actions">
        <label>
            {t("notes.moveTo")}
            <select
                onchange={(e) => {
                    const v = /** @type {HTMLSelectElement} */ (e.target).value;
                    onMoveNote?.(
                        ns.activeNote.id,
                        v === "null" ? null : Number(v),
                    );
                }}
            >
                <option value="null">{t("folders.unfiled")}</option>
                {#each fs.folders as f (f.id)}
                    <option
                        value={f.id}
                        selected={ns.activeNote.folder_id === f.id}
                        >{f.name}</option
                    >
                {/each}
            </select>
        </label>
        <button
            class="save-note-btn"
            onclick={onSave}
            disabled={!ns.isDirty}
            class:index-error={!ns.isDirty && ns.indexState === "error"}
        >
            {ns.isDirty
                ? t("notes.saveShortcut")
                : ns.indexState === "indexing"
                  ? t("notes.indexing")
                  : ns.indexState === "error"
                    ? t("notes.indexFailedIcon")
                    : t("notes.saved")}
        </button>
        <span class="sr-only" aria-live="polite" aria-atomic="true">
            {ns.isDirty
                ? t("notes.unsaved")
                : ns.indexState === "indexing"
                  ? t("notes.indexingShort")
                  : ns.indexState === "error"
                    ? t("notes.indexFailed")
                    : t("notes.saved")}
        </span>
        {#if fs.folderHasProperties}
            <button
                class="graph-toggle"
                aria-label={t("notes.switchTable")}
                onclick={onOpenTableView}>{t("notes.tableBack")}</button
            >
        {/if}
        {#if ns.activeNote.folder_id != null && fs.folders.some((f) => f.id === ns.activeNote.folder_id)}
            <button
                class="graph-toggle"
                aria-label={t("notes.switchBoard")}
                onclick={() =>
                    onOpenKanbanTab?.(
                        ns.activeNote.folder_id,
                        fs.folders.find((f) => f.id === ns.activeNote.folder_id)
                            ?.name ?? "",
                    )}
            >
                {t("notes.boardBack")}
            </button>
        {/if}
        {#if ns.activeNote.folder_id}
            <button
                class="graph-toggle"
                onclick={() => onRevealFolder?.(ns.activeNote.folder_id)}
                title={t("notes.revealInFolders")}
                aria-label={t("notes.revealInFolders")}>{t("notes.reveal")}</button
            >
        {/if}
        <button
            class="graph-toggle"
            aria-label={t("notes.suggestImprovements")}
            title={improveTooltip}
            onclick={is.startImprove}
            disabled={llmImproveDisabled || is.improveState.status !== "idle" || !ns.editorContent}
        >
            {t("notes.improve")}
        </button>
        {#if !ns.activeNote.locked}
            <details
                class="toolbar-export"
                bind:this={exportDetailsEl}
                ontoggle={handleExportMenuToggle}
            >
                <summary
                    class="graph-toggle export-summary"
                    aria-haspopup="menu"
                    aria-expanded={exportMenuOpen}
                    aria-label={t("notes.exportNote")}
                    title={t("notes.exportNote")}
                    >{t("common.export")}</summary
                >
                <div class="toolbar-export-menu" role="menu" tabindex="-1" onkeydown={handleExportMenuKeydown}>
                    <button
                        type="button"
                        role="menuitem"
                        class="toolbar-export-item"
                        onclick={() => {
                            exportNoteMarkdown({
                                noteId: ns.activeNote.id,
                                title: ns.editorTitle,
                                body: ns.editorContent,
                                onError: onExportError,
                            });
                            closeExportMenu();
                        }}
                        >{t("contextMenu.markdown")}</button
                    >
                    <button
                        type="button"
                        role="menuitem"
                        class="toolbar-export-item"
                        onclick={() => {
                            exportNoteHtml({
                                noteId: ns.activeNote.id,
                                title: ns.editorTitle,
                                body: ns.editorContent,
                                onError: onExportError,
                            });
                            closeExportMenu();
                        }}
                        >{t("contextMenu.html")}</button
                    >
                    <button
                        type="button"
                        role="menuitem"
                        class="toolbar-export-item"
                        onclick={() => {
                            exportNotePdfPrint({
                                noteId: ns.activeNote.id,
                                title: ns.editorTitle,
                                body: ns.editorContent,
                                onError: onExportError,
                            });
                            closeExportMenu();
                        }}
                        >{t("contextMenu.pdf")}</button
                    >
                </div>
            </details>
        {/if}
        <button
            class="graph-toggle"
            aria-label={activeTab?.readMode
                ? t("notes.editMode")
                : t("notes.readMode")}
            onclick={ts.toggleReadMode}
        >
            {activeTab?.readMode ? t("notes.editModeShort") : t("notes.readModeShort")}
        </button>
        <button
            class="close-note-btn"
            aria-label={t("notes.closeNote")}
            title={t("notes.closeNote")}
            onclick={onCloseNote}>✕</button
        >
        {#if readableStats}
            <span class="word-count" aria-label={wordCountLabel}>{wordCountLabel}</span>
        {/if}
    </div>
</div>

{#if ns.noteTags.length > 0}
    <div class="note-tags-strip">
        {#each ns.noteTags as tag}
            <button class="tag-pill" onclick={() => onFilterByTag?.(tag)}
                >#{tag}</button
            >
        {/each}
    </div>
{/if}

{#key ns.activeNote.id}
    <NoteProperties
        noteId={ns.activeNote.id}
        folderId={ns.activeNote.folder_id}
        activeTitle={ns.editorTitle}
        activeContent={ns.editorContent}
        onPropertiesLoad={handlePropertiesLoad}
        onVersionRestore={onVersionRestore}
    />
{/key}

{#if propertiesReady}
    {#if is.improveState.status === "diff"}
        <DiffView
            hunks={is.improveState.hunks}
            instruction={is.improveState.instruction}
            onAcceptAll={is.handleImproveAcceptAll}
            onRejectAll={is.handleImproveRejectAll}
            onAcceptHunk={is.handleImproveAcceptHunk}
            onRejectHunk={is.handleImproveRejectHunk}
            onRefineHunk={is.handleRefineHunk}
            rejectedIndices={is.improveState.rejectedIndices}
            acceptedIndices={is.improveState.acceptedIndices}
        />
    {:else if is.improveState.status === "streaming"}
        <div
            class="content-area"
            style="overflow-y: auto; white-space: pre-wrap; font-family: var(--mono); padding: 24px;"
        >
            {is.improveState.improvedText || t("notes.improveThinking")}
        </div>
    {:else if activeTab?.readMode}
        <div
            class="content-area read-mode-content"
            onchange={handleReadModeChecklistChange}
        >
            {@html readModeHtml}
        </div>
    {:else}
        <NoteBodyTextarea
            noteId={ns.activeNote.id}
            bind:value={ns.editorContent}
            onkeydown={(e) => handleEditorKeydown(e)}
        />
    {/if}
{/if}

{#if is.improveState.status === "prompt"}
    <ImprovePopover
        x={200}
        y={100}
        onSend={is.handleImproveStart}
        onCancel={is.handleImproveRejectAll}
    />
{/if}

{#if is.refineState.status === "prompt"}
    <ImprovePopover
        x={is.refineState.x}
        y={is.refineState.y}
        label={t("notes.refineSectionLabel")}
        onSend={is.handleRefineSend}
        onCancel={is.handleRefineCancel}
    />
{/if}

{#if ns.noteLinks.length > 0 || ns.noteBacklinks.length > 0 || ns.unlinkedMentions.length > 0}
    <div class="note-footer">
        {#if ns.noteLinks.length > 0}
            <div class="note-footer-section">
                <span class="note-footer-label">{t("notes.links")}</span>
                {#each ns.noteLinks as link}
                    <button
                        class="link-pill"
                        onclick={() => onOpenNoteById?.(link.id)}
                        >{link.title}</button
                    >
                {/each}
            </div>
        {/if}
        {#if ns.noteBacklinks.length > 0}
            <div class="note-footer-section">
                <span class="note-footer-label">{t("notes.backlinks")}</span>
                {#each ns.noteBacklinks as link}
                    <button
                        class="link-pill"
                        onclick={() => onOpenNoteById?.(link.id)}
                        >{link.title}</button
                    >
                {/each}
            </div>
        {/if}
        {#if ns.unlinkedMentions.length > 0}
            <div class="note-footer-section">
                <span class="note-footer-label">{t("notes.unlinkedMentions")}</span>
                {#each ns.unlinkedMentions as mention}
                    <span class="link-pill-group">
                        <button
                            class="link-pill"
                            onclick={() => onOpenNoteById?.(mention.id)}
                            >{mention.title}</button
                        >
                        <button
                            class="link-pill-action"
                            onclick={() => onConvertMention?.(mention)}
                            title={t("notes.convertToWikiLink")}>{t("notes.convertToLink")}</button
                        >
                    </span>
                {/each}
            </div>
        {/if}
    </div>
{/if}
