import { describe, it, expect } from 'vitest';
import {
  buildFolderTree,
  isFolderDescendantOrSelf,
  folderSubtreeIds,
} from './folderTree.js';

describe('buildFolderTree', () => {
  it('nests children under parents', () => {
    const flat = [
      { id: 1, parent_id: null, name: 'root' },
      { id: 2, parent_id: 1, name: 'child' },
    ];
    const tree = buildFolderTree(flat, null);
    expect(tree).toHaveLength(1);
    expect(tree[0].folder.id).toBe(1);
    expect(tree[0].children).toHaveLength(1);
    expect(tree[0].children[0].folder.id).toBe(2);
  });
});

describe('isFolderDescendantOrSelf', () => {
  it('returns true for self', () => {
    const folders = [{ id: 1, parent_id: null }];
    expect(isFolderDescendantOrSelf(folders, 1, 1)).toBe(true);
  });

  it('detects descendant chain', () => {
    const folders = [
      { id: 1, parent_id: null },
      { id: 2, parent_id: 1 },
      { id: 3, parent_id: 2 },
    ];
    expect(isFolderDescendantOrSelf(folders, 3, 1)).toBe(true);
    expect(isFolderDescendantOrSelf(folders, 1, 3)).toBe(false);
  });
});

describe('folderSubtreeIds', () => {
  it('includes root and descendants', () => {
    const folders = [
      { id: 1, parent_id: null },
      { id: 2, parent_id: 1 },
    ];
    const ids = folderSubtreeIds(folders, 1);
    expect(ids.has(1)).toBe(true);
    expect(ids.has(2)).toBe(true);
    expect(ids.size).toBe(2);
  });
});
