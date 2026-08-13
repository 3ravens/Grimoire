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

    const ns = getContext("ns");
    const ts = getContext("ts");
    const is = getContext("is");
    const fs = getContext("fs");
    const settings = getContext("settings");

    const llmImproveDisabled = $derived(!settings.llmEnabled);
    const improveTooltip = $derived(
        llmImproveDisabled
            ? "AI features are disabled for this hardware — enable in Settings → Hardware"
            : "Suggest improvements",
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
    const wordCount = $derived(
        ns.editorContent
            ? ns.editorContent.trim().split(/\s+/).filter(Boolean).length
            : 0,
    );
    const readingTime = $derived(Math.max(1, Math.round(wordCount / 200)));

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
        }).then((html) => {
            if (!cancelled) readModeHtml = html;
        });
        return () => {
            cancelled = true;
        };
    });

    // ── Editor keydown (Tab indent + wiki-link brackets) ──────────────────────
    function handleEditorKeydown(e) {
        const el = /** @type {HTMLTextAreaElement} */ (e.currentTarget);
        const { selectionStart: start, selectionEnd: end, value } = el;

        // Tab / Shift+Tab: indent in the note instead of leaving the textarea.
        if (e.key === "Tab" && !e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            const next = applyEditorTab(value, start, end, {
                shiftKey: e.shiftKey,
            });
            ns.editorContent = next.value;
            ns.markDirty();
            requestAnimationFrame(() => {
                el.selectionStart = next.selectionStart;
                el.selectionEnd = next.selectionEnd;
            });
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
        placeholder="Note title"
        aria-label="Note title"
    />
    <div class="toolbar-actions">
        <label>
            Move to:
            <select
                onchange={(e) => {
                    const v = /** @type {HTMLSelectElement} */ (e.target).value;
                    onMoveNote?.(
                        ns.activeNote.id,
                        v === "null" ? null : Number(v),
                    );
                }}
            >
                <option value="null">Unfiled</option>
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
                ? "Save (Ctrl+S)"
                : ns.indexState === "indexing"
                  ? "Indexing…"
                  : ns.indexState === "error"
                    ? "⚠ Index failed"
                    : "Saved"}
        </button>
        <span class="sr-only" aria-live="polite" aria-atomic="true">
            {ns.isDirty
                ? "Unsaved changes"
                : ns.indexState === "indexing"
                  ? "Indexing"
                  : ns.indexState === "error"
                    ? "Index failed"
                    : "Saved"}
        </span>
        {#if fs.folderHasProperties}
            <button
                class="graph-toggle"
                aria-label="Switch to table view"
                onclick={onOpenTableView}>← Table</button
            >
        {/if}
        {#if ns.activeNote.folder_id != null && fs.folders.some((f) => f.id === ns.activeNote.folder_id)}
            <button
                class="graph-toggle"
                aria-label="Switch to board view"
                onclick={() =>
                    onOpenKanbanTab?.(
                        ns.activeNote.folder_id,
                        fs.folders.find((f) => f.id === ns.activeNote.folder_id)
                            ?.name ?? "",
                    )}
            >
                ← Board
            </button>
        {/if}
        {#if ns.activeNote.folder_id}
            <button
                class="graph-toggle"
                onclick={() => onRevealFolder?.(ns.activeNote.folder_id)}
                title="Reveal in folder panel"
                aria-label="Reveal in folder panel">Reveal</button
            >
        {/if}
        <button
            class="graph-toggle"
            aria-label="Suggest improvements"
            title={improveTooltip}
            onclick={is.startImprove}
            disabled={llmImproveDisabled || is.improveState.status !== "idle" || !ns.editorContent}
        >
            Improve
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
                    aria-label="Export note"
                    title="Export note"
                    >Export</summary
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
                        >Markdown…</button
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
                        >HTML…</button
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
                        >PDF…</button
                    >
                </div>
            </details>
        {/if}
        <button
            class="graph-toggle"
            aria-label={activeTab?.readMode
                ? "Switch to edit mode"
                : "Switch to read mode"}
            onclick={ts.toggleReadMode}
        >
            {activeTab?.readMode ? "Edit" : "Read"}
        </button>
        <button
            class="close-note-btn"
            aria-label="Close note"
            title="Close note"
            onclick={onCloseNote}>✕</button
        >
        <span class="word-count"
            >{wordCount} word{wordCount === 1 ? "" : "s"} · {readingTime} min</span
        >
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
            {is.improveState.improvedText || "Thinking\u2026"}
        </div>
    {:else if activeTab?.readMode}
        <div class="content-area read-mode-content">
            {@html readModeHtml}
        </div>
    {:else}
        <NoteBodyTextarea
            noteId={ns.activeNote.id}
            bind:value={ns.editorContent}
            onkeydown={handleEditorKeydown}
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
        label="How should this section be refined?"
        onSend={is.handleRefineSend}
        onCancel={is.handleRefineCancel}
    />
{/if}

{#if ns.noteLinks.length > 0 || ns.noteBacklinks.length > 0 || ns.unlinkedMentions.length > 0}
    <div class="note-footer">
        {#if ns.noteLinks.length > 0}
            <div class="note-footer-section">
                <span class="note-footer-label">Links</span>
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
                <span class="note-footer-label">Backlinks</span>
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
                <span class="note-footer-label">Unlinked mentions</span>
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
                            title="Convert to wiki-link">→ link</button
                        >
                    </span>
                {/each}
            </div>
        {/if}
    </div>
{/if}
