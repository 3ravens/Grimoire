/**
 * Sort notes for the folder note list: pinned block first (explicit order),
 * then the rest by the active sort mode.
 *
 * @param {Array<{ id: number, title?: string, created_at?: number, updated_at?: number }>} notes
 * @param {'modified' | 'created' | 'name'} sortMode
 * @param {number[]} pinOrder note ids in pin order (unknown ids skipped)
 */
export function sortNotesWithFolderPins(notes, sortMode, pinOrder = []) {
  const byId = new Map(notes.map((n) => [n.id, n]));
  const pinned = [];
  const pinnedIds = new Set();
  for (const id of pinOrder) {
    const note = byId.get(id);
    if (note) {
      pinned.push(note);
      pinnedIds.add(id);
    }
  }

  const rest = notes.filter((n) => !pinnedIds.has(n.id));
  if (sortMode === 'name') {
    rest.sort((a, b) => (a.title ?? '').localeCompare(b.title ?? ''));
  } else if (sortMode === 'created') {
    rest.sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
  } else {
    rest.sort((a, b) => (b.updated_at ?? 0) - (a.updated_at ?? 0));
  }

  return [...pinned, ...rest];
}
