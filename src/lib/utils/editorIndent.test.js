import { describe, expect, it } from "vitest";
import { EDITOR_INDENT, applyEditorTab } from "./editorIndent.js";

describe("applyEditorTab", () => {
  it("inserts a 4-space soft tab at the caret", () => {
    const result = applyEditorTab("hello", 5, 5);
    expect(result).toEqual({
      value: "hello" + EDITOR_INDENT,
      selectionStart: 9,
      selectionEnd: 9,
    });
    expect(EDITOR_INDENT).toBe("    ");
  });

  it("replaces a single-line selection with a soft tab", () => {
    const result = applyEditorTab("abXXXcd", 2, 5);
    expect(result).toEqual({
      value: "ab" + EDITOR_INDENT + "cd",
      selectionStart: 6,
      selectionEnd: 6,
    });
  });

  it("indents every line in a multi-line selection", () => {
    const value = "one\ntwo\nthree";
    // select from 'n' in one through 't' in two
    const result = applyEditorTab(value, 2, 6);
    expect(result.value).toBe(
      EDITOR_INDENT + "one\n" + EDITOR_INDENT + "two\nthree",
    );
    expect(result.selectionStart).toBe(2 + EDITOR_INDENT.length);
    expect(result.selectionEnd).toBe(6 + EDITOR_INDENT.length * 2);
  });

  it("outdents the current line on Shift+Tab", () => {
    const value = EDITOR_INDENT + "hello";
    const result = applyEditorTab(value, 6, 6, { shiftKey: true });
    expect(result).toEqual({
      value: "hello",
      selectionStart: 2,
      selectionEnd: 2,
    });
  });

  it("outdents a hard tab", () => {
    const result = applyEditorTab("\thello", 3, 3, { shiftKey: true });
    expect(result).toEqual({
      value: "hello",
      selectionStart: 2,
      selectionEnd: 2,
    });
  });

  it("outdents partial leading spaces", () => {
    const result = applyEditorTab("  hello", 4, 4, { shiftKey: true });
    expect(result).toEqual({
      value: "hello",
      selectionStart: 2,
      selectionEnd: 2,
    });
  });

  it("outdents every selected line", () => {
    const value = EDITOR_INDENT + "a\n" + EDITOR_INDENT + "b";
    const result = applyEditorTab(value, 4, 10, { shiftKey: true });
    expect(result.value).toBe("a\nb");
    expect(result.selectionStart).toBe(0);
    expect(result.selectionEnd).toBe(2);
  });

  it("is a no-op outdent when there is no leading indent", () => {
    const result = applyEditorTab("hello", 2, 2, { shiftKey: true });
    expect(result).toEqual({
      value: "hello",
      selectionStart: 2,
      selectionEnd: 2,
    });
  });
});
