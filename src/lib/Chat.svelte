<script>
  import { invoke } from '@tauri-apps/api/core';

  import { untrack, tick, onMount, getContext } from 'svelte';
  import { FEATURE_GUIDE } from './utils/featureGuide.js';
  import { t } from './i18n/t.js';
  import { formatAppError } from './i18n/formatAppError.js';
  import {
    openNoteSystemPart,
    userNotesSystemPart,
    wikipediaSystemPart,
    scannedFilesSystemPart,
    assembleChatSystemContent,
  } from './llm/prompts.js';
  import { CURATED_CHAT_MODELS, DEFAULT_CHAT_MODEL, isExtraInstalledModel, statsForAnyModelId, isEmbeddingModelId } from './constants/chatModels.js';
  import { assessChatModelHardware } from './utils/chatModelHardware.js';
  import { firstInstalledFullName } from './utils/ollamaModelMatch.js';
  import {
    checkChatModelInstalled,
    saveChatModelSetting,
    pullChatModel,
    isPullInFlight,
    deleteOllamaModel,
  } from './services/chatModelSelection.js';
  import { CLEAR_CONVERSATION_LABEL } from './services/chatSessionService.svelte.js';
  import ModelDownloadModal from './ModelDownloadModal.svelte';
  import ChatModelCombobox from './ChatModelCombobox.svelte';
  import SlidersIcon from './icons/SlidersIcon.svelte';
  import TrashIcon from './icons/TrashIcon.svelte';

  const ns       = getContext('ns');
  const ts       = getContext('ts');
  const fs       = getContext('fs');
  const settings = getContext('settings');
  const chatSession = getContext('chatSession');

  // ── Props ──────────────────────────────────────────────────────────────────

  // suppressNoteContext: set to true in the Chat tab (where there is no active note).
  // onClose/onInsertIntoNote/onContextMenu/onOpenWikipediaArticle differ per usage.
  let {
    suppressNoteContext = false,
    onClose = null,
    onContextMenu = null,
    onInsertIntoNote = null,
    onOpenWikipediaArticle = null,
  } = $props();

  // Settings props replaced by context.
  const keepInMemory    = $derived(settings.keepModelInMemory);
  const llmEnabled      = $derived(settings.llmEnabled);
  const chatEnabled   = $derived(settings.chatEnabled);
  const wizardAiSkipped = $derived(settings.wizardAiSkipped);
  const wikipediaEnabled = $derived(settings.wikipediaEnabled);
  const hardwareReport  = $derived(settings.hardwareReport ?? null);

  // pendingInsert and activeNote: when suppressNoteContext is true (chat tab), both are null.
  const pendingInsert = $derived(suppressNoteContext ? null : ts.chatInsert);
  const activeNote    = $derived(suppressNoteContext ? null : ns.activeNote);

  // activeView deriveds — computed locally from ts + fs.
  const activeView = $derived(ts.activeView);
  const activeViewFolderId = $derived(
    activeView === 'kanban'   ? ts.activeTab?.folderId
    : activeView === 'database' ? fs.selectedFolderId
    : null
  );
  const activeViewLabel = $derived.by(() => {
    if (!activeView || !activeViewFolderId) return '';
    const folder = fs.folders.find(f => f.id === activeViewFolderId);
    const name = folder?.name ?? t('common.unknown');
    return activeView === 'kanban'
      ? t('chat.activeViewKanban', { name })
      : t('chat.activeViewTable', { name });
  });
  const activeViewFilters = $derived(ts.activeViewFilters);

  // ── Helpers ────────────────────────────────────────────────────────────────

  // ── State ──────────────────────────────────────────────────────────────────

  // ── Chat input placeholder ──────────────────────────────────────────────────

  const PLACEHOLDERS = [
    t('chat.placeholders.p0'),
    t('chat.placeholders.p1'),
    t('chat.placeholders.p2'),
    t('chat.placeholders.p3'),
    t('chat.placeholders.p4'),
    t('chat.placeholders.p5'),
    t('chat.placeholders.p6'),
    t('chat.placeholders.p7'),
    t('chat.placeholders.p8'),
    t('chat.placeholders.p9'),
  ];

  const inputPlaceholder = PLACEHOLDERS[Math.floor(Math.random() * PLACEHOLDERS.length)];

  let input = $state('');
  /** Committed chat model id (persisted); used for Ollama chat requests. */
  let model = $state(DEFAULT_CHAT_MODEL);
  /** Mirrors chat model picker; may revert while a download prompt is open. */
  let modelSelectUi = $state(DEFAULT_CHAT_MODEL);
  let extraInstalledModels = $state([]);
  let selectModelBusy = $state(false);
  /** `value` of the row currently being removed from Ollama, if any. */
  let uninstallBusy = $state(/** @type {string | null} */ (null));
  /** @type {null | { model: string, phase: 'confirm' | 'pulling' | 'error', confirmKind?: 'downloadMissing' | 'installedRisk', hardwareWarning?: { level: string, lines: string[] } | null, statusLine: string, progress: { completed: number, total: number } | null, errorMessage: string }} */
  let modelDownloadModal = $state(null);

  const chatModelSelectOptions = $derived.by(() => {
    const inst = extraInstalledModels;
    const rows = CURATED_CHAT_MODELS.map((p) => ({
      value: p.value,
      label: p.label,
      installedFull: firstInstalledFullName(p.value, inst),
      ...statsForAnyModelId(p.value),
    }));
    const seen = new Set(rows.map((r) => r.value));
    const extras = [];
    for (const n of inst) {
      if (!isExtraInstalledModel(n)) continue;
      if (isEmbeddingModelId(n)) continue;
      if (seen.has(n)) continue;
      seen.add(n);
      extras.push({ value: n, label: n, installedFull: n, ...statsForAnyModelId(n) });
    }
    extras.sort((a, b) => a.value.localeCompare(b.value));
    rows.push(...extras);
    if (model && !seen.has(model)) {
      rows.push({
        value: model,
        label: model,
        installedFull: firstInstalledFullName(model, inst),
        ...statsForAnyModelId(model),
      });
    }
    return rows;
  });

  /**
   * @param {{ value: string, installedFull?: string | null }} opt
   */
  async function uninstallChatModelOption(opt) {
    if (!opt.installedFull || uninstallBusy) return;
    const label = opt.installedFull;
    if (
      !confirm(t('chat.uninstallConfirm', { label }))
    ) {
      return;
    }
    uninstallBusy = opt.value;
    error = '';
    try {
      await deleteOllamaModel(opt.value);
      if (!(await checkChatModelInstalled(model))) {
        await saveChatModelSetting(DEFAULT_CHAT_MODEL);
      }
      await refreshExtraInstalledModels();
    } catch (e) {
      error = formatAppError(e);
    } finally {
      uninstallBusy = null;
    }
  }

  async function refreshExtraInstalledModels() {
    try {
      const list = await invoke('list_ollama_installed_models');
      extraInstalledModels = Array.isArray(list) ? list : [];
    } catch {
      extraInstalledModels = [];
    }
    try {
      if (modelDownloadModal) return;
      const val = await invoke('get_setting', { key: 'chat_model' });
      if (typeof val === 'string' && val.trim()) {
        const v = val.trim();
        model = v;
        modelSelectUi = v;
      }
    } catch {
      /* ignore */
    }
  }

  /**
   * @param {string} next
   */
  async function commitChatModelChoice(next) {
    const chosen = String(next).trim();
    if (!chosen || selectModelBusy || uninstallBusy) return;
    if (isPullInFlight()) return;
    if (isEmbeddingModelId(chosen)) {
      error = t('chat.embeddingNotChat');
      modelSelectUi = model;
      return;
    }
    if (chosen === model) {
      modelSelectUi = model;
      return;
    }
    selectModelBusy = true;
    error = '';
    try {
      const installed = await checkChatModelInstalled(chosen);
      const hwWarn = assessChatModelHardware(chosen, hardwareReport);

      if (!installed) {
        modelSelectUi = model;
        modelDownloadModal = {
          model: chosen,
          phase: 'confirm',
          confirmKind: 'downloadMissing',
          hardwareWarning: hwWarn.level === 'ok' ? null : hwWarn,
          statusLine: '',
          progress: null,
          errorMessage: '',
        };
        return;
      }

      if (hwWarn.level !== 'ok') {
        modelSelectUi = chosen;
        modelDownloadModal = {
          model: chosen,
          phase: 'confirm',
          confirmKind: 'installedRisk',
          hardwareWarning: hwWarn,
          statusLine: '',
          progress: null,
          errorMessage: '',
        };
        return;
      }

      model = chosen;
      modelSelectUi = chosen;
      await saveChatModelSetting(chosen);
      await refreshExtraInstalledModels();
    } catch (e) {
      error = formatAppError(e);
      modelSelectUi = model;
    } finally {
      selectModelBusy = false;
    }
  }

  async function onModelDownloadConfirm() {
    const m = modelDownloadModal;
    if (!m || m.phase !== 'confirm') return;

    if (m.confirmKind === 'installedRisk') {
      const name = m.model;
      model = name;
      modelSelectUi = name;
      await saveChatModelSetting(name);
      await refreshExtraInstalledModels();
      modelDownloadModal = null;
      return;
    }

    const name = m.model;
    const hwW = m.hardwareWarning ?? null;
    modelDownloadModal = {
      model: name,
      phase: 'pulling',
      confirmKind: 'downloadMissing',
      hardwareWarning: hwW,
      statusLine: '',
      progress: null,
      errorMessage: '',
    };
    try {
      let pullProgress = null;
      await pullChatModel(name, (payload) => {
        const status = typeof payload?.status === 'string' ? payload.status : '';
        const completed = typeof payload?.completed === 'number' ? payload.completed : null;
        const total = typeof payload?.total === 'number' ? payload.total : null;
        if (completed != null && total != null && total > 0) {
          pullProgress = { completed, total };
        }
        const line = status || JSON.stringify(payload);
        modelDownloadModal = {
          model: name,
          phase: 'pulling',
          confirmKind: 'downloadMissing',
          hardwareWarning: hwW,
          statusLine: line,
          progress: pullProgress,
          errorMessage: '',
        };
      });
      const ok = await checkChatModelInstalled(name);
      if (!ok) {
        throw new Error(t('errors.modelNotInstalledAfterPull'));
      }
      model = name;
      modelSelectUi = name;
      await saveChatModelSetting(name);
      await refreshExtraInstalledModels();
      modelDownloadModal = null;
    } catch (e) {
      modelDownloadModal = {
        model: name,
        phase: 'error',
        confirmKind: 'downloadMissing',
        hardwareWarning: null,
        statusLine: '',
        progress: null,
        errorMessage: formatAppError(e),
      };
    }
  }

  function closeModelDownloadModal() {
    if (modelDownloadModal?.confirmKind === 'installedRisk') {
      modelSelectUi = model;
    }
    modelDownloadModal = null;
  }

  let error = $state('');
  // Initialise toggles directly from localStorage so they are correct on first render,
  // avoiding the write-before-read race when effects run in definition order.
  function readPref(key, fallback) {
    try { const v = localStorage.getItem(key); return v !== null ? JSON.parse(v) : fallback; }
    catch { return fallback; }
  }
  let useNotes       = $state(readPref('grimoire:chat:useNotes',       true));
  let useWiki        = $state(readPref('grimoire:chat:useWiki',        true));
  let useFiles       = $state(readPref('grimoire:chat:useFiles',       true));
  let useViewContext = $state(readPref('grimoire:chat:useViewContext', true));
  let useFeatureGuide = $state(readPref('grimoire:chat:useFeatureGuide', true));
  let chatOptsOpen = $state(false);

  // Persist all context toggles to localStorage (write-only — initial values come from readPref above).
  $effect(() => { localStorage.setItem('grimoire:chat:useNotes',        JSON.stringify(useNotes)); });
  $effect(() => { localStorage.setItem('grimoire:chat:useWiki',         JSON.stringify(useWiki)); });
  $effect(() => { localStorage.setItem('grimoire:chat:useFiles',        JSON.stringify(useFiles)); });
  $effect(() => { localStorage.setItem('grimoire:chat:useViewContext',  JSON.stringify(useViewContext)); });
  $effect(() => { localStorage.setItem('grimoire:chat:useFeatureGuide', JSON.stringify(useFeatureGuide)); });

  // Load chat model from SQLite on mount and merge local Ollama tags into the selector.
  onMount(() => {
    void refreshExtraInstalledModels();
  });

  // Reference to the chat textarea so we can focus it after injection.
  let inputEl = $state(null);

  // Watch for new injections from the editor keybind.
  $effect(() => {
    if (!pendingInsert) return;
    // Format the selection as a blockquote.
    const quoted = pendingInsert.text
      .split('\n')
      .map(line => `> ${line}`)
      .join('\n');
    // Read input without tracking it as a dependency — avoids an infinite loop
    // where writing input would re-trigger this effect.
    const current = untrack(() => input);
    input = current ? `${current}\n\n${quoted}\n` : `${quoted}\n`;
    inputEl?.focus();
  });


  // Reference to the scrollable messages container so we can auto-scroll.
  let messagesEl = $state(null);

  // Scroll to the bottom whenever the messages array changes.
  // tick() waits for Svelte to finish updating the DOM before measuring scrollHeight,
  // otherwise we'd scroll to the pre-update height and land one message short.
  $effect(() => {
    if (chatSession.messages.length && messagesEl) {
      tick().then(() => {
        messagesEl.scrollTop = messagesEl.scrollHeight;
      });
    }
  });

  // ── Actions ────────────────────────────────────────────────────────────────

  // ── Filter helper (mirrored from DatabaseView) ──────────────────────────────

  function applyFilters(rows, filters, defs) {
    const active = Object.entries(filters).filter(([, f]) => f && !isFilterEmpty(f));
    if (active.length === 0) return rows;
    return rows.filter(note => {
      return active.every(([defId, f]) => {
        const def = defs.find(d => d.id === Number(defId));
        if (!def) return true;
        const prop = note.properties.find(p => p.def_id === Number(defId));
        const val = prop?.value ?? null;
        if (def.type === 'text') {
          if (f.op === 'is empty')     return val === null || val === '';
          if (f.op === 'is not empty') return val !== null && val !== '';
          if (val === null) return false;
          if (f.op === 'contains') return val.toLowerCase().includes(f.value.toLowerCase());
          if (f.op === 'equals')   return val.toLowerCase() === f.value.toLowerCase();
        }
        if (def.type === 'number') {
          if (val === null || val === '') return false;
          const n = parseFloat(val);
          const v = parseFloat(f.value);
          if (isNaN(n) || isNaN(v)) return false;
          if (f.op === '=')  return n === v;
          if (f.op === '≠')  return n !== v;
          if (f.op === '>')  return n > v;
          if (f.op === '<')  return n < v;
          if (f.op === '≥')  return n >= v;
          if (f.op === '≤')  return n <= v;
        }
        if (def.type === 'date') {
          if (val === null || val === '') return false;
          if (f.op === 'on')     return val === f.value;
          if (f.op === 'before') return val < f.value;
          if (f.op === 'after')  return val > f.value;
          if (f.op === 'between') {
            const [from, to] = Array.isArray(f.value) ? f.value : ['', ''];
            return val >= from && val <= to;
          }
        }
        if (def.type === 'boolean') {
          const b = val === 'true';
          if (f.op === 'is true')  return b === true;
          if (f.op === 'is false') return b === false;
        }
        if (def.type === 'select') {
          const selected = Array.isArray(f.value) ? f.value : [];
          if (selected.length === 0) return true;
          if (f.op === 'any of') return selected.includes(val ?? '');
          if (f.op === 'none of') return !selected.includes(val ?? '');
        }
        return true;
      });
    });
  }

  function isFilterEmpty(f) {
    if (!f || !f.op) return true;
    const { op, value } = f;
    if (op === 'is empty' || op === 'is not empty' || op === 'is true' || op === 'is false') return false;
    if (Array.isArray(value)) return value.every(v => v === '');
    return value === '';
  }

  // Builds the system prompt, pushes a placeholder assistant message, and
  // streams the response. `history` must be the full message list ending with
  // a user message — it is never mutated.
  async function streamResponse(history, params = {}) {
    const {
      temperature = 0.8,
      top_p = 0.9,
      top_k = 40,
      repeat_penalty = 1.1,
      num_ctx = 8192,
      verbosity = 'concise',
    } = params;
    let payload = history;
    const systemParts = [];

    // ── 1. Active note ───────────────────────────────────────────────────────
    if (activeNote) {
      const openTitle = (ns.editorTitle ?? activeNote.title ?? '').trim() || activeNote.title;
      const openBody = ns.editorContent ?? activeNote.content ?? '';
      systemParts.push(openNoteSystemPart(openTitle, openBody));
    }

    // ── 1b. Board/table context ──────────────────────────────────────────────
    if (useViewContext && activeView && activeViewFolderId != null) {
      try {
        const [defs, notes] = await Promise.all([
          invoke('get_property_defs', { folderId: activeViewFolderId }),
          invoke('list_notes_with_properties', { folderId: activeViewFolderId }),
        ]);

        const contextLines = [`## ${activeViewLabel}`];

        // Property schema
        contextLines.push('Property definitions:');
        for (const def of defs) {
          let desc = `${def.name} (${def.type})`;
          if (def.type === 'select' && def.options) {
            try {
              const opts = JSON.parse(def.options).join(', ');
              desc += ` — options: ${opts}`;
            } catch {}
          }
          contextLines.push(`- ${desc}`);
        }

        // Apply table filters if any
        let visibleNotes = notes;
        if (activeView === 'database' && Object.keys(activeViewFilters).length > 0) {
          visibleNotes = applyFilters(notes, activeViewFilters, defs);
        }

        const MAX_NOTES = 50;
        let total = 0;

        if (activeView === 'kanban') {
          const selectDefs = defs.filter(d => d.type === 'select');
          const savedGb = localStorage.getItem(`grimoire:kanban:${activeViewFolderId}:groupBy`);
          const savedMatch = savedGb
            ? selectDefs.find(d => String(d.id) === savedGb)
            : null;
          const selectDef = savedMatch ?? selectDefs[0];
          if (selectDef) {
            let options = [];
            try { options = JSON.parse(selectDef.options ?? '[]'); } catch { options = []; }
            const groups = {};
            for (const opt of options) groups[opt] = [];
            groups['__unset__'] = [];

            for (const note of visibleNotes) {
              const prop = note.properties.find(p => p.def_id === selectDef.id);
              const val = prop?.value?.trim() || '';
              const key = val === '' ? '__unset__' : val;
              if (!groups[key]) groups[key] = [];
              groups[key].push(note);
            }

            for (const [colKey, colNotes] of Object.entries(groups)) {
              if (total >= MAX_NOTES) break;
              const label = colKey === '__unset__' ? 'Unset' : colKey;
              const remaining = MAX_NOTES - total;
              const shown = colNotes.slice(0, remaining);
              contextLines.push(`\n### ${label} (${colNotes.length})`);
              for (const note of shown) {
                const props = note.properties
                  .filter(p => p.value?.trim())
                  .map(p => `${p.name}: ${p.value}`)
                  .join(', ');
                contextLines.push(`- ${note.title}${props ? ` [${props}]` : ''}`);
              }
              total += shown.length;
            }
            const hidden = notes.length - total;
            if (hidden > 0) contextLines.push(`\n... and ${hidden} more cards not shown`);
          }
        } else if (activeView === 'database') {
          for (const note of visibleNotes) {
            if (total >= MAX_NOTES) {
              contextLines.push(`\n... and ${visibleNotes.length - total} more rows not shown`);
              break;
            }
            const props = note.properties
              .filter(p => p.value?.trim())
              .map(p => `${p.name}: ${p.value}`)
              .join(', ');
            contextLines.push(`- **${note.title}**${props ? ` — ${props}` : ''}`);
            total++;
          }
        }

        systemParts.push(contextLines.join('\n'));
      } catch (e) {
        // Silently fail — don't interrupt chat if context fetch fails
      }
    }

    // ── 1c. Feature guide ───────────────────────────────────────────────────
    if (useFeatureGuide) {
      systemParts.push(FEATURE_GUIDE);
    }

    // ── 1d. Verbosity instruction ────────────────────────────────────────────
    // Appended after the INSTRUCTIONS block (below) so it takes highest priority.

    // ── 2. RAG context ───────────────────────────────────────────────────────
    // Always search even when a note is open — the question may be relevant
    // to other notes beyond the one currently being edited.

    // Build the RAG query from recent user messages (hoisted so wikipedia can reuse it).
    const recentUserMessages = history
      .filter(m => m.role === 'user')
      .slice(-2)
      .map(m => m.content)
      .join(' ');
    const ragQuery = recentUserMessages
      .replace(/what (have i|did i|do i have) (written?|noted?|said?) (about|on|regarding)\s*/gi, '')
      .replace(/tell me (about|what i (wrote|know|noted) about)\s*/gi, '')
      .replace(/what (are|is) my (notes?|thoughts?) (on|about)\s*/gi, '')
      .replace(/show me (my notes? on|what i wrote about)\s*/gi, '')
      .trim() || recentUserMessages;

    /** @type {{ iso_date: string, display_title: string, note?: { title: string, content: string, locked: boolean } } | null} */
    let dailyNoteResolution = null;
    if (useNotes) {
      try {
        dailyNoteResolution = await invoke('resolve_daily_note_from_query', {
          query: recentUserMessages,
          dateFormat: settings.dailyNoteFormat,
        });
      } catch (_) {
        // Best-effort — semantic search still runs without a pinned daily note.
      }
    }

    const notesSearchQuery = dailyNoteResolution?.display_title
      ? `${ragQuery} ${dailyNoteResolution.display_title}`.trim()
      : ragQuery;

    if (useNotes) {
      let matches = [];
      try {
        matches = await invoke('search_notes', { query: notesSearchQuery });
      } catch (e) {
        chatSession.notesError = t('errors.noteSearchFailed', { msg: formatAppError(e) });
      }
      const byTitle = {};
      const pinned = dailyNoteResolution?.note;
      if (pinned && !pinned.locked) {
        byTitle[pinned.title] = [pinned.content?.trim() ? pinned.content : '(empty note)'];
      }
      for (const m of matches) {
        if (pinned && m.title === pinned.title) continue;
        if (!byTitle[m.title]) byTitle[m.title] = [];
        byTitle[m.title].push(...m.excerpts);
      }
      if (Object.keys(byTitle).length > 0) {
        chatSession.sourcesUsed = Object.keys(byTitle);
        const context = Object.entries(byTitle)
          .map(([title, excerpts]) => `[Note: "${title}"]\n${excerpts.join('\n')}`)
          .join('\n\n');
        systemParts.push(userNotesSystemPart(context));
      }
    }

    // ── 3. Wikipedia RAG context ─────────────────────────────────────────────
    if (useWiki && wikipediaEnabled) {
      let wikiMatches = [];
      try {
        wikiMatches = await invoke('search_wikipedia', { query: ragQuery });
      } catch (_) {
        // Wikipedia search is best-effort — don't surface errors in the UI.
      }
      if (wikiMatches.length > 0) {
        chatSession.wikiSourcesUsed = wikiMatches.map(m => ({
          title: m.title,
          bundleId: m.bundle_id,
          articlePath: m.article_id.includes('/') ? m.article_id.slice(m.article_id.indexOf('/') + 1) : m.article_id,
        }));
        const wikiContext = wikiMatches
          .map(m => `[Wikipedia: "${m.title}"]\n${m.excerpts.join('\n')}`)
          .join('\n\n');
        systemParts.push(wikipediaSystemPart(wikiContext));
      }
    }

    // ── 4. Scanned files RAG context ─────────────────────────────────────────
    let fileMatches = [];
    if (useFiles) {
      try {
        fileMatches = await invoke('search_scanned_files', { query: ragQuery });
      } catch (e) {
        // Best-effort — don't block the chat if scanned file search fails.
        console.warn('Scanned file search failed:', e);
      }
      if (fileMatches.length > 0) {
        const fileContext = fileMatches
          .map(m => `[File: "${m.title}" (${m.file_path})]\n${m.excerpts.join('\n')}`)
          .join('\n\n');
        systemParts.push(scannedFilesSystemPart(fileContext));
      }
    }

    // ── Assemble system message ──────────────────────────────────────────────
    if (systemParts.length > 0) {
      const hasWikiContext = chatSession.wikiSourcesUsed.length > 0;
      const hasNotesContext = chatSession.sourcesUsed.length > 0;
      const hasFilesContext = fileMatches.length > 0;
      const content = assembleChatSystemContent({
        systemParts,
        hasNotesContext,
        hasWikiContext,
        hasFilesContext,
        dailyNoteResolution,
        dailyNoteFormat: settings.dailyNoteFormat,
        verbosity,
      });
      payload = [{ role: 'system', content }, ...history];
    }

    // Push a placeholder assistant message and subscribe to tokens via the session service.
    await chatSession.beginStream(history);

    try {
      await invoke('chat', { model, messages: payload, keepInMemory, temperature, topP: top_p, topK: top_k, repeatPenalty: repeat_penalty, numCtx: num_ctx });
    } finally {
      chatSession.endStream();
    }
  }

  async function send() {
    const text = input.trim();
    if (!text || chatSession.isLoading) return;

    // Append the user message before the await so the UI updates immediately.
    const updated = [...chatSession.messages, { role: 'user', content: text }];
    chatSession.messages = updated;
    input = '';
    chatSession.isLoading = true;
    error = '';
    chatSession.streamError = '';
    chatSession.sourcesUsed = [];
    chatSession.wikiSourcesUsed = [];
    chatSession.notesError = '';

    // Load inference params from settings so changes take effect immediately.
    let params = {};
    try {
      const [temperature, top_p, top_k, repeat_penalty, num_ctx, verbosity] = await Promise.all([
        invoke('get_setting', { key: 'chat_temperature' }),
        invoke('get_setting', { key: 'chat_top_p' }),
        invoke('get_setting', { key: 'chat_top_k' }),
        invoke('get_setting', { key: 'chat_repeat_penalty' }),
        invoke('get_setting', { key: 'chat_num_ctx' }),
        invoke('get_setting', { key: 'chat_verbosity' }),
      ]);
      params = {
        temperature: temperature !== '' ? parseFloat(temperature) : 0.8,
        top_p:       top_p !== ''       ? parseFloat(top_p)       : 0.9,
        top_k:       top_k !== ''       ? parseInt(top_k, 10)     : 40,
        repeat_penalty: repeat_penalty !== '' ? parseFloat(repeat_penalty) : 1.1,
        num_ctx:     num_ctx !== ''     ? parseInt(num_ctx, 10)   : 8192,
        verbosity:   verbosity !== ''   ? verbosity               : 'concise',
      };
    } catch {
      params = {};
    }

    try {
      await streamResponse(updated, params);
    } catch (e) {
      if (chatSession.messages.length > 0 && chatSession.messages[chatSession.messages.length - 1].role === 'assistant'
          && chatSession.messages[chatSession.messages.length - 1].content === '') {
        chatSession.messages = chatSession.messages.slice(0, -1);
      }
      chatSession.streamError = formatAppError(e);
    } finally {
      chatSession.isLoading = false;
    }
  }

  function deleteMessage(index) {
    if (chatSession.isLoading) return;
    chatSession.messages = chatSession.messages.filter((_, i) => i !== index);
  }

  async function regenerate() {
    if (chatSession.isLoading) return;
    const history = chatSession.messages[chatSession.messages.length - 1]?.role === 'assistant'
      ? chatSession.messages.slice(0, -1)
      : chatSession.messages;
    if (history.length === 0 || history[history.length - 1]?.role !== 'user') return;

    chatSession.isLoading = true;
    error = '';
    chatSession.streamError = '';
    chatSession.sourcesUsed = [];
    chatSession.wikiSourcesUsed = [];
    chatSession.notesError = '';

    // Load inference params from settings (same as send()).
    let params = {};
    try {
      const [temperature, top_p, top_k, repeat_penalty, num_ctx, verbosity] = await Promise.all([
        invoke('get_setting', { key: 'chat_temperature' }),
        invoke('get_setting', { key: 'chat_top_p' }),
        invoke('get_setting', { key: 'chat_top_k' }),
        invoke('get_setting', { key: 'chat_repeat_penalty' }),
        invoke('get_setting', { key: 'chat_num_ctx' }),
        invoke('get_setting', { key: 'chat_verbosity' }),
      ]);
      params = {
        temperature: temperature !== '' ? parseFloat(temperature) : 0.8,
        top_p:       top_p !== ''       ? parseFloat(top_p)       : 0.9,
        top_k:       top_k !== ''       ? parseInt(top_k, 10)     : 40,
        repeat_penalty: repeat_penalty !== '' ? parseFloat(repeat_penalty) : 1.1,
        num_ctx:     num_ctx !== ''     ? parseInt(num_ctx, 10)   : 8192,
        verbosity:   verbosity !== ''   ? verbosity               : 'concise',
      };
    } catch {
      params = {};
    }

    try {
      await streamResponse(history, params);
    } catch (e) {
      if (chatSession.messages.length > 0 && chatSession.messages[chatSession.messages.length - 1].role === 'assistant'
          && chatSession.messages[chatSession.messages.length - 1].content === '') {
        chatSession.messages = chatSession.messages.slice(0, -1);
      }
      chatSession.streamError = formatAppError(e);
    } finally {
      chatSession.isLoading = false;
    }
  }

  // ── Context menu ────────────────────────────────────────────────────────────

  function handleMessageContextMenu(e, msg, i) {
    e.preventDefault();
    const isLastAssistant = i === chatSession.messages.length - 1 && msg.role === 'assistant';

    const items = [
      {
        label: t('chat.copy'),
        action: () => navigator.clipboard.writeText(msg.content),
      },
      {
        label: t('chat.copyAsQuote'),
        action: () => navigator.clipboard.writeText(`"${msg.content}"`),
      },
      ...(onInsertIntoNote ? [
        { divider: true },
        {
          label: t('chat.insertIntoNote'),
          action: () => onInsertIntoNote(msg.content),
        },
      ] : []),
      { divider: true },
      ...(isLastAssistant ? [{
        label: t('chat.regenerate'),
        disabled: chatSession.isLoading,
        action: regenerate,
      }] : []),
      {
        label: t('common.delete'),
        danger: true,
        disabled: chatSession.isLoading,
        action: () => deleteMessage(i),
      },
      { divider: true },
      {
        label: CLEAR_CONVERSATION_LABEL,
        disabled: chatSession.isLoading,
        action: () => chatSession.clearConversation(),
      },
    ];

    e.stopPropagation();
    onContextMenu?.(e.clientX, e.clientY, items);
  }

  function handlePanelContextMenu(e) {
    if (/** @type {Element} */ (e.target).closest('.chat-message')) return;
    e.preventDefault();
    const items = [
      {
        label: CLEAR_CONVERSATION_LABEL,
        disabled: chatSession.isLoading || chatSession.messages.length === 0,
        action: () => chatSession.clearConversation(),
      },
    ];
    onContextMenu?.(e.clientX, e.clientY, items);
  }

  // ── Debug search ───────────────────────────────────────────────────────────

  let debugQuery = $state('');
  let debugResults = $state([]);
  let debugWikiResults = $state([]);
  let debugOpen = $state(false);

  async function runDebugSearch() {
    const q = debugQuery.trim();
    if (!q) return;
    try {
      debugResults = await invoke('debug_search', { query: q });
    } catch (e) {
      debugResults = [{ title: 'Error', excerpt: String(e), distance: -1 }];
    }
    try {
      debugWikiResults = await invoke('debug_search_wikipedia', { query: q });
    } catch (e) {
      debugWikiResults = [{ title: 'Error', excerpt: String(e), distance: -1 }];
    }
    debugOpen = true;
  }

  function handleKeydown(e) {
    // Enter sends; Shift+Enter inserts a newline.
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  // ── Inline Markdown renderer ───────────────────────────────────────────────
  // Handles the subset the LLM commonly emits: fenced code blocks, inline code,
  // **bold**, *italic*, and paragraph breaks. No external dependency.
  function renderMarkdown(text) {
    // Escape HTML in a plain text segment to prevent XSS.
    function esc(s) {
      return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    // Split on fenced code blocks first so we never process their contents.
    const parts = text.split(/(```[\s\S]*?```)/g);
    return parts.map((part, i) => {
      if (i % 2 === 1) {
        // Code block — extract optional language tag and content.
        const m = part.match(/^```(\w*)\n?([\s\S]*?)```$/);
        const code = m ? m[2] : part.slice(3, -3);
        const lang = m && m[1] ? ` class="language-${esc(m[1])}"` : '';
        return `<pre><code${lang}>${esc(code)}</code></pre>`;
      }
      // Plain text segment — apply inline rules then convert newlines.
      return esc(part)
        // Inline code (must come before bold/italic so backticks aren't double-processed).
        .replace(/`([^`\n]+)`/g, '<code>$1</code>')
        // Bold.
        .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
        // Italic (single *, not double).
        .replace(/(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)/g, '<em>$1</em>')
        // Paragraph breaks (two+ newlines) → double br.
        .replace(/\n{2,}/g, '<br><br>')
        // Single newlines → br.
        .replace(/\n/g, '<br>');
    }).join('');
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<svelte:window onclick={(e) => { if (chatOptsOpen && !(/** @type {Element} */ (e.target)).closest('.chat-opts-wrap')) chatOptsOpen = false; }} />

<aside class="chat-panel" data-tour="chat" oncontextmenu={handlePanelContextMenu}>
  {#if modelDownloadModal}
    <ModelDownloadModal
      model={modelDownloadModal.model}
      phase={modelDownloadModal.phase}
      confirmKind={modelDownloadModal.confirmKind ?? 'downloadMissing'}
      hardwareWarning={modelDownloadModal.hardwareWarning ?? null}
      statusLine={modelDownloadModal.statusLine}
      progress={modelDownloadModal.progress}
      errorMessage={modelDownloadModal.errorMessage}
      onDownload={onModelDownloadConfirm}
      onCancel={closeModelDownloadModal}
    />
  {/if}
  {#if wizardAiSkipped}
    <div class="chat-hw-banner">
      <strong>{t('chat.wizardSkippedTitle')}</strong>
      {t('chat.wizardSkippedBanner')}
    </div>
  {:else if !llmEnabled}
    <div class="chat-hw-banner">
      <strong>{t('chat.llmDisabledTitle')}</strong>
      {t('chat.llmDisabledBanner')}
    </div>
  {/if}

  <div class="chat-header">
    <span class="chat-title">{t('chat.title')}</span>
    <ChatModelCombobox
      variant="chat"
      selected={modelSelectUi}
      options={chatModelSelectOptions}
      disabled={selectModelBusy || isPullInFlight() || !!uninstallBusy || !chatEnabled}
      ariaLabel={t('chat.chatModelAria')}
      onOpenChange={(o) => {
        if (o) {
          chatOptsOpen = false;
          void refreshExtraInstalledModels();
        }
      }}
      onSelect={(v) => commitChatModelChoice(v)}
      onUninstall={uninstallChatModelOption}
      uninstallBusyKey={uninstallBusy}
    />
    <div class="chat-opts-wrap">
      <button
        class="chat-opts-btn"
        class:active={chatOptsOpen}
        onclick={() => (chatOptsOpen = !chatOptsOpen)}
        title={t('chat.contextOptions')}
        aria-label={t('chat.contextOptions')}
        aria-expanded={chatOptsOpen}
      >
        <SlidersIcon size={14} />
      </button>

      {#if chatOptsOpen}
        <div class="chat-opts-dropdown" role="menu">
          <label class="chat-opt-row" title={t('chat.optNotesTitle')}>
            <input type="checkbox" bind:checked={useNotes} />
            {t('chat.useNotes')}
          </label>
          <label class="chat-opt-row" class:disabled={!wikipediaEnabled} title={t('chat.optWikiTitle')}>
            <input type="checkbox" bind:checked={useWiki} disabled={!wikipediaEnabled} />
            {t('chat.useWiki')}
          </label>
          <label class="chat-opt-row" title={t('chat.optFilesTitle')}>
            <input type="checkbox" bind:checked={useFiles} />
            {t('chat.useFiles')}
          </label>
          <label class="chat-opt-row" class:disabled={!activeView} title={t('chat.optViewTitle')}>
            <input type="checkbox" bind:checked={useViewContext} disabled={!activeView} />
            {t('chat.useView')}
          </label>
          <label class="chat-opt-row" title={t('chat.optGuideTitle')}>
            <input type="checkbox" bind:checked={useFeatureGuide} />
            {t('chat.useGuide')}
          </label>
        </div>
      {/if}
    </div>
    <button
      class="chat-clear-btn"
      onclick={() => chatSession.clearConversation()}
      disabled={chatSession.isLoading || chatSession.messages.length === 0}
      title={CLEAR_CONVERSATION_LABEL}
      aria-label={CLEAR_CONVERSATION_LABEL}
    >
      <TrashIcon size={15} />
    </button>
    {#if onClose}
      <button class="chat-close-btn" onclick={onClose} aria-label={t('chat.closeChat')}>✕</button>
    {/if}
  </div>

  <div class="chat-messages" role="log" aria-live="polite" aria-atomic="false" bind:this={messagesEl}>
    {#each chatSession.messages as msg, i (i)}
      {#if msg.role !== 'assistant' || msg.content !== ''}
        <div class="chat-message {msg.role}" role="listitem" oncontextmenu={(e) => handleMessageContextMenu(e, msg, i)}>
          {#if msg.role === 'assistant'}
            <div class="msg-body">{@html renderMarkdown(msg.content)}</div>
          {:else}
            <p>{msg.content}</p>
          {/if}
        </div>
      {/if}
    {:else}
      <p class="chat-empty">{t('chat.emptyState')}</p>
    {/each}

    {#if chatSession.isLoading && (chatSession.messages.length === 0 || chatSession.messages[chatSession.messages.length - 1]?.role !== 'assistant' || chatSession.messages[chatSession.messages.length - 1]?.content === '')}
      <div class="chat-message assistant loading">
        <p>{t('chat.thinking')}</p>
      </div>
    {/if}
  </div>

  {#if chatSession.notesError}
    <p class="chat-error">{chatSession.notesError}</p>
  {/if}

  {#if error || chatSession.streamError}
    <p class="chat-error">{error || chatSession.streamError}</p>
  {/if}

  {#if chatSession.sourcesUsed.length > 0 || chatSession.wikiSourcesUsed.length > 0}
    {#if !chatSession.isLoading}
    <details class="chat-sources">
      <summary class="chat-sources-summary">{t('chat.sourcesSummary', { count: chatSession.sourcesUsed.length + chatSession.wikiSourcesUsed.length })}</summary>
      <div class="chat-sources-pills">
        {#each chatSession.sourcesUsed as title}
          <span class="chat-source-pill">{title}</span>
        {/each}
        {#each chatSession.wikiSourcesUsed as src}
          <button
            class="chat-source-pill chat-source-wiki"
            onclick={() => onOpenWikipediaArticle?.(src.bundleId, src.articlePath, src.title)}
            title={t('chat.openWikiArticle', { title: src.title })}
          >{t('chat.wikiPillPrefix')}{src.title}</button>
        {/each}
      </div>
    </details>
    {/if}
  {/if}

  {#if import.meta.env.DEV}
  <details class="debug-search" bind:open={debugOpen}>
    <summary>{t('chat.debugRawScores')}</summary>
    <div class="debug-input-row">
      <input bind:value={debugQuery} placeholder={t('chat.debugQueryPlaceholder')} onkeydown={e => e.key === 'Enter' && runDebugSearch()} />
      <button onclick={runDebugSearch}>{t('chat.debugSearch')}</button>
    </div>
    {#if debugResults.length > 0}
      <p class="debug-section-label">{t('chat.debugNotes')}</p>
      <table class="debug-table">
        <thead><tr><th>{t('chat.debugDist')}</th><th>{t('chat.debugTitle')}</th><th>{t('chat.debugExcerpt')}</th></tr></thead>
        <tbody>
          {#each debugResults as r}
            <tr class:debug-pass={r.distance <= 1.1} class:debug-fail={r.distance > 1.1}>
              <td>{r.distance.toFixed(3)}</td>
              <td>{r.title}</td>
              <td>{r.excerpt}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#if debugWikiResults.length > 0}
      <p class="debug-section-label">{t('chat.debugWikipedia')}</p>
      <table class="debug-table">
        <thead><tr><th>{t('chat.debugDist')}</th><th>{t('chat.debugTitle')}</th><th>{t('chat.debugExcerpt')}</th></tr></thead>
        <tbody>
          {#each debugWikiResults as r}
            <tr class:debug-pass={r.distance <= 1.35} class:debug-fail={r.distance > 1.35}>
              <td>{r.distance.toFixed(3)}</td>
              <td>{r.title}</td>
              <td>{r.excerpt}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </details>
  {/if}

  <div class="chat-input-row">
    <textarea
      bind:this={inputEl}
      bind:value={input}
      onkeydown={handleKeydown}
      placeholder={inputPlaceholder}
      aria-label={t('chat.messageAria')}
      rows="6"
      disabled={chatSession.isLoading || !chatEnabled}
    ></textarea>
    <button onclick={send} disabled={chatSession.isLoading || !input.trim() || !chatEnabled} aria-busy={chatSession.isLoading}>{t('chat.send')}</button>
  </div>
</aside>
