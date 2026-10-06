import { t } from './t.js';

/**
 * Display label for a tab. Built-in view types resolve via the catalog at
 * render time so a future locale does not depend on persisted English.
 * @param {{ type?: string, label?: string, customLabel?: string | null, folderId?: number | null, folderName?: string }} tab
 * @param {{ folderName?: string }} [opts]
 * @returns {string}
 */
export function displayTabLabel(tab, opts = {}) {
  if (tab?.customLabel) return tab.customLabel;
  switch (tab?.type) {
    case 'graph':
      return t('tabs.graph');
    case 'calendar':
      return t('tabs.calendar');
    case 'chat':
      return t('tabs.chat');
    case 'kanban': {
      const name = opts.folderName ?? tab.folderName ?? tab.label?.replace(/^Kanban — /, '') ?? '';
      return t('tabs.kanban', { name });
    }
    case 'note':
      return tab.label || t('tabs.newTab');
    case 'wikipedia':
      return tab.label || t('common.untitled');
    default:
      return tab?.label || t('common.untitled');
  }
}
