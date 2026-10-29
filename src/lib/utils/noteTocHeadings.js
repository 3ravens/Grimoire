import { isInsideFencedCode } from './editorLists.js';

/**
 * Normalize line endings the same way a browser textarea does (CRLF/CR → LF).
 * @param {string} markdown
 */
export function normalizeNoteSourceNewlines(markdown) {
  return String(markdown ?? '').replace(/\r\n/g, '\n').replace(/\r/g, '\n');
}

/**
 * Compare source heading chrome to rendered textContent.
 * @param {string} text
 */
export function normalizeTocHeadingText(text) {
  return String(text ?? '')
    .replace(/!\[([^\]]*)\]\((?:[^()\\]|\\.)*?\)/g, '$1')
    .replace(/\[([^\]]+)\]\((?:[^()\\]|\\.)*?\)/g, '$1')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/__([^_]+)__/g, '$1')
    .replace(/(?<!\*)\*([^*]+)\*(?!\*)/g, '$1')
    .replace(/(?<!_)_([^_]+)_(?!_)/g, '$1')
    .replace(/~~([^~]+)~~/g, '$1')
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * ATX headings (H1–H4) from note Markdown source for the note table of contents.
 * Skips fenced code, empty titles, and H5+. Does not parse setext.
 *
 * @param {string} markdown
 * @returns {Array<{ id: string, text: string, level: number, lineIndex: number, charOffset: number }>}
 */
export function extractNoteTocHeadings(markdown) {
  const source = normalizeNoteSourceNewlines(markdown);
  if (!source) return [];

  const lines = source.split('\n');
  /** @type {Array<{ id: string, text: string, level: number, lineIndex: number, charOffset: number }>} */
  const out = [];
  let offset = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const lineStart = offset;
    offset += line.length + (i < lines.length - 1 ? 1 : 0);

    if (isInsideFencedCode(source, lineStart)) continue;

    // 0–3 spaces, 1–4 hashes not followed by another hash, then EOL or whitespace + title.
    const match = /^( {0,3})(#{1,4})(?!#)(?:[ \t]+(.*))?$/.exec(line);
    if (!match) continue;

    const level = match[2].length;
    let text = (match[3] ?? '').trim();
    text = text.replace(/[ \t]+#+$/, '').trim();
    if (!text) continue;

    const id = `note-h-${out.length}`;
    out.push({ id, text, level, lineIndex: i, charOffset: lineStart });
  }

  return out;
}

/**
 * Map extracted ToC entries onto rendered Read-mode headings.
 * Skips headings inside `.note-embed` and unmatched extras (e.g. setext).
 *
 * @param {ParentNode | null | undefined} root
 * @param {Array<{ id: string, text: string, level: number }>} entries
 */
export function annotateReadModeTocTargets(root, entries) {
  if (!root) return;
  for (const el of root.querySelectorAll('[data-note-toc-id]')) {
    if (el instanceof HTMLElement) el.removeAttribute('data-note-toc-id');
  }
  if (!entries?.length) return;

  const nodes = root.querySelectorAll('h1, h2, h3, h4');
  let si = 0;
  for (const el of nodes) {
    if (!(el instanceof HTMLElement)) continue;
    if (el.closest('.note-embed')) continue;
    if (si >= entries.length) continue;
    const want = entries[si];
    const level = Number(el.tagName[1]);
    const text = normalizeTocHeadingText(el.textContent ?? '');
    if (level === want.level && text === normalizeTocHeadingText(want.text)) {
      el.setAttribute('data-note-toc-id', want.id);
      si += 1;
    }
  }
}

/**
 * Focus a textarea at a character offset so the browser scrolls the caret into view.
 * Offset must be computed against LF-normalized source (see extractNoteTocHeadings).
 * @param {HTMLTextAreaElement | null | undefined} textarea
 * @param {number} charOffset
 */
export function focusTextareaAtOffset(textarea, charOffset) {
  if (!textarea) return;
  const max = textarea.value.length;
  const pos = Math.max(0, Math.min(charOffset, max));
  textarea.focus();
  textarea.setSelectionRange(pos, pos);
}
