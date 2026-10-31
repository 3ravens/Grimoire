import { describe, expect, it } from "vitest";
import {
  applyAutoPair,
  applyMarkdownWrap,
  isInsideInlineCode,
} from "./editorWrap.js";

describe("applyMarkdownWrap", () => {
  it("wraps a selection with bold markers", () => {
    expect(applyMarkdownWrap("hello world", 0, 5, "**")).toEqual({
      value: "**hello** world",
      selectionStart: 2,
      selectionEnd: 7,
    });
  });

  it("wraps with italic asterisks", () => {
    expect(applyMarkdownWrap("hello", 0, 5, "*")).toEqual({
      value: "*hello*",
      selectionStart: 1,
      selectionEnd: 6,
    });
  });

  it("wraps with inline code backticks", () => {
    expect(applyMarkdownWrap("code", 0, 4, "`")).toEqual({
      value: "`code`",
      selectionStart: 1,
      selectionEnd: 5,
    });
  });

  it("inserts empty wrap with caret between markers", () => {
    expect(applyMarkdownWrap("ab", 1, 1, "**")).toEqual({
      value: "a****b",
      selectionStart: 3,
      selectionEnd: 3,
    });
  });

  it("unwraps when markers sit outside the selection", () => {
    expect(applyMarkdownWrap("**hello**", 2, 7, "**")).toEqual({
      value: "hello",
      selectionStart: 0,
      selectionEnd: 5,
    });
  });

  it("unwraps when selection includes the markers", () => {
    expect(applyMarkdownWrap("**hello**", 0, 9, "**")).toEqual({
      value: "hello",
      selectionStart: 0,
      selectionEnd: 5,
    });
  });

  it("unwraps italic with outside markers", () => {
    expect(applyMarkdownWrap("*hi*", 1, 3, "*")).toEqual({
      value: "hi",
      selectionStart: 0,
      selectionEnd: 2,
    });
  });
});

describe("applyAutoPair", () => {
  it("pairs asterisk at caret", () => {
    expect(applyAutoPair("ab", 1, 1, "*")).toEqual({
      value: "a**b",
      selectionStart: 2,
      selectionEnd: 2,
    });
  });

  it("pairs underscore, backtick, and paren", () => {
    expect(applyAutoPair("", 0, 0, "_")?.value).toBe("__");
    expect(applyAutoPair("", 0, 0, "`")?.value).toBe("``");
    expect(applyAutoPair("", 0, 0, "(")?.value).toBe("()");
  });

  it("returns null for a non-collapsed selection", () => {
    expect(applyAutoPair("ab", 0, 1, "*")).toBeNull();
  });

  it("returns null inside fenced code", () => {
    const value = "```\nx\n```";
    const caret = value.indexOf("x") + 1;
    expect(applyAutoPair(value, caret, caret, "*")).toBeNull();
  });

  it("returns null inside inline code for non-backtick", () => {
    const value = "`code`";
    const caret = value.indexOf("o");
    expect(isInsideInlineCode(value, caret)).toBe(true);
    expect(applyAutoPair(value, caret, caret, "*")).toBeNull();
  });

  it("returns null inside inline code for backtick", () => {
    const value = "`code`";
    const caret = value.indexOf("d");
    expect(applyAutoPair(value, caret, caret, "`")).toBeNull();
  });

  it("type-through when next char is already the closer", () => {
    expect(applyAutoPair("a)", 1, 1, "(")).toBeNull();
  });
});
