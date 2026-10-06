import { describe, expect, it } from "vitest";
import { marked } from "marked";
import { EDITOR_INDENT } from "./editorIndent.js";
import {
  annotateTaskListItems,
  applyChecklistToggle,
  applyListEnter,
  applyListTab,
  enableRootChecklistInputs,
  isInsideFencedCode,
  normalizeChecklistMarkdownForRender,
  parseListLine,
  toggleChecklistAtIndex,
} from "./editorLists.js";

describe("parseListLine", () => {
  it("parses unordered markers", () => {
    expect(parseListLine("- item")).toMatchObject({
      kind: "unordered",
      marker: "- ",
      content: "item",
      isEmpty: false,
    });
    expect(parseListLine("* item")).toMatchObject({ marker: "* " });
    expect(parseListLine("+ item")).toMatchObject({ marker: "+ " });
  });

  it("parses ordered markers", () => {
    expect(parseListLine("1. item")).toMatchObject({
      kind: "ordered",
      marker: "1. ",
      orderedDigits: "1",
      content: "item",
    });
  });

  it("parses checklist lines", () => {
    expect(parseListLine("- [ ] todo")).toMatchObject({
      kind: "checklist",
      marker: "- [ ] ",
      checked: false,
      content: "todo",
    });
    expect(parseListLine("* [x] done")).toMatchObject({
      kind: "checklist",
      marker: "* [x] ",
      checked: true,
      content: "done",
    });
    expect(parseListLine("+ [X] done")).toMatchObject({
      checked: true,
      marker: "+ [x] ",
    });
    expect(parseListLine("- [ ]todo")).toMatchObject({
      kind: "checklist",
      content: "todo",
    });
    expect(parseListLine("- [ ] todo\r")).toMatchObject({
      kind: "checklist",
      content: "todo",
    });
  });

  it("rejects non-list lines", () => {
    expect(parseListLine("plain")).toBeNull();
    expect(parseListLine("# heading")).toBeNull();
  });
});

describe("isInsideFencedCode", () => {
  it("detects content inside ``` fences", () => {
    const value = "before\n```\ncode here\n```\nafter";
    const codeIdx = value.indexOf("code");
    expect(isInsideFencedCode(value, codeIdx)).toBe(true);
    expect(isInsideFencedCode(value, value.indexOf("before"))).toBe(false);
    expect(isInsideFencedCode(value, value.indexOf("after"))).toBe(false);
  });

  it("detects content inside ~~~ fences", () => {
    const value = "~~~\nsecret\n~~~";
    expect(isInsideFencedCode(value, value.indexOf("secret"))).toBe(true);
  });

  it("treats unclosed fences as code through EOF", () => {
    const value = "```\nstill code";
    expect(isInsideFencedCode(value, value.indexOf("still"))).toBe(true);
  });

  it("handles longer fences", () => {
    const value = "````\ninner\n````";
    expect(isInsideFencedCode(value, value.indexOf("inner"))).toBe(true);
  });
});

describe("applyListEnter", () => {
  it("continues an unordered list at end of line", () => {
    const value = "- item";
    const result = applyListEnter(value, value.length, value.length);
    expect(result).toEqual({
      value: "- item\n- ",
      selectionStart: "- item\n- ".length,
      selectionEnd: "- item\n- ".length,
    });
  });

  it("continues * and + with the same marker", () => {
    expect(applyListEnter("* a", 3, 3)?.value).toBe("* a\n* ");
    expect(applyListEnter("+ a", 3, 3)?.value).toBe("+ a\n+ ");
  });

  it("continues ordered lists with the next number", () => {
    const value = "1. item";
    const result = applyListEnter(value, value.length, value.length);
    expect(result?.value).toBe("1. item\n2. ");
  });

  it("splits mid-item content onto the new list line", () => {
    // caret between "hel" and "lo"
    const value = "- hello";
    const caret = value.indexOf("lo");
    const result = applyListEnter(value, caret, caret);
    expect(result?.value).toBe("- hel\n- lo");
    expect(result?.selectionStart).toBe("- hel\n- ".length);
  });

  it("exits a top-level empty list item", () => {
    const value = "- ";
    const result = applyListEnter(value, 2, 2);
    expect(result).toEqual({
      value: "",
      selectionStart: 0,
      selectionEnd: 0,
    });
  });

  it("outdents a nested empty list item before exiting", () => {
    const value = `${EDITOR_INDENT}- `;
    const caret = value.length;
    const result = applyListEnter(value, caret, caret);
    expect(result?.value).toBe("- ");
    expect(result?.selectionStart).toBe("- ".length);
  });

  it("leaves following lines unchanged", () => {
    const value = "- item\nnext";
    const caret = "- item".length;
    const result = applyListEnter(value, caret, caret);
    expect(result?.value).toBe("- item\n- \nnext");
  });

  it("returns null inside fenced code", () => {
    const value = "```\n- item\n```";
    const caret = value.indexOf("item") + 4;
    expect(applyListEnter(value, caret, caret)).toBeNull();
  });

  it("continues checklist lines with the same checked state", () => {
    expect(applyListEnter("- [ ] todo", "- [ ] todo".length, "- [ ] todo".length)?.value)
      .toBe("- [ ] todo\n- [ ] ");
    expect(applyListEnter("- [x] done", "- [x] done".length, "- [x] done".length)?.value)
      .toBe("- [x] done\n- [x] ");
  });

  it("exits an empty checklist item", () => {
    const value = "- [ ]";
    const result = applyListEnter(value, value.length, value.length);
    expect(result?.value).toBe("");
  });

  it("returns null when caret is before contentStart", () => {
    const value = "- item";
    expect(applyListEnter(value, 0, 0)).toBeNull();
    expect(applyListEnter(value, 1, 1)).toBeNull(); // on the marker
  });

  it("returns null for a non-collapsed selection", () => {
    expect(applyListEnter("- item", 2, 4)).toBeNull();
  });

  it("increments large ordered markers with BigInt", () => {
    const value = "999999999999999999. item";
    const result = applyListEnter(value, value.length, value.length);
    expect(result?.value).toBe(
      "999999999999999999. item\n1000000000000000000. ",
    );
  });
});

