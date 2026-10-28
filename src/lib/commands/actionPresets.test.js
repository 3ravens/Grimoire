// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { getCommand } from './commandRegistry.js';
import { t } from '../i18n/t.js';
import {
  ACTION_PRESETS,
  needsPresetConfirm,
  samePinOrder,
} from './actionPresets.js';

describe('actionPresets', () => {
  it('defines the four curated presets in contract order', () => {
    expect(ACTION_PRESETS.map((p) => p.id)).toEqual([
      'minimal',
      'knowledge-builder',
      'bullet-journal',
      'para',
    ]);
    expect([...ACTION_PRESETS[0].ids]).toEqual([]);
    expect([...ACTION_PRESETS[1].ids]).toEqual([
      'search',
      'graph',
      'quick-switcher',
      'daily-note',
    ]);
    expect([...ACTION_PRESETS[2].ids]).toEqual(['daily-note', 'calendar']);
    expect([...ACTION_PRESETS[3].ids]).toEqual(['daily-note', 'search']);
  });

  it('resolves every preset id via getCommand with always availability', () => {
    for (const preset of ACTION_PRESETS) {
      for (const id of preset.ids) {
        const cmd = getCommand(id);
        expect(cmd, `missing command ${id} in ${preset.id}`).not.toBeNull();
        expect(cmd?.availability).toBe('always');
      }
    }
  });

  it('resolves every preset nameKey in en.js', () => {
    for (const preset of ACTION_PRESETS) {
      expect(t(preset.nameKey), preset.nameKey).not.toBe(preset.nameKey);
    }
  });

  it('samePinOrder checks exact ordered equality', () => {
    expect(samePinOrder([], [])).toBe(true);
    expect(samePinOrder(['a'], ['a'])).toBe(true);
    expect(samePinOrder(['a', 'b'], ['b', 'a'])).toBe(false);
    expect(samePinOrder(['a'], ['a', 'b'])).toBe(false);
  });

  it('needsPresetConfirm: empty current never confirms', () => {
    expect(needsPresetConfirm([], [])).toBe(false);
    expect(needsPresetConfirm([], ['search'])).toBe(false);
  });

  it('needsPresetConfirm: identical list skips confirm', () => {
    const ids = ['daily-note', 'calendar'];
    expect(needsPresetConfirm(ids, ids)).toBe(false);
    expect(needsPresetConfirm(['daily-note', 'calendar'], ['daily-note', 'calendar'])).toBe(
      false,
    );
  });

  it('needsPresetConfirm: reorder or extra ids needs confirm', () => {
    expect(
      needsPresetConfirm(['calendar', 'daily-note'], ['daily-note', 'calendar']),
    ).toBe(true);
    expect(needsPresetConfirm(['wikipedia', 'help'], ['daily-note', 'search'])).toBe(
      true,
    );
    expect(needsPresetConfirm(['unknown-id'], [])).toBe(true);
  });
});
