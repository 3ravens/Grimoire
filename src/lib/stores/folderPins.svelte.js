// Folder pins store — ordered pin ids for the currently viewed folder.

import { invoke } from '@tauri-apps/api/core';

/**
 * @param {{ onError?: (e: unknown) => void }} [opts]
 */
export function createFolderPins({ onError } = {}) {
  /** @type {number[]} */
  let orderedIds = $state([]);
  /** @type {number | null} */
  let loadedFolderId = $state(null);
  let loadGen = 0;

  const pinnedNoteIds = $derived(new Set(orderedIds));

  function clear() {
    loadGen += 1;
    orderedIds = [];
    loadedFolderId = null;
  }

  /** @param {number} folderId */
  async function load(folderId) {
    if (typeof folderId !== 'number') {
      clear();
      return;
    }
    const gen = ++loadGen;
    try {
      const rows = await invoke('list_folder_pins', { folderId });
      if (gen !== loadGen) return;
      orderedIds = rows.map((r) => r.note_id);
      loadedFolderId = folderId;
    } catch (e) {
      if (gen !== loadGen) return;
      orderedIds = [];
      loadedFolderId = folderId;
      onError?.(e);
    }
  }

  /** Reload pins for the currently loaded folder, if any. */
  async function reload() {
    if (typeof loadedFolderId === 'number') {
      await load(loadedFolderId);
    }
  }

  /**
   * @param {number} noteId
   * @param {number} folderId
   */
  async function pin(noteId, folderId) {
    try {
      await invoke('pin_folder_note', { noteId, folderId });
      if (loadedFolderId === folderId) await load(folderId);
    } catch (e) {
      onError?.(e);
      throw e;
    }
  }

  /**
   * @param {number} noteId
   * @param {number} folderId
   */
  async function unpin(noteId, folderId) {
    try {
      await invoke('unpin_folder_note', { noteId, folderId });
      if (loadedFolderId === folderId) await load(folderId);
    } catch (e) {
      onError?.(e);
      throw e;
    }
  }

  /**
   * @param {number} folderId
   * @param {number[]} nextOrder
   */
  async function reorder(folderId, nextOrder) {
    try {
      await invoke('reorder_folder_pins', {
        folderId,
        orderedNoteIds: nextOrder,
      });
      if (loadedFolderId === folderId) {
        orderedIds = [...nextOrder];
      }
    } catch (e) {
      onError?.(e);
      throw e;
    }
  }

  /**
   * @param {number} noteId
   * @param {number} folderId
   * @returns {Promise<number | null>} new 1-based position, or null if unchanged
   */
  async function moveUp(noteId, folderId) {
    const idx = orderedIds.indexOf(noteId);
    if (idx <= 0) return null;
    const next = [...orderedIds];
    [next[idx - 1], next[idx]] = [next[idx], next[idx - 1]];
    await reorder(folderId, next);
    return idx; // 1-based after move: old idx becomes new position
  }

  /**
   * @param {number} noteId
   * @param {number} folderId
   * @returns {Promise<number | null>} new 1-based position, or null if unchanged
   */
  async function moveDown(noteId, folderId) {
    const idx = orderedIds.indexOf(noteId);
    if (idx < 0 || idx >= orderedIds.length - 1) return null;
    const next = [...orderedIds];
    [next[idx], next[idx + 1]] = [next[idx + 1], next[idx]];
    await reorder(folderId, next);
    return idx + 2;
  }

  return {
    get orderedIds() {
      return orderedIds;
    },
    get loadedFolderId() {
      return loadedFolderId;
    },
    get pinnedNoteIds() {
      return pinnedNoteIds;
    },
    clear,
    load,
    reload,
    pin,
    unpin,
    reorder,
    moveUp,
    moveDown,
  };
}
