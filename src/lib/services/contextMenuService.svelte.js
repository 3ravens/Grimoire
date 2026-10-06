import { invoke } from "@tauri-apps/api/core";
import {
  exportNoteHtml,
  exportNoteMarkdown,
  exportNotePdfPrint,
  resolveExportPayload,
} from "../utils/noteExportActions.js";
import { t } from "../i18n/t.js";

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

  /** @param {Element} target */
  function buildCtxItemsForTarget(target) {
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
    const tabEl = target.closest("[data-tab-id]");
    const noteLiEl = target.closest("[data-note-id]");
    const folderLiEl = target.closest("[data-folder-id]");
    const createNoteBtn = target.closest('[data-action="create-note-btn"]');
    const isEditor = !!target.closest(".content-area");

    let items = /** @type {any[]} */ ([]);

    if (createNoteBtn) {
      items = tmpl.templates.map((tmplItem) => ({
        label: tmplItem.name,
        action: () => startNoteInline(tmplItem.id),
      }));
    } else if (tabEl) {
      const tabId = /** @type {HTMLElement} */ (tabEl).dataset.tabId;
      items = [
        { label: t("contextMenu.close"), action: () => closeTab(tabId) },
        { label: t("contextMenu.closeOthers"), action: () => closeOtherTabs(tabId) },
        {
          label: t("contextMenu.rename"),
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
          label: t("contextMenu.openInNewTab"),
          action: () => note && openNoteInNewTab(note),
        },
        {
          label: t("contextMenu.duplicate"),
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
                label: t("contextMenu.export"),
                submenu: [
                  {
                    label: t("contextMenu.markdown"),
                    action: () =>
                      exportNoteMarkdown({
                        noteId,
                        title: payload.title,
                        body: payload.body,
                        onError,
                      }),
                  },
                  {
                    label: t("contextMenu.html"),
                    action: () =>
                      exportNoteHtml({
                        noteId,
                        title: payload.title,
                        body: payload.body,
                        onError,
                      }),
                  },
                  {
                    label: t("contextMenu.pdf"),
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
              label: t("contextMenu.removeFromBookmarks"),
              action: () => bm.removeBookmark(noteId),
            }
          : {
              label: t("contextMenu.addToBookmarks"),
              action: () => bm.addBookmark(noteId),
            },
        { divider: true },
        {
          label: t("contextMenu.delete"),
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
              label: t("contextMenu.openAsTable"),
              action: async () => {
                await selectFolder(folderId);
                ts.tableViewOpen = true;
              },
            },
            {
              label: t("contextMenu.openAsKanban"),
              action: () => openKanbanTab(folderId, folder.name),
            },
            { divider: true },
            ...(folder.password_protected && !folder.locked
              ? [
                  {
                    label: t("contextMenu.lockFolder"),
                    action: () => void lockFolderSession(folderId),
                  },
                ]
              : []),
            fs.unlockedFolderIds.has(folderId)
              ? {
                  label: t("contextMenu.removePassword"),
                  action: () => fs.openFolderPwModal(folderId, "remove"),
                }
              : {
                  label: t("contextMenu.setPassword"),
                  action: () => fs.openFolderPwModal(folderId, "set"),
                },
            {
              label: t("contextMenu.delete"),
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
              label: t("contextMenu.bold"),
              action: () => applyInlineFormat(start, end, val, "**", "**"),
            },
            {
              label: t("contextMenu.italic"),
              action: () => applyInlineFormat(start, end, val, "*", "*"),
            },
            {
              label: t("contextMenu.strikethrough"),
              action: () => applyInlineFormat(start, end, val, "~~", "~~"),
            },
            {
              label: t("contextMenu.inlineCode"),
              action: () => applyInlineFormat(start, end, val, "`", "`"),
            },
            { divider: true },
            {
              label: t("contextMenu.heading1"),
              action: () => applyLinePrefix(start, end, val, "# "),
            },
            {
              label: t("contextMenu.heading2"),
              action: () => applyLinePrefix(start, end, val, "## "),
            },
            {
              label: t("contextMenu.heading3"),
              action: () => applyLinePrefix(start, end, val, "### "),
            },
            { divider: true },
            {
              label: t("contextMenu.codeBlock"),
              action: () =>
                applyInlineFormat(start, end, val, "```\n", "\n```"),
            },
          ]
        : [];

      items = [
        ...(hasSel
          ? [{ label: t("contextMenu.format"), submenu: formatSubmenu }, { divider: true }]
          : []),
        {
          label: t("contextMenu.cut"),
          disabled: !hasSel,
          action: () => {
            navigator.clipboard.writeText(selText);
            ns.editorContent = val.slice(0, start) + val.slice(end);
            ns.markDirty();
          },
        },
        {
          label: t("contextMenu.copy"),
          disabled: !hasSel,
          action: () => navigator.clipboard.writeText(selText),
        },
        {
          label: t("contextMenu.paste"),
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
                label: t("contextMenu.sendToChat"),
                action: () => sendSelectionToChat(),
              },
            ]
          : []),
        { divider: true },
        {
          label: t("contextMenu.suggestImprovements"),
          action: () => is.startImprove(),
          disabled: !settings.llmEnabled || !ns.editorContent || is.improveState.status !== "idle",
        },
      ];
    }

    return items;
  }

  /** @param {MouseEvent} e */
  function buildCtxItems(e) {
    return buildCtxItemsForTarget(/** @type {Element} */ (e.target));
  }

  /** @param {Element} el */
  function openFromElement(el) {
    if (deps.settings.devNativeContextMenu) {
      ctxMenu = null;
      return;
    }
    const items = buildCtxItemsForTarget(el);
    if (items.length === 0) return;
    const rect = el.getBoundingClientRect();
    const x = Math.min(rect.left, window.innerWidth - 174);
    const y = Math.min(rect.bottom, window.innerHeight - items.length * 28 - 16);
    ctxMenu = { x, y, items };
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
    openFromElement,
  };
}
