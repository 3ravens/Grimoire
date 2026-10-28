import { describe, it, expect } from 'vitest';
import { sortNotesWithFolderPins } from './sortNotesWithFolderPins.js';

const notes = [
  { id: 1, title: 'Charlie', created_at: 10, updated_at: 30 },
  { id: 2, title: 'Alpha', created_at: 20, updated_at: 10 },
  { id: 3, title: 'Bravo', created_at: 30, updated_at: 20 },
];

describe('sortNotesWithFolderPins', () => {
  it('puts pins first in grip order under every sort mode', () => {
    for (const mode of ['modified', 'created', 'name']) {
      const sorted = sortNotesWithFolderPins(notes, mode, [3, 1]);
      expect(sorted.map((n) => n.id).slice(0, 2)).toEqual([3, 1]);
      expect(sorted[2].id).toBe(2);
    }
  });

  it('sorts unpinned by modified', () => {
    const sorted = sortNotesWithFolderPins(notes, 'modified', [2]);
    expect(sorted.map((n) => n.id)).toEqual([2, 1, 3]);
  });

  it('sorts unpinned by created', () => {
    const sorted = sortNotesWithFolderPins(notes, 'created', []);
    expect(sorted.map((n) => n.id)).toEqual([3, 2, 1]);
  });

  it('sorts unpinned by name', () => {
    const sorted = sortNotesWithFolderPins(notes, 'name', []);
    expect(sorted.map((n) => n.id)).toEqual([2, 3, 1]);
  });

  it('skips unknown pin ids', () => {
    const sorted = sortNotesWithFolderPins(notes, 'name', [99, 2]);
    expect(sorted.map((n) => n.id)).toEqual([2, 3, 1]);
  });

  it('handles empty notes', () => {
    expect(sortNotesWithFolderPins([], 'modified', [1])).toEqual([]);
  });
});
