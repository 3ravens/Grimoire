/**
 * Static command registry metadata for pinned actions and (later) the command palette.
 * Handlers and live availability live in the command service — not here.
 */

import SearchIcon from '../icons/SearchIcon.svelte';
import GraphIcon from '../icons/GraphIcon.svelte';
import CalendarIcon from '../icons/CalendarIcon.svelte';
import DailyNoteIcon from '../icons/DailyNoteIcon.svelte';
import QuickSwitcherIcon from '../icons/QuickSwitcherIcon.svelte';
import DocumentIcon from '../icons/DocumentIcon.svelte';
import VaultLockIcon from '../icons/VaultLockIcon.svelte';
import SettingsIcon from '../icons/SettingsIcon.svelte';
import HelpIcon from '../icons/HelpIcon.svelte';
import DocsIcon from '../icons/DocsIcon.svelte';
import BugIcon from '../icons/BugIcon.svelte';
import PencilIcon from '../icons/PencilIcon.svelte';
import NewFolderIcon from '../icons/NewFolderIcon.svelte';

/** @typedef {'always' | 'wikipedia' | 'lock'} CommandAvailability */

/**
 * @typedef {{
 *   id: string,
 *   labelKey: string,
 *   titleKey: string,
 *   ariaKey: string,
 *   icon: import('svelte').Component,
 *   availability: CommandAvailability,
 * }} CommandDef
 */

/** @type {CommandDef[]} */
export const COMMANDS = [
  {
    id: 'search',
    labelKey: 'commands.search',
    titleKey: 'activityBar.searchTitle',
    ariaKey: 'activityBar.searchAria',
    icon: SearchIcon,
    availability: 'always',
  },
  {
    id: 'graph',
    labelKey: 'commands.graph',
    titleKey: 'activityBar.graphTitle',
    ariaKey: 'activityBar.graphAria',
    icon: GraphIcon,
    availability: 'always',
  },
  {
    id: 'calendar',
    labelKey: 'commands.calendar',
    titleKey: 'activityBar.calendarTitle',
    ariaKey: 'activityBar.calendarAria',
    icon: CalendarIcon,
    availability: 'always',
  },
  {
    id: 'daily-note',
    labelKey: 'commands.dailyNote',
    titleKey: 'activityBar.dailyNoteTitle',
    ariaKey: 'activityBar.dailyNoteAria',
    icon: DailyNoteIcon,
    availability: 'always',
  },
  {
    id: 'quick-switcher',
    labelKey: 'commands.quickSwitcher',
    titleKey: 'activityBar.quickSwitcherTitle',
    ariaKey: 'activityBar.quickSwitcherAria',
    icon: QuickSwitcherIcon,
    availability: 'always',
  },
  {
    id: 'wikipedia',
    labelKey: 'commands.wikipedia',
    titleKey: 'activityBar.wikipediaTitle',
    ariaKey: 'activityBar.wikipediaAria',
    icon: DocumentIcon,
    availability: 'wikipedia',
  },
  {
    id: 'lock-vault',
    labelKey: 'commands.lockVault',
    titleKey: 'activityBar.lockVaultTitle',
    ariaKey: 'activityBar.lockVaultAria',
    icon: VaultLockIcon,
    availability: 'lock',
  },
  {
    id: 'settings',
    labelKey: 'commands.settings',
    titleKey: 'activityBar.settingsTitle',
    ariaKey: 'activityBar.settingsAria',
    icon: SettingsIcon,
    availability: 'always',
  },
  {
    id: 'help',
    labelKey: 'commands.help',
    titleKey: 'activityBar.helpTitle',
    ariaKey: 'activityBar.helpAria',
    icon: HelpIcon,
    availability: 'always',
  },
  {
    id: 'docs',
    labelKey: 'commands.docs',
    titleKey: 'activityBar.docsTitle',
    ariaKey: 'activityBar.docsAria',
    icon: DocsIcon,
    availability: 'always',
  },
  {
    id: 'report-bug',
    labelKey: 'commands.reportBug',
    titleKey: 'activityBar.bugTitle',
    ariaKey: 'activityBar.bugAria',
    icon: BugIcon,
    availability: 'always',
  },
  {
    id: 'new-note',
    labelKey: 'commands.newNote',
    titleKey: 'activityBar.newNoteTitle',
    ariaKey: 'activityBar.newNoteAria',
    icon: PencilIcon,
    availability: 'always',
  },
  {
    id: 'new-folder',
    labelKey: 'commands.newFolder',
    titleKey: 'activityBar.newFolderTitle',
    ariaKey: 'activityBar.newFolderAria',
    icon: NewFolderIcon,
    availability: 'always',
  },
];

/** @type {ReadonlyMap<string, CommandDef>} */
export const COMMAND_BY_ID = new Map(COMMANDS.map((c) => [c.id, c]));

/** @param {string} id */
export function getCommand(id) {
  return COMMAND_BY_ID.get(id) ?? null;
}
