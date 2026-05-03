/**
 * Keyboard shortcut handler.
 *
 * Receives coordinator callbacks and service references at construction time
 * so the handler function remains pure — App.svelte is the sole coordinator.
 *
 * @param {{
 *   ns: any,
 *   ts: any,
 *   fs: any,
 *   vault: any,
 *   layout: any,
 *   ui: any,
 *   tmpl: any,
 *   activateTab: (id: string) => void,
 *   closeTab: (id: string) => void,
 *   saveNote: () => void,
 *   newTab: () => void,
 *   startNoteInline: (templateId?: number) => void,
 *   lockVault: () => void,
 *   sendSelectionToChat: () => void,
 *   deleteNote: (id: any) => void,
 * }} deps
 */
export function createKeyboardService(deps) {
  /** @param {KeyboardEvent} e */
  function handle(e) {
    const { ns, ts, fs, vault, layout, ui, tmpl,
            activateTab, closeTab, saveNote, newTab,
            startNoteInline, lockVault, sendSelectionToChat, deleteNote } = deps;
    // Ctrl+P — Quick switcher
    if (
      (e.ctrlKey || e.metaKey) &&
      e.key === "p" &&
      !e.shiftKey &&
      !e.altKey
    ) {
      e.preventDefault();
      ui.quickSwitcherOpen = true;
    }
    // Ctrl+F — Search
    if (
      (e.ctrlKey || e.metaKey) &&
      e.key === "f" &&
      !e.shiftKey &&
      !e.altKey
    ) {
      e.preventDefault();
      ts.searchOpen = true;
    }
    // Ctrl+N — New note
    if (
      (e.ctrlKey || e.metaKey) &&
      e.key === "n" &&
      !e.shiftKey &&
      !e.altKey
    ) {
      e.preventDefault();
      startNoteInline();
    }
    // Ctrl+T — New tab
    if (
      (e.ctrlKey || e.metaKey) &&
      e.key === "t" &&
      !e.shiftKey &&
      !e.altKey
    ) {
      e.preventDefault();
      newTab();
    }
    // Ctrl+Tab / Ctrl+Shift+Tab — Cycle tabs
    if ((e.ctrlKey || e.metaKey) && e.key === "Tab" && ts.tabs.length > 1) {
      e.preventDefault();
      const idx = ts.tabs.findIndex((t) => t.id === ts.activeTabId);
      const next = e.shiftKey
        ? ts.tabs[(idx - 1 + ts.tabs.length) % ts.tabs.length]
        : ts.tabs[(idx + 1) % ts.tabs.length];
      activateTab(next.id);
    }
    // Ctrl+W — Close tab
    if (
      (e.ctrlKey || e.metaKey) &&
      e.key === "w" &&
      !e.shiftKey &&
      !e.altKey
    ) {
      e.preventDefault();
      if (ts.activeTabId) closeTab(ts.activeTabId);
    }
    // Delete — Delete active note
    if (e.key === "Delete" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const tag =
        /** @type {HTMLElement} */ (document.activeElement)?.tagName ?? "";
      const isEditing =
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        tag === "SELECT" ||
        /** @type {HTMLElement} */ (document.activeElement)
          ?.isContentEditable;
      if (!isEditing && ns.activeNote && !ns.activeNote.locked) {
        e.preventDefault();
        deleteNote(ns.activeNote.id);
      }
    }
    // Ctrl+S — Save
    if ((e.ctrlKey || e.metaKey) && e.key === "s") {
      e.preventDefault();
      saveNote();
    }
    // Ctrl+Shift+L — Lock vault
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "L") {
      e.preventDefault();
      if (vault.vaultHasPassword && !vault.vaultLocked) lockVault();
    }
    // Ctrl+Shift+Enter — Send selection to chat
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "Enter") {
      e.preventDefault();
      sendSelectionToChat();
    }
    // F11 — Toggle focus mode
    if (
      e.key === "F11" &&
      !e.ctrlKey &&
      !e.metaKey &&
      !e.altKey &&
      !e.shiftKey
    ) {
      e.preventDefault();
      layout.toggleFocusMode();
    }
    // Escape — Exit focus mode (when no modals are open)
    if (e.key === "Escape" && layout.focusMode) {
      const noModal =
        !ui.settingsOpen &&
        !ts.searchOpen &&
        !ui.quickSwitcherOpen &&
        !tmpl.templateModalOpen &&
        !ns.noteDeletePending &&
        !fs.folderDeletePending &&
        !vault.vaultPwModal &&
        !fs.folderPwModal &&
        !fs.folderUnlockTarget;
      if (noModal) {
        e.preventDefault();
        layout.toggleFocusMode();
      }
    }
  }

  return { handle };
}
