import { describe, it, expect, vi } from 'vitest';

vi.mock('./transclusion.js', () => ({
  renderTransclusionMarkdownToHtml: vi.fn(async (md) => `<p>${md}</p>`),
}));

import { buildStandaloneReadModeHtml } from './noteExportHtml.js';

describe('buildStandaloneReadModeHtml', () => {
  it('wraps rendered markdown in a full HTML document', async () => {
    const html = await buildStandaloneReadModeHtml('My Title', '# Hello', { rootNoteId: 5 });
    expect(html).toContain('<!DOCTYPE html>');
    expect(html).toContain('<title>My Title</title>');
    expect(html).toContain('<p># Hello</p>');
    expect(html).toContain('read-mode-content');
  });

  it('escapes title in document title element', async () => {
    const html = await buildStandaloneReadModeHtml('<x>', 'body', {});
    expect(html).toContain('&lt;x&gt;');
    expect(html).not.toContain('<title><x></title>');
  });
});
