import { describe, expect, it } from "vitest";
import {
  DEFAULT_READING_WPM,
  MAX_READING_WPM,
  MIN_READING_WPM,
  countReadableWords,
  editorReadableStats,
  normalizeReadingWpm,
  readingMinutes,
  toReadablePlainText,
} from "./readableText.js";

describe("readableText", () => {
  it("strips heading chrome and counts heading words only", () => {
    expect(toReadablePlainText("# Project plan")).toBe("Project plan");
    expect(countReadableWords("# Project plan")).toBe(2);
    expect(countReadableWords("Project plan with more words in prose.")).toBe(7);
  });

  it("counts wiki-link labels instead of targets", () => {
    expect(toReadablePlainText("[[Target Note|hi there]]")).toBe("hi there");
    expect(countReadableWords("[[Target Note|hi there]]")).toBe(2);
    expect(countReadableWords("[[Target Note]]")).toBe(2);
  });

  it("does not expand transclusions beyond the source token text", () => {
    const text = "Start ![[Very Long Other Note]] end";
    expect(toReadablePlainText(text)).toBe("Start Very Long Other Note end");
    expect(countReadableWords(text)).toBe(6);
  });

  it("keeps fenced code content but removes fence chrome", () => {
    const source = "```js\nconst hello = 42;\n```\n~~~sql\nselect * from notes;\n~~~";
    expect(toReadablePlainText(source)).toBe(
      "const hello = 42; select * from notes;",
    );
    expect(countReadableWords(source)).toBe(6);
  });

  it("protects inline code from wiki and markdown rewriting", () => {
    expect(toReadablePlainText("Use `[[Target|Alias]]` and `**bold**` literally."))
      .toBe("Use [[Target|Alias]] and **bold** literally.");
    expect(countReadableWords("Use `[[Target|Alias]]` and `**bold**` literally."))
      .toBe(6);
  });

  it("uses readable labels for links and alt text for images", () => {
    const source = "[Read me](https://example.com) ![diagram here](img.png) <https://example.com>";
    expect(toReadablePlainText(source)).toBe("Read me diagram here");
    expect(countReadableWords(source)).toBe(4);
  });

  it("strips reference-style markdown and ignores definitions", () => {
    const source = "[Read me][guide] ![diagram][img]\n\n[guide]: https://example.com\n[img]: img.png";
    expect(toReadablePlainText(source)).toBe("Read me diagram");
    expect(countReadableWords(source)).toBe(3);
  });

  it("drops html comments but keeps visible html text", () => {
    const source = "Visible <!-- hidden words --> <span>inline text</span>";
    expect(toReadablePlainText(source)).toBe("Visible inline text");
    expect(countReadableWords(source)).toBe(3);
  });

  it("removes punctuation-only leftovers from word counts", () => {
    expect(countReadableWords("*** ~~ ---")).toBe(0);
  });

  it("counts punctuation-separated prose as separate words", () => {
    expect(countReadableWords("hello/world one-two three,four five—six")).toBe(8);
  });

  it("normalizes reading speed and rounds minutes up", () => {
    expect(DEFAULT_READING_WPM).toBe(200);
    expect(normalizeReadingWpm("")).toBe(DEFAULT_READING_WPM);
    expect(normalizeReadingWpm("nope")).toBe(DEFAULT_READING_WPM);
    expect(normalizeReadingWpm(20)).toBe(MIN_READING_WPM);
    expect(normalizeReadingWpm(999)).toBe(MAX_READING_WPM);
    expect(readingMinutes(0, 200)).toBe(0);
    expect(readingMinutes(199, 200)).toBe(1);
    expect(readingMinutes(200, 200)).toBe(1);
    expect(readingMinutes(201, 200)).toBe(2);
    expect(readingMinutes(201, "bad")).toBe(2);
  });

  it("keeps the lock guard outside the pure helper", () => {
    expect(editorReadableStats({ locked: true, body: "secret words", wpm: 200 }))
      .toBeNull();
    expect(editorReadableStats({ locked: false, body: "# One two", wpm: 100 }))
      .toEqual({
        wordCount: 2,
        readingMinutes: 1,
      });
  });

  it("treats an unclosed fenced block as code through EOF", () => {
    const source = "```js\n[[Alias|Display]]\n**bold**";
    expect(toReadablePlainText(source)).toBe("[[Alias|Display]] **bold**");
    expect(countReadableWords(source)).toBe(3);
  });
});
