import { isInsideFencedCode } from "./editorLists.js";

/**
 * Wrap or unwrap a selection with Markdown emphasis / inline-code markers.
 *
 * Unwraps when markers sit immediately outside the selection, or when the
 * selection itself starts and ends with the markers.
 *
 * Empty selection inserts `marker|marker` with the caret between.
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {string} marker  e.g. "**", "*", "`"
 * @returns {{ value: string, selectionStart: number, selectionEnd: number }}
 */
export function applyMarkdownWrap(value, start, end, marker) {
  const from = Math.max(0, Math.min(start, end));
  const to = Math.max(0, Math.max(start, end));
  const mLen = marker.length;

  // Markers immediately outside the selection.
  if (
    from >= mLen &&
    value.slice(from - mLen, from) === marker &&
    value.slice(to, to + mLen) === marker
  ) {
    const inner = value.slice(from, to);
    return {
      value: value.slice(0, from - mLen) + inner + value.slice(to + mLen),
      selectionStart: from - mLen,
      selectionEnd: from - mLen + inner.length,
    };
  }

  // Selection includes the wrapping markers.
  if (
    to - from >= mLen * 2 &&
    value.slice(from, from + mLen) === marker &&
    value.slice(to - mLen, to) === marker
  ) {
    const inner = value.slice(from + mLen, to - mLen);
    return {
      value: value.slice(0, from) + inner + value.slice(to),
      selectionStart: from,
      selectionEnd: from + inner.length,
    };
  }

  const selected = value.slice(from, to);
  const next = value.slice(0, from) + marker + selected + marker + value.slice(to);
  if (from === to) {
    const cursor = from + mLen;
    return { value: next, selectionStart: cursor, selectionEnd: cursor };
  }
  return {
    value: next,
    selectionStart: from + mLen,
    selectionEnd: from + mLen + selected.length,
  };
}

/**
 * Auto-insert a closing pair for caret-only typing of `*`, `_`, `` ` ``, `(`.
 * Returns null when the key should fall through to default insertion.
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {string} openChar
 * @returns {{ value: string, selectionStart: number, selectionEnd: number } | null}
 */
export function applyAutoPair(value, start, end, openChar) {
  if (start !== end) return null;
  if (isInsideFencedCode(value, start)) return null;

  const closers = { "*": "*", _: "_", "`": "`", "(": ")" };
  const closeChar = closers[openChar];
  if (closeChar == null) return null;

  const inInline = isInsideInlineCode(value, start);

  // Inside inline code: never pair non-backtick openers; for backtick, skip
  // (would close / extend the span awkwardly).
  if (inInline) return null;

  // Type-through: next char is already the closer.
  if (value[start] === closeChar) {
    return null;
  }

  const next =
    value.slice(0, start) + openChar + closeChar + value.slice(start);
  const cursor = start + 1;
  return {
    value: next,
    selectionStart: cursor,
    selectionEnd: cursor,
  };
}

/**
 * Cheap detection: odd number of unescaped backticks on the same line before index.
 *
 * @param {string} value
 * @param {number} index
 */
export function isInsideInlineCode(value, index) {
  const ls = lineStart(value, index);
  const before = value.slice(ls, index);
  let count = 0;
  for (let i = 0; i < before.length; i++) {
    if (before[i] === "`" && before[i - 1] !== "\\") count += 1;
  }
  return count % 2 === 1;
}

/** @param {string} value @param {number} index */
function lineStart(value, index) {
  const i = value.lastIndexOf("\n", Math.max(0, index - 1));
  return i === -1 ? 0 : i + 1;
}
