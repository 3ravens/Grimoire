import { describe, expect, it, vi } from 'vitest';
import {
  copyNoteMarkdown,
  copyNotePlainText,
  effectiveCopyLockFlag,
  isNoteBodyReadable,
} from './noteCopyActions.js';

describe('isNoteBodyReadable', () => {
  it('is fail-closed', () => {
    expect(isNoteBodyReadable(false)).toBe(true);
    expect(isNoteBodyReadable(true)).toBe(false);
    expect(isNoteBodyReadable(undefined)).toBe(false);
    expect(isNoteBodyReadable(null)).toBe(false);
  });
});

describe('effectiveCopyLockFlag', () => {
  it('prefers the list row over a stale unlocked active note', () => {
    expect(effectiveCopyLockFlag({ locked: true }, { locked: false })).toBe(true);
    expect(effectiveCopyLockFlag({ locked: false }, { locked: false })).toBe(false);
    expect(effectiveCopyLockFlag(null, { locked: false })).toBe(false);
    expect(effectiveCopyLockFlag(null, { locked: true })).toBe(true);
    expect(effectiveCopyLockFlag(undefined, undefined)).toBe(true);
  });
});

describe('copyNoteMarkdown', () => {
  it('writes raw body when unlocked', async () => {
    const writeText = vi.fn(async () => {});
    const ok = await copyNoteMarkdown({
      locked: false,
      body: '# Hello **world**',
      writeText,
    });
    expect(ok).toBe(true);
    expect(writeText).toHaveBeenCalledWith('# Hello **world**');
  });

  it('refuses locked notes and does not write', async () => {
    const writeText = vi.fn(async () => {});
    const onError = vi.fn();
    const ok = await copyNoteMarkdown({
      locked: true,
      body: 'secret ciphertext-looking body',
      writeText,
      onError,
    });
    expect(ok).toBe(false);
    expect(writeText).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalled();
  });

  it('refuses when lock flag is unknown', async () => {
    const writeText = vi.fn(async () => {});
    const ok = await copyNoteMarkdown({
      locked: undefined,
      body: 'maybe secret',
      writeText,
    });
    expect(ok).toBe(false);
    expect(writeText).not.toHaveBeenCalled();
  });
});

describe('copyNotePlainText', () => {
  it('writes stripped readable text when unlocked', async () => {
    const writeText = vi.fn(async () => {});
    const ok = await copyNotePlainText({
      locked: false,
      body: '# Hello **world**',
      writeText,
    });
    expect(ok).toBe(true);
    expect(writeText).toHaveBeenCalledWith('Hello world');
  });

  it('refuses locked notes and does not write', async () => {
    const writeText = vi.fn(async () => {});
    const onError = vi.fn();
    const ok = await copyNotePlainText({
      locked: true,
      body: '# Secret',
      writeText,
      onError,
    });
    expect(ok).toBe(false);
    expect(writeText).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalled();
  });

  it('reports clipboard failures', async () => {
    const writeText = vi.fn(async () => {
      throw new Error('denied');
    });
    const onError = vi.fn();
    const ok = await copyNotePlainText({
      locked: false,
      body: 'ok',
      writeText,
      onError,
    });
    expect(ok).toBe(false);
    expect(onError).toHaveBeenCalled();
  });
});
