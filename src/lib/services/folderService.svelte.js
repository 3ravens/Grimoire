import { invoke } from "@tauri-apps/api/core";
import { tick } from "svelte";

/**
 * @param {{
 *   onError?: (e: unknown) => void,
 *   onBeforeFolderChange?: () => void,
 *   onFolderUnlocked?: (folderId: any) => void,
 * }} opts
 */
export function createFolderService({
  onError,
  onBeforeFolderChange,
  onFolderUnlocked,
}) {
  /** @type {any[]} */
  let folders = $state([]);
  /** @type {number | string | null} */
  let selectedFolderId = $state(null);
  let unlockedFolderIds = $state(new Set());
  /** @type {Record<number, boolean>} */
  let folderExpanded = $state({});
  /** @type {{ id: any, type: string, value: string } | null} */
  let inlineRenaming = $state(null);
  /** @type {{ id: any, name: string } | null} */
  let folderDeletePending = $state(null);
  /** @type {{ mode: string, folderId: any } | null} */
  let folderPwModal = $state(null);
  /** @type {{ id: any, [k: string]: any } | null} */
  let folderUnlockTarget = $state(null);

  let folderHasProperties = $state(false);

  // ── Note drag state ───────────────────────────────────────────────────────
  let isDragging = $state(false);
  let dragOverFolderId = $state(null);

  /** @param {DragEvent} e @param {any} note */
  function onNoteDragStart(e, note) {
    e.dataTransfer.setData("text/plain", String(note.id));
    e.dataTransfer.effectAllowed = "move";
    isDragging = true;
  }

  function onNoteDragEnd() {
    isDragging = false;
    dragOverFolderId = null;
  }

  async function loadFolders() {
    try {
      folders = await invoke("list_folders");
    } catch (e) {
      onError?.(e);
    }
  }

  async function startFolderInline() {
    try {
      const parentId =
        typeof selectedFolderId === "number" ? selectedFolderId : null;
      const folder = await invoke("create_folder", {
        name: "Untitled",
        parentId,
      });
      await loadFolders();
      if (parentId) folderExpanded = { ...folderExpanded, [parentId]: true };
      inlineRenaming = { id: folder.id, type: "folder", value: "Untitled" };
    } catch (e) {
      onError?.(e);
    }
  }

  async function confirmInlineRename() {
    if (!inlineRenaming) return null;
    const { id, type, value } = inlineRenaming;
    const name = value.trim() || "Untitled";
    inlineRenaming = null;
    try {
      if (type === "folder") {
        await invoke("rename_folder", { id, name });
        await loadFolders();
      } else {
        await invoke("rename_note", { id, name });
      }
      return { id, type, name };
    } catch (e) {
      onError?.(e);
      return null;
    }
  }

  /** @param {any} folderId @param {boolean} [openFolders] */
  async function revealFolder(folderId, openFolders) {
    if (!openFolders) loadFolders();
    const newExpanded = { ...folderExpanded };
    let current = folders.find((f) => f.id === folderId);
    while (current?.parent_id) {
      newExpanded[current.parent_id] = true;
      current = folders.find((f) => f.id === current.parent_id);
    }
    folderExpanded = newExpanded;
    selectFolder(folderId);
    await tick();
    document
      .querySelector(`[data-folder-id="${folderId}"]`)
      ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }

  /** @param {any} id */
  function deleteFolder(id) {
    const folder = folders.find((f) => f.id === id);
    folderDeletePending = { id, name: folder?.name ?? "this folder" };
  }

  async function confirmDeleteFolder() {
    if (!folderDeletePending) return null;
    const id = folderDeletePending.id;
    folderDeletePending = null;
    try {
      await invoke("delete_folder", { id });
      if (selectedFolderId === id) selectedFolderId = null;
      await loadFolders();
      return "refresh_notes";
    } catch (e) {
      onError?.(e);
      return null;
    }
  }

  /** @param {any} id @param {(() => any) | null} [saveFn] */
  async function selectFolder(id, saveFn) {
    if (saveFn) await saveFn();
    selectedFolderId = id;
    return id;
  }

  /** @param {{ id: any, [k: string]: any }} folder */
  function requestFolderUnlock(folder) {
    folderUnlockTarget = folder;
  }

  /** @param {string} password */
  async function handleFolderUnlockSafe(password) {
    if (!folderUnlockTarget) return false;
    const targetId = folderUnlockTarget.id;
    const ok = await invoke("unlock_folder", { folderId: targetId, password });
    if (ok) {
      folderUnlockTarget = null;
      unlockedFolderIds = new Set([...unlockedFolderIds, targetId]);
      await loadFolders();
      onFolderUnlocked?.(targetId);
    }
    return ok;
  }

  /** @param {string} password */
  async function handleFolderPwSubmit(password) {
    if (!folderPwModal) return true;
    if (folderPwModal.mode === "set") {
      await invoke("set_folder_password", {
        folderId: folderPwModal.folderId,
        password,
      });
      const next = new Set(unlockedFolderIds);
      next.delete(folderPwModal.folderId);
      unlockedFolderIds = next;
    } else if (folderPwModal.mode === "remove") {
      await invoke("remove_folder_password", {
        folderId: folderPwModal.folderId,
        password,
      });
      const next = new Set(unlockedFolderIds);
      next.delete(folderPwModal.folderId);
      unlockedFolderIds = next;
      onFolderUnlocked?.(folderPwModal.folderId);
    }
    folderPwModal = null;
    await loadFolders();
    return true;
  }

  /** @param {any} folderId */
  async function loadFolderPropertyDefs(folderId) {
    if (!folderId || folderId === "all") {
      folderHasProperties = false;
      return;
    }
    try {
      const defs = await invoke("get_property_defs", { folderId });
      folderHasProperties = defs.length > 0;
      return defs;
    } catch {
      folderHasProperties = false;
      return [];
    }
  }

  /** @param {any} fid @param {string} mode */
  function openFolderPwModal(fid, mode) {
    folderPwModal = { mode, folderId: fid };
  }

  return {
    // State
    get folders() {
      return folders;
    },
    set folders(v) {
      folders = v;
    },
    get selectedFolderId() {
      return selectedFolderId;
    },
    set selectedFolderId(v) {
      selectedFolderId = v;
    },
    get unlockedFolderIds() {
      return unlockedFolderIds;
    },
    set unlockedFolderIds(v) {
      unlockedFolderIds = v;
    },
    get folderExpanded() {
      return folderExpanded;
    },
    set folderExpanded(v) {
      folderExpanded = v;
    },
    get inlineRenaming() {
      return inlineRenaming;
    },
    set inlineRenaming(v) {
      inlineRenaming = v;
    },
    get folderDeletePending() {
      return folderDeletePending;
    },
    set folderDeletePending(v) {
      folderDeletePending = v;
    },
    get folderPwModal() {
      return folderPwModal;
    },
    set folderPwModal(v) {
      folderPwModal = v;
    },
    get folderUnlockTarget() {
      return folderUnlockTarget;
    },
    set folderUnlockTarget(v) {
      folderUnlockTarget = v;
    },
    get folderHasProperties() {
      return folderHasProperties;
    },
    set folderHasProperties(v) {
      folderHasProperties = v;
    },
    get isDragging() {
      return isDragging;
    },
    set isDragging(v) {
      isDragging = v;
    },
    get dragOverFolderId() {
      return dragOverFolderId;
    },
    set dragOverFolderId(v) {
      dragOverFolderId = v;
    },
    // Functions
    loadFolders,
    startFolderInline,
    confirmInlineRename,
    revealFolder,
    deleteFolder,
    confirmDeleteFolder,
    selectFolder,
    requestFolderUnlock,
    handleFolderUnlockSafe,
    handleFolderPwSubmit,
    loadFolderPropertyDefs,
    openFolderPwModal,
    onNoteDragStart,
    onNoteDragEnd,
  };
}
