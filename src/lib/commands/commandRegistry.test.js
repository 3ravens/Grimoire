// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { COMMANDS, COMMAND_BY_ID, getCommand } from './commandRegistry.js';
import { t } from '../i18n/t.js';

describe('commandRegistry', () => {
  it('has unique ids', () => {
    const ids = COMMANDS.map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('includes the v1.1 core set', () => {
    const expected = [
      'search',
      'graph',
      'calendar',
      'daily-note',
      'quick-switcher',
      'wikipedia',
      'lock-vault',
      'settings',
      'help',
      'docs',
      'report-bug',
      'new-note',
      'new-folder',
    ];
    expect(COMMANDS.map((c) => c.id)).toEqual(expected);
  });

  it('resolves every labelKey, titleKey, and ariaKey in en.js', () => {
    for (const c of COMMANDS) {
      expect(t(c.labelKey), c.labelKey).not.toBe(c.labelKey);
      expect(t(c.titleKey), c.titleKey).not.toBe(c.titleKey);
      expect(t(c.ariaKey), c.ariaKey).not.toBe(c.ariaKey);
    }
  });

  it('looks up by id', () => {
    expect(getCommand('search')?.id).toBe('search');
    expect(COMMAND_BY_ID.get('daily-note')?.availability).toBe('always');
    expect(getCommand('nope')).toBeNull();
  });
});
