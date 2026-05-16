import { describe, it, expect } from 'vitest';
import { computeDiff, coalesceSingleLineChangeHunks, applyAcceptedHunks } from './diff.js';

describe('computeDiff', () => {
  it('returns unchanged hunk for identical strings', () => {
    const hunks = computeDiff('same', 'same');
    expect(hunks).toHaveLength(1);
    expect(hunks[0].type).toBe('unchanged');
  });

  it('reports added lines', () => {
    const hunks = computeDiff('line1', 'line1\nline2');
    expect(hunks.some((h) => h.type === 'add')).toBe(true);
  });

  it('reports removed lines', () => {
    const hunks = computeDiff('a\nb', 'a');
    expect(hunks.some((h) => h.type === 'remove')).toBe(true);
  });
});

describe('coalesceSingleLineChangeHunks', () => {
  it('merges adjacent single-line remove and add into modified', () => {
    const hunks = [
      { type: 'remove', lines: ['old line'] },
      { type: 'add', lines: ['new line'] },
    ];
    const out = coalesceSingleLineChangeHunks(hunks);
    expect(out).toHaveLength(1);
    expect(out[0].type).toBe('modified');
    expect(out[0].oldLine).toBe('old line');
    expect(out[0].newLine).toBe('new line');
  });
});

describe('applyAcceptedHunks', () => {
  it('applies accepted add hunk', () => {
    const hunks = computeDiff('x', 'x\ny');
    const accepted = new Set();
    hunks.forEach((h, i) => {
      if (h.type === 'add') accepted.add(i);
    });
    const merged = applyAcceptedHunks(hunks, accepted, 'x', 'x\ny');
    expect(merged).toContain('y');
  });
});
