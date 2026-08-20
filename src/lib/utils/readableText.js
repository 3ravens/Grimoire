const PLACEHOLDER_PREFIX = "\u0000READABLECODE";

export const DEFAULT_READING_WPM = 200;
export const MIN_READING_WPM = 50;
export const MAX_READING_WPM = 600;

/**
 * @param {unknown} raw
 */
export function normalizeReadingWpm(raw) {
  if (raw == null) return DEFAULT_READING_WPM;
  if (typeof raw === "string" && raw.trim() === "") return DEFAULT_READING_WPM;
  const value = Number(raw);
  if (!Number.isFinite(value)) return DEFAULT_READING_WPM;
  const rounded = Math.round(value);
  if (rounded < MIN_READING_WPM) return MIN_READING_WPM;
  if (rounded > MAX_READING_WPM) return MAX_READING_WPM;
  return rounded;
}

/**
 * @param {string} markdown
 */
export function countReadableWords(markdown) {
  const text = toReadablePlainText(markdown);
  if (!text) return 0;
  return countWordsInText(text);
}

/**
 * @param {number} wordCount
 * @param {unknown} wpm
 */
export function readingMinutes(wordCount, wpm) {
  const words = Number(wordCount);
  if (!Number.isFinite(words) || words <= 0) return 0;
  return Math.ceil(words / normalizeReadingWpm(wpm));
}

/**
 * Helper for editor wiring so the lock guard stays outside the pure stripper.
 *
 * @param {{ locked?: boolean, body?: string | null | undefined, wpm?: unknown }} input
 */
export function editorReadableStats(input) {
  if (input?.locked) return null;
  const body = input?.body ?? "";
  const wordCount = countReadableWords(body);
  return {
    wordCount,
    readingMinutes: readingMinutes(wordCount, input?.wpm),
  };
}

/**
 * Convert markdown source to the "readable words" a user sees in this note's
 * own body, without expanding transclusions or rendering HTML.
 *
 * @param {string} markdown
 */
export function toReadablePlainText(markdown) {
  if (!markdown) return "";

  const placeholders = [];
  const createPlaceholder = (value) => {
    const token = `${PLACEHOLDER_PREFIX}${placeholders.length}\u0000`;
    placeholders.push(value);
    return token;
  };

  let text = String(markdown);
  text = protectFencedCode(text, createPlaceholder);
  text = protectInlineCode(text, createPlaceholder);

  text = text.replace(/!\[\[([\s\S]*?)\]\]/g, (_, inner) =>
    wikiDisplayText(inner),
  );
  text = text.replace(/\[\[([\s\S]*?)\]\]/g, (_, inner) =>
    wikiDisplayText(inner),
  );
  text = text.replace(/!\[([^\]]*)\]\[[^\]]*\]/g, "$1");
  text = text.replace(/\[([^\]]+)\]\[[^\]]*\]/g, "$1");
  text = text.replace(/!\[([^\]]*)\]\((?:[^()\\]|\\.)*?\)/g, "$1");
  text = text.replace(/\[([^\]]*)\]\((?:[^()\\]|\\.)*?\)/g, "$1");
  text = text.replace(/^\s{0,3}\[[^\]]+\]:\s+\S.*$/gm, " ");
  text = text.replace(/<https?:\/\/[^>\s]+>/gi, " ");
  text = text.replace(/<!--[\s\S]*?-->/g, " ");
  text = text.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, " ");
  text = text.replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, " ");
  text = text.replace(/^ {0,3}(#{1,6})[ \t]+/gm, "");
  text = text.replace(/^ {0,3}>\s?/gm, "");
  text = text.replace(/^ {0,3}(?:[-+*]|\d+[.)])\s+(?:\[[ xX]\]\s+)?/gm, "");
  text = text.replace(/^ {0,3}\[[ xX]\]\s+/gm, "");
  text = text.replace(/^\s{0,3}(?:=+|-+)\s*$/gm, " ");
  text = text.replace(/(^|[^\*])\*{1,3}([^\s*][\s\S]*?[^\s*])\*{1,3}/g, "$1$2");
  text = text.replace(/(^|[^_])_{1,3}([^\s_][\s\S]*?[^\s_])_{1,3}/g, "$1$2");
  text = text.replace(/~~([\s\S]*?)~~/g, "$1");
  text = text.replace(/<[^>]+>/g, " ");

  text = restorePlaceholders(text, placeholders);
  text = text.replace(/[ \t]+\n/g, "\n");
  text = text.replace(/\n{2,}/g, "\n");
  text = text.replace(/[^\S\r\n]+/g, " ");
  text = text.replace(/\s+/g, " ").trim();
  return text;
}

/**
 * @param {string} source
 * @param {(value: string) => string} createPlaceholder
 */
function protectFencedCode(source, createPlaceholder) {
  const lines = source.split("\n");
  const out = [];

  for (let i = 0; i < lines.length; i++) {
    const match = lines[i].match(/^( {0,3})(`{3,}|~{3,})[^\n]*$/);
    if (!match) {
      out.push(lines[i]);
      continue;
    }

    const fence = match[2];
    const fenceChar = fence[0];
    const minLength = fence.length;
    const inner = [];
    let closedAt = -1;

    for (let j = i + 1; j < lines.length; j++) {
      const closeMatch = lines[j].match(/^( {0,3})(`{3,}|~{3,})\s*$/);
      if (
        closeMatch &&
        closeMatch[2][0] === fenceChar &&
        closeMatch[2].length >= minLength
      ) {
        closedAt = j;
        break;
      }
      inner.push(lines[j]);
    }

    if (closedAt === -1) {
      out.push(createPlaceholder(lines.slice(i + 1).join("\n")));
      break;
    }

    out.push(createPlaceholder(inner.join("\n")));
    i = closedAt;
  }

  return out.join("\n");
}

/**
 * @param {string} source
 * @param {(value: string) => string} createPlaceholder
 */
function protectInlineCode(source, createPlaceholder) {
  let out = "";

  for (let i = 0; i < source.length; i++) {
    if (source[i] !== "`") {
      out += source[i];
      continue;
    }

    let tickCount = 1;
    while (source[i + tickCount] === "`") tickCount++;
    const fence = "`".repeat(tickCount);
    const closeIndex = source.indexOf(fence, i + tickCount);
    if (closeIndex === -1) {
      out += fence;
      i += tickCount - 1;
      continue;
    }

    const inner = source.slice(i + tickCount, closeIndex);
    out += createPlaceholder(inner);
    i = closeIndex + tickCount - 1;
  }

  return out;
}

/**
 * @param {string} source
 * @param {string[]} placeholders
 */
function restorePlaceholders(source, placeholders) {
  return source.replace(
    /\u0000READABLECODE(\d+)\u0000/g,
    (_, index) => placeholders[Number(index)] ?? "",
  );
}

/**
 * @param {string} inner
 */
function wikiDisplayText(inner) {
  const text = String(inner).trim();
  if (!text) return " ";
  const pipe = text.indexOf("|");
  return pipe === -1 ? text : text.slice(pipe + 1).trim() || text.slice(0, pipe).trim();
}

/**
 * @param {string} text
 */
function countWordsInText(text) {
  if (typeof Intl !== "undefined" && typeof Intl.Segmenter === "function") {
    const segmenter = new Intl.Segmenter(undefined, { granularity: "word" });
    let count = 0;
    for (const segment of segmenter.segment(text)) {
      if (segment.isWordLike) count++;
    }
    return count;
  }

  const matches = text.match(/[\p{L}\p{N}]+(?:['’-][\p{L}\p{N}]+)*/gu);
  return matches ? matches.length : 0;
}
