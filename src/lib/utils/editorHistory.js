/**
 * Custom undo/redo stack for the note body textarea.
 * Programmatic el.value assignment does not enter the browser undo stack,
 * so the editor owns history for both typing and helper mutations.
 *
 * @typedef {{ value: string, selectionStart: number, selectionEnd: number }} EditorSnapshot
 */

/**
 * Commit a new note body through ns, recording undo history when available.
 * Used by NoteEditor helpers and the editor context menu.
 *
 * @param {{
 *   editorContent: string,
 *   editorTextareaEl: HTMLTextAreaElement | null,
 *   editorHistory?: ReturnType<typeof createEditorHistory> | null,
 *   markDirty: () => void,
 * }} ns
 * @param {string} nextValue
 * @param {{ selectionStart: number, selectionEnd: number } | null} [selection]
 */
export function commitEditorBody(ns, nextValue, selection = null) {
  const el = ns.editorTextareaEl;
  const hist = ns.editorHistory;
  if (hist) {
    if (el) {
      hist.push({
        value: el.value,
        selectionStart: el.selectionStart,
        selectionEnd: el.selectionEnd,
      });
    } else {
      hist.push({
        value: ns.editorContent ?? "",
        selectionStart: 0,
        selectionEnd: 0,
      });
    }
    hist.markCoalesceBoundary();
  }

  ns.editorContent = nextValue;
  ns.markDirty();

  if (el) {
    el.value = nextValue;
    if (selection) {
      el.selectionStart = selection.selectionStart;
      el.selectionEnd = selection.selectionEnd;
    }
  } else if (selection) {
    requestAnimationFrame(() => {
      const later = ns.editorTextareaEl;
      if (!later) return;
      later.selectionStart = selection.selectionStart;
      later.selectionEnd = selection.selectionEnd;
    });
  }
}

/**
 * @param {{ maxDepth?: number, coalesceMs?: number }} [opts]
 */
export function createEditorHistory({ maxDepth = 100, coalesceMs = 400 } = {}) {
  /** @type {EditorSnapshot[]} */
  let undoStack = [];
  /** @type {EditorSnapshot[]} */
  let redoStack = [];
  /** @type {number | null} */
  let lastPushAt = null;
  let coalesceOpen = false;

  /**
   * Clear stacks (note open / switch).
   * @param {EditorSnapshot | null} [_snapshot]
   */
  function reset(_snapshot = null) {
    undoStack = [];
    redoStack = [];
    lastPushAt = null;
    coalesceOpen = false;
    void _snapshot;
  }

  /** Force the next typing capture to start a new undo step. */
  function markCoalesceBoundary() {
    coalesceOpen = false;
    lastPushAt = null;
  }

  /**
   * Record state before a mutation. Clears redo.
   * When coalesce is true, consecutive pushes within coalesceMs reuse one slot.
   *
   * @param {EditorSnapshot} before
   * @param {{ coalesce?: boolean }} [opts]
   */
  function push(before, { coalesce = false } = {}) {
    const snap = cloneSnapshot(before);
    const now = Date.now();

    if (
      coalesce &&
      coalesceOpen &&
      lastPushAt != null &&
      now - lastPushAt <= coalesceMs
    ) {
      lastPushAt = now;
      return;
    }

    const top = undoStack[undoStack.length - 1];
    if (top && sameSnapshot(top, snap)) {
      lastPushAt = now;
      coalesceOpen = coalesce;
      return;
    }

    undoStack.push(snap);
    if (undoStack.length > maxDepth) {
      undoStack.shift();
    }
    redoStack = [];
    lastPushAt = now;
    coalesceOpen = coalesce;
  }

  /**
   * @param {EditorSnapshot} current
   * @returns {EditorSnapshot | null}
   */
  function undo(current) {
    if (undoStack.length === 0) return null;
    const target = undoStack.pop();
    redoStack.push(cloneSnapshot(current));
    markCoalesceBoundary();
    return target ?? null;
  }

  /**
   * @param {EditorSnapshot} current
   * @returns {EditorSnapshot | null}
   */
  function redo(current) {
    if (redoStack.length === 0) return null;
    const target = redoStack.pop();
    undoStack.push(cloneSnapshot(current));
    if (undoStack.length > maxDepth) {
      undoStack.shift();
    }
    markCoalesceBoundary();
    return target ?? null;
  }

  return {
    reset,
    push,
    undo,
    redo,
    markCoalesceBoundary,
    get undoDepth() {
      return undoStack.length;
    },
    get redoDepth() {
      return redoStack.length;
    },
  };
}

/** @param {EditorSnapshot} s */
function cloneSnapshot(s) {
  return {
    value: s.value,
    selectionStart: s.selectionStart,
    selectionEnd: s.selectionEnd,
  };
}

/**
 * @param {EditorSnapshot} a
 * @param {EditorSnapshot} b
 */
function sameSnapshot(a, b) {
  return (
    a.value === b.value &&
    a.selectionStart === b.selectionStart &&
    a.selectionEnd === b.selectionEnd
  );
}
