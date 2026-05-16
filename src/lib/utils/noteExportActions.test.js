import { describe, it, expect } from 'vitest';
import { resolveExportPayload, sanitiseExportBasename } from './noteExportActions.js';

describe('sanitiseExportBasename', () => {
  it('strips illegal filename characters', () => {
    expect(sanitiseExportBasename('a/b:c?')).toBe('a-b-c-');
  });

  it('uses default when title empty', () => {
    expect(sanitiseExportBasename('   ')).toBe('note');
    expect(sanitiseExportBasename(null)).toBe('note');
  });
});

describe('resolveExportPayload', () => {
  it('uses editor buffer when note is active', () => {
    const ns = { activeNote: { id: 1 }, editorTitle: 'E', editorContent: 'Body' };
    const note = { id: 1, title: 'List', content: 'Old' };
    expect(resolveExportPayload(ns, note)).toEqual({ title: 'E', body: 'Body' });
  });

  it('falls back to list snapshot when not active', () => {
    const ns = { activeNote: { id: 2 }, editorTitle: 'E', editorContent: 'Body' };
    const note = { id: 1, title: 'List', content: 'Snap' };
    expect(resolveExportPayload(ns, note)).toEqual({ title: 'List', body: 'Snap' });
  });
});
