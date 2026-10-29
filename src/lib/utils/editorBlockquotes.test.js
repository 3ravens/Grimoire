import { describe, expect, it } from "vitest";
import { applyListEnter } from "./editorLists.js";
import {
  applyBlockquoteEnter,
  parseBlockquoteLine,
} from "./editorBlockquotes.js";

describe("parseBlockquoteLine", () => {
  it("parses a single-level quote", () => {
    expect(parseBlockquoteLine("> hello")).toMatchObject({
      indent: "",
      prefix: "> ",
      content: "hello",
      isEmpty: false,
      contentStart: 2,
    });
  });

  it("parses nested quotes and leading indent", () => {
    expect(parseBlockquoteLine("  > > nested")).toMatchObject({
      indent: "  ",
      prefix: "> > ",
      content: "nested",
      isEmpty: false,
    });
  });

  it("parses tight >> markers", () => {
    expect(parseBlockquoteLine(">>nested")).toMatchObject({
      prefix: ">>",
      content: "nested",
    });
  });

  it("treats empty content and empty quoted list items as empty", () => {
    expect(parseBlockquoteLine("> ")).toMatchObject({ isEmpty: true, content: "" });
    expect(parseBlockquoteLine(">")).toMatchObject({ isEmpty: true, content: "" });
    expect(parseBlockquoteLine("> - ")).toMatchObject({ isEmpty: true });
    expect(parseBlockquoteLine("> - [ ] ")).toMatchObject({ isEmpty: true });
  });

  it("is CRLF-tolerant on a single line slice", () => {
    expect(parseBlockquoteLine("> hello\r")).toMatchObject({
      content: "hello",
      isEmpty: false,
    });
  });

  it("rejects non-quote lines", () => {
    expect(parseBlockquoteLine("plain")).toBeNull();
    expect(parseBlockquoteLine("- item")).toBeNull();
  });
});

describe("applyBlockquoteEnter", () => {
  it("continues a quote at end of line", () => {
    const value = "> item";
    const result = applyBlockquoteEnter(value, value.length, value.length);
    expect(result).toEqual({
      value: "> item\n> ",
      selectionStart: "> item\n> ".length,
      selectionEnd: "> item\n> ".length,
    });
  });

  it("continues nested quotes with the same prefix", () => {
    const value = "> > nested";
    const result = applyBlockquoteEnter(value, value.length, value.length);
    expect(result?.value).toBe("> > nested\n> > ");
    expect(result?.selectionStart).toBe("> > nested\n> > ".length);
  });

  it("splits mid-line content onto the new quote line", () => {
    const value = "> hello";
    const caret = value.indexOf("lo");
    const result = applyBlockquoteEnter(value, caret, caret);
    expect(result?.value).toBe("> hel\n> lo");
    expect(result?.selectionStart).toBe("> hel\n> ".length);
  });

  it("leaves following lines unchanged", () => {
    const value = "> item\nnext";
    const caret = "> item".length;
    const result = applyBlockquoteEnter(value, caret, caret);
    expect(result?.value).toBe("> item\n> \nnext");
  });

  it("exits a top-level empty quote to a blank line", () => {
    const value = "> ";
    const result = applyBlockquoteEnter(value, value.length, value.length);
    expect(result).toEqual({
      value: "",
      selectionStart: 0,
      selectionEnd: 0,
    });
  });

  it("preserves leading indent when exiting a single-level empty quote", () => {
    const value = "  > ";
    const result = applyBlockquoteEnter(value, value.length, value.length);
    expect(result).toEqual({
      value: "  ",
      selectionStart: 2,
      selectionEnd: 2,
    });
  });

  it("outdents one level on an empty nested quote", () => {
    const value = "> > ";
    const result = applyBlockquoteEnter(value, value.length, value.length);
    expect(result?.value).toBe("> ");
    expect(result?.selectionStart).toBe("> ".length);
  });

  it("returns null inside fenced code", () => {
    const value = "```\n> item\n```";
    const caret = value.indexOf("item") + 4;
    expect(applyBlockquoteEnter(value, caret, caret)).toBeNull();
  });

  it("returns null for non-quote lines", () => {
    expect(applyBlockquoteEnter("- item", 6, 6)).toBeNull();
    expect(applyBlockquoteEnter("plain", 5, 5)).toBeNull();
  });

  it("returns null for a non-collapsed selection", () => {
    expect(applyBlockquoteEnter("> item", 2, 4)).toBeNull();
  });

  it("returns null when caret is on the marker prefix", () => {
    const value = "> item";
    expect(applyBlockquoteEnter(value, 0, 0)).toBeNull();
    expect(applyBlockquoteEnter(value, 1, 1)).toBeNull();
  });

  it("exits an empty quoted list item instead of spawning quotes", () => {
    const value = "> - ";
    const result = applyBlockquoteEnter(value, value.length, value.length);
    expect(result?.value).toBe("");
    expect(result?.selectionStart).toBe(0);
  });

  it("continues non-empty quoted lists with the quote prefix only", () => {
    // Out of scope for full list+quote; pin naive quote-only continuation.
    const value = "> - foo";
    const result = applyBlockquoteEnter(value, value.length, value.length);
    expect(result?.value).toBe("> - foo\n> ");
  });
});

describe("list vs blockquote precedence", () => {
  it("leaves plain list lines to applyListEnter", () => {
    expect(applyBlockquoteEnter("- item", 6, 6)).toBeNull();
    expect(applyListEnter("- item", 6, 6)?.value).toBe("- item\n- ");
  });

  it("lets applyListEnter own lines that start with a list marker", () => {
    const value = "- > foo";
    expect(applyListEnter(value, value.length, value.length)?.value).toBe(
      "- > foo\n- ",
    );
    expect(applyBlockquoteEnter(value, value.length, value.length)).toBeNull();
  });
});
