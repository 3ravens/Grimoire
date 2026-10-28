/**
 * App-curated action presets: named pin lists applied from Settings.
 * Applying a preset replaces the pin strip only (fire-and-forget).
 */

/**
 * @typedef {{
 *   id: string,
 *   nameKey: string,
 *   ids: readonly string[],
 * }} ActionPreset
 */

/** @type {readonly ActionPreset[]} */
export const ACTION_PRESETS = Object.freeze([
  {
    id: 'minimal',
    nameKey: 'settings.pinnedActions.presets.minimal',
    ids: Object.freeze([]),
  },
  {
    id: 'knowledge-builder',
    nameKey: 'settings.pinnedActions.presets.knowledgeBuilder',
    ids: Object.freeze(['search', 'graph', 'quick-switcher', 'daily-note']),
  },
  {
    id: 'bullet-journal',
    nameKey: 'settings.pinnedActions.presets.bulletJournal',
    ids: Object.freeze(['daily-note', 'calendar']),
  },
  {
    id: 'para',
    nameKey: 'settings.pinnedActions.presets.para',
    ids: Object.freeze(['daily-note', 'search']),
  },
]);

/**
 * Exact ordered equality of two pin id lists.
 * @param {readonly string[]} a
 * @param {readonly string[]} b
 */
export function samePinOrder(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

/**
 * Whether applying a preset should ask for confirmation.
 * True only when the current list is non-empty and not already identical.
 * @param {readonly string[]} currentIds
 * @param {readonly string[]} presetIds
 */
export function needsPresetConfirm(currentIds, presetIds) {
  if (currentIds.length === 0) return false;
  return !samePinOrder(currentIds, presetIds);
}
