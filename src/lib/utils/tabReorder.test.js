import { describe, it, expect } from 'vitest';
import { TAB_MIME, isTabDrag, reorderTabs } from './tabReorder.js';

const tabs = (ids) => ids.map((id) => ({ id }));

describe('isTabDrag', () => {
  it('detects the private MIME', () => {
    expect(isTabDrag({ types: [TAB_MIME, 'text/plain'] })).toBe(true);
    expect(isTabDrag({ types: ['text/plain'] })).toBe(false);
    expect(isTabDrag(null)).toBe(false);
  });
});

describe('reorderTabs', () => {
  it('returns same reference for missing or identical ids', () => {
    const list = tabs(['a', 'b', 'c']);
    expect(reorderTabs(list, 'a', 'a', false)).toBe(list);
    expect(reorderTabs(list, 'x', 'b', false)).toBe(list);
    expect(reorderTabs(list, 'a', 'x', true)).toBe(list);
  });

  it('moves leftward before / after a target', () => {
    const list = tabs(['a', 'b', 'c', 'd']);
    expect(reorderTabs(list, 'd', 'b', false).map((t) => t.id)).toEqual([
      'a',
      'd',
      'b',
      'c',
    ]);
    expect(reorderTabs(list, 'd', 'b', true).map((t) => t.id)).toEqual([
      'a',
      'b',
      'd',
      'c',
    ]);
  });

  it('moves rightward before / after a target', () => {
    const list = tabs(['a', 'b', 'c', 'd']);
    expect(reorderTabs(list, 'a', 'c', false).map((t) => t.id)).toEqual([
      'b',
      'a',
      'c',
      'd',
    ]);
    expect(reorderTabs(list, 'a', 'c', true).map((t) => t.id)).toEqual([
      'b',
      'c',
      'a',
      'd',
    ]);
  });

  it('is a no-op for adjacent slots that do not change order', () => {
    const list = tabs(['a', 'b', 'c']);
    expect(reorderTabs(list, 'a', 'b', false)).toBe(list);
    expect(reorderTabs(list, 'b', 'a', true)).toBe(list);
  });
});
