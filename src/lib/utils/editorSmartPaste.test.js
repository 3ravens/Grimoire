import { describe, expect, it } from "vitest";
import {
  isSingleUrl,
  trySmartPasteMarkdownLink,
} from "./editorSmartPaste.js";

describe("isSingleUrl", () => {
  it("accepts http(s) and other schemes", () => {
    expect(isSingleUrl("https://example.com")).toBe(true);
    expect(isSingleUrl("http://a")).toBe(true);
    expect(isSingleUrl("mailto:a@b.c")).toBe(true);
  });

  it("accepts www. hosts", () => {
    expect(isSingleUrl("www.example.com/path")).toBe(true);
  });

  it("rejects empty, whitespace, and non-urls", () => {
    expect(isSingleUrl("")).toBe(false);
    expect(isSingleUrl("https://a b")).toBe(false);
    expect(isSingleUrl("not a url")).toBe(false);
    expect(isSingleUrl("example.com")).toBe(false);
  });
});

describe("trySmartPasteMarkdownLink", () => {
  it("wraps selection as a markdown link", () => {
    expect(
      trySmartPasteMarkdownLink("see docs here", 4, 8, "https://ex.com"),
    ).toEqual({
      value: "see [docs](https://ex.com) here",
      selectionStart: "see [docs](https://ex.com)".length,
      selectionEnd: "see [docs](https://ex.com)".length,
    });
  });

  it("escapes ] in the label", () => {
    expect(
      trySmartPasteMarkdownLink("a]b", 0, 3, "https://ex.com")?.value,
    ).toBe("[a\\]b](https://ex.com)");
  });

  it("trims clipboard whitespace around a URL", () => {
    expect(
      trySmartPasteMarkdownLink("x", 0, 1, "  https://ex.com  ")?.value,
    ).toBe("[x](https://ex.com)");
  });

  it("returns null with no selection", () => {
    expect(
      trySmartPasteMarkdownLink("text", 2, 2, "https://ex.com"),
    ).toBeNull();
  });

  it("returns null when clipboard is not a URL", () => {
    expect(trySmartPasteMarkdownLink("sel", 0, 3, "plain")).toBeNull();
  });
});
