// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import {
  PINNED_ACTIONS_SETTING_KEY,
  createPinnedActions,
} from './pinnedActions.svelte.js';

function mockInvoke(map = {}) {
  return vi.fn(async (cmd, args) => {
    if (cmd === 'get_setting') {
      return map[args.key] ?? '';
    }
    if (cmd === 'set_setting') {
      map[args.key] = args.value;
      return;
    }
    throw new Error(`unexpected ${cmd}`);
  });
}

describe('createPinnedActions', () => {
  it('treats missing/empty setting as empty list', async () => {
    const invoke = mockInvoke({});
    const pins = createPinnedActions({ invoke });
    await pins.load();
    expect(pins.loaded).toBe(true);
    expect(pins.loadFailed).toBe(false);
    expect(pins.orderedIds).toEqual([]);
  });

  it('loads, dedupes, and drops non-strings', async () => {
    const invoke = mockInvoke({
      [PINNED_ACTIONS_SETTING_KEY]: JSON.stringify([
        'search',
        'search',
        3,
        'daily-note',
        '',
        'graph',
      ]),
    });
    const pins = createPinnedActions({ invoke });
    await pins.load();
    expect(pins.orderedIds).toEqual(['search', 'daily-note', 'graph']);
  });

  it('keeps unknown ids on load', async () => {
    const invoke = mockInvoke({
      [PINNED_ACTIONS_SETTING_KEY]: JSON.stringify(['search', 'future-cmd']),
    });
    const pins = createPinnedActions({ invoke });
    await pins.load();
    expect(pins.orderedIds).toEqual(['search', 'future-cmd']);
    await pins.unpin('search');
    expect(pins.orderedIds).toEqual(['future-cmd']);
    const saved = JSON.parse(
      /** @type {string} */ (
        invoke.mock.calls.find((c) => c[0] === 'set_setting')[1].value
      ),
    );
    expect(saved).toEqual(['future-cmd']);
  });

  it('marks corrupt JSON as loadFailed and refuses to save', async () => {
    const invoke = mockInvoke({
      [PINNED_ACTIONS_SETTING_KEY]: '{not-json',
    });
    const pins = createPinnedActions({ invoke });
    await pins.load();
    expect(pins.loaded).toBe(false);
    expect(pins.loadFailed).toBe(true);
    await pins.pin('search');
    expect(invoke.mock.calls.some((c) => c[0] === 'set_setting')).toBe(false);
  });

  it('refuses to save before load', async () => {
    const invoke = mockInvoke({});
    const pins = createPinnedActions({ invoke });
    await pins.pin('search');
    expect(pins.orderedIds).toEqual([]);
    expect(invoke.mock.calls.some((c) => c[0] === 'set_setting')).toBe(false);
  });

  it('pins, unpins, toggles, and reorders without duplicates', async () => {
    const store = {};
    const invoke = mockInvoke(store);
    const pins = createPinnedActions({ invoke });
    await pins.load();
    await pins.pin('search');
    await pins.pin('graph');
    await pins.pin('search');
    expect(pins.orderedIds).toEqual(['search', 'graph']);
    await pins.setOrder(['graph', 'search', 'graph']);
    expect(pins.orderedIds).toEqual(['graph', 'search']);
    await pins.moveUp('search');
    expect(pins.orderedIds).toEqual(['search', 'graph']);
    await pins.moveDown('search');
    expect(pins.orderedIds).toEqual(['graph', 'search']);
    await pins.toggle('graph');
    expect(pins.orderedIds).toEqual(['search']);
    expect(JSON.parse(store[PINNED_ACTIONS_SETTING_KEY])).toEqual(['search']);
  });

  it('recovers after a failed save and still persists later changes', async () => {
    const store = { [PINNED_ACTIONS_SETTING_KEY]: '[]' };
    let failOnce = true;
    const onError = vi.fn();
    const invoke = vi.fn(async (cmd, args) => {
      if (cmd === 'get_setting') return store[args.key] ?? '';
      if (cmd === 'set_setting') {
        if (failOnce) {
          failOnce = false;
          throw new Error('db busy');
        }
        store[args.key] = args.value;
        return;
      }
      throw new Error(`unexpected ${cmd}`);
    });
    const pins = createPinnedActions({ invoke, onError });
    await pins.load();
    await expect(pins.pin('search')).rejects.toThrow('db busy');
    expect(pins.orderedIds).toEqual([]);
    expect(onError).toHaveBeenCalled();
    await pins.pin('graph');
    expect(pins.orderedIds).toEqual(['graph']);
    expect(JSON.parse(store[PINNED_ACTIONS_SETTING_KEY])).toEqual(['graph']);
  });

  it('resetToEmpty clears a corrupt setting', async () => {
    const store = { [PINNED_ACTIONS_SETTING_KEY]: '{bad' };
    const invoke = mockInvoke(store);
    const pins = createPinnedActions({ invoke });
    await pins.load();
    expect(pins.loadFailed).toBe(true);
    await pins.resetToEmpty();
    expect(pins.loaded).toBe(true);
    expect(pins.loadFailed).toBe(false);
    expect(pins.orderedIds).toEqual([]);
    expect(store[PINNED_ACTIONS_SETTING_KEY]).toBe('[]');
  });
});
