import { describe, expect, it } from "vitest";
import { cycleHeadingLevel } from "./editorHeadings.js";

describe("cycleHeadingLevel", () => {
  it("promotes paragraph to H1", () => {
    expect(cycleHeadingLevel("hello", 0, 0)).toEqual({
      value: "# hello",
      selectionStart: 2,
      selectionEnd: 2,
    });
  });

  it("cycles H1 → H2 → H3 → H4 → paragraph", () => {
    expect(cycleHeadingLevel("# a", 0, 0).value).toBe("## a");
    expect(cycleHeadingLevel("## a", 0, 0).value).toBe("### a");
    expect(cycleHeadingLevel("### a", 0, 0).value).toBe("#### a");
    expect(cycleHeadingLevel("#### a", 0, 0).value).toBe("a");
  });

  it("reverse cycles paragraph → H4 → H3 → … → paragraph", () => {
    expect(cycleHeadingLevel("a", 0, 0, { reverse: true }).value).toBe(
      "#### a",
    );
    expect(cycleHeadingLevel("#### a", 0, 0, { reverse: true }).value).toBe(
      "### a",
    );
    expect(cycleHeadingLevel("# a", 0, 0, { reverse: true }).value).toBe("a");
  });

  it("applies the same step to each line in a multi-line selection", () => {
    const value = "one\ntwo";
    const result = cycleHeadingLevel(value, 0, value.length);
    expect(result.value).toBe("# one\n# two");
  });

  it("leaves setext-looking content alone only when not ATX (plain lines still cycle)", () => {
    // Setext is not detected; plain "Title" still becomes ATX H1 (contract: ignore setext).
    expect(cycleHeadingLevel("Title\n=====", 0, 0).value).toBe("# Title\n=====");
  });

  it("leaves H5+ alone", () => {
    expect(cycleHeadingLevel("##### x", 0, 0).value).toBe("##### x");
  });

  it("preserves leading indent up to 3 spaces", () => {
    expect(cycleHeadingLevel("  hello", 0, 0).value).toBe("  # hello");
  });
});
