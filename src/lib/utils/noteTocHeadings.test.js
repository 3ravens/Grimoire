// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import {
  annotateReadModeTocTargets,
  extractNoteTocHeadings,
  focusTextareaAtOffset,
  normalizeTocHeadingText,
} from './noteTocHeadings.js';

describe('extractNoteTocHeadings', () => {
  it('extracts ATX H1–H4 with ids and offsets', () => {
    const md = '# One\n\n## Two\n### Three\n#### Four';
    const got = extractNoteTocHeadings(md);
    expect(got.map((h) => [h.level, h.text, h.id])).toEqual([
      [1, 'One', 'note-h-0'],
      [2, 'Two', 'note-h-1'],
      [3, 'Three', 'note-h-2'],
      [4, 'Four', 'note-h-3'],
    ]);
    expect(got[0].lineIndex).toBe(0);
    expect(got[0].charOffset).toBe(0);
    expect(got[1].lineIndex).toBe(2);
  });

  it('skips H5+, empty titles, and setext', () => {
    const md = '##### Five\n#\n##\nTitle\n=====\nOther\n-----';
    expect(extractNoteTocHeadings(md)).toEqual([]);
  });

  it('ignores headings inside fenced code', () => {
    const md = '```\n# Secret\n```\n## Real\n~~~\n## Also secret\n~~~';
    expect(extractNoteTocHeadings(md).map((h) => h.text)).toEqual(['Real']);
  });

  it('allows 0–3 space indent and strips closing hashes', () => {
    const md = '   ## Indented ##\n#### Deep ###';
    const got = extractNoteTocHeadings(md);
    expect(got.map((h) => [h.level, h.text])).toEqual([
      [2, 'Indented'],
      [4, 'Deep'],
    ]);
  });

  it('keeps duplicate titles as separate occurrences', () => {
    const md = '## Same\n## Same';
    const got = extractNoteTocHeadings(md);
    expect(got.map((h) => h.id)).toEqual(['note-h-0', 'note-h-1']);
    expect(got[0].text).toBe('Same');
    expect(got[1].text).toBe('Same');
  });

  it('handles CRLF line endings with LF offsets', () => {
    const md = '# A\r\n\r\n## B\r\n';
    const got = extractNoteTocHeadings(md);
    expect(got.map((h) => h.text)).toEqual(['A', 'B']);
    expect(got[0].charOffset).toBe(0);
    expect(got[1].charOffset).toBe(5);
  });
});

describe('normalizeTocHeadingText', () => {
  it('strips common markdown chrome', () => {
    expect(normalizeTocHeadingText('**Bold** title')).toBe('Bold title');
    expect(normalizeTocHeadingText('A   B')).toBe('A B');
  });
});

describe('annotateReadModeTocTargets', () => {
  it('tags matching root headings and skips embeds', () => {
    const root = document.createElement('div');
    root.innerHTML = `
      <h2>Alpha</h2>
      <div class="note-embed"><div class="note-embed-inner"><h2>Embedded</h2></div></div>
      <h2>Beta</h2>
      <h1>Setext-looking</h1>
      <h2>Gamma</h2>
    `;
    const entries = [
      { id: 'note-h-0', text: 'Alpha', level: 2 },
      { id: 'note-h-1', text: 'Beta', level: 2 },
      { id: 'note-h-2', text: 'Gamma', level: 2 },
    ];
    annotateReadModeTocTargets(root, entries);
    expect(root.querySelector('[data-note-toc-id="note-h-0"]')?.textContent).toBe('Alpha');
    expect(root.querySelector('[data-note-toc-id="note-h-1"]')?.textContent).toBe('Beta');
    expect(root.querySelector('[data-note-toc-id="note-h-2"]')?.textContent).toBe('Gamma');
    expect(root.querySelector('.note-embed [data-note-toc-id]')).toBeNull();
    expect(root.querySelector('h1')?.hasAttribute('data-note-toc-id')).toBe(false);
  });

  it('maps bold source headings to rendered text', () => {
    const root = document.createElement('div');
    root.innerHTML = '<h2>Bold title</h2>';
    const entries = extractNoteTocHeadings('## **Bold** title');
    annotateReadModeTocTargets(root, entries);
    expect(root.querySelector('[data-note-toc-id="note-h-0"]')?.textContent).toBe(
      'Bold title',
    );
  });
});

describe('focusTextareaAtOffset', () => {
  it('clamps and selects the caret', () => {
    const el = document.createElement('textarea');
    el.value = 'abc\ndef';
    document.body.appendChild(el);
    focusTextareaAtOffset(el, 4);
    expect(el.selectionStart).toBe(4);
    expect(el.selectionEnd).toBe(4);
    focusTextareaAtOffset(el, 999);
    expect(el.selectionStart).toBe(el.value.length);
    el.remove();
  });
});
