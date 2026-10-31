/**
 * If the clipboard holds a single URL and there is a non-empty selection,
 * replace the selection with `[label](url)`. Otherwise return null.
 *
 * Local string rewrite only — no network fetch or URL resolution.
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {string} clipboardText
 * @returns {{ value: string, selectionStart: number, selectionEnd: number } | null}
 */
export function trySmartPasteMarkdownLink(value, start, end, clipboardText) {
  const from = Math.max(0, Math.min(start, end));
  const to = Math.max(0, Math.max(start, end));
  if (from === to) return null;

  const url = String(clipboardText ?? "").trim();
  if (!isSingleUrl(url)) return null;

  const label = escapeLinkLabel(value.slice(from, to));
  const insertion = `[${label}](${url})`;
  const next = value.slice(0, from) + insertion + value.slice(to);
  const cursor = from + insertion.length;
  return {
    value: next,
    selectionStart: cursor,
    selectionEnd: cursor,
  };
}

/**
 * @param {string} text
 */
export function isSingleUrl(text) {
  if (!text || /\s/.test(text)) return false;
  // Scheme-based (http, https, mailto, file, custom URI schemes).
  if (/^[a-z][a-z0-9+.-]*:/i.test(text)) return true;
  // Bare www. host.
  if (/^www\.[^\s/]+/i.test(text)) return true;
  return false;
}

/** @param {string} label */
function escapeLinkLabel(label) {
  return label.replace(/\\/g, "\\\\").replace(/]/g, "\\]");
}
