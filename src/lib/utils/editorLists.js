import { EDITOR_INDENT } from "./editorIndent.js";

/**
 * Parse a single line as a Markdown list item (unordered / ordered / checklist).
 *
 * @param {string} line
 * @returns {{
 *   indent: string,
 *   kind: "unordered" | "ordered" | "checklist",
 *   marker: string,
 *   orderedDigits: string | null,
 *   checked: boolean | null,
 *   contentStart: number,
 *   content: string,
 *   isEmpty: boolean,
 * } | null}
 */
export function parseListLine(line) {
  // Tolerant of Windows CRLF leftovers on a single line slice.
  const raw = line.replace(/\r$/, "");
  const indentMatch = /^[ \t]*/.exec(raw);
  const indent = indentMatch ? indentMatch[0] : "";
  const afterIndent = raw.slice(indent.length);

  // Optional whitespace after "]" (GFM uses a space; tolerate "- [ ]task").
  const checklist = /^([-*+]) \[([ xX])\]\s*(.*)$/.exec(afterIndent);
  if (checklist) {
    const bullet = checklist[1];
    const checked = checklist[2] === "x" || checklist[2] === "X";
    const state = checked ? "x" : " ";
    // Continue marker always includes a trailing space after the checkbox.
    const marker = `${bullet} [${state}] `;
    const content = checklist[3] ?? "";
    // Prefix is everything before content in the source line.
    const prefixLen = afterIndent.length - content.length;
    return {
      indent,
      kind: "checklist",
      marker,
      orderedDigits: null,
      checked,
      contentStart: indent.length + prefixLen,
      content,
      isEmpty: content.trimEnd() === "",
    };
  }

  const unordered = /^([-*+]) (.*)$/.exec(afterIndent);
  if (unordered) {
    const marker = `${unordered[1]} `;
    const content = unordered[2];
    return {
      indent,
      kind: "unordered",
      marker,
      orderedDigits: null,
      checked: null,
      contentStart: indent.length + marker.length,
      content,
      isEmpty: content.trimEnd() === "",
    };
  }

  const ordered = /^(\d+)\. (.*)$/.exec(afterIndent);
  if (ordered) {
    const orderedDigits = ordered[1];
    const marker = `${orderedDigits}. `;
    const content = ordered[2];
    return {
      indent,
      kind: "ordered",
      marker,
      orderedDigits,
      checked: null,
      contentStart: indent.length + marker.length,
      content,
      isEmpty: content.trimEnd() === "",
    };
  }

  return null;
}

/**
 * Whether `index` lies inside a fenced code block (``` or ~~~).
 * Matches opener/closer rules used by readableText (char + min length).
 *
 * @param {string} value
 * @param {number} index
 */
