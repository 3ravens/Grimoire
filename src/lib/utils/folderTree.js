// Pure utility functions for folder tree structure — no reactivity, no side effects.

/**
 * Builds a nested tree from a flat array of folders.
 * @param {any[]} flatFolders
 * @param {number|null} parentId
 * @returns {{ folder: any, children: any[] }[]}
 */
export function buildFolderTree(flatFolders, parentId = null) {
  return flatFolders
    .filter(f => (f.parent_id ?? null) === parentId)
    .map(f => ({ folder: f, children: buildFolderTree(flatFolders, f.id) }));
}

/**
 * Returns true if `targetId` is a descendant-or-self of `ancestorId`.
 * Used to prevent dragging a folder into one of its own descendants.
 * @param {any[]} folders
 * @param {number} targetId
 * @param {number} ancestorId
 * @returns {boolean}
 */
export function isFolderDescendantOrSelf(folders, targetId, ancestorId) {
  if (targetId === ancestorId) return true;
  const node = folders.find(f => f.id === targetId);
  if (!node || node.parent_id == null) return false;
  return isFolderDescendantOrSelf(folders, node.parent_id, ancestorId);
}

/**
 * Folder ids in the subtree rooted at `rootId` (including `rootId`).
 * @param {{ id: number, parent_id?: number|null }[]} folders
 * @param {number} rootId
 * @returns {Set<number>}
 */
export function folderSubtreeIds(folders, rootId) {
  const byParent = new Map();
  for (const f of folders) {
    const p = f.parent_id ?? null;
    if (!byParent.has(p)) byParent.set(p, []);
    byParent.get(p).push(f.id);
  }
  const ids = new Set([rootId]);
  const stack = [...(byParent.get(rootId) ?? [])];
  while (stack.length) {
    const id = stack.pop();
    ids.add(id);
    for (const c of byParent.get(id) ?? []) stack.push(c);
  }
  return ids;
}
