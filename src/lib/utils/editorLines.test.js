import { describe, expect, it } from "vitest";
import { duplicateEditorLines, moveEditorLines } from "./editorLines.js";

describe("moveEditorLines", () => {
  it("moves a single line down", () => {
    const value = "a\nb\nc";
    const result = moveEditorLines(value, 0, 0, 1);
    expect(result?.value).toBe("b\na\nc");
    expect(result?.selectionStart).toBe(2);
  });

  it("moves a single line up", () => {
    const value = "a\nb\nc";
    const result = moveEditorLines(value, 2, 2, -1);
    expect(result?.value).toBe("b\na\nc");
    expect(result?.selectionStart).toBe(0);
  });

  it("moves a multi-line selection down", () => {
    const value = "a\nb\nc\nd";
    // select lines a and b (caret spanning)
    const result = moveEditorLines(value, 0, 3, 1);
    expect(result?.value).toBe("c\na\nb\nd");
  });

  it("returns null at top when moving up", () => {
    expect(moveEditorLines("a\nb", 0, 0, -1)).toBeNull();
  });

  it("returns null at bottom when moving down", () => {
    expect(moveEditorLines("a\nb", 2, 2, 1)).toBeNull();
  });

  it("preserves relative selection", () => {
    const value = "hello\nworld";
    // select "ell" within hello
    const result = moveEditorLines(value, 1, 4, 1);
    expect(result?.value).toBe("world\nhello");
    expect(result?.selectionStart).toBe(7);
    expect(result?.selectionEnd).toBe(10);
  });
});

describe("duplicateEditorLines", () => {
  it("duplicates a line below and selects the copy", () => {
    const value = "a\nb";
    const result = duplicateEditorLines(value, 0, 0, 1);
    expect(result?.value).toBe("a\na\nb");
    expect(result?.selectionStart).toBe(2);
  });

  it("duplicates a line above and selects the copy", () => {
    const value = "a\nb";
    const result = duplicateEditorLines(value, 2, 2, -1);
    expect(result?.value).toBe("a\nb\nb");
    // wait - duplicating "b" above should give "b\na\nb"? No - block is "b" at end
    // blockStart for caret on b: index 2, block is "b", insert above at 2: "a\n" + "b\n" + "b" = "a\nb\nb"
    expect(result?.value).toBe("a\nb\nb");
    expect(result?.selectionStart).toBe(2);
  });

  it("duplicates multi-line below", () => {
    const value = "a\nb\nc";
    const result = duplicateEditorLines(value, 0, 3, 1);
    expect(result?.value).toBe("a\nb\na\nb\nc");
  });

  it("duplicates last line without trailing newline", () => {
    const value = "a\nb";
    const result = duplicateEditorLines(value, 2, 2, 1);
    expect(result?.value).toBe("a\nb\nb");
    expect(result?.selectionStart).toBe(4);
  });
});
