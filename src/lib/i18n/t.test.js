import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { t, tp, tParts } from './t.js';
import { formatAppError } from './formatAppError.js';

describe('t', () => {
  it('resolves dotted keys', () => {
    expect(t('common.cancel')).toBe('Cancel');
  });

  it('interpolates params', () => {
    expect(t('errors.notFound', { msg: 'Note 3' })).toBe('Not found: Note 3');
  });

  it('returns the key and warns in DEV when missing', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const prev = import.meta.env.DEV;
    import.meta.env.DEV = true;
    expect(t('does.not.exist')).toBe('does.not.exist');
    expect(warn).toHaveBeenCalled();
    import.meta.env.DEV = prev;
    warn.mockRestore();
  });
});

describe('tp', () => {
  it('selects one/other for English', () => {
    expect(tp('editor.wordCountLabel', 1, { readingTime: 1 })).toMatch(/1 word/);
    expect(tp('editor.wordCountLabel', 5, { readingTime: 2 })).toMatch(/5 words/);
  });
});

describe('tParts', () => {
  it('splits slot markers for rich text', () => {
    const parts = tParts('sidebar.noFoldersYet');
    expect(parts.some((p) => p.type === 'slot' && p.name === 'newFolder')).toBe(true);
    expect(parts.some((p) => p.type === 'text')).toBe(true);
  });

  it('fills slot values from params when provided', () => {
    const parts = tParts('sidebar.noFoldersYet', { newFolder: 'New folder' });
    const slot = parts.find((p) => p.type === 'slot' && p.name === 'newFolder');
    expect(slot && 'value' in slot ? slot.value : null).toBe('New folder');
  });
});

describe('formatAppError', () => {
  beforeEach(() => {});
  afterEach(() => {});

  it('wraps known kinds', () => {
    expect(formatAppError({ kind: 'NotFound', message: 'x' })).toBe('Not found: x');
    expect(formatAppError({ kind: 'Auth', message: 'y' })).toContain('Authentication error');
    expect(formatAppError({ kind: 'OllamaUnavailable', message: 'down' })).toContain('ollama serve');
    expect(formatAppError({ kind: 'EmbeddingFailed', message: 'emb' })).toContain('embedding model');
  });

  it('passes through other kinds and unknowns', () => {
    expect(formatAppError({ kind: 'Internal', message: 'boom' })).toBe('boom');
    expect(formatAppError('plain')).toBe('plain');
  });
});
