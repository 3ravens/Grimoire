import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export function createNoteService({ onError }) {
  let notes = $state([]);
  let activeNote = $state(null);
  let editorTitle = $state("");
  let editorContent = $state("");
  let isDirty = $state(false);
  let indexState = $state("idle");
  let editorTextareaEl = $state(null);
  let noteTags = $state([]);
  let noteLinks = $state([]);
  let noteBacklinks = $state([]);
  let unlinkedMentions = $state([]);
  let tagFilter = $state(null);
  let allTags = $state([]);
  let noteDeletePending = $state(null);
  let isReindexing = $state(false);
  /** Latest `test_data:progress` payload while `generateTestData` runs (dev builds). */
  /** @type {{ phase: string, message: string, current?: number, total?: number } | null} */
  let testDataProgress = $state(null);
  /** Bumped after a successful note save so read-mode transclusion re-fetches embedded bodies. */
  let transclusionRefresh = $state(0);
  /** @type {{ processed: number, total: number, indexed: number, phase?: string | null, embeddingChunks?: { done: number, total: number, note_title: string } | null } | null} */
  let reindexProgress = $state(null);

  function markDirty() {
    isDirty = true;
  }

  async function loadNotes(folderId, filterTag) {
    try {
      if (filterTag) {
        notes = await invoke("list_notes_by_tag", { tag: filterTag });
      } else if (folderId === "all") {
        notes = await invoke("list_notes", { all: true });
      } else {
        notes = await invoke("list_notes", {
          folderId: folderId ?? null,
          all: false,
        });
      }
    } catch (e) {
      onError?.(e);
    }
  }

  async function loadAllTags() {
    try {
      allTags = await invoke("list_all_tags");
    } catch {
      /* non-fatal */
    }
  }

  function loadActiveNoteMeta(note) {
    noteTags = [];
    noteLinks = [];
    noteBacklinks = [];
    unlinkedMentions = [];
    invoke("get_note_tags", { noteId: note.id })
      .then((t) => (noteTags = t))
      .catch(() => {});
    invoke("get_note_links", { noteId: note.id })
      .then((l) => (noteLinks = l))
      .catch(() => {});
    invoke("get_backlinks", { noteId: note.id })
      .then((b) => (noteBacklinks = b))
      .catch(() => {});
    invoke("get_unlinked_mentions", { noteId: note.id, title: note.title })
      .then((m) => (unlinkedMentions = m))
      .catch(() => {});
  }

  function openNote(note, searchOpenRef) {
    if (searchOpenRef) searchOpenRef.value = false;
    activeNote = note;
    editorTitle = note.title;
    editorContent = note.content;
    isDirty = false;
    loadActiveNoteMeta(note);
  }

  function clearActiveNote() {
    activeNote = null;
    editorTitle = "";
    editorContent = "";
    isDirty = false;
    noteTags = [];
    noteLinks = [];
    noteBacklinks = [];
  }

  async function saveNote(onSaved) {
    if (!activeNote) return;
    try {
      const updated = await invoke("save_note_with_version", {
        id: activeNote.id,
        title: editorTitle,
        content: editorContent,
      });
      activeNote = updated;
      isDirty = false;
      transclusionRefresh += 1;
      indexState = "indexing";
      invoke("index_note", {
        noteId: updated.id,
        title: editorTitle,
        content: editorContent,
      })
        .then(() => {
          indexState = "idle";
        })
        .catch(() => {
          indexState = "error";
        });
      invoke("sync_note_relations", {
        noteId: updated.id,
        content: editorContent,
      })
        .then(() =>
          Promise.all([
            invoke("get_note_tags", { noteId: updated.id }),
            invoke("get_note_links", { noteId: updated.id }),
            invoke("get_backlinks", { noteId: updated.id }),
            invoke("get_unlinked_mentions", {
              noteId: updated.id,
              title: editorTitle,
            }),
            invoke("list_all_tags"),
          ]),
        )
        .then(([tags, links, backlinks, mentions, updatedAllTags]) => {
          noteTags = tags;
          noteLinks = links;
          noteBacklinks = backlinks;
          unlinkedMentions = mentions;
          allTags = updatedAllTags;
        })
        .catch(() => {});
      onSaved?.();
      return updated;
    } catch (e) {
      onError?.(e);
    }
  }

  async function startNoteInline(
    folderId,
    templateId,
    templates,
    navigateToNoteFn,
  ) {
    try {
      const note = await invoke("create_note", {
        title: "Untitled",
        folderId: folderId === "all" ? null : folderId,
      });
      if (folderId && templateId > 0) {
        try {
          await invoke("apply_template_to_note", {
            noteId: note.id,
            folderId,
            templateId,
          });
        } catch {
          /* non-fatal */
        }
      }
      navigateToNoteFn?.(note);
      const template = (templates ?? []).find((t) => t.id === templateId);
      const templateContent = template?.content ?? "";
      if (templateContent) {
        editorContent = templateContent;
        isDirty = true;
        invoke("save_note_with_version", {
          id: note.id,
          title: "Untitled",
          content: templateContent,
        }).catch(() => {});
      }
      indexState = "indexing";
      invoke("index_note", {
        noteId: note.id,
        title: "Untitled",
        content: templateContent,
      })
        .then(() => {
          indexState = "idle";
        })
        .catch(() => {
          indexState = "error";
        });
      return note;
    } catch (e) {
      onError?.(e);
      return null;
    }
  }

  async function convertMention(mention) {
    if (!activeNote) return;
    try {
      const updatedContent = await invoke("convert_mention_to_link", {
        noteId: mention.id,
        title: activeNote.title,
      });
      if (activeNote.id === mention.id) {
        editorContent = updatedContent;
      }
      const [mentions, backlinks] = await Promise.all([
        invoke("get_unlinked_mentions", {
          noteId: activeNote.id,
          title: activeNote.title,
        }),
        invoke("get_backlinks", { noteId: activeNote.id }),
      ]);
      unlinkedMentions = mentions;
      noteBacklinks = backlinks;
    } catch (e) {
      onError?.(e);
    }
  }

  function deleteNote(id) {
    const note = notes.find((n) => n.id === id) ?? activeNote;
    noteDeletePending = { id, title: note?.title ?? "this note" };
  }

  async function confirmDeleteNote(closeTabFn, loadBookmarksFn) {
    const id = noteDeletePending.id;
    noteDeletePending = null;
    try {
      await invoke("delete_note", { id });
      await closeTabFn?.(id);
      if (activeNote?.id === id) clearActiveNote();
      loadBookmarksFn?.();
      invoke("remove_note_index", { noteId: id }).catch(() => {});
      return "refresh_notes";
    } catch (e) {
      onError?.(e);
      return null;
    }
  }

  async function openNoteById(id, navigateToNoteFn) {
    try {
      const note = await invoke("get_note", { id });
      navigateToNoteFn?.(note);
    } catch (e) {
      onError?.(e);
    }
  }

  async function filterByTag(tag, selectFolderFn, loadNotesFn, clearSelection) {
    tagFilter = tag;
    clearSelection?.();
    await loadNotesFn();
  }

  async function clearTagFilter(loadNotesFn) {
    tagFilter = null;
    await loadNotesFn();
  }

  async function moveNote(noteId, targetFolderId) {
    try {
      await invoke("move_note", { id: noteId, folderId: targetFolderId });
      return "refresh_notes";
    } catch (e) {
      onError?.(e);
      return null;
    }
  }

  function applyRestoredNote(note) {
    activeNote = note;
    editorTitle = note.title;
    editorContent = note.content;
    isDirty = false;
    transclusionRefresh += 1;
    loadActiveNoteMeta(note);
  }

  async function generateTestData(params) {
    testDataProgress = null;
    let unlisten = null;
    try {
      unlisten = await listen("test_data:progress", (ev) => {
        testDataProgress = /** @type {any} */ (ev.payload);
      });
      const summary = await invoke("generate_test_data", params);
      window.dispatchEvent(new CustomEvent("grimoire:vault-data-changed"));
      return summary;
    } catch (e) {
      onError?.(e);
      return null;
    } finally {
      unlisten?.();
      testDataProgress = null;
    }
  }
  async function reindexAll() {
    isReindexing = true;
    reindexProgress = { processed: 0, total: 0, indexed: 0 };
    let unlisten = null;
    try {
      unlisten = await listen("reindex:progress", (ev) => {
        const pl = ev.payload;
        reindexProgress = {
          processed: pl.processed ?? 0,
          total: pl.total ?? 0,
          indexed: pl.indexed ?? 0,
          phase: pl.phase ?? null,
          embeddingChunks: pl.embedding_chunks ?? null,
        };
      });
      const msg = await invoke("reindex_all");
      return { msg };
    } catch (e) {
      onError?.(e);
      return null;
    } finally {
      unlisten?.();
      isReindexing = false;
      reindexProgress = null;
    }
  }

  return {
    get notes() {
      return notes;
    },
    set notes(v) {
      notes = v;
    },
    get activeNote() {
      return activeNote;
    },
    set activeNote(v) {
      activeNote = v;
    },
    get editorTitle() {
      return editorTitle;
    },
    set editorTitle(v) {
      editorTitle = v;
    },
    get editorContent() {
      return editorContent;
    },
    set editorContent(v) {
      editorContent = v;
    },
    get isDirty() {
      return isDirty;
    },
    set isDirty(v) {
      isDirty = v;
    },
    get indexState() {
      return indexState;
    },
    set indexState(v) {
      indexState = v;
    },
    get editorTextareaEl() {
      return editorTextareaEl;
    },
    set editorTextareaEl(v) {
      editorTextareaEl = v;
    },
    get noteTags() {
      return noteTags;
    },
    get noteLinks() {
      return noteLinks;
    },
    get noteBacklinks() {
      return noteBacklinks;
    },
    get unlinkedMentions() {
      return unlinkedMentions;
    },
    get tagFilter() {
      return tagFilter;
    },
    set tagFilter(v) {
      tagFilter = v;
    },
    get allTags() {
      return allTags;
    },
    set allTags(v) {
      allTags = v;
    },
    get noteDeletePending() {
      return noteDeletePending;
    },
    set noteDeletePending(v) {
      noteDeletePending = v;
    },
    get testDataProgress() {
      return testDataProgress;
    },
    get isReindexing() {
      return isReindexing;
    },
    get reindexProgress() {
      return reindexProgress;
    },
    get transclusionRefresh() {
      return transclusionRefresh;
    },
    markDirty,
    loadNotes,
    loadAllTags,
    loadActiveNoteMeta,
    openNote,
    clearActiveNote,
    saveNote,
    startNoteInline,
    convertMention,
    deleteNote,
    confirmDeleteNote,
    openNoteById,
    filterByTag,
    clearTagFilter,
    moveNote,
    applyRestoredNote,
    generateTestData,
    reindexAll,
  };
}
