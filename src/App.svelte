<!-- Copyright (C) 2026 Wim Palland

This file is part of Grimoire.

Grimoire is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

Grimoire is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with Grimoire. If not, see <https://www.gnu.org/licenses/>. -->

<script>
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import { onMount, tick, untrack } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { computeDiff, applyAcceptedHunks } from './lib/utils/diff.js';
  import ActivityBar from './lib/ActivityBar.svelte';
  import Calendar from './lib/Calendar.svelte';
  import Chat from './lib/Chat.svelte';
  import Graph from './lib/Graph.svelte';
  import Kanban from './lib/Kanban.svelte';
  import WikipediaReader from './lib/WikipediaReader.svelte';
  import WikipediaSearchModal from './lib/WikipediaSearchModal.svelte';
  import TabBar from './lib/TabBar.svelte';
  import LockScreen from './lib/LockScreen.svelte';
  import PasswordModal from './lib/PasswordModal.svelte';
  import TemplateModal from './lib/TemplateModal.svelte';
  import DatabaseView from './lib/DatabaseView.svelte';
  import Settings from './lib/Settings.svelte';
  import Search from './lib/Search.svelte';
  import ConfirmModal from './lib/ConfirmModal.svelte';
  import QuickSwitcher from './lib/QuickSwitcher.svelte';
  import ContextMenu from './lib/ContextMenu.svelte';
  import FolderSidebar from './lib/FolderSidebar.svelte';
  import NoteList from './lib/NoteList.svelte';
  import NoteEditor from './lib/NoteEditor.svelte';
  import { createSettings } from './lib/stores/settings.svelte.js';
  import { createPanelLayout } from './lib/stores/panelLayout.svelte.js';
  import { createBookmarks } from './lib/stores/bookmarks.svelte.js';
  import { createTemplates } from './lib/stores/templates.svelte.js';

  const appWindow = getCurrentWindow();

  // ── Stores ─────────────────────────────────────────────────────────────────
  const settings = createSettings();
  const layout   = createPanelLayout();
  const bm       = createBookmarks();
  const tmpl     = createTemplates();

  // ── Core state ─────────────────────────────────────────────────────────────

  let folders = $state([]);
  let notes = $state([]);
  let selectedFolderId = $state(null);
  let activeNote = $state(null);

  // ── Tab state ─────────────────────────────────────────────────────────────────
  let tabs = $state([]);
  let activeTabId = $state(null);
  const activeTab = $derived(tabs.find(t => t.id === activeTabId) ?? null);
  function makeTabId() { return Math.random().toString(36).slice(2, 9); }

  let editorTitle   = $state('');
  let editorContent = $state('');
  let isDirty       = $state(false);
  let indexState    = $state('idle');
  let editorTextareaEl = $state(null);

  let errorMsg = $state('');

  // When set, Chat.svelte will inject this text as a blockquote into its input.
  let chatInsert = $state(null); // { text: string, seq: number } | null

  // Improve note state
  let improveState = $state({ status: 'idle', instruction: '', improvedText: '', hunks: [], originalContent: '', acceptedIndices: [], rejectedIndices: [] });

  // Refine hunk state
  let refineState = $state({ status: 'idle', hunkIndex: null, x: 0, y: 0 });

  // Search panel
  let searchOpen = $state(false);

  // Seed/reindex state
  let isSeeding    = $state(false);
  let isReindexing = $state(false);

  // Tags and links for the active note
  let noteTags        = $state([]);
  let noteLinks       = $state([]);
  let noteBacklinks   = $state([]);
  let unlinkedMentions = $state([]);
  let tagFilter       = $state(null);
  let allTags         = $state([]);

  // Inline-rename state (shared between FolderSidebar and NoteList)
  let inlineRenaming = $state(null); // { id, type: 'folder'|'note', value }

  // Per-folder expand state — true (default) means expanded. Shared with FolderSidebar.
  let folderExpanded = $state({}); // { [folderId]: boolean }

  // Settings overlay
  let settingsOpen          = $state(false);
  let quickSwitcherOpen     = $state(false);
  let wikiSearchOpen        = $state(false);

  // Database / table view
  let tableViewOpen      = $state(false);
  let folderHasProperties = $state(false);
  let noteProperties     = $state([]);
  let propertiesReady    = $state(true);
  let activeViewFilters  = $state({});

  // Password / lock state
  let lockCheckDone    = $state(false);
  let vaultLocked      = $state(false);
  let vaultHasPassword = $state(false);
  let folderUnlockTarget = $state(null);
  let vaultPwModal     = $state(null);
  let folderPwModal    = $state(null);
  let noteDeletePending   = $state(null);
  let folderDeletePending = $state(null);
  let unlockedFolderIds   = $state(new Set());

  // Context menu
  let ctxMenu = $state(null);

  // Tab rename signal for TabBar
  let externalRenameTabId = $state(null);

  // Note drag state
  let isDragging       = $state(false);
  let dragOverFolderId = $state(null); // not used directly in markup anymore but needed for note drag logic

  // Derived
  const activeView = $derived(
    activeTab?.type === 'kanban' ? 'kanban'
    : tableViewOpen ? 'database'
    : null
  );
  const activeViewFolderId = $derived(
    activeView === 'kanban' ? activeTab.folderId
    : activeView === 'database' ? selectedFolderId
    : null
  );
  const activeViewLabel = $derived.by(() => {
    if (!activeView || !activeViewFolderId) return '';
    const folder = folders.find(f => f.id === activeViewFolderId);
    const name = folder?.name ?? 'Unknown';
    return activeView === 'kanban' ? `Kanban — ${name}` : `Table — ${name}`;
  });

  // Prevent "not allowed" drag cursor in Tauri/WebView2.
  $effect(() => {
    const allow = (e) => e.preventDefault();
    document.addEventListener('dragover', allow);
    return () => document.removeEventListener('dragover', allow);
  });

  // Persist tab state to localStorage.
  $effect(() => {
    localStorage.setItem('grimoire_tabs', JSON.stringify({
      tabs: tabs.map(t => ({ id: t.id, type: t.type, noteId: t.noteId, label: t.label, customLabel: t.customLabel })),
      activeTabId,
    }));
  });

  // ── Context menu ─────────────────────────────────────────────────────────────────

  $effect(() => {
    if (import.meta.env.DEV) {
      // devNativeContextMenu persistence is handled inside the settings store.
    }
    if (settings.devNativeContextMenu) ctxMenu = null;

    const handler = (e) => {
      if (settings.devNativeContextMenu) return;
      e.preventDefault();
      const items = buildCtxItems(e);
      if (items.length === 0) return;
      const x = Math.min(e.clientX, window.innerWidth  - 174);
      const y = Math.min(e.clientY, window.innerHeight - items.length * 28 - 16);
      ctxMenu = { x, y, items };
    };

    document.addEventListener('contextmenu', handler);
    return () => document.removeEventListener('contextmenu', handler);
  });

  function buildCtxItems(e) {
    const tabEl      = e.target.closest('[data-tab-id]');
    const noteLiEl   = e.target.closest('[data-note-id]');
    const folderLiEl = e.target.closest('[data-folder-id]');
    const createNoteBtn = e.target.closest('[data-action="create-note-btn"]');
    const isEditor   = !!e.target.closest('.content-area');

    let items = /** @type {any[]} */ ([]);

    if (createNoteBtn) {
      items = tmpl.templates.map(t => ({
        label: t.name,
        action: () => startNoteInline(t.id),
      }));
    } else if (tabEl) {
      const tabId = tabEl.dataset.tabId;
      items = [
        { label: 'Close',        action: () => closeTab(tabId) },
        { label: 'Close Others', action: () => closeOtherTabs(tabId) },
        { label: 'Rename',       action: () => startTabRenameExternal(tabId) },
      ];
    } else if (noteLiEl && !noteLiEl.classList.contains('locked-row')) {
      const noteId = Number(noteLiEl.dataset.noteId);
      const note = notes.find(n => n.id === noteId);
      items = [
        { label: 'Open in New Tab', action: () => note && openNoteInNewTab(note) },
        { label: 'Duplicate', action: async () => {
            await invoke('duplicate_note', { id: noteId });
            await loadNotes();
          }
        },
        { divider: true },
        bm.bookmarkedNoteIds.has(noteId)
          ? { label: 'Remove from Bookmarks', action: () => bm.removeBookmark(noteId) }
          : { label: 'Add to Bookmarks',      action: () => bm.addBookmark(noteId) },
        { divider: true },
        { label: 'Delete', action: () => deleteNote(noteId), danger: true },
      ];
    } else if (folderLiEl) {
      const raw = folderLiEl.dataset.folderId;
      if (raw && raw !== 'all' && raw !== 'unfiled') {
        const folderId = Number(raw);
        const folder = folders.find(f => f.id === folderId);
        if (folder && !folder.locked) {
          items = [
            { label: 'Open as Table',  action: async () => { await selectFolder(folderId); tableViewOpen = true; } },
            { label: 'Open as Kanban', action: () => openKanbanTab(folderId, folder.name) },
            { divider: true },
            unlockedFolderIds.has(folderId)
              ? { label: 'Remove password', action: () => (folderPwModal = { mode: 'remove', folderId }) }
              : { label: 'Set password',    action: () => (folderPwModal = { mode: 'set',    folderId }) },
            { label: 'Delete', action: () => deleteFolder(folderId), danger: true },
          ];
        }
      }
    } else if (isEditor) {
      const el = editorTextareaEl;
      const start = el?.selectionStart ?? 0;
      const end   = el?.selectionEnd   ?? 0;
      const val   = el?.value ?? '';
      const selText = val.slice(start, end);
      const hasSel  = selText.length > 0;

      const formatSubmenu = hasSel ? [
        { label: 'Bold',          action: () => applyInlineFormat(start, end, val, '**', '**') },
        { label: 'Italic',        action: () => applyInlineFormat(start, end, val, '*',  '*')  },
        { label: 'Strikethrough', action: () => applyInlineFormat(start, end, val, '~~', '~~') },
        { label: 'Inline Code',   action: () => applyInlineFormat(start, end, val, '`',  '`')  },
        { divider: true },
        { label: 'Heading 1',     action: () => applyLinePrefix(start, end, val, '# ')   },
        { label: 'Heading 2',     action: () => applyLinePrefix(start, end, val, '## ')  },
        { label: 'Heading 3',     action: () => applyLinePrefix(start, end, val, '### ') },
        { divider: true },
        { label: 'Code Block',    action: () => applyInlineFormat(start, end, val, '```\n', '\n```') },
      ] : [];

      items = [
        ...(hasSel ? [{ label: 'Format', submenu: formatSubmenu }, { divider: true }] : []),
        { label: 'Cut',   disabled: !hasSel, action: () => { navigator.clipboard.writeText(selText); editorContent = val.slice(0, start) + val.slice(end); markDirty(); } },
        { label: 'Copy',  disabled: !hasSel, action: () => navigator.clipboard.writeText(selText) },
        { label: 'Paste', action: async () => { const text = await navigator.clipboard.readText(); editorContent = val.slice(0, start) + text + val.slice(end); markDirty(); } },
        ...(hasSel ? [{ divider: true }, { label: 'Send to Chat', action: () => sendSelectionToChat() }] : []),
        { divider: true },
        { label: 'Suggest improvements', action: () => startImprove(), disabled: !editorContent || improveState.status !== 'idle' },
      ];
    }

    return items;
  }

  function applyInlineFormat(start, end, val, prefix, suffix) {
    const sel     = val.slice(start, end);
    const trimmed = sel.trimEnd();
    editorContent = val.slice(0, start) + prefix + trimmed + suffix + val.slice(end);
    markDirty();
  }

  function applyLinePrefix(start, end, val, prefix) {
    const sel        = val.slice(start, end).trimEnd();
    const needBefore = start > 0 && val[start - 1] !== '\n';
    const needAfter  = (start + sel.length) < val.length && val[start + sel.length] !== '\n';
    const prefixed   = sel.split('\n').map(line => prefix + line.replace(/^#{1,6}\s*/, '')).join('\n');
    editorContent    = val.slice(0, start)
      + (needBefore ? '\n' : '')
      + prefixed
      + (needAfter ? '\n' : '')
      + val.slice(end);
    markDirty();
  }

  async function closeOtherTabs(keepId) {
    if (isDirty) await saveNote();
    tabs = tabs.filter(t => t.id === keepId);
    if (activeTabId !== keepId) {
      activeTabId = keepId;
      const tab = tabs[0];
      if (tab?.type === 'note' && tab.noteId != null) {
        try {
          const note = await invoke('get_note', { id: tab.noteId });
          openNote(note);
        } catch { activeNote = null; editorContent = ''; editorTitle = ''; }
      } else {
        activeNote = null; editorContent = ''; editorTitle = ''; isDirty = false;
      }
    }
  }

  function startTabRenameExternal(id) {
    externalRenameTabId = id;
    setTimeout(() => { externalRenameTabId = null; }, 50);
  }

  function sendSelectionToChat() {
    let text = '';
    if (editorTextareaEl) {
      const { selectionStart, selectionEnd, value } = editorTextareaEl;
      text = value.slice(selectionStart, selectionEnd).trim();
    }
    if (!text && activeNote) text = activeNote.title;
    if (!text) return;
    layout.chatOpen = true;
    chatInsert = { text, seq: (chatInsert?.seq ?? 0) + 1 };
  }

  function insertIntoActiveNote(text) {
    if (!editorTextareaEl || !activeNote) return;
    const { selectionStart, value } = editorTextareaEl;
    editorContent = value.slice(0, selectionStart) + '\n\n' + text + '\n\n' + value.slice(selectionStart);
    markDirty();
  }

  // ── Improve note ─────────────────────────────────────────────────────────────────

  /** @type {import('@tauri-apps/api/event').UnlistenFn | null} */
  let improveUnlisten = null;

  // Accumulated improved text — lives outside improveState so the Tauri event
  // listener always captures the latest value via a local variable.
  let improveAccumulated = $state('');

  /** @type {import('@tauri-apps/api/event').UnlistenFn | null} */
  let refineUnlisten = null;

  // Accumulated refined hunk text — same pattern as improveAccumulated.
  let refineAccumulated = '';

  async function handleImproveStart(instruction) {
    if (!activeNote) return;
    improveAccumulated = '';
    improveState = { status: 'streaming', instruction, improvedText: '', hunks: [], originalContent: editorContent, acceptedIndices: [], rejectedIndices: [] };

    const unlistenP = listen('note:improve-token', (event) => {
      const token = /** @type {string} */ (event.payload);
      improveAccumulated += token;
      improveState = { ...improveState, improvedText: improveAccumulated };
    });
    const unlisten = await unlistenP;
    improveUnlisten = unlisten;

    try {
      const model = await invoke('get_setting', { key: 'chat_model' }) || 'llama3.2';
      const temperature = await invoke('get_setting', { key: 'chat_temperature' });
      const topP = await invoke('get_setting', { key: 'chat_top_p' });
      const topK = await invoke('get_setting', { key: 'chat_top_k' });
      const repeatPenalty = await invoke('get_setting', { key: 'chat_repeat_penalty' });
      const numCtx = await invoke('get_setting', { key: 'chat_num_ctx' });
      await invoke('suggest_note_improvement', {
        model, noteContent: editorContent, instruction,
        noteId: activeNote?.id ?? null,
        noteTitle: activeNote?.title ?? null,
        temperature: temperature !== '' ? Number(temperature) : 0.8,
        topP: topP !== '' ? Number(topP) : 0.9,
        topK: topK !== '' ? Number(topK) : 40,
        repeatPenalty: repeatPenalty !== '' ? Number(repeatPenalty) : 1.1,
        numCtx: numCtx !== '' ? Number(numCtx) : 8192,
      });
      unlisten();
      improveUnlisten = null;
      const hunks = computeDiff(improveState.originalContent, improveAccumulated);
      improveState = { ...improveState, status: 'diff', hunks, acceptedIndices: [], rejectedIndices: [] };
    } catch (e) {
      unlisten();
      improveUnlisten = null;
      showError(e);
      improveState = { status: 'idle', instruction: '', improvedText: '', hunks: [], originalContent: '', acceptedIndices: [], rejectedIndices: [] };
    }
  }

  function allChangedIndices(hunks) {
    return hunks
      .map((h, i) => h.type !== 'unchanged' ? i : -1)
      .filter(i => i !== -1);
  }

  function pairedHunkIndex(hunks, index) {
    const hunk = hunks[index];
    if (!hunk || hunk.type === 'unchanged') return -1;
    if (hunk.type === 'remove') {
      for (let i = index + 1; i < hunks.length; i++) {
        if (hunks[i].type !== 'unchanged') return hunks[i].type === 'add' ? i : -1;
      }
    } else {
      for (let i = index - 1; i >= 0; i--) {
        if (hunks[i].type !== 'unchanged') return hunks[i].type === 'remove' ? i : -1;
      }
    }
    return -1;
  }

  function applyAndClose(state) {
    const { hunks, originalContent, improvedText, acceptedIndices: accepted, rejectedIndices } = state;
    const allChanged = allChangedIndices(hunks);
    const undecided = allChanged.filter(i => !accepted.includes(i) && !rejectedIndices.includes(i));
    const finalAccepted = new Set([...accepted, ...undecided]);
    if (finalAccepted.size > 0) {
      if (finalAccepted.size === allChanged.length && rejectedIndices.length === 0 && accepted.length === 0) {
        editorContent = improvedText;
      } else {
        editorContent = applyAcceptedHunks(hunks, finalAccepted, originalContent, improvedText);
      }
      markDirty();
    }
    improveState = { status: 'idle', instruction: '', improvedText: '', hunks: [], originalContent: '', acceptedIndices: [], rejectedIndices: [] };
  }

  function handleImproveAcceptAll() {
    applyAndClose(improveState);
  }

  function handleImproveRejectAll() {
    improveState = { status: 'idle', instruction: '', improvedText: '', hunks: [], originalContent: '', acceptedIndices: [], rejectedIndices: [] };
  }

  function handleImproveAcceptHunk(hunkIndex) {
    const hunk = improveState.hunks[hunkIndex];
    if (!hunk || hunk.type === 'unchanged') return;
    if (improveState.acceptedIndices.includes(hunkIndex)) return;
    const pair = pairedHunkIndex(improveState.hunks, hunkIndex);
    const indices = pair !== -1 && !improveState.acceptedIndices.includes(pair) && !improveState.rejectedIndices.includes(pair)
      ? [hunkIndex, pair] : [hunkIndex];
    const newAccepted = [...improveState.acceptedIndices, ...indices];
    const allChanged = allChangedIndices(improveState.hunks);
    const allDecided = allChanged.every(i => newAccepted.includes(i) || improveState.rejectedIndices.includes(i));
    if (allDecided) {
      applyAndClose({ ...improveState, acceptedIndices: newAccepted });
    } else {
      improveState = { ...improveState, acceptedIndices: newAccepted };
    }
  }

  function handleImproveRejectHunk(hunkIndex) {
    const hunk = improveState.hunks[hunkIndex];
    if (!hunk || hunk.type === 'unchanged') return;
    if (improveState.rejectedIndices.includes(hunkIndex)) return;
    const pair = pairedHunkIndex(improveState.hunks, hunkIndex);
    const indices = pair !== -1 && !improveState.rejectedIndices.includes(pair) && !improveState.acceptedIndices.includes(pair)
      ? [hunkIndex, pair] : [hunkIndex];
    const newRejected = [...improveState.rejectedIndices, ...indices];
    const allChanged = allChangedIndices(improveState.hunks);
    const allDecided = allChanged.every(i => improveState.acceptedIndices.includes(i) || newRejected.includes(i));
    if (allDecided) {
      applyAndClose({ ...improveState, rejectedIndices: newRejected });
    } else {
      improveState = { ...improveState, rejectedIndices: newRejected };
    }
  }

  function startImprove() {
    if (improveState.status !== 'idle') return;
    improveState = { ...improveState, status: 'prompt' };
  }

  function handleRefineHunk(hunkIndex, x, y) {
    refineState = { status: 'prompt', hunkIndex, x, y };
  }

  function handleRefineCancel() {
    if (refineUnlisten) { refineUnlisten(); refineUnlisten = null; }
    refineState = { status: 'idle', hunkIndex: null, x: 0, y: 0 };
  }

  async function handleRefineSend(instruction) {
    const { hunkIndex } = refineState;
    const hunks = improveState.hunks;
    const hunk = hunks[hunkIndex];
    if (!hunk) return;

    // Determine what text to send to the LLM.
    // Prefer the original (remove) lines so the LLM works on the pre-improve text.
    // Fall back to the add lines for pure additions.
    let hunkContent;
    if (hunk.type === 'add') {
      const removePair = pairedHunkIndex(hunks, hunkIndex);
      hunkContent = (removePair !== -1 ? hunks[removePair].lines : hunk.lines).join('\n');
    } else {
      hunkContent = hunk.lines.join('\n');
    }

    refineAccumulated = '';
    refineState = { ...refineState, status: 'streaming' };

    const unlistenP = listen('note:refine-hunk-token', (event) => {
      refineAccumulated += /** @type {string} */ (event.payload);
    });
    const unlisten = await unlistenP;
    refineUnlisten = unlisten;

    try {
      const model = await invoke('get_setting', { key: 'chat_model' }) || 'llama3.2';
      const temperature = await invoke('get_setting', { key: 'chat_temperature' });
      const topP = await invoke('get_setting', { key: 'chat_top_p' });
      const topK = await invoke('get_setting', { key: 'chat_top_k' });
      const repeatPenalty = await invoke('get_setting', { key: 'chat_repeat_penalty' });
      const numCtx = await invoke('get_setting', { key: 'chat_num_ctx' });
      await invoke('suggest_hunk_refinement', {
        model, hunkContent, instruction,
        temperature: temperature !== '' ? Number(temperature) : 0.8,
        topP: topP !== '' ? Number(topP) : 0.9,
        topK: topK !== '' ? Number(topK) : 40,
        repeatPenalty: repeatPenalty !== '' ? Number(repeatPenalty) : 1.1,
        numCtx: numCtx !== '' ? Number(numCtx) : 8192,
      });
      unlisten();
      refineUnlisten = null;

      // Find the add hunk to replace (either the clicked hunk if it's an add,
      // or the paired add if the clicked hunk is a remove).
      const addIndex = hunk.type === 'add' ? hunkIndex : pairedHunkIndex(hunks, hunkIndex);
      if (addIndex !== -1) {
        const newHunks = hunks.map((h, i) =>
          i === addIndex ? { ...h, lines: refineAccumulated.split('\n') } : h
        );
        improveState = { ...improveState, hunks: newHunks };
      }
    } catch (e) {
      unlisten();
      refineUnlisten = null;
      showError(e);
    }

    refineState = { status: 'idle', hunkIndex: null, x: 0, y: 0 };
  }

  // ── Helpers ─────────────────────────────────────────────────────────────────────

  function showError(e) {
    errorMsg = String(e);
    setTimeout(() => (errorMsg = ''), 4000);
  }

  function markDirty() { isDirty = true; }

  // ── Data loading ────────────────────────────────────────────────────────────────

  async function loadFolders() {
    try {
      folders = await invoke('list_folders');
    } catch (e) {
      showError(e);
    }
  }

  async function loadAllTags() {
    try {
      allTags = await invoke('list_all_tags');
    } catch { /* non-fatal */ }
  }

  async function loadNotes() {
    try {
      if (tagFilter) {
        notes = await invoke('list_notes_by_tag', { tag: tagFilter });
      } else if (selectedFolderId === 'all') {
        notes = await invoke('list_notes', { all: true });
      } else {
        notes = await invoke('list_notes', { folderId: selectedFolderId ?? null, all: false });
      }
    } catch (e) {
      showError(e);
    }
  }

  onMount(async () => {
    try {
      const [locked, hasPw] = await Promise.all([
        invoke('is_vault_locked'),
        invoke('vault_has_password'),
      ]);
      vaultLocked = locked;
      vaultHasPassword = hasPw;
    } catch { /* treat as unlocked */ }
    lockCheckDone = true;

    if (!vaultLocked) {
      await Promise.all([loadFolders(), loadNotes(), restoreTabs()]);
      if (tabs.length === 0) newTab();
      loadAllTags();
      tmpl.loadTemplates();
      bm.loadBookmarks();

      invoke('get_hardware_info')
        .then(hw => { settings.hwCapability = hw.capability; settings.llmForceEnabled = hw.llmForceEnabled; })
        .catch(() => {});

      invoke('get_setting', { key: 'wikipedia_enabled' })
        .then(v => { settings.wikipediaEnabled = v === 'true'; })
        .catch(() => {});
    }

    await tick();
    await getCurrentWindow().show();

    // Refresh notes when the file scanner imports a file as a note.
    window.addEventListener('grimoire:note-imported', (e) => {
      loadNotes();
    });

    // Navigate directly to a note (used by file scanner's "View note" button).
    window.addEventListener('grimoire:navigate-note', async (e) => {
      const noteId = /** @type {CustomEvent} */ (e).detail?.noteId;
      if (!noteId) return;
      try {
        const note = await invoke('get_note', { id: noteId });
        settingsOpen = false;
        navigateToNote(note);
      } catch { /* ignore */ }
    });
  });

  // ── Folder actions ──────────────────────────────────────────────────────────────

  async function startFolderInline() {
    try {
      const parentId = typeof selectedFolderId === 'number' ? selectedFolderId : null;
      const folder = await invoke('create_folder', { name: 'Untitled', parentId });
      await loadFolders();
      // Expand the parent so the new folder is immediately visible.
      if (parentId) folderExpanded = { ...folderExpanded, [parentId]: true };
      inlineRenaming = { id: folder.id, type: 'folder', value: 'Untitled' };
    } catch (e) {
      showError(e);
    }
  }

  async function confirmInlineRename() {
    if (!inlineRenaming) return;
    const { id, type, value } = inlineRenaming;
    const name = value.trim() || 'Untitled';
    inlineRenaming = null;
    try {
      if (type === 'folder') {
        await invoke('rename_folder', { id, name });
        await loadFolders();
      } else {
        await invoke('rename_note', { id, name });
        await loadNotes();
        if (activeNote?.id === id) {
          editorTitle = name;
          activeNote = { ...activeNote, title: name };
          tabs = tabs.map(t => t.noteId === id ? { ...t, label: name } : t);
        }
      }
    } catch (e) {
      showError(e);
    }
  }

  async function revealFolder(folderId) {
    if (!layout.foldersOpen) layout.foldersOpen = true;
    // Walk ancestors upward and expand each one so the folder is in the DOM.
    const newExpanded = { ...folderExpanded };
    let current = folders.find(f => f.id === folderId);
    while (current?.parent_id) {
      newExpanded[current.parent_id] = true;
      current = folders.find(f => f.id === current.parent_id);
    }
    folderExpanded = newExpanded;
    selectFolder(folderId);
    await tick();
    document.querySelector(`[data-folder-id="${folderId}"]`)
      ?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }

  async function deleteFolder(id) {
    const folder = folders.find(f => f.id === id);
    folderDeletePending = { id, name: folder?.name ?? 'this folder' };
  }

  async function confirmDeleteFolder() {
    const id = folderDeletePending.id;
    folderDeletePending = null;
    try {
      await invoke('delete_folder', { id });
      if (selectedFolderId === id) selectedFolderId = null;
      await loadFolders();
      await loadNotes();
    } catch (e) {
      showError(e);
    }
  }

  async function selectFolder(id) {
    if (isDirty) await saveNote();
    selectedFolderId = id;
    tagFilter = null;
    activeNote = null;
    tableViewOpen = false;
    await loadNotes();
    if (id && id !== 'all') {
      invoke('get_property_defs', { folderId: id })
        .then(defs => { folderHasProperties = defs.length > 0; })
        .catch(() => { folderHasProperties = false; });
    } else {
      folderHasProperties = false;
    }
  }

  // ── Note actions ─────────────────────────────────────────────────────────────────

  async function handleImportNote() {
    const selected = await openFileDialog({
      directory: false,
      multiple: false,
      filters: [{ name: 'Supported files', extensions: ['txt', 'md', 'pdf'] }],
    }).catch(() => null);
    if (!selected) return;
    const filePath = Array.isArray(selected) ? selected[0] : selected;
    const folderId = selectedFolderId === 'all' ? null : (selectedFolderId ?? null);
    try {
      const note = await invoke('import_file_as_note', { filePath, folderId });
      await loadNotes();
      navigateToNote(note);
      // Vector-index the note, then register it in the file scanner.
      // Use .finally() so add_scanned_path always runs even if index_note fails
      // (e.g. large PDFs with many chunks can time out or exceed Ollama capacity).
      // Both operations use the embed model so they must be sequential, not concurrent.
      invoke('index_note', { noteId: note.id, title: note.title, content: note.content })
        .catch(() => {})
        .finally(() => {
          invoke('add_scanned_path', { path: filePath, kind: 'file' }).catch(() => {});
        });
    } catch (e) {
      showError(e);
    }
  }

  async function startNoteInline(templateId = -1) {
    layout.notesOpen = true;
    const folderId = selectedFolderId === 'all' ? null : (selectedFolderId ?? null);
    try {
      const note = await invoke('create_note', { title: 'Untitled', folderId });
      await loadNotes();
      if (folderId && templateId > 0) {
        try {
          const defs = await invoke('apply_template_to_note', { noteId: note.id, folderId, templateId });
          folderHasProperties = defs.length > 0;
        } catch { /* non-fatal */ }
      }
      navigateToNote(note);
      const template = tmpl.templates.find(t => t.id === templateId);
      const templateContent = template?.content ?? '';
      if (templateContent) {
        editorContent = templateContent;
        isDirty = true;
        invoke('update_note', { id: note.id, title: 'Untitled', content: templateContent }).catch(() => {});
      }
      indexState = 'indexing';
      invoke('index_note', { noteId: note.id, title: 'Untitled', content: templateContent })
        .then(() => { indexState = 'idle'; })
        .catch(() => { indexState = 'error'; });
      inlineRenaming = { id: note.id, type: 'note', value: 'Untitled' };
    } catch (e) {
      showError(e);
    }
  }

  function openNote(note) {
    if (isDirty && activeNote && activeNote.id !== note.id) saveNote();
    enhanceStateCancelIfDiff();
    searchOpen = false;
    activeNote = note;
    editorTitle = note.title;
    editorContent = note.content;
    isDirty = false;
    propertiesReady = !note.folder_id;
    noteTags = [];
    noteLinks = [];
    noteBacklinks = [];
    unlinkedMentions = [];
    invoke('get_note_tags', { noteId: note.id }).then(t => (noteTags = t)).catch(() => {});
    invoke('get_note_links', { noteId: note.id }).then(l => (noteLinks = l)).catch(() => {});
    invoke('get_backlinks', { noteId: note.id }).then(b => (noteBacklinks = b)).catch(() => {});
    invoke('get_unlinked_mentions', { noteId: note.id, title: note.title }).then(m => (unlinkedMentions = m)).catch(() => {});
  }

  function toggleReadMode() {
    if (!activeTabId) return;
    tabs = tabs.map(t => t.id === activeTabId ? { ...t, readMode: !t.readMode } : t);
  }

  function navigateToNote(note) {
    if (isDirty) saveNote();
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
    openNote(note);
  }

  function openNoteInNewTab(note) {
    if (isDirty) saveNote();
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'note', noteId: note.id, label: note.title, customLabel: null, readMode: false }];
    activeTabId = id;
    openNote(note);
  }

  function newTab() {
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'note', noteId: null, label: 'New Tab', customLabel: null, readMode: false }];
    activeTabId = id;
    activeNote = null;
    editorTitle = '';
    editorContent = '';
    isDirty = false;
    noteTags = [];
    noteLinks = [];
    noteBacklinks = [];
  }

  async function closeNote() {
    if (improveState.status !== 'idle') enhanceStateCancelIfDiff();
    if (isDirty) await saveNote();
    tabs = tabs.map(t => t.id === activeTabId ? { ...t, noteId: null, label: 'New Tab' } : t);
    activeNote = null;
    editorTitle = '';
    editorContent = '';
    isDirty = false;
    noteTags = [];
    noteLinks = [];
    noteBacklinks = [];
  }

  function enhanceStateCancelIfDiff() {
    if (improveState.status === 'diff' || improveState.status === 'prompt' || improveState.status === 'streaming') {
      if (improveUnlisten) { improveUnlisten(); improveUnlisten = null; }
      improveState = { status: 'idle', instruction: '', improvedText: '', hunks: [], originalContent: '', acceptedIndices: [], rejectedIndices: [] };
    }
    if (refineState.status !== 'idle') {
      if (refineUnlisten) { refineUnlisten(); refineUnlisten = null; }
      refineState = { status: 'idle', hunkIndex: null, x: 0, y: 0 };
    }
  }

  async function activateTab(id) {
    if (activeTabId === id) return;
    enhanceStateCancelIfDiff();
    if (isDirty) await saveNote();
    activeTabId = id;
    const tab = tabs.find(t => t.id === id);
    if (!tab) return;
    if (tab.type === 'note' && tab.noteId != null) {
      try {
        const note = await invoke('get_note', { id: tab.noteId });
        openNote(note);
      } catch (e) {
        showError(e);
        closeTab(id);
      }
    } else {
      activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false;
      noteTags = []; noteLinks = []; noteBacklinks = [];
    }
  }

  async function closeTab(id) {
    const idx = tabs.findIndex(t => t.id === id);
    if (idx === -1) return;
    if (id === activeTabId && isDirty) await saveNote();
    const newTabs = tabs.filter(t => t.id !== id);
    tabs = newTabs;
    if (activeTabId === id) {
      if (newTabs.length === 0) {
        newTab();
      } else {
        const next = newTabs[idx] ?? newTabs[idx - 1];
        activeTabId = next.id;
        if (next.type === 'note' && next.noteId != null) {
          try {
            const note = await invoke('get_note', { id: next.noteId });
            openNote(note);
          } catch {
            activeNote = null; editorTitle = ''; editorContent = '';
          }
        } else {
          activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false;
          noteTags = []; noteLinks = []; noteBacklinks = [];
        }
      }
    }
  }

  function openGraphTab() {
    if (isDirty) saveNote();
    const existing = tabs.find(t => t.type === 'graph');
    if (existing) { activeTabId = existing.id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false; return; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'graph', noteId: null, label: 'Graph', customLabel: null }];
    activeTabId = id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false;
  }

  function openCalendarTab() {
    if (isDirty) saveNote();
    const existing = tabs.find(t => t.type === 'calendar');
    if (existing) { activeTabId = existing.id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false; return; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'calendar', noteId: null, label: 'Calendar', customLabel: null }];
    activeTabId = id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false;
  }

  function openKanbanTab(folderId, folderName) {
    if (isDirty) saveNote();
    selectedFolderId = folderId;
    tagFilter = null;
    tableViewOpen = false;
    loadNotes();
    invoke('get_property_defs', { folderId }).then(d => { folderHasProperties = d.length > 0; }).catch(() => {});
    const existing = tabs.find(t => t.type === 'kanban' && t.folderId === folderId);
    if (existing) { activeTabId = existing.id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false; return; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'kanban', noteId: null, label: `Kanban — ${folderName}`, customLabel: null, folderId }];
    activeTabId = id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false;
  }

  function openWikipediaArticle(bundleId, articlePath, title) {
    if (isDirty) saveNote();
    const existing = tabs.find(t => t.type === 'wikipedia' && t.bundleId === bundleId && t.articlePath === articlePath);
    if (existing) { activeTabId = existing.id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false; return; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'wikipedia', noteId: null, label: title || 'Wikipedia', customLabel: null, bundleId, articlePath }];
    activeTabId = id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false;
  }

  function updateWikipediaTab(bundleId, articlePath, title) {
    tabs = tabs.map(t => t.id === activeTabId ? { ...t, bundleId, articlePath, label: title || t.label, customLabel: t.customLabel } : t);
  }

  function openChatTab() {
    if (isDirty) saveNote();
    const existing = tabs.find(t => t.type === 'chat');
    if (existing) { activeTabId = existing.id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false; return; }
    const id = makeTabId();
    tabs = [...tabs, { id, type: 'chat', noteId: null, label: 'Chat', customLabel: null }];
    activeTabId = id; activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false;
  }

  function renameTab(id, label) {
    tabs = tabs.map(t => t.id === id ? { ...t, customLabel: label || null } : t);
  }

  async function createDailyNote() {
    try {
      const note = await invoke('create_daily_note', { dateFormat: settings.dailyNoteFormat });
      await loadNotes();
      openNoteInNewTab(note);
    } catch (e) {
      showError(e);
    }
  }

  async function restoreTabs() {
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
          }
          return null;
        })
      );

      const valid = results.filter(Boolean);
      if (valid.length === 0) return;
      tabs = valid.map(r => r.tab);
      const target = valid.find(r => r.tab.id === savedActiveId) ?? valid[valid.length - 1];
      activeTabId = target.tab.id;
      if (target.tab.type === 'note' && target.note) {
        openNote(target.note);
      }
    } catch { /* start fresh */ }
  }

  async function saveNote() {
    if (!activeNote) return;
    try {
      const updated = await invoke('update_note', {
        id: activeNote.id,
        title: editorTitle,
        content: editorContent,
      });
      activeNote = updated;
      isDirty = false;
      await loadNotes();
      indexState = 'indexing';
      invoke('index_note', { noteId: updated.id, title: editorTitle, content: editorContent })
        .then(() => { indexState = 'idle'; })
        .catch(() => { indexState = 'error'; });
      invoke('sync_note_relations', { noteId: updated.id, content: editorContent })
        .then(() => Promise.all([
          invoke('get_note_tags', { noteId: updated.id }),
          invoke('get_note_links', { noteId: updated.id }),
          invoke('get_backlinks', { noteId: updated.id }),
          invoke('get_unlinked_mentions', { noteId: updated.id, title: editorTitle }),
          invoke('list_all_tags'),
        ]))
        .then(([tags, links, backlinks, mentions, updatedAllTags]) => {
          noteTags = tags;
          noteLinks = links;
          noteBacklinks = backlinks;
          unlinkedMentions = mentions;
          allTags = updatedAllTags;
        })
        .catch(() => {});
    } catch (e) {
      showError(e);
    }
  }

  async function convertMention(mention) {
    if (!activeNote) return;
    try {
      const updatedContent = await invoke('convert_mention_to_link', {
        noteId: mention.id,
        title: activeNote.title,
      });
      if (activeNote.id === mention.id) {
        editorContent = updatedContent;
      }
      const [mentions, backlinks] = await Promise.all([
        invoke('get_unlinked_mentions', { noteId: activeNote.id, title: activeNote.title }),
        invoke('get_backlinks', { noteId: activeNote.id }),
      ]);
      unlinkedMentions = mentions;
      noteBacklinks = backlinks;
    } catch (e) {
      showError(e);
    }
  }

  async function deleteNote(id) {
    const note = notes.find(n => n.id === id) ?? activeNote;
    noteDeletePending = { id, title: note?.title ?? 'this note' };
  }

  async function confirmDeleteNote() {
    const id = noteDeletePending.id;
    noteDeletePending = null;
    try {
      await invoke('delete_note', { id });
      const tab = tabs.find(t => t.type === 'note' && t.noteId === id);
      if (tab) await closeTab(tab.id);
      else if (activeNote?.id === id) {
        activeNote = null; editorTitle = ''; editorContent = '';
        noteTags = []; noteLinks = []; noteBacklinks = [];
      }
      await loadNotes();
      bm.loadBookmarks();
      invoke('remove_note_index', { noteId: id }).catch(() => {});
    } catch (e) {
      showError(e);
    }
  }

  async function moveNote(noteId, targetFolderId) {
    try {
      await invoke('move_note', { id: noteId, folderId: targetFolderId });
      await loadNotes();
    } catch (e) {
      showError(e);
    }
  }

  function onNoteDragStart(e, note) {
    e.dataTransfer.setData('text/plain', String(note.id));
    e.dataTransfer.effectAllowed = 'move';
    isDragging = true;
  }

  function onNoteDragEnd() {
    isDragging = false;
    dragOverFolderId = null;
  }

  async function openNoteById(id) {
    try {
      const note = await invoke('get_note', { id });
      navigateToNote(note);
    } catch (e) {
      showError(e);
    }
  }

  async function filterByTag(tag) {
    tagFilter = tag;
    selectedFolderId = null;
    await loadNotes();
  }

  async function clearTagFilter() {
    tagFilter = null;
    await loadNotes();
  }

  // ── Lock / unlock ───────────────────────────────────────────────────────────────

  async function onVaultUnlocked() {
    vaultLocked = false;
    await loadFolders();
    await loadNotes();
    loadAllTags();
    await restoreTabs();
    if (tabs.length === 0) newTab();
    invoke('reindex_all').catch(() => {});
  }

  async function lockVault() {
    if (!vaultHasPassword) return;
    try {
      await invoke('lock_vault');
      vaultLocked = true;
      activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false;
      tabs = []; activeTabId = null;
      notes = []; folders = []; allTags = [];
    } catch (e) {
      showError(e);
    }
  }

  async function handleVaultPwSubmit(password) {
    if (vaultPwModal === 'set' || vaultPwModal === 'change') {
      await invoke('set_vault_password', { password });
      vaultHasPassword = true;
      vaultPwModal = null;
      invoke('reindex_all').catch(() => {});
    } else if (vaultPwModal === 'remove') {
      await invoke('remove_vault_password', { password });
      vaultHasPassword = false;
      vaultPwModal = null;
    }
    return true;
  }

  function requestFolderUnlock(folder) {
    folderUnlockTarget = folder;
  }

  async function handleFolderUnlockSafe(password) {
    if (!folderUnlockTarget) return false;
    const targetId = folderUnlockTarget.id;
    const ok = await invoke('unlock_folder', { folderId: targetId, password });
    if (ok) {
      folderUnlockTarget = null;
      unlockedFolderIds = new Set([...unlockedFolderIds, targetId]);
      await loadFolders();
      await loadNotes();
      invoke('list_notes', { folderId: targetId })
        .then(ns => { for (const n of ns) invoke('index_note', { noteId: n.id, title: n.title, content: n.content }).catch(() => {}); })
        .catch(() => {});
    }
    return ok;
  }

  async function handleFolderPwSubmit(password) {
    if (!folderPwModal) return true;
    if (folderPwModal.mode === 'set') {
      await invoke('set_folder_password', { folderId: folderPwModal.folderId, password });
      if (activeNote?.folder_id === folderPwModal.folderId) {
        activeNote = null; editorTitle = ''; editorContent = '';
      }
      const next = new Set(unlockedFolderIds);
      next.delete(folderPwModal.folderId);
      unlockedFolderIds = next;
    } else if (folderPwModal.mode === 'remove') {
      await invoke('remove_folder_password', { folderId: folderPwModal.folderId, password });
      const next = new Set(unlockedFolderIds);
      next.delete(folderPwModal.folderId);
      unlockedFolderIds = next;
      invoke('list_notes', { folderId: folderPwModal.folderId })
        .then(ns => { for (const n of ns) invoke('index_note', { noteId: n.id, title: n.title, content: n.content }).catch(() => {}); })
        .catch(() => {});
    }
    folderPwModal = null;
    await loadFolders();
    await loadNotes();
    return true;
  }

  // ── Keyboard shortcuts ──────────────────────────────────────────────────────────

  function handleEditorKeydown(e) {
    if (e.key !== '[') return;
    const el = /** @type {HTMLTextAreaElement} */ (e.currentTarget);
    const { selectionStart: start, selectionEnd: end, value } = el;
    const prevChar = value[start - 1];
    e.preventDefault();
    if (prevChar === '[') {
      const before = value.slice(0, start - 1);
      const after  = value.slice(end + (value[end] === ']' ? 1 : 0));
      const cursor = before.length + 2;
      editorContent = before + '[[]]' + after;
      markDirty();
      requestAnimationFrame(() => { el.selectionStart = cursor; el.selectionEnd = cursor; });
    } else {
      const before = value.slice(0, start);
      const after  = value.slice(end);
      const cursor = before.length + 1;
      editorContent = before + '[]' + after;
      markDirty();
      requestAnimationFrame(() => { el.selectionStart = cursor; el.selectionEnd = cursor; });
    }
  }

  function handleKeydown(e) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'p' && !e.shiftKey && !e.altKey) {
      e.preventDefault(); quickSwitcherOpen = true;
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'f' && !e.shiftKey && !e.altKey) {
      e.preventDefault(); searchOpen = true;
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'n' && !e.shiftKey && !e.altKey) {
      e.preventDefault(); startNoteInline();
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 't' && !e.shiftKey && !e.altKey) {
      e.preventDefault(); newTab();
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'Tab' && tabs.length > 1) {
      e.preventDefault();
      const idx = tabs.findIndex(t => t.id === activeTabId);
      const next = e.shiftKey
        ? tabs[(idx - 1 + tabs.length) % tabs.length]
        : tabs[(idx + 1) % tabs.length];
      activateTab(next.id);
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'w' && !e.shiftKey && !e.altKey) {
      e.preventDefault(); if (activeTabId) closeTab(activeTabId);
    }
    if (e.key === 'Delete' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const tag = /** @type {HTMLElement} */ (document.activeElement)?.tagName ?? '';
      const isEditing = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT'
        || /** @type {HTMLElement} */ (document.activeElement)?.isContentEditable;
      if (!isEditing && activeNote && !activeNote.locked) {
        e.preventDefault(); deleteNote(activeNote.id);
      }
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 's') {
      e.preventDefault(); saveNote();
    }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'L') {
      e.preventDefault(); if (vaultHasPassword && !vaultLocked) lockVault();
    }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'Enter') {
      e.preventDefault(); sendSelectionToChat();
    }
    if (e.key === 'F11' && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
      e.preventDefault(); layout.toggleFocusMode();
    }
    if (e.key === 'Escape' && layout.focusMode) {
      const noModal = !settingsOpen && !searchOpen && !quickSwitcherOpen
        && !tmpl.templateModalOpen && !noteDeletePending && !folderDeletePending
        && !vaultPwModal && !folderPwModal && !folderUnlockTarget;
      if (noModal) { e.preventDefault(); layout.toggleFocusMode(); }
    }
  }

  async function seedNotes() {
    isSeeding = true;
    try {
      const n = await invoke('seed_notes');
      await loadNotes();
      showError(`✓ Seeded ${n} notes and indexed them.`);
    } catch (e) {
      showError(e);
    } finally {
      isSeeding = false;
    }
  }

  async function reindexAll() {
    isReindexing = true;
    try {
      const msg = await invoke('reindex_all');
      showError(`✓ ${msg}.`);
    } catch (e) {
      showError(e);
    } finally {
      isReindexing = false;
    }
  }

