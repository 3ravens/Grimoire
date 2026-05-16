import { invoke } from "@tauri-apps/api/core";
import {
  exportNoteHtml,
  exportNoteMarkdown,
  exportNotePdfPrint,
  resolveExportPayload,
} from "../utils/noteExportActions.js";

/**
 * Context menu state and logic.
 *
 * Constructed with coordinator callbacks so the service remains pure
 * state+logic — App.svelte stays the sole coordinator.
 *
 * @param {{
 *   ns: any,
 *   ts: any,
 *   fs: any,
 *   bm: any,
 *   tmpl: any,
 *   is: any,
 *   settings: any,
 *   closeTab: (id: string) => void,
 *   closeOtherTabs: (keepId: string) => void,
 *   startTabRenameExternal: (id: string) => void,
 *   openNoteInNewTab: (note: any) => void,
 *   deleteNote: (id: any) => void,
 *   deleteFolder: (id: any) => void,
 *   selectFolder: (id: any) => void,
 *   openKanbanTab: (folderId: any, folderName: string) => void,
 *   startNoteInline: (templateId?: number) => void,
 *   sendSelectionToChat: () => void,
 *   loadNotes: () => void,
 *   lockFolderSession: (id: any) => void | Promise<void>,
 *   onError: (e: unknown) => void,
 * }} deps
 */
