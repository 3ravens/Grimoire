import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { createEditorHistory } from "./editorHistory.js";

/** @param {string} value @param {number} [sel] */
function snap(value, sel = value.length) {
  return { value, selectionStart: sel, selectionEnd: sel };
}

describe("createEditorHistory", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("undo restores the pushed before-state and redo restores current", () => {
    const h = createEditorHistory();
    h.push(snap("a"));
    const undone = h.undo(snap("ab"));
    expect(undone).toEqual(snap("a"));
    expect(h.undoDepth).toBe(0);
    expect(h.redoDepth).toBe(1);

    const redone = h.redo(snap("a"));
    expect(redone).toEqual(snap("ab"));
    expect(h.undoDepth).toBe(1);
    expect(h.redoDepth).toBe(0);
  });

  it("returns null when stacks are empty", () => {
    const h = createEditorHistory();
    expect(h.undo(snap(""))).toBeNull();
    expect(h.redo(snap(""))).toBeNull();
  });

  it("clears redo on a new push", () => {
    const h = createEditorHistory();
    h.push(snap("a"));
    h.undo(snap("b"));
    expect(h.redoDepth).toBe(1);
    h.push(snap("c"));
    expect(h.redoDepth).toBe(0);
  });

  it("respects maxDepth", () => {
    const h = createEditorHistory({ maxDepth: 2 });
    h.push(snap("1"));
    h.push(snap("2"));
    h.push(snap("3"));
    expect(h.undoDepth).toBe(2);
    expect(h.undo(snap("4"))?.value).toBe("3");
    expect(h.undo(snap("3"))?.value).toBe("2");
    expect(h.undo(snap("2"))).toBeNull();
  });

  it("coalesces rapid typing pushes into one undo step", () => {
    const h = createEditorHistory({ coalesceMs: 400 });
    h.push(snap(""), { coalesce: true });
    vi.setSystemTime(100);
    h.push(snap("a"), { coalesce: true });
    vi.setSystemTime(200);
    h.push(snap("ab"), { coalesce: true });
    expect(h.undoDepth).toBe(1);
    expect(h.undo(snap("abc"))).toEqual(snap(""));
  });

  it("starts a new step after markCoalesceBoundary", () => {
    const h = createEditorHistory({ coalesceMs: 400 });
    h.push(snap(""), { coalesce: true });
    vi.setSystemTime(50);
    h.push(snap("a"), { coalesce: true });
    h.markCoalesceBoundary();
    vi.setSystemTime(60);
    h.push(snap("a"), { coalesce: true });
    expect(h.undoDepth).toBe(2);
  });

  it("reset clears both stacks", () => {
    const h = createEditorHistory();
    h.push(snap("a"));
    h.undo(snap("b"));
    h.reset();
    expect(h.undoDepth).toBe(0);
    expect(h.redoDepth).toBe(0);
  });

  it("skips pushing an identical top snapshot", () => {
    const h = createEditorHistory();
    h.push(snap("a", 1));
    h.push(snap("a", 1));
    expect(h.undoDepth).toBe(1);
  });
});
