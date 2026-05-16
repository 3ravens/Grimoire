// Bookmarks store — bookmark list, derived ID set, and CRUD operations.

import { invoke } from '@tauri-apps/api/core';

export function createBookmarks() {
  /** @type {{ note_id: number, title: string }[]} */
  let bookmarks = $state([]);
  const bookmarkedNoteIds = $derived(new Set(bookmarks.map(b => b.note_id)));

  async function loadBookmarks() {
    try {
      bookmarks = await invoke('list_bookmarks');
    } catch {
      // Non-fatal — bookmarks section just stays empty
    }
  }

  /** @param {number} noteId */
  async function addBookmark(noteId) {
    await invoke('add_bookmark', { noteId });
    await loadBookmarks();
  }

  /** @param {number} noteId */
  async function removeBookmark(noteId) {
    await invoke('remove_bookmark', { noteId });
    await loadBookmarks();
  }

  return {
    get bookmarks() { return bookmarks; },
    get bookmarkedNoteIds() { return bookmarkedNoteIds; },
    loadBookmarks,
    addBookmark,
    removeBookmark,
  };
}
