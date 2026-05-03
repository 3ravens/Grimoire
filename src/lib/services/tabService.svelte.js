import { invoke } from '@tauri-apps/api/core';

/**
 * @param {{ onError?: (e: unknown) => void }} opts
 */
export function createTabService({ onError }) {
  /** @type {any[]} */
  let tabs = $state([]);
  /** @type {string | null} */
  let activeTabId = $state(null);
  let tableViewOpen = $state(false);
  let searchOpen = $state(false);
  let chatInsert = $state(null);
  /** @type {string | null} */
  let externalRenameTabId = $state(null);
  let activeViewFilters = $state({});

  const activeTab = $derived(tabs.find(t => t.id === activeTabId) ?? null);

  const activeView = $derived(
    activeTab?.type === 'kanban' ? 'kanban'
    : tableViewOpen ? 'database'
    : null
  );

  function makeTabId() { return Math.random().toString(36).slice(2, 9); }

  function persistTabs() {
    localStorage.setItem('grimoire_tabs', JSON.stringify({
      tabs: tabs.map(t => ({
        id: t.id, type: t.type, noteId: t.noteId,
        label: t.label, customLabel: t.customLabel,
        // wikipedia tab fields
        ...(t.type === 'wikipedia' ? {
          bundleId: t.bundleId, articlePath: t.articlePath,
        } : {}),
      })),
      activeTabId,
    }));
  }

  // Tab persistence effect — call from the component's $effect
  function setupPersistence() {
    $effect(() => {
      persistTabs();
    });
  }

  async function newTab() {
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'note', noteId: null, label: 'New Tab', customLabel: null, readMode: false }];
    activeTabId = id;
    return { id, isNew: true };
  }

  /** @param {any} note @param {(() => any) | null} [saveFn] */
  async function navigateToNote(note, saveFn) {
    if (saveFn) await saveFn();
    if (!activeTabId || tabs.length === 0) {
      const id = makeTabId();
      tabs = [{ id, type: 'note', noteId: note.id, label: note.title, customLabel: null, readMode: false }];
      activeTabId = id;
    } else {
      const currentTab = tabs.find(t => t.id === activeTabId);
      if (currentTab?.type !== 'note') {
        const id = makeTabId();
        tabs = [...tabs, { id, type: 'note', noteId: note.id, label: note.title, customLabel: null, readMode: false }];
        activeTabId = id;
      } else {
        tabs = tabs.map(t => t.id === activeTabId
          ? { ...t, type: 'note', noteId: note.id, label: note.title }
          : t
        );
      }
    }
  }

  /** @param {any} note @param {(() => any) | null} [saveFn] */
  async function openNoteInNewTab(note, saveFn) {
    if (saveFn) await saveFn();
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'note', noteId: note.id, label: note.title, customLabel: null, readMode: false }];
    activeTabId = id;
  }

  /**
   * @param {string} id
   * @param {(() => any) | null} [saveFn]
   * @param {((note: any) => void) | null} [openNoteFn]
   */
  async function activateTab(id, saveFn, openNoteFn) {
    if (activeTabId === id) return;
    if (saveFn) await saveFn();
    activeTabId = id;
    const tab = tabs.find(t => t.id === id);
    if (!tab) return;
    if (tab.type === 'note' && tab.noteId != null) {
      try {
        const note = await invoke('get_note', { id: tab.noteId });
        openNoteFn?.(note);
      } catch (e) {
        onError?.(e);
        closeTab(id);
      }
    } else {
      openNoteFn?.(null);
    }
  }

  /**
   * @param {string} id
   * @param {(() => any) | null} [saveFn]
   * @param {((note: any) => void) | null} [openNoteFn]
   * @param {(() => any) | null} [newTabFn]
   */
  async function closeTab(id, saveFn, openNoteFn, newTabFn) {
    const idx = tabs.findIndex(t => t.id === id);
    if (idx === -1) return;
    if (id === activeTabId && saveFn) await saveFn();
    const newTabs = tabs.filter(t => t.id !== id);
    tabs = newTabs;
    if (activeTabId === id) {
      if (newTabs.length === 0) {
        newTabFn?.();
      } else {
        const next = newTabs[idx] ?? newTabs[idx - 1];
        activeTabId = next.id;
        if (next.type === 'note' && next.noteId != null) {
          try {
            const note = await invoke('get_note', { id: next.noteId });
            openNoteFn?.(note);
          } catch { openNoteFn?.(null); }
        } else {
          openNoteFn?.(null);
        }
      }
    }
  }

  /**
   * @param {string} keepId
   * @param {(() => any) | null} [saveFn]
   * @param {((note: any) => void) | null} [openNoteFn]
   */
  async function closeOtherTabs(keepId, saveFn, openNoteFn) {
    if (saveFn) await saveFn();
    tabs = tabs.filter(t => t.id === keepId);
    if (activeTabId !== keepId) {
      activeTabId = keepId;
      const tab = tabs[0];
      if (tab?.type === 'note' && tab.noteId != null) {
        try {
          const note = await invoke('get_note', { id: tab.noteId });
          openNoteFn?.(note);
        } catch { openNoteFn?.(null); }
      } else {
        openNoteFn?.(null);
      }
    }
  }

  /** @param {string} id */
  function startTabRenameExternal(id) {
    externalRenameTabId = id;
    setTimeout(() => { externalRenameTabId = null; }, 50);
  }

  /** @param {string} id @param {string | null} label */
  function renameTab(id, label) {
    tabs = tabs.map(t => t.id === id ? { ...t, customLabel: label || null } : t);
  }

  function toggleReadMode() {
    if (!activeTabId) return;
    tabs = tabs.map(t => t.id === activeTabId ? { ...t, readMode: !t.readMode } : t);
  }

  function openGraphTab() {
    const existing = tabs.find(t => t.type === 'graph');
    if (existing) { activeTabId = existing.id; return 'existing'; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'graph', noteId: null, label: 'Graph', customLabel: null }];
    activeTabId = id;
    return 'new';
  }

  function openCalendarTab() {
    const existing = tabs.find(t => t.type === 'calendar');
    if (existing) { activeTabId = existing.id; return 'existing'; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'calendar', noteId: null, label: 'Calendar', customLabel: null }];
    activeTabId = id;
    return 'new';
  }

  /** @param {any} folderId @param {string} folderName */
  function openKanbanTab(folderId, folderName) {
    const existing = tabs.find(t => t.type === 'kanban' && t.folderId === folderId);
    if (existing) { activeTabId = existing.id; return 'existing'; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'kanban', noteId: null, label: `Kanban — ${folderName}`, customLabel: null, folderId }];
    activeTabId = id;
    return 'new';
  }

  function openChatTab() {
    const existing = tabs.find(t => t.type === 'chat');
    if (existing) { activeTabId = existing.id; return 'existing'; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'chat', noteId: null, label: 'Chat', customLabel: null }];
    activeTabId = id;
    return 'new';
  }

  /** @param {any} bundleId @param {string} articlePath @param {string} [title] */
  function openWikipediaArticle(bundleId, articlePath, title) {
    // Deduplicate: if already open, activate the existing tab
    const existing = tabs.find(
      t => t.type === 'wikipedia' && t.bundleId === bundleId && t.articlePath === articlePath
    );
    if (existing) { activeTabId = existing.id; return; }
    const id = makeTabId();
    tabs = [...tabs, {
      id, type: 'wikipedia', noteId: null,
      label: title || articlePath, customLabel: null,
      bundleId, articlePath,
    }];
    activeTabId = id;
  }

  // Called when the reader navigates internally (cross-link or search).
  // Updates the active tab's path and label in place so no extra tab is opened.
  /** @param {any} bundleId @param {string} articlePath @param {string} [title] */
  function updateWikipediaTab(bundleId, articlePath, title) {
    tabs = tabs.map(t =>
      t.id === activeTabId
        ? { ...t, bundleId, articlePath, label: title || articlePath, customLabel: null }
        : t
    );
  }

  function closeNoteInTab() {
    tabs = tabs.map(t => t.id === activeTabId ? { ...t, noteId: null, label: 'New Tab' } : t);
  }

  /** @param {((note: any) => void) | null} [openNoteFn] */
  async function restoreTabs(openNoteFn) {
    try {
      const saved = localStorage.getItem('grimoire_tabs');
      if (!saved) return;
      const { tabs: savedTabs, activeTabId: savedActiveId } = JSON.parse(saved);
      if (!Array.isArray(savedTabs) || savedTabs.length === 0) return;

      const results = await Promise.all(
        savedTabs.map(async t => {
          if (t.type === 'note' && t.noteId != null) {
            try {
              const note = await invoke('get_note', { id: t.noteId });
              return { tab: { ...t, label: note.title }, note };
            } catch { return null; }
          } else if (t.type === 'graph') {
            return { tab: t, note: null };
          } else if (t.type === 'wikipedia' && t.bundleId && t.articlePath) {
            return { tab: { ...t, readMode: false }, note: null };
          }
          return null;
        })
      );

      const valid = /** @type {Array<{tab: any, note: any}>} */ (results.filter(Boolean));
      if (valid.length === 0) return;
      tabs = valid.map(r => r.tab);
      const target = valid.find(r => r.tab.id === savedActiveId) ?? valid[valid.length - 1];
      if (!target) return;
      activeTabId = target.tab.id;
      if (target.tab.type === 'note' && target.note) {
        openNoteFn?.(target.note);
      }
    } catch { /* start fresh */ }
  }

  return {
    get tabs() { return tabs; },
    set tabs(v) { tabs = v; },
    get activeTabId() { return activeTabId; },
    set activeTabId(v) { activeTabId = v; },
    get activeTab() { return activeTab; },
    get activeView() { return activeView; },
    get tableViewOpen() { return tableViewOpen; },
    set tableViewOpen(v) { tableViewOpen = v; },
    get searchOpen() { return searchOpen; },
    set searchOpen(v) { searchOpen = v; },
    get chatInsert() { return chatInsert; },
    set chatInsert(v) { chatInsert = v; },
    get externalRenameTabId() { return externalRenameTabId; },
    set externalRenameTabId(v) { externalRenameTabId = v; },
    get activeViewFilters() { return activeViewFilters; },
    set activeViewFilters(v) { activeViewFilters = v; },
    makeTabId,
    setupPersistence,
    newTab,
    navigateToNote,
    openNoteInNewTab,
    activateTab,
    closeTab,
    closeOtherTabs,
    startTabRenameExternal,
    renameTab,
    toggleReadMode,
    openGraphTab,
    openCalendarTab,
    openKanbanTab,
    openChatTab,
    openWikipediaArticle,
    updateWikipediaTab,
    closeNoteInTab,
    restoreTabs,
  };
}