</script>

<svelte:window onkeydown={handleKeydown} />
<svelte:document onmousemove={layout.onDragMove} onmouseup={layout.onDragEnd} />

{#if !lockCheckDone}
  <!-- Blank while we check vault lock state to avoid a flash of content -->
{:else if vaultLocked}
  <LockScreen onUnlocked={onVaultUnlocked} />
{:else}

{#if errorMsg}
  <div class="error-banner" role="alert">{errorMsg}</div>
{/if}

<!-- Password modals (rendered above everything) -->
{#if folderUnlockTarget}
  <PasswordModal
    title="Locked folder"
    confirmLabel="Unlock"
    onSubmit={handleFolderUnlockSafe}
    onCancel={() => (folderUnlockTarget = null)}
  />
{/if}

{#if vaultPwModal === 'set'}
  <PasswordModal
    title="Set vault password"
    confirmLabel="Set password"
    warning="If you forget this password, your notes cannot be recovered. There is no reset option."
    requireAck={true}
    onSubmit={handleVaultPwSubmit}
    onCancel={() => (vaultPwModal = null)}
  />
{:else if vaultPwModal === 'change'}
  <PasswordModal
    title="Change vault password"
    confirmLabel="Set new password"
    warning="If you forget this password, your notes cannot be recovered. There is no reset option."
    requireAck={true}
    onSubmit={handleVaultPwSubmit}
    onCancel={() => (vaultPwModal = null)}
  />
{:else if vaultPwModal === 'remove'}
  <PasswordModal
    title="Remove vault password"
    confirmLabel="Remove password"
    onSubmit={handleVaultPwSubmit}
    onCancel={() => (vaultPwModal = null)}
  />
{/if}

{#if folderPwModal?.mode === 'set'}
  <PasswordModal
    title="Set folder password"
    confirmLabel="Set password"
    warning="If you forget this password, notes in this folder cannot be recovered."
    requireAck={true}
    onSubmit={handleFolderPwSubmit}
    onCancel={() => (folderPwModal = null)}
  />
{:else if folderPwModal?.mode === 'remove'}
  <PasswordModal
    title="Remove folder password"
    confirmLabel="Remove password"
    onSubmit={handleFolderPwSubmit}
    onCancel={() => (folderPwModal = null)}
  />
{/if}

{#if tmpl.templateModalOpen}
  <TemplateModal onSave={tmpl.saveTemplate} onCancel={() => (tmpl.templateModalOpen = false)} />
{:else if tmpl.editingTemplate}
  <TemplateModal template={tmpl.editingTemplate} onSave={tmpl.updateTemplate} onCancel={() => (tmpl.editingTemplate = null)} />
{/if}

{#if noteDeletePending}
  <ConfirmModal
    title="Delete note"
    message={'Are you sure you want to delete \u201c' + noteDeletePending.title + '\u201d?'}
    confirmLabel="Delete"
    onConfirm={confirmDeleteNote}
    onCancel={() => (noteDeletePending = null)}
  />
{/if}

{#if folderDeletePending}
  <ConfirmModal
    title="Delete folder"
    message={'Are you sure you want to delete \u201c' + folderDeletePending.name + '\u201d? Notes inside will become unfiled.'}
    confirmLabel="Delete"
    onConfirm={confirmDeleteFolder}
    onCancel={() => (folderDeletePending = null)}
  />
{/if}

<!-- ── Activity bar ───────────────────────────────────────────────────── -->
<ActivityBar
  searchActive={searchOpen}
  showLock={vaultHasPassword}
  onSearch={() => (searchOpen = !searchOpen)}
  onGraph={openGraphTab}
  onCalendar={openCalendarTab}
  onDailyNote={createDailyNote}
  onQuickSwitcher={() => (quickSwitcherOpen = true)}
  wikipediaEnabled={settings.wikipediaEnabled}
  onWikipedia={() => (wikiSearchOpen = true)}
  onLock={lockVault}
  onSettings={() => (settingsOpen = true)}
  onHelp={() => openUrl('https://grimoire.app')}
  onForum={() => openUrl('https://grimoire.app/forum')}
/>

<!-- ── Custom title bar ─────────────────────────────────────────────── -->
<div class="titlebar">
  <div class="titlebar-left">
    <button class="titlebar-btn" onclick={() => (layout.foldersOpen = !layout.foldersOpen)} title="Toggle folders">
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
        <rect x="1" y="1" width="13" height="13" rx="1"/>
        <line x1="5" y1="1" x2="5" y2="14"/>
      </svg>
    </button>
    <button class="titlebar-btn" onclick={() => (layout.notesOpen = !layout.notesOpen)} title="Toggle notes list">
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
        <line x1="4" y1="4" x2="11" y2="4"/>
        <line x1="4" y1="7.5" x2="11" y2="7.5"/>
        <line x1="4" y1="11" x2="9" y2="11"/>
      </svg>
    </button>
  </div>

  <!-- Tab strip — replaces the old "Grimoire" drag region -->
  <TabBar
    {tabs}
    {activeTabId}
    onActivate={activateTab}
    onClose={closeTab}
    onRename={renameTab}
    onNew={newTab}
    renameRequestId={externalRenameTabId}
  />

  <div class="titlebar-right">
    <button
      class="titlebar-btn"
      class:titlebar-btn-active={layout.focusMode}
      onclick={layout.toggleFocusMode}
      title="Focus mode (F11)"
    >
      <!-- Compress icon when in focus mode, expand icon when normal -->
      {#if layout.focusMode}
        <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="14,6 14,1 9,1"/>
          <polyline points="1,9 1,14 6,14"/>
          <line x1="14" y1="1" x2="9" y2="6"/>
          <line x1="1" y1="14" x2="6" y2="9"/>
        </svg>
      {:else}
        <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="1,6 1,1 6,1"/>
          <polyline points="9,14 14,14 14,9"/>
          <line x1="1" y1="1" x2="6" y2="6"/>
          <line x1="14" y1="14" x2="9" y2="9"/>
        </svg>
      {/if}
    </button>
    <button
      class="titlebar-btn"
      class:titlebar-btn-active={layout.chatOpen}
      onclick={() => (layout.chatOpen = !layout.chatOpen)}
      title={settings.llmEnabled ? 'Toggle chat' : 'Chat unavailable — check Hardware settings'}
      disabled={!settings.llmEnabled}
    >
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M2 2h11a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H5l-3 3V3a1 1 0 0 1 1-1z"/>
      </svg>
    </button>
  </div>

  <div class="titlebar-winctl">
    <button class="winctl-btn" onclick={() => appWindow.minimize()} title="Minimise" aria-label="Minimise">
      <svg width="11" height="11" viewBox="0 0 11 11" fill="currentColor"><rect x="0" y="5" width="11" height="1"/></svg>
    </button>
    <button class="winctl-btn" onclick={() => appWindow.toggleMaximize()} title="Maximise" aria-label="Maximise">
      <svg width="11" height="11" viewBox="0 0 11 11" fill="none" stroke="currentColor" stroke-width="1"><rect x="0.5" y="0.5" width="10" height="10"/></svg>
    </button>
    <button class="winctl-btn close" onclick={() => appWindow.close()} title="Close" aria-label="Close">
      <svg width="11" height="11" viewBox="0 0 11 11" stroke="currentColor" stroke-width="1.2" stroke-linecap="round">
        <line x1="1" y1="1" x2="10" y2="10"/><line x1="10" y1="1" x2="1" y2="10"/>
      </svg>
    </button>
  </div>
</div>

<div class="layout" style:grid-template-columns={layout.gridCols}>
  <!-- Sidebar: Folders -->
  <aside class="sidebar" class:collapsed={!layout.foldersOpen}>
    {#if layout.foldersOpen}
      <FolderSidebar
        {folders}
        {notes}
        bookmarks={bm.bookmarks}
        bookmarkedNoteIds={bm.bookmarkedNoteIds}
        {allTags}
        templates={tmpl.templates}
        {selectedFolderId}
        {tagFilter}
        {unlockedFolderIds}
        {isDragging}
        bind:inlineRenaming
        bind:folderExpanded
        onSelectFolder={selectFolder}
        onCreateNote={() => startNoteInline()}
        onImportNote={() => handleImportNote()}
        onCreateFolder={() => startFolderInline()}
        onDeleteFolder={deleteFolder}
        onOpenNoteById={openNoteById}
        onFilterByTag={filterByTag}
        onClearTagFilter={clearTagFilter}
        onOpenNoteInNewTab={openNoteInNewTab}
        onRemoveBookmark={(id) => bm.removeBookmark(id)}
        onRequestFolderUnlock={requestFolderUnlock}
        onSetFolderPassword={(fid, mode) => (folderPwModal = { mode, folderId: fid })}
        onNewTemplate={() => (tmpl.templateModalOpen = true)}
        onEditTemplate={(t) => (tmpl.editingTemplate = t)}
        onDeleteTemplate={(id) => tmpl.deleteTemplate(id, showError)}
        onConfirmInlineRename={confirmInlineRename}
        onMoveNote={moveNote}
        onMoveFolder={loadFolders}
      />
    {:else}
      <button class="collapsed-strip" onclick={() => (layout.foldersOpen = true)} title="Expand folders">
        <span>Folders</span>
      </button>
    {/if}
  </aside>

  <button class="panel-divider folders-divider" aria-label="Resize folders panel" class:dragging={layout.activeDrag?.panel === 'folders'} onmousedown={(e) => layout.startDrag('folders', e)}></button>

  <!-- Note list -->
  <div class="note-list" class:collapsed={!layout.notesOpen}>
    {#if layout.notesOpen}
      <NoteList
        {notes}
        {folders}
        {activeNote}
        {selectedFolderId}
        {tagFilter}
        {isSeeding}
        {isReindexing}
        bind:inlineRenaming
        onOpenNote={navigateToNote}
        onOpenNoteInNewTab={openNoteInNewTab}
        onDeleteNote={deleteNote}
        onConfirmInlineRename={confirmInlineRename}
        onOpenKanbanTab={openKanbanTab}
        onSaveNote={saveNote}
        onClearTagFilter={clearTagFilter}
        onSeedNotes={seedNotes}
        onReindexAll={reindexAll}
        {tableViewOpen}
        onTableViewToggle={() => {
          if (isDirty) saveNote();
          const kanban = tabs.find(t => t.type === 'kanban' && t.folderId === selectedFolderId);
          if (kanban) { tabs = tabs.filter(t => t.id !== kanban.id); if (activeTabId === kanban.id) { activeNote = null; editorTitle = ''; editorContent = ''; isDirty = false; } }
          tableViewOpen = !tableViewOpen;
        }}
        onNoteDragStart={onNoteDragStart}
        onNoteDragEnd={onNoteDragEnd}
      />
    {:else}
      <button class="collapsed-strip" onclick={() => (layout.notesOpen = true)} title="Expand notes">
        <span>Notes</span>
      </button>
    {/if}
  </div>

  <button class="panel-divider notes-divider" aria-label="Resize notes panel" class:dragging={layout.activeDrag?.panel === 'notes'} onmousedown={(e) => layout.startDrag('notes', e)}></button>

  <!-- Editor -->
  <main class="editor">
    <div style="display: {searchOpen ? 'contents' : 'none'};">
      <Search
        {folders}
        open={searchOpen}
        onSelectNote={(id) => { openNoteById(id); searchOpen = false; }}
      />
    </div>
    {#if !searchOpen}
      {#if tableViewOpen && selectedFolderId && selectedFolderId !== 'all'}
        <div class="tab-fullview">
          <button class="tab-fullview-close" onclick={() => (tableViewOpen = false)} title="Close table">✕ Close</button>
          {#key tmpl.dbKey}
          <DatabaseView
            folderId={selectedFolderId}
            onOpenNote={(id) => { tableViewOpen = false; openNoteById(id); }}
            onFiltersChange={(f) => (activeViewFilters = f)}
          />
          {/key}
        </div>
      {:else if activeTab?.type === 'graph'}
        <div class="tab-fullview">
          <button class="tab-fullview-close" onclick={() => closeTab(activeTabId)} title="Close graph">✕ Close</button>
          <Graph
            onSelectNote={(id) => openNoteById(id)}
            activeNoteId={activeNote?.id ?? null}
            theme={settings.theme}
          />
        </div>
      {:else if activeTab?.type === 'calendar'}
        <div class="tab-fullview">
          <button class="tab-fullview-close" onclick={() => closeTab(activeTabId)} title="Close calendar">✕ Close</button>
          <Calendar
            onSelectNote={(note) => navigateToNote(note)}
            onRefresh={() => { loadFolders(); loadNotes(); }}
            onSelectFolder={selectFolder}
            dateFormat={settings.dailyNoteFormat}
          />
        </div>
      {:else if activeTab?.type === 'kanban'}
        <div class="tab-fullview">
          <button class="tab-fullview-close" onclick={() => closeTab(activeTabId)} title="Close kanban">✕ Close</button>
          <Kanban folderId={activeTab.folderId} onOpenNote={(id) => openNoteById(id)} />
        </div>
      {:else if activeTab?.type === 'wikipedia'}
        <div class="tab-fullview">
          <WikipediaReader
            bundleId={activeTab.bundleId}
            articlePath={activeTab.articlePath}
            bundleName={activeTab.label}
            onArticleNavigate={(bid, apath, title) => updateWikipediaTab(bid, apath, title)}
            onOpenArticle={(bid, apath, title) => openWikipediaArticle(bid, apath, title)}
            onClose={() => closeTab(activeTabId)}
          />
        </div>
      {:else if activeTab?.type === 'chat'}
        <Chat activeNote={null} pendingInsert={null} keepInMemory={settings.keepModelInMemory} llmEnabled={settings.llmEnabled} wikipediaEnabled={settings.wikipediaEnabled} onClose={() => closeTab(activeTabId)} onContextMenu={(x, y, items) => (ctxMenu = { x, y, items })} onInsertIntoNote={null} onOpenWikipediaArticle={openWikipediaArticle} {activeView} {activeViewFolderId} {activeViewLabel} {activeViewFilters} />
      {:else if activeNote}
        <NoteEditor
          {activeNote}
          bind:editorTitle
          bind:editorContent
          {isDirty}
          {indexState}
          {folders}
          {tabs}
          {activeTabId}
          {noteTags}
          {noteLinks}
          {noteBacklinks}
          {unlinkedMentions}
          {folderHasProperties}
          {propertiesReady}
          bind:editorTextareaEl
          {improveState}
          onStartImprove={startImprove}
          onImproveSend={handleImproveStart}
          onImproveCancel={() => (improveState = { status: 'idle', instruction: '', improvedText: '', hunks: [], originalContent: '', acceptedIndices: [], rejectedIndices: [] })}
          onImproveAcceptAll={handleImproveAcceptAll}
          onImproveRejectAll={handleImproveRejectAll}
          onImproveAcceptHunk={handleImproveAcceptHunk}
          onImproveRejectHunk={handleImproveRejectHunk}
          {refineState}
          onRefineHunk={handleRefineHunk}
          onRefineSend={handleRefineSend}
          onRefineCancel={handleRefineCancel}
          onMarkDirty={markDirty}
          onSave={saveNote}
          onCloseNote={closeNote}
          onToggleReadMode={toggleReadMode}
          onMoveNote={moveNote}
          onRevealFolder={revealFolder}
          onOpenKanbanTab={openKanbanTab}
          onOpenNoteById={openNoteById}
          onFilterByTag={filterByTag}
          onConvertMention={convertMention}
          onPropertiesLoad={(defs) => { noteProperties = defs; propertiesReady = true; folderHasProperties = defs.length > 0; }}
          onHandleEditorKeydown={handleEditorKeydown}
          onOpenTableView={() => {
            if (isDirty) saveNote();
            const kanban = tabs.find(t => t.type === 'kanban' && t.folderId === activeNote?.folder_id);
            if (kanban) { tabs = tabs.filter(t => t.id !== kanban.id); if (activeTabId === kanban.id) { activeTabId = tabs[0]?.id ?? null; } }
            selectedFolderId = activeNote?.folder_id;
            activeNote = null; tableViewOpen = true;
          }}
        />
      {:else}
        <div class="empty-editor">Select or create a note</div>
      {/if}
    {/if}
  </main>

  {#if layout.chatOpen && activeTab?.type !== 'chat'}
    <button class="panel-divider chat-divider" aria-label="Resize chat panel" class:dragging={layout.activeDrag?.panel === 'chat'} onmousedown={(e) => layout.startDrag('chat', e)}></button>
    <Chat {activeNote} pendingInsert={chatInsert} keepInMemory={settings.keepModelInMemory} llmEnabled={settings.llmEnabled} wikipediaEnabled={settings.wikipediaEnabled} onClose={() => (layout.chatOpen = false)} onContextMenu={(x, y, items) => (ctxMenu = { x, y, items })} onInsertIntoNote={activeNote ? insertIntoActiveNote : null} onOpenWikipediaArticle={openWikipediaArticle} {activeView} {activeViewFolderId} {activeViewLabel} {activeViewFilters} />
  {/if}
</div>

{#if quickSwitcherOpen}
  <QuickSwitcher
    onSelect={(note) => navigateToNote(note)}
    onSelectNewTab={(note) => openNoteInNewTab(note)}
    onClose={() => (quickSwitcherOpen = false)}
  />
{/if}

{#if wikiSearchOpen}
  <WikipediaSearchModal
    onOpenArticle={(bid, apath, title) => { openWikipediaArticle(bid, apath, title); wikiSearchOpen = false; }}
    onClose={() => (wikiSearchOpen = false)}
  />
{/if}

{#if settingsOpen}
  <Settings
    onClose={() => (settingsOpen = false)}
    vaultHasPassword={vaultHasPassword}
    onSetVaultPassword={() => (vaultPwModal = 'set')}
    onChangeVaultPassword={() => (vaultPwModal = 'change')}
    onRemoveVaultPassword={() => (vaultPwModal = 'remove')}
    onLockVault={lockVault}
    keepInMemory={settings.keepModelInMemory}
    onKeepInMemoryChange={(v) => (settings.keepModelInMemory = v)}
    accent={settings.accent}
    onAccentChange={(v) => (settings.accent = v)}
    theme={settings.theme}
    onThemeChange={(v) => (settings.theme = v)}
    dateFormat={settings.dailyNoteFormat}
    onDateFormatChange={(v) => (settings.dailyNoteFormat = v)}
    devNativeContextMenu={settings.devNativeContextMenu}
    onDevNativeContextMenuChange={(v) => (settings.devNativeContextMenu = v)}
    llmEnabled={settings.llmEnabled}
    onHardwareChange={(cap, force) => { settings.hwCapability = cap; settings.llmForceEnabled = force; }}
    wikipediaEnabled={settings.wikipediaEnabled}
    onWikipediaEnabledChange={(v) => (settings.wikipediaEnabled = v)}
  />
{/if}

{#if ctxMenu}
  <ContextMenu
    x={ctxMenu.x}
    y={ctxMenu.y}
    items={ctxMenu.items}
    onClose={() => (ctxMenu = null)}
  />
{/if}

{/if} <!-- end of vault-unlocked block -->
