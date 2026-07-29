// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import {
  sanitizeNoteHtml,
  sanitizeSearchSnippet,
  sanitizeWikipediaHtml,
} from './sanitize.js';

describe('sanitizeNoteHtml', () => {
  it('strips script tags and event handlers', () => {
    const dirty = '<p>ok</p><img src=x onerror=alert(1)><script>alert(1)</script>';
    const clean = sanitizeNoteHtml(dirty);
    expect(clean).not.toContain('<script');
    expect(clean).not.toContain('onerror');
    expect(clean).toContain('ok');
  });
});

describe('sanitizeWikipediaHtml', () => {
  it('preserves wiki navigation attributes', () => {
    const dirty =
      '<a href="#" data-wiki-path="Photosynthesis" title="Go">Link</a><script>x</script>';
    const clean = sanitizeWikipediaHtml(dirty);
    expect(clean).toContain('data-wiki-path="Photosynthesis"');
    expect(clean).not.toContain('<script');
  });
});

describe('sanitizeSearchSnippet', () => {
  it('allows highlight tags only', () => {
    const dirty = 'before <b>match</b> <img onerror=x><script>y</script>';
    const clean = sanitizeSearchSnippet(dirty);
    expect(clean).toContain('<b>match</b>');
    expect(clean).not.toContain('<img');
    expect(clean).not.toContain('<script');
  });
});
