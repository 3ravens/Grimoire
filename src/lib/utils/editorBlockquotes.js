import { isInsideFencedCode, parseListLine } from "./editorLists.js";

/**
 * Parse a single line as a Markdown blockquote (`>` / nested `> >`).
 *
 * @param {string} line
 * @returns {{
 *   indent: string,
 *   prefix: string,
 *   contentStart: number,
 *   content: string,
 *   isEmpty: boolean,
 * } | null}
 */
export function parseBlockquoteLine(line) {
  const raw = line.replace(/\r$/, "");
  const match = /^([ \t]*)((?:>[ \t]*)+)(.*)$/.exec(raw);
  if (!match) return null;

  const indent = match[1];
  const prefix = match[2];
  const content = match[3];
  const listParsed = parseListLine(content);
  const isEmpty = content.trimEnd() === "" || Boolean(listParsed?.isEmpty);

  return {
    indent,
    prefix,
    contentStart: indent.length + prefix.length,
    content,
    isEmpty,
  };
}

/**
 * Continue / exit / split a Markdown blockquote on Enter.
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @returns {{ value: string, selectionStart: number, selectionEnd: number } | null}
 */
export function applyBlockquoteEnter(value, start, end) {
  if (start !== end) return null;
  if (isInsideFencedCode(value, start)) return null;

  const ls = lineStart(value, start);
  const le = lineEnd(value, start);
  const line = value.slice(ls, le);
  const parsed = parseBlockquoteLine(line);
  if (!parsed) return null;

  const absoluteContentStart = ls + parsed.contentStart;
  if (start < absoluteContentStart) return null;

  if (parsed.isEmpty) {
    return exitOrUnquoteEmptyLine(value, ls, le, parsed);
  }

  const textBefore = value.slice(absoluteContentStart, start);
  const textAfter = value.slice(start, le);
  const insertion = `\n${parsed.indent}${parsed.prefix}${textAfter}`;
  const before =
    value.slice(0, ls) + parsed.indent + parsed.prefix + textBefore;
  const after = value.slice(le);
  const nextValue = before + insertion + after;
  const cursor = before.length + 1 + parsed.indent.length + parsed.prefix.length;

  return {
    value: nextValue,
    selectionStart: cursor,
    selectionEnd: cursor,
  };
}

/**
 * Drop the innermost `>` segment from a quote prefix, or null if only one level.
 *
 * @param {string} prefix
 * @returns {string | null}
 */
function dropOneQuoteSegment(prefix) {
  const segments = prefix.match(/>[ \t]*/g);
  if (!segments || segments.length <= 1) return null;
  return segments.slice(0, -1).join("");
}

/**
 * @param {string} value
 * @param {number} ls
 * @param {number} le
 * @param {NonNullable<ReturnType<typeof parseBlockquoteLine>>} parsed
 */
function exitOrUnquoteEmptyLine(value, ls, le, parsed) {
  const remaining = dropOneQuoteSegment(parsed.prefix);
  if (remaining !== null) {
    const nextLine = `${parsed.indent}${remaining}`;
    const nextValue = value.slice(0, ls) + nextLine + value.slice(le);
    const cursor = ls + nextLine.length;
    return {
      value: nextValue,
      selectionStart: cursor,
      selectionEnd: cursor,
    };
  }

  const nextLine = parsed.indent;
  const nextValue = value.slice(0, ls) + nextLine + value.slice(le);
  const cursor = ls + nextLine.length;
  return {
    value: nextValue,
    selectionStart: cursor,
    selectionEnd: cursor,
  };
}

/** @param {string} value @param {number} index */
function lineStart(value, index) {
  const i = value.lastIndexOf("\n", Math.max(0, index - 1));
  return i === -1 ? 0 : i + 1;
}

/** @param {string} value @param {number} index */
function lineEnd(value, index) {
  const i = value.indexOf("\n", index);
  return i === -1 ? value.length : i;
}
