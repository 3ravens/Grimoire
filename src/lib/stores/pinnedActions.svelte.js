/** SQLite-backed ordered list of pinned command ids. */

export const PINNED_ACTIONS_SETTING_KEY = 'pinned_actions';

/**
 * @param {{
 *   invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>,
 *   onError?: (e: unknown) => void,
 * }} deps
 */
export function createPinnedActions({ invoke, onError }) {
  /** @type {string[]} */
  let orderedIds = $state([]);
  let loaded = $state(false);
  let loadFailed = $state(false);
  /** @type {string | null} */
  let loadError = $state(null);

  /** @type {Promise<void>} */
  let saveChain = Promise.resolve();

  /**
   * @param {unknown} raw
   * @returns {{ ok: true, ids: string[] } | { ok: false, error: string }}
   */
  function parsePinnedValue(raw) {
    if (raw == null || raw === '') {
      return { ok: true, ids: [] };
    }
    if (typeof raw !== 'string') {
      return { ok: false, error: 'Pinned actions setting is not a string' };
    }
    let parsed;
    try {
      parsed = JSON.parse(raw);
    } catch {
      return { ok: false, error: 'Pinned actions setting is not valid JSON' };
    }
    if (!Array.isArray(parsed)) {
      return { ok: false, error: 'Pinned actions setting must be a JSON array' };
    }
    /** @type {string[]} */
    const ids = [];
    const seen = new Set();
    for (const entry of parsed) {
      if (typeof entry !== 'string' || entry.length === 0) continue;
      if (seen.has(entry)) continue;
      seen.add(entry);
      ids.push(entry);
    }
    return { ok: true, ids };
  }

  async function persist() {
    if (!loaded || loadFailed) return;
    const write = saveChain.catch(() => {}).then(async () => {
      if (!loaded || loadFailed) return;
      await invoke('set_setting', {
        key: PINNED_ACTIONS_SETTING_KEY,
        value: JSON.stringify(orderedIds.slice()),
      });
    });
    saveChain = write.catch(() => {});
    await write;
  }

  /**
   * Apply a mutation, persist, and roll back + report on failure.
   * @param {() => void} mutate
   */
  async function mutateAndPersist(mutate) {
    if (!loaded || loadFailed) return;
    const previous = orderedIds.slice();
    mutate();
    try {
      await persist();
    } catch (e) {
      orderedIds = previous;
      onError?.(e);
      throw e;
    }
  }

  async function load() {
    await saveChain.catch(() => {});
    try {
      const raw = await invoke('get_setting', { key: PINNED_ACTIONS_SETTING_KEY });
      const result = parsePinnedValue(raw);
      if (!result.ok) {
        loadFailed = true;
        loadError = result.error;
        loaded = false;
        return;
      }
      orderedIds = result.ids;
      loadFailed = false;
      loadError = null;
      loaded = true;
    } catch (e) {
      loadFailed = true;
      loadError = e instanceof Error ? e.message : String(e);
      loaded = false;
    }
  }

  /** Clear a corrupt setting so the user can pin again. */
  async function resetToEmpty() {
    try {
      await invoke('set_setting', {
        key: PINNED_ACTIONS_SETTING_KEY,
        value: '[]',
      });
      orderedIds = [];
      loadFailed = false;
      loadError = null;
      loaded = true;
    } catch (e) {
      onError?.(e);
      throw e;
    }
  }

  /** @param {string} id */
  function isPinned(id) {
    return orderedIds.includes(id);
  }

  /** @param {string} id */
  async function pin(id) {
    if (!loaded || loadFailed) return;
    if (orderedIds.includes(id)) return;
    await mutateAndPersist(() => {
      orderedIds = [...orderedIds, id];
    });
  }

  /** @param {string} id */
  async function unpin(id) {
    if (!loaded || loadFailed) return;
    if (!orderedIds.includes(id)) return;
    await mutateAndPersist(() => {
      orderedIds = orderedIds.filter((x) => x !== id);
    });
  }

  /** @param {string} id */
  async function toggle(id) {
    if (isPinned(id)) await unpin(id);
    else await pin(id);
  }

  /**
   * Replace the full ordered list (e.g. after drag reorder).
   * @param {string[]} next
   */
  async function setOrder(next) {
    if (!loaded || loadFailed) return;
    const seen = new Set();
    /** @type {string[]} */
    const ids = [];
    for (const id of next) {
      if (typeof id !== 'string' || !id || seen.has(id)) continue;
      seen.add(id);
      ids.push(id);
    }
    await mutateAndPersist(() => {
      orderedIds = ids;
    });
  }

  /** @param {string} id */
  async function moveUp(id) {
    if (!loaded || loadFailed) return;
    const i = orderedIds.indexOf(id);
    if (i <= 0) return;
    await mutateAndPersist(() => {
      const next = orderedIds.slice();
      [next[i - 1], next[i]] = [next[i], next[i - 1]];
      orderedIds = next;
    });
  }

  /** @param {string} id */
  async function moveDown(id) {
    if (!loaded || loadFailed) return;
    const i = orderedIds.indexOf(id);
    if (i < 0 || i >= orderedIds.length - 1) return;
    await mutateAndPersist(() => {
      const next = orderedIds.slice();
      [next[i], next[i + 1]] = [next[i + 1], next[i]];
      orderedIds = next;
    });
  }

  return {
    get orderedIds() {
      return orderedIds;
    },
    get loaded() {
      return loaded;
    },
    get loadFailed() {
      return loadFailed;
    },
    get loadError() {
      return loadError;
    },
    load,
    resetToEmpty,
    isPinned,
    pin,
    unpin,
    toggle,
    setOrder,
    moveUp,
    moveDown,
    /** @internal exposed for tests */
    parsePinnedValue,
  };
}