export function isInsideFencedCode(value, index) {
  const lines = value.split("\n");
  let offset = 0;
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    const lineEndPos = offset + line.length;
    const spanEnd =
      i < lines.length - 1 ? lineEndPos + 1 : Math.max(lineEndPos, value.length);

    const openMatch = line.match(/^( {0,3})(`{3,}|~{3,})[^\n]*$/);
    if (!openMatch) {
      offset = spanEnd;
      i += 1;
      continue;
    }

    const fence = openMatch[2];
    const fenceChar = fence[0];
    const minLength = fence.length;
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
    }

    const contentStart = spanEnd;
    let contentEndExclusive;
    if (closedAt === -1) {
      contentEndExclusive = value.length;
    } else {
      let closeOffset = spanEnd;
      for (let k = i + 1; k < closedAt; k++) {
        closeOffset += lines[k].length + 1;
      }
      contentEndExclusive = closeOffset;
    }

    if (index >= contentStart && index < contentEndExclusive) {
      return true;
    }

    if (closedAt === -1) {
      return false;
    }

    let nextOffset = contentEndExclusive;
    nextOffset += lines[closedAt].length;
    if (closedAt < lines.length - 1) nextOffset += 1;
    offset = nextOffset;
    i = closedAt + 1;
  }

  return false;
}

/**
 * Continue / exit / split a Markdown list on Enter.
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @returns {{ value: string, selectionStart: number, selectionEnd: number } | null}
 */
export function applyListEnter(value, start, end) {
  if (start !== end) return null;
  if (isInsideFencedCode(value, start)) return null;

  const ls = lineStart(value, start);
  const le = lineEnd(value, start);
  const line = value.slice(ls, le);
  const parsed = parseListLine(line);
  if (!parsed) return null;

  const absoluteContentStart = ls + parsed.contentStart;
  if (start < absoluteContentStart) return null;

  if (parsed.isEmpty) {
    return exitOrOutdentEmptyListLine(value, ls, le, parsed);
  }

  const textBefore = value.slice(absoluteContentStart, start);
  const textAfter = value.slice(start, le);
  const nextMarker = nextListMarker(parsed);
  const insertion = `\n${parsed.indent}${nextMarker}${textAfter}`;
  const before = value.slice(0, ls) + parsed.indent + parsed.marker + textBefore;
  const after = value.slice(le);
  const nextValue = before + insertion + after;
  const cursor = before.length + 1 + parsed.indent.length + nextMarker.length;

  return {
    value: nextValue,
    selectionStart: cursor,
    selectionEnd: cursor,
  };
}

/**
 * Nest / unnest a list line on Tab / Shift+Tab (collapsed caret only).
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {{ shiftKey?: boolean }} [opts]
 * @returns {{ value: string, selectionStart: number, selectionEnd: number } | null}
 */
export function applyListTab(value, start, end, { shiftKey = false } = {}) {
  if (start !== end) return null;
  if (isInsideFencedCode(value, start)) return null;

  const ls = lineStart(value, start);
  const le = lineEnd(value, start);
  const line = value.slice(ls, le);
  const parsed = parseListLine(line);
  if (!parsed) return null;

  if (shiftKey) {
    const removed = leadingIndentLength(line);
    if (removed === 0) {
      return { value, selectionStart: start, selectionEnd: end };
    }
    const nextLine = line.slice(removed);
    const nextValue = value.slice(0, ls) + nextLine + value.slice(le);
    const cursor = Math.max(ls, start - removed);
    return {
      value: nextValue,
      selectionStart: cursor,
      selectionEnd: cursor,
    };
  }

  const nextValue =
    value.slice(0, ls) + EDITOR_INDENT + line + value.slice(le);
  const cursor = start + EDITOR_INDENT.length;
  return {
    value: nextValue,
    selectionStart: cursor,
    selectionEnd: cursor,
  };
}

/**
 * Toggle `[ ]` ↔ `[x]` on the checklist line containing the caret (or selection start).
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @returns {{ value: string, selectionStart: number, selectionEnd: number } | null}
 */
export function applyChecklistToggle(value, start, end) {
  const caret = Math.min(start, end);
  if (isInsideFencedCode(value, caret)) return null;

  const ls = lineStart(value, caret);
  const le = lineEnd(value, caret);
  const line = value.slice(ls, le);
  const parsed = parseListLine(line);
  if (!parsed || parsed.kind !== "checklist") return null;

  const next = toggleChecklistLine(line, parsed);
  const nextValue = value.slice(0, ls) + next + value.slice(le);
  // Keep a collapsed caret; prefer original offset clamped into the new line.
  const cursor = Math.min(Math.max(caret, ls), ls + next.length);
  return {
    value: nextValue,
    selectionStart: cursor,
    selectionEnd: cursor,
  };
}

/**
 * Line-start offsets of checklist lines in document order (skipping fences).
 *
 * @param {string} value
 * @returns {number[]}
 */
export function listChecklistLineStarts(value) {
  const starts = [];
  let offset = 0;
  const lines = value.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (
      !isInsideFencedCode(value, offset) &&
      parseListLine(line)?.kind === "checklist"
    ) {
      starts.push(offset);
    }
    offset += line.length + (i < lines.length - 1 ? 1 : 0);
  }
  return starts;
}

/**
 * Flip the `index`-th checklist line in the document (0-based).
 *
 * @param {string} value
 * @param {number} index
 * @returns {{ value: string } | null}
 */
export function toggleChecklistAtIndex(value, index) {
  const starts = listChecklistLineStarts(value);
  if (index < 0 || index >= starts.length) return null;

  const ls = starts[index];
  const le = lineEnd(value, ls);
  const line = value.slice(ls, le);
  const parsed = parseListLine(line);
  if (!parsed || parsed.kind !== "checklist") return null;

  const nextLine = toggleChecklistLine(line, parsed);
  return { value: value.slice(0, ls) + nextLine + value.slice(le) };
}

/**
 * Enable root GFM task checkboxes for click-to-toggle (remove disabled, add index).
 * Returns updated HTML and the next counter value.
 *
 * @param {string} html
 * @param {number} [startIndex=0]
 * @returns {{ html: string, nextIndex: number }}
 */
export function enableRootChecklistInputs(html, startIndex = 0) {
  let index = startIndex;
  // marked emits: <input disabled="" type="checkbox"> or with checked=""
  let nextHtml = html.replace(/<input\b([^>]*)>/gi, (full) => {
    if (!/\btype\s*=\s*["']?checkbox["']?/i.test(full)) {
      return full;
    }
    let cleaned = full
      .replace(/\sdisabled(?:\s*=\s*(?:""|''|disabled))?/gi, "")
      .replace(/\sdata-checklist-index\s*=\s*["']?\d+["']?/gi, "");
    const n = index;
    index += 1;
    cleaned = cleaned.replace(/\s*\/?\s*>$/, ` data-checklist-index="${n}">`);
    return cleaned;
  });
  return { html: nextHtml, nextIndex: index };
}

/**
 * Add task-list-item class to GFM checkbox list items (for styling).
 * Safe for embeds — does not enable inputs.
 *
 * @param {string} html
 */
export function annotateTaskListItems(html) {
  return html.replace(
    /<li>(\s*<input\b[^>]*\btype=["']?checkbox["']?[^>]*>)/gi,
    '<li class="task-list-item">$1',
  );
}

/**
 * marked only treats `- [x] text` as a task item when there is non-empty text
 * after the checkbox. Empty items (`- [x]` / `- [ ] `) render as literal `[x]`.
 * Append a zero-width space so Read mode still gets a real checkbox.
 *
 * @param {string} markdown
 */
export function normalizeChecklistMarkdownForRender(markdown) {
  return String(markdown ?? "")
    .split("\n")
    .map((line) => {
      const trimmedCr = line.replace(/\r$/, "");
      const m = /^([ \t]*)([-*+]) \[([ xX])\]\s*$/.exec(trimmedCr);
      if (!m) return line;
      const state = m[3] === "x" || m[3] === "X" ? "x" : " ";
      return `${m[1]}${m[2]} [${state}] \u200b`;
    })
    .join("\n");
}

/**
 * @param {string} line
 * @param {NonNullable<ReturnType<typeof parseListLine>>} parsed
 */
function toggleChecklistLine(line, parsed) {
  const bullet = parsed.marker[0];
  const nextChecked = !parsed.checked;
  const state = nextChecked ? "x" : " ";
  const newMarker = `${bullet} [${state}] `;
  return `${parsed.indent}${newMarker}${parsed.content}`;
}

/**
 * @param {NonNullable<ReturnType<typeof parseListLine>>} parsed
 */
function nextListMarker(parsed) {
  if (parsed.kind === "unordered") return parsed.marker;
  if (parsed.kind === "checklist") return parsed.marker;
  const next = BigInt(parsed.orderedDigits) + 1n;
  return `${next.toString()}. `;
}

/**
 * @param {string} value
 * @param {number} ls
 * @param {number} le
 * @param {NonNullable<ReturnType<typeof parseListLine>>} parsed
 */
function exitOrOutdentEmptyListLine(value, ls, le, parsed) {
  if (parsed.indent.startsWith(EDITOR_INDENT)) {
    const newIndent = parsed.indent.slice(EDITOR_INDENT.length);
    const nextLine = `${newIndent}${parsed.marker}`;
    const nextValue = value.slice(0, ls) + nextLine + value.slice(le);
    const cursor = ls + nextLine.length;
    return {
      value: nextValue,
      selectionStart: cursor,
      selectionEnd: cursor,
    };
  }

  const nextValue = value.slice(0, ls) + value.slice(le);
  const cursor = ls;
  return {
    value: nextValue,
    selectionStart: cursor,
    selectionEnd: cursor,
  };
}

/** @param {string} line */
function leadingIndentLength(line) {
  if (line.startsWith(EDITOR_INDENT)) return EDITOR_INDENT.length;
  if (line.startsWith("\t")) return 1;
  const match = /^( {1,3})/.exec(line);
  return match ? match[1].length : 0;
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