export function createContextMenuService(deps) {
  /** @type {{ x: number, y: number, items: any[] } | null} */
  let ctxMenu = $state(null);

  // ── Formatting helpers ────────────────────────────────────────────────────

  /** @param {number} start @param {number} end @param {string} val
   *  @param {string} prefix @param {string} suffix */
  function applyInlineFormat(start, end, val, prefix, suffix) {
    const sel = val.slice(start, end);
    const trimmed = sel.trimEnd();
    deps.ns.editorContent =
      val.slice(0, start) + prefix + trimmed + suffix + val.slice(end);
    deps.ns.markDirty();
  }

  /** @param {number} start @param {number} end @param {string} val
   *  @param {string} prefix */
  function applyLinePrefix(start, end, val, prefix) {
    const sel = val.slice(start, end).trimEnd();
    const needBefore = start > 0 && val[start - 1] !== "\n";
    const needAfter =
      start + sel.length < val.length && val[start + sel.length] !== "\n";
    const prefixed = sel
      .split("\n")
      .map((line) => prefix + line.replace(/^#{1,6}\s*/, ""))
      .join("\n");
    deps.ns.editorContent =
      val.slice(0, start) +
      (needBefore ? "\n" : "") +
      prefixed +
      (needAfter ? "\n" : "") +
      val.slice(end);
    deps.ns.markDirty();
  }

  // ── Menu builder ──────────────────────────────────────────────────────────

  /** @param {MouseEvent} e */
  function buildCtxItems(e) {
    const {
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
      onError,
    } = deps;
    const target = /** @type {Element} */ (e.target);
    const tabEl = target.closest("[data-tab-id]");
    const noteLiEl = target.closest("[data-note-id]");
    const folderLiEl = target.closest("[data-folder-id]");
    const createNoteBtn = target.closest('[data-action="create-note-btn"]');
    const isEditor = !!target.closest(".content-area");

    let items = /** @type {any[]} */ ([]);

    if (createNoteBtn) {
      items = tmpl.templates.map((t) => ({
        label: t.name,
        action: () => startNoteInline(t.id),
      }));
    } else if (tabEl) {
      const tabId = /** @type {HTMLElement} */ (tabEl).dataset.tabId;
      items = [
        { label: "Close", action: () => closeTab(tabId) },
        { label: "Close Others", action: () => closeOtherTabs(tabId) },
        {
          label: "Rename",
          action: () => startTabRenameExternal(tabId),
        },
      ];
    } else if (noteLiEl && !noteLiEl.classList.contains("locked-row")) {
      const noteId = Number(
        /** @type {HTMLElement} */ (noteLiEl).dataset.noteId,
      );
      const note = ns.notes.find((n) => n.id === noteId);
      const payload = note ? resolveExportPayload(ns, note) : null;
      items = [
        {
          label: "Open in New Tab",
          action: () => note && openNoteInNewTab(note),
        },
        {
          label: "Duplicate",
          action: async () => {
            try {
              await invoke("duplicate_note", { id: noteId });
              loadNotes();
            } catch (err) {
              onError?.(err);
            }
          },
        },
        ...(note && payload
          ? [
              {
                label: "Export",
                submenu: [
                  {
                    label: "Markdown…",
                    action: () =>
                      exportNoteMarkdown({
                        noteId,
                        title: payload.title,
                        body: payload.body,
                        onError,
                      }),
                  },
                  {
                    label: "HTML…",
                    action: () =>
                      exportNoteHtml({
                        noteId,
                        title: payload.title,
                        body: payload.body,
                        onError,
                      }),
                  },
                  {
                    label: "PDF…",
                    action: () =>
                      exportNotePdfPrint({
                        noteId,
                        title: payload.title,
                        body: payload.body,
                        onError,
                      }),
                  },
                ],
              },
            ]
          : []),
        { divider: true },
        bm.bookmarkedNoteIds.has(noteId)
          ? {
              label: "Remove from Bookmarks",
              action: () => bm.removeBookmark(noteId),
            }
          : {
              label: "Add to Bookmarks",
              action: () => bm.addBookmark(noteId),
            },
        { divider: true },
        {
          label: "Delete",
          action: () => deleteNote(noteId),
          danger: true,
        },
      ];
    } else if (folderLiEl) {
      const raw = /** @type {HTMLElement} */ (folderLiEl).dataset.folderId;
      if (raw && raw !== "all" && raw !== "unfiled") {
        const folderId = Number(raw);
        const folder = fs.folders.find((f) => f.id === folderId);
        if (folder && !folder.locked) {
          items = [
            {
              label: "Open as Table",
              action: async () => {
                await selectFolder(folderId);
                ts.tableViewOpen = true;
              },
            },
            {
              label: "Open as Kanban",
              action: () => openKanbanTab(folderId, folder.name),
            },
            { divider: true },
            ...(folder.password_protected && !folder.locked
              ? [
                  {
                    label: "Lock folder",
                    action: () => void lockFolderSession(folderId),
                  },
                ]
              : []),
            fs.unlockedFolderIds.has(folderId)
              ? {
                  label: "Remove password",
                  action: () => fs.openFolderPwModal(folderId, "remove"),
                }
              : {
                  label: "Set password",
                  action: () => fs.openFolderPwModal(folderId, "set"),
                },
            {
              label: "Delete",
              action: () => deleteFolder(folderId),
              danger: true,
            },
          ];
        }
      }
    } else if (isEditor) {
      const el = ns.editorTextareaEl;
      const start = el?.selectionStart ?? 0;
      const end = el?.selectionEnd ?? 0;
      const val = el?.value ?? "";
      const selText = val.slice(start, end);
      const hasSel = selText.length > 0;

      const formatSubmenu = hasSel
        ? [
            {
              label: "Bold",
              action: () => applyInlineFormat(start, end, val, "**", "**"),
            },
            {
              label: "Italic",
              action: () => applyInlineFormat(start, end, val, "*", "*"),
            },
            {
              label: "Strikethrough",
              action: () => applyInlineFormat(start, end, val, "~~", "~~"),
            },
            {
              label: "Inline Code",
              action: () => applyInlineFormat(start, end, val, "`", "`"),
            },
            { divider: true },
            {
              label: "Heading 1",
              action: () => applyLinePrefix(start, end, val, "# "),
            },
            {
              label: "Heading 2",
              action: () => applyLinePrefix(start, end, val, "## "),
            },
            {
              label: "Heading 3",
              action: () => applyLinePrefix(start, end, val, "### "),
            },
            { divider: true },
            {
              label: "Code Block",
              action: () =>
                applyInlineFormat(start, end, val, "```\n", "\n```"),
            },
          ]
        : [];

      items = [
        ...(hasSel
          ? [{ label: "Format", submenu: formatSubmenu }, { divider: true }]
          : []),
        {
          label: "Cut",
          disabled: !hasSel,
          action: () => {
            navigator.clipboard.writeText(selText);
            ns.editorContent = val.slice(0, start) + val.slice(end);
            ns.markDirty();
          },
        },
        {
          label: "Copy",
          disabled: !hasSel,
          action: () => navigator.clipboard.writeText(selText),
        },
        {
          label: "Paste",
          action: async () => {
            const text = await navigator.clipboard.readText();
            ns.editorContent = val.slice(0, start) + text + val.slice(end);
            ns.markDirty();
          },
        },
        ...(hasSel
          ? [
              { divider: true },
              {
                label: "Send to Chat",
                action: () => sendSelectionToChat(),
              },
            ]
          : []),
        { divider: true },
        {
          label: "Suggest improvements",
          action: () => is.startImprove(),
          disabled: !ns.editorContent || is.improveState.status !== "idle",
        },
      ];
    }

    return items;
  }

  // ── Event listener management ─────────────────────────────────────────────

  /** Registers the contextmenu listener and returns a cleanup function.
   *  The caller should invoke the cleanup when the component unmounts.
   *  This pattern works correctly with Svelte's $effect() during HMR. */
  function setup() {
    const handler = (e) => {
      if (deps.settings.devNativeContextMenu) {
        ctxMenu = null;
        return;
      }
      e.preventDefault();
      const items = buildCtxItems(e);
      if (items.length === 0) return;
      const x = Math.min(e.clientX, window.innerWidth - 174);
      const y = Math.min(
        e.clientY,
        window.innerHeight - items.length * 28 - 16,
      );
      ctxMenu = { x, y, items };
    };
    document.addEventListener("contextmenu", handler);
    return () => document.removeEventListener("contextmenu", handler);
  }

  function close() {
    ctxMenu = null;
  }

  return {
    get ctxMenu() {
      return ctxMenu;
    },
    set ctxMenu(v) {
      ctxMenu = v;
    },
    setup,
    close,
  };
}