describe("applyListTab", () => {
  it("nests a list line on Tab", () => {
    const value = "- item";
    const caret = 2;
    const result = applyListTab(value, caret, caret);
    expect(result?.value).toBe(`${EDITOR_INDENT}- item`);
    expect(result?.selectionStart).toBe(caret + EDITOR_INDENT.length);
  });

  it("unnests a list line on Shift+Tab", () => {
    const value = `${EDITOR_INDENT}- item`;
    const caret = EDITOR_INDENT.length + 2;
    const result = applyListTab(value, caret, caret, { shiftKey: true });
    expect(result?.value).toBe("- item");
    expect(result?.selectionStart).toBe(2);
  });

  it("returns null for non-list lines", () => {
    expect(applyListTab("plain", 2, 2)).toBeNull();
  });

  it("returns null for multi-line / non-collapsed selection", () => {
    expect(applyListTab("- a\n- b", 0, 6)).toBeNull();
  });

  it("returns null inside fenced code", () => {
    const value = "```\n- item\n```";
    const caret = value.indexOf("-");
    expect(applyListTab(value, caret, caret)).toBeNull();
  });

  it("nests a checklist line on Tab", () => {
    const value = "- [ ] todo";
    const caret = value.length;
    const result = applyListTab(value, caret, caret);
    expect(result?.value).toBe(`${EDITOR_INDENT}- [ ] todo`);
  });
});

describe("applyChecklistToggle", () => {
  it("toggles unchecked to checked", () => {
    const value = "- [ ] todo";
    const result = applyChecklistToggle(value, value.length, value.length);
    expect(result?.value).toBe("- [x] todo");
  });

  it("toggles with a selection on the line", () => {
    const value = "- [ ] todo";
    expect(applyChecklistToggle(value, 2, 8)?.value).toBe("- [x] todo");
  });

  it("toggles checked to unchecked and normalizes X", () => {
    expect(applyChecklistToggle("- [X] done", 8, 8)?.value).toBe("- [ ] done");
    expect(applyChecklistToggle("- [x] done", 8, 8)?.value).toBe("- [ ] done");
  });

  it("returns null for non-checklist and fenced lines", () => {
    expect(applyChecklistToggle("- item", 3, 3)).toBeNull();
    const fenced = "```\n- [ ] x\n```";
    const caret = fenced.indexOf("x");
    expect(applyChecklistToggle(fenced, caret, caret)).toBeNull();
  });
});

describe("toggleChecklistAtIndex", () => {
  it("toggles by document index skipping fences", () => {
    const value = "- [ ] a\n```\n- [ ] ignore\n```\n- [x] b";
    expect(toggleChecklistAtIndex(value, 0)?.value).toBe(
      "- [x] a\n```\n- [ ] ignore\n```\n- [x] b",
    );
    expect(toggleChecklistAtIndex(value, 1)?.value).toBe(
      "- [ ] a\n```\n- [ ] ignore\n```\n- [ ] b",
    );
    expect(toggleChecklistAtIndex(value, 2)).toBeNull();
  });
});

describe("enableRootChecklistInputs", () => {
  it("removes disabled and assigns sequential indices", () => {
    const html =
      '<ul><li><input disabled="" type="checkbox"> a</li>' +
      '<li><input checked="" disabled="" type="checkbox"> b</li></ul>';
    const { html: out, nextIndex } = enableRootChecklistInputs(html, 0);
    expect(nextIndex).toBe(2);
    expect(out).toContain('data-checklist-index="0"');
    expect(out).toContain('data-checklist-index="1"');
    expect(out).not.toMatch(/\bdisabled\b/i);
  });

  it("continues numbering from startIndex", () => {
    const html = '<input disabled="" type="checkbox">';
    const { html: out, nextIndex } = enableRootChecklistInputs(html, 3);
    expect(nextIndex).toBe(4);
    expect(out).toContain('data-checklist-index="3"');
  });
});

describe("annotateTaskListItems", () => {
  it("adds task-list-item class around checkbox lis", () => {
    const html = '<ul><li><input disabled="" type="checkbox"> a</li></ul>';
    expect(annotateTaskListItems(html)).toContain('class="task-list-item"');
  });
});

describe("normalizeChecklistMarkdownForRender", () => {
  it("pads empty checklist lines so marked emits checkboxes", () => {
    const normalized = normalizeChecklistMarkdownForRender("- [x]\n- [ ] \n- [x] keep");
    expect(normalized.split("\n")[0]).toContain("\u200b");
    expect(normalized.split("\n")[2]).toBe("- [x] keep");

    const html = marked.parse(normalized);
    expect(html.match(/type="checkbox"/g)?.length).toBe(3);
    expect(html).not.toMatch(/<li>\[x\]<\/li>/);
  });
});
