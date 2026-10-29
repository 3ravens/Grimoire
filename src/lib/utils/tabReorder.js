/** Private MIME so tab reorder never becomes a note/folder drop. */
export const TAB_MIME = 'application/x-grimoire-tab';

/** @param {DataTransfer | null | undefined} dt */
export function isTabDrag(dt) {
  if (!dt) return false;
  return [...dt.types].includes(TAB_MIME);
}

/**
 * Immutable reorder of a tab list.
 * @template {{ id: string }} T
 * @param {T[]} tabs
 * @param {string} fromId
 * @param {string} targetId
 * @param {boolean} placeAfter  insert after target when true, before when false
 * @returns {T[]}
 */
export function reorderTabs(tabs, fromId, targetId, placeAfter) {
  const fromIdx = tabs.findIndex((t) => t.id === fromId);
  const toIdx = tabs.findIndex((t) => t.id === targetId);
  if (fromIdx < 0 || toIdx < 0 || fromId === targetId) return tabs;

  let dest = placeAfter ? toIdx + 1 : toIdx;
  if (fromIdx < dest) dest -= 1;
  if (dest === fromIdx) return tabs;

  const next = [...tabs];
  const [item] = next.splice(fromIdx, 1);
  next.splice(dest, 0, item);
  return next;
}
