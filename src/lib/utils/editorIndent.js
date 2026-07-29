/**
 * Soft-tab width for the note editor.
 * Matches the common desktop-editor default (VS Code, Sublime, JetBrains).
 */
export const EDITOR_INDENT = "    ";

/**
 * Apply Tab / Shift+Tab indent behavior to textarea content.
 *
 * - Tab with a caret or single-line selection: insert a soft tab (replacing selection).
 * - Tab with a multi-line selection: indent every line that overlaps the selection.
 * - Shift+Tab: remove one indent level from the current line / each selected line.
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {{ shiftKey?: boolean }} [opts]
 * @returns {{ value: string, selectionStart: number, selectionEnd: number }}
 */
export function applyEditorTab(value, start, end, { shiftKey = false } = {}) {
  const from = Math.max(0, Math.min(start, end));
  const to = Math.max(0, Math.max(start, end));

  if (shiftKey) {
    return outdentLines(value, from, to);
  }

  if (from !== to && value.slice(from, to).includes("\n")) {
    return indentLines(value, from, to);
  }

  const next = value.slice(0, from) + EDITOR_INDENT + value.slice(to);
  const cursor = from + EDITOR_INDENT.length;
  return { value: next, selectionStart: cursor, selectionEnd: cursor };
}

/**
 * @param {string} value
 * @param {number} start
 * @param {number} end
 */
function indentLines(value, start, end) {
  const blockStart = lineStart(value, start);
  const blockEnd = lineEnd(value, end);
  const block = value.slice(blockStart, blockEnd);
  const lines = block.split("\n");
  const indented = lines.map((line) => EDITOR_INDENT + line).join("\n");

  return {
    value: value.slice(0, blockStart) + indented + value.slice(blockEnd),
    selectionStart: start + EDITOR_INDENT.length,
    selectionEnd: end + EDITOR_INDENT.length * lines.length,
  };
}

/**
 * @param {string} value
 * @param {number} start
 * @param {number} end
 */
function outdentLines(value, start, end) {
  const blockStart = lineStart(value, start);
  const blockEnd = lineEnd(value, end);
  const block = value.slice(blockStart, blockEnd);
  const lines = block.split("\n");

  let selStart = start;
  let selEnd = end;
  let pos = blockStart;
  const outdented = [];

  for (const line of lines) {
    const removed = leadingIndentLength(line);
    if (start > pos) {
      selStart -= Math.min(removed, start - pos);
    }
    if (end > pos) {
      selEnd -= Math.min(removed, end - pos);
    }
    outdented.push(line.slice(removed));
    pos += line.length + 1;
  }

  return {
    value:
      value.slice(0, blockStart) + outdented.join("\n") + value.slice(blockEnd),
    selectionStart: Math.max(blockStart, selStart),
    selectionEnd: Math.max(blockStart, selEnd),
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
