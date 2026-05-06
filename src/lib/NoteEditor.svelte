<!-- Copyright (C) 2026 Wim Palland — see App.svelte for license header. -->
<script>
    import { getContext } from "svelte";
    import NoteProperties from "./NoteProperties.svelte";
    import DiffView from "./DiffView.svelte";
    import ImprovePopover from "./ImprovePopover.svelte";
    import { renderTransclusionMarkdownToHtml } from "./utils/transclusion.js";
    import {
        exportNoteHtml,
        exportNoteMarkdown,
        exportNotePdfPrint,
    } from "./utils/noteExportActions.js";

    const ns = getContext("ns");
    const ts = getContext("ts");
    const is = getContext("is");
    const fs = getContext("fs");

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
        onExportError = () => {},
    } = $props();

    /** @type {HTMLDetailsElement | null} */
    let exportDetailsEl = $state(null);

    function closeExportMenu() {
        if (exportDetailsEl) exportDetailsEl.open = false;
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

    // Reset propertiesReady when the active note changes.
    $effect(() => {
        // reading ns.activeNote causes this to re-run on note change
        if (ns.activeNote) {
            propertiesReady = !ns.activeNote.folder_id;
        }
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

    // ── Editor keydown (wiki-link brackets) ───────────────────────────────────
    function handleEditorKeydown(e) {
        if (e.key !== "[") return;
        const el = /** @type {HTMLTextAreaElement} */ (e.currentTarget);
        const { selectionStart: start, selectionEnd: end, value } = el;
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
            onclick={onSave}
            disabled={!ns.isDirty}
            class:index-error={!ns.isDirty && ns.indexState === "error"}
            aria-live="polite"
            aria-atomic="true"
        >
            {ns.isDirty
                ? "Save (Ctrl+S)"
                : ns.indexState === "indexing"
                  ? "Indexing…"
                  : ns.indexState === "error"
                    ? "⚠ Index failed"
                    : "Saved"}
        </button>
        {#if fs.folderHasProperties}
            <button
                class="graph-toggle"
                aria-label="Switch to table view"
                onclick={onOpenTableView}>← Table</button
            >
        {/if}
        {#if ns.activeNote.folder_id && ts.tabs.some((t) => t.type === "kanban" && t.folderId === ns.activeNote.folder_id)}
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
            title="Suggest improvements"
            onclick={is.startImprove}
            disabled={is.improveState.status !== "idle" || !ns.editorContent}
        >
            Improve
        </button>
        {#if !ns.activeNote.locked}
            <details
                class="toolbar-export"
                bind:this={exportDetailsEl}
            >
                <summary
                    class="graph-toggle export-summary"
                    aria-haspopup="menu"
                    aria-label="Export note"
                    title="Export note"
                    >Export</summary
                >
                <div class="toolbar-export-menu" role="menu">
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

{#if ns.activeNote.folder_id}
    {#key ns.activeNote.id}
        <NoteProperties
            noteId={ns.activeNote.id}
            folderId={ns.activeNote.folder_id}
            onPropertiesLoad={handlePropertiesLoad}
        />
    {/key}
{/if}

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
        <textarea
            class="content-area"
            bind:this={ns.editorTextareaEl}
            bind:value={ns.editorContent}
            oninput={ns.markDirty}
            onkeydown={handleEditorKeydown}
            placeholder="Write your note…"
        ></textarea>
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
