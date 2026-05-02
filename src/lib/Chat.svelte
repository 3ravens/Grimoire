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
  import { listen } from '@tauri-apps/api/event';
  import { untrack, tick, onMount } from 'svelte';
  import { FEATURE_GUIDE } from './utils/featureGuide.js';

  // ── Props ──────────────────────────────────────────────────────────────────

  // The note currently open in the editor, passed down from App.svelte.
  // Shape: { id, title, content, ... } | null
  // pendingInsert: { text: string, seq: number } — injected quote from the editor keybind.
  // keepInMemory: when true, keep_alive: -1 is sent so Ollama never unloads the model.
  // llmEnabled: false disables the chat UI and shows a hardware warning banner.
  let { activeNote = null, pendingInsert = null, keepInMemory = false, llmEnabled = true, wikipediaEnabled = false, onClose = null, onContextMenu = null, onInsertIntoNote = null, onOpenWikipediaArticle = null, activeView = null, activeViewFolderId = null, activeViewLabel = '', activeViewFilters = {} } = $props();

  // ── Helpers ────────────────────────────────────────────────────────────────

  /** Format a Tauri AppError `{kind, message}` object into a human-readable string.
   *  Appends an actionable hint for known error kinds. */
  function fmtAppError(e) {
    const msg = e?.message ?? String(e);
    if (e?.kind === 'OllamaUnavailable') return `${msg} — Make sure Ollama is running: ollama serve`;
    if (e?.kind === 'EmbeddingFailed')   return `${msg} — Check that your embedding model is pulled (ollama pull <model>)`;
    return msg;
  }

  // ── State ──────────────────────────────────────────────────────────────────

  // ── Chat input placeholder ──────────────────────────────────────────────────

  const PLACEHOLDERS = [
    'Consult the grimoire…',
    'How to cast a fireball?',
    'What are the ingredients for a healing potion?',
    'Translate this ancient rune…',
    'Ask the oracle…',
    'Summon an answer from the void…',
    'Which spell works best against undead?',
    'Where did I write about the lost city?',
    'What does my future hold? (Ask about your notes)',
    'Speak, mortal…',
  ];

  const inputPlaceholder = PLACEHOLDERS[Math.floor(Math.random() * PLACEHOLDERS.length)];

  // Each message: { role: 'user' | 'assistant', content: string }
  let messages = $state([]);
  let input = $state('');
  let model = $state('llama3.2');
  let isLoading = $state(false);
  let error = $state('');
  let useNotes = $state(true);
  let useWiki = $state(true);
  let useFiles = $state(true);
  let useViewContext = $state(true);
  let useFeatureGuide = $state(true);
  let chatOptsOpen = $state(false);

  // Persist all context toggles to localStorage.
  $effect(() => { localStorage.setItem('grimoire:chat:useNotes',       JSON.stringify(useNotes)); });
  $effect(() => { const v = localStorage.getItem('grimoire:chat:useNotes');       if (v !== null) useNotes       = JSON.parse(v); });
  $effect(() => { localStorage.setItem('grimoire:chat:useWiki',        JSON.stringify(useWiki)); });
  $effect(() => { const v = localStorage.getItem('grimoire:chat:useWiki');        if (v !== null) useWiki        = JSON.parse(v); });
  $effect(() => { localStorage.setItem('grimoire:chat:useFiles',       JSON.stringify(useFiles)); });
  $effect(() => { const v = localStorage.getItem('grimoire:chat:useFiles');       if (v !== null) useFiles       = JSON.parse(v); });
  $effect(() => { localStorage.setItem('grimoire:chat:useViewContext', JSON.stringify(useViewContext)); });
  $effect(() => { const v = localStorage.getItem('grimoire:chat:useViewContext'); if (v !== null) useViewContext = JSON.parse(v); });
  $effect(() => { localStorage.setItem('grimoire:chat:useFeatureGuide', JSON.stringify(useFeatureGuide)); });
  $effect(() => { const v = localStorage.getItem('grimoire:chat:useFeatureGuide'); if (v !== null) useFeatureGuide = JSON.parse(v); });

  // Load chat model from SQLite on mount.
  onMount(() => {
    invoke('get_setting', { key: 'chat_model' })
      .then(val => { if (val !== '') model = val; })
      .catch(() => {});
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

  // Titles of notes injected as context for the most recent message.
  // Empty when notes search is off, failed, or returned nothing.
  let sourcesUsed = $state([]);
  let wikiSourcesUsed = $state([]);
  let notesError = $state('');

  // Reference to the scrollable messages container so we can auto-scroll.
  let messagesEl = $state(null);

  // Scroll to the bottom whenever the messages array changes.
  // tick() waits for Svelte to finish updating the DOM before measuring scrollHeight,
  // otherwise we'd scroll to the pre-update height and land one message short.
  $effect(() => {
    if (messages.length && messagesEl) {
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
      systemParts.push(
        `## Note the user currently has open\n### ${activeNote.title}\n${activeNote.content}`
      );
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
          const selectDef = defs.find(d => d.type === 'select');
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

    if (useNotes) {
      let matches = [];
      try {
        matches = await invoke('search_notes', { query: ragQuery });
      } catch (e) {
        notesError = `Note search failed: ${fmtAppError(e)}`;
      }
      if (matches.length > 0) {
        const byTitle = {};
        for (const m of matches) {
          if (!byTitle[m.title]) byTitle[m.title] = [];
          byTitle[m.title].push(...m.excerpts);
        }
        sourcesUsed = Object.keys(byTitle);
        const context = Object.entries(byTitle)
          .map(([title, excerpts]) => `[Note: "${title}"]\n${excerpts.join('\n')}`)
          .join('\n\n');
        systemParts.push(`USER NOTES:\n${context}`);
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
        wikiSourcesUsed = wikiMatches.map(m => ({
          title: m.title,
          bundleId: m.bundle_id,
          articlePath: m.article_id.includes('/') ? m.article_id.slice(m.article_id.indexOf('/') + 1) : m.article_id,
        }));
        const wikiContext = wikiMatches
          .map(m => `[Wikipedia: "${m.title}"]\n${m.excerpts.join('\n')}`)
          .join('\n\n');
        systemParts.push(`WIKIPEDIA ARTICLES:\n${wikiContext}`);
      }
    }

    // ── 4. Scanned files RAG context ─────────────────────────────────────────
    let fileMatches = [];
    if (useFiles) {
      try {
        fileMatches = await invoke('search_scanned_files', { query: ragQuery });
      } catch (_) {
        // Best-effort — don't block the chat if scanned file search fails.
      }
      if (fileMatches.length > 0) {
        const fileContext = fileMatches
          .map(m => `[File: "${m.title}" (${m.file_path})]\n${m.excerpts.join('\n')}`)
          .join('\n\n');
        systemParts.push(`SCANNED FILES:\n${fileContext}`);
      }
    }

    // ── Assemble system message ──────────────────────────────────────────────
    if (systemParts.length > 0) {
      const hasWikiContext  = wikiSourcesUsed.length > 0;
      const hasNotesContext = sourcesUsed.length > 0;
      const hasFilesContext = fileMatches.length > 0;
      const hasAnyContext   = hasNotesContext || hasWikiContext || hasFilesContext;

      // Build a strict source-priority preamble that varies based on what was actually retrieved.
      let preamble =
        `You are a personal knowledge assistant embedded in Grimoire, a local note-taking app.\n` +
        `You also have access to a feature guide that documents Grimoire's keyboard shortcuts and features.\n`;

      const sourceList = [hasNotesContext && 'notes', hasWikiContext && 'wiki', hasFilesContext && 'files'].filter(Boolean);

      if (sourceList.length >= 2) {
        const labels = { notes: "the user's notes", wiki: 'Wikipedia articles', files: 'scanned files' };
        const sourcesDesc = sourceList.map(s => labels[s]).join(' AND ');
        preamble +=
          `You have been given ${sourcesDesc} for this question.\n` +
          `STRICT SOURCE PRIORITY — follow this order without exception:\n` +
          (hasNotesContext ? `  1. Answer first from the user's notes if they contain genuinely relevant information.\n` : '') +
          (hasWikiContext  ? `  ${hasNotesContext ? 2 : 1}. Then draw on the Wikipedia articles provided.\n` : '') +
          (hasFilesContext ? `  ${(hasNotesContext ? 1 : 0) + (hasWikiContext ? 1 : 0) + 1}. Then draw on the scanned files provided.\n` : '') +
          `  ${sourceList.length + 1}. Only use your own general knowledge to fill gaps that none of the provided sources cover. Do NOT lead with general knowledge when provided sources exist.`;
      } else if (hasWikiContext) {
        preamble +=
          `You have been given Wikipedia articles for this question. The user's notes were not relevant.\n` +
          `STRICT SOURCE PRIORITY — follow this order without exception:\n` +
          `  1. Answer from the Wikipedia articles provided. They are your primary source.\n` +
          `  2. Only use your own general knowledge to fill gaps the Wikipedia articles do not cover. Do NOT lead with general knowledge when Wikipedia articles are available.`;
      } else if (hasFilesContext) {
        preamble +=
          `You have been given scanned files for this question. The user's notes were not relevant.\n` +
          `STRICT SOURCE PRIORITY — follow this order without exception:\n` +
          `  1. Answer from the scanned files provided. They are your primary source.\n` +
          `  2. Only use your own general knowledge to fill gaps the scanned files do not cover.`;
      } else if (hasNotesContext) {
        preamble +=
          `You have been given the user's notes for this question.\n` +
          `STRICT SOURCE PRIORITY — follow this order without exception:\n` +
          `  1. Answer from the user's notes where they are genuinely relevant.\n` +
          `  2. Only use your own general knowledge to fill gaps the notes do not cover.`;
      } else {
        preamble += `No relevant notes, Wikipedia articles, or scanned files were found for this question. Answer from your own general knowledge.`;
      }

      let content;
      if (verbosity === 'caveman') {
        content =
          `${preamble}\n\n` +
          systemParts.join('\n\n') +
          `\n\nINSTRUCTIONS:\n` +
          `- Compress the key facts into telegraphic bullet points — no full sentences, no filler words.\n` +
          `- Example: "• metabolic process • microbes convert sugars → acids/gas/alcohol • lactic: yoghurt, kimchi"\n` +
          `- Only use sources that are directly relevant to the question. Ignore off-topic sources.\n` +
          `- Prefix each bullet with its source: "note:" for user notes, "wiki:" for Wikipedia, "general:" for your own knowledge.\n` +
          `- Wrap any code or diagrams in triple-backtick code blocks.\n` +
          `- Ignore [[ ]] and **.`;
      } else {
        let styleInstruction = '';
        if (verbosity === 'thorough') {
          styleInstruction = '\n\nSTYLE: Provide thorough, detailed answers with full context. Do not skip nuance.';
        }
        content =
          `${preamble}\n\n` +
          systemParts.join('\n\n') +
          `\n\nINSTRUCTIONS:\n` +
          `1. RELEVANCE GATE — Before using any source, ask: does this source directly answer the question? If not, discard it completely. Do not mention discarded sources. A source that merely shares vocabulary with the question is NOT relevant.\n` +
          `2. Answer in natural prose. Do not output section labels, headers, or structural markers like [Note: …] or [Wikipedia: …].\n` +
          `3. Follow the source priority above strictly. Do not open with general knowledge if provided sources cover the topic.\n` +
          `4. Wrap all code samples, command examples, ASCII art, and diagrams in triple-backtick fenced code blocks.\n` +
          `5. Every sentence drawn from a source MUST be attributed inline — no exceptions:\n` +
          `   - User notes: begin with "In your note on X, …" or "Your note on X explains that …" (use the exact note title)\n` +
          `   - Wikipedia: begin with "According to Wikipedia's article on X, …" or "Wikipedia (X) explains that …" (use the exact article title)\n` +
          `   - Scanned files: begin with "From your file X, …" or "Your file X states that …" (use the exact file title)\n` +
          `   - General knowledge (only as fallback): begin with "Based on general knowledge, …"\n` +
          `   - When switching sources mid-answer, explicitly signal the transition.\n` +
          `6. Never fabricate a source attribution. Ignore formatting like [[ ]] or **.` +
          styleInstruction;
      }
      payload = [{ role: 'system', content }, ...history];
    }

    // Push a placeholder assistant message filled in token by token.
    messages = [...history, { role: 'assistant', content: '' }];

    const unlisten = await listen('chat:token', (event) => {
      messages = messages.map((m, i) =>
        i === messages.length - 1
          ? { ...m, content: m.content + event.payload }
          : m
      );
    });

    try {
      await invoke('chat', { model, messages: payload, keepInMemory, temperature, topP: top_p, topK: top_k, repeatPenalty: repeat_penalty, numCtx: num_ctx });
    } finally {
      unlisten();
    }
  }

  async function send() {
    const text = input.trim();
    if (!text || isLoading) return;

    // Append the user message before the await so the UI updates immediately.
    const updated = [...messages, { role: 'user', content: text }];
    messages = updated;
    input = '';
    isLoading = true;
    error = '';
    sourcesUsed = [];
    wikiSourcesUsed = [];
    notesError = '';

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
      // Remove the empty placeholder if the request failed before any tokens arrived.
      if (messages.length > 0 && messages[messages.length - 1].role === 'assistant'
          && messages[messages.length - 1].content === '') {
        messages = messages.slice(0, -1);
      }
      error = fmtAppError(e);
    } finally {
      isLoading = false;
    }
  }

  function deleteMessage(index) {
    if (isLoading) return;
    messages = messages.filter((_, i) => i !== index);
  }

  async function regenerate() {
    if (isLoading) return;
    // Strip the last assistant message so we end on a user message.
    const history = messages[messages.length - 1]?.role === 'assistant'
      ? messages.slice(0, -1)
      : messages;
    if (history.length === 0 || history[history.length - 1]?.role !== 'user') return;

    isLoading = true;
    error = '';
    sourcesUsed = [];
    wikiSourcesUsed = [];
    notesError = '';

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
      if (messages.length > 0 && messages[messages.length - 1].role === 'assistant'
          && messages[messages.length - 1].content === '') {
        messages = messages.slice(0, -1);
      }
      error = fmtAppError(e);
    } finally {
      isLoading = false;
    }
  }

  // ── Context menu ────────────────────────────────────────────────────────────

  function handleMessageContextMenu(e, msg, i) {
    e.preventDefault();
    const isLastAssistant = i === messages.length - 1 && msg.role === 'assistant';

    const items = [
      {
        label: 'Copy',
        action: () => navigator.clipboard.writeText(msg.content),
      },
      {
        label: 'Copy as quote',
        action: () => navigator.clipboard.writeText(`"${msg.content}"`),
      },
      ...(onInsertIntoNote ? [
        { divider: true },
        {
          label: 'Insert into note',
          action: () => onInsertIntoNote(msg.content),
        },
      ] : []),
      { divider: true },
      ...(isLastAssistant ? [{
        label: 'Regenerate',
        disabled: isLoading,
        action: regenerate,
      }] : []),
      {
        label: 'Delete',
        danger: true,
        disabled: isLoading,
        action: () => deleteMessage(i),
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

<aside class="chat-panel">
  {#if !llmEnabled}
    <div class="chat-hw-banner">
      <strong>LLM features unavailable.</strong>
      Your hardware doesn’t meet the minimum requirements for chat. You can override this in
      <strong>Settings → Hardware</strong>.
    </div>
  {/if}

  <div class="chat-header">
    <span class="chat-title">Chat</span>
    <input
      class="model-input"
      bind:value={model}
      placeholder="model"
      title="Ollama model name (e.g. llama3.2)"
      onchange={() => invoke('set_setting', { key: 'chat_model', value: model }).catch(() => {})}
    />
    <div class="chat-opts-wrap">
      <button
        class="chat-opts-btn"
        class:active={chatOptsOpen}
        onclick={() => (chatOptsOpen = !chatOptsOpen)}
        title="Context options"
        aria-label="Context options"
        aria-expanded={chatOptsOpen}
      >
        <!-- Sliders icon -->
        <svg width="14" height="14" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <line x1="3"  y1="3"  x2="3"  y2="12"/>
          <line x1="8"  y1="3"  x2="8"  y2="12"/>
          <line x1="13" y1="3"  x2="13" y2="12"/>
          <rect x="1"   y="5.5" width="4"  height="2" rx="1"/>
          <rect x="6"   y="8.5" width="4"  height="2" rx="1"/>
          <rect x="11"  y="4"   width="4"  height="2" rx="1"/>
        </svg>
      </button>

      {#if chatOptsOpen}
        <div class="chat-opts-dropdown" role="menu">
          <label class="chat-opt-row" title="Search your notes and inject the most relevant ones as context before each message (requires nomic-embed-text)">
            <input type="checkbox" bind:checked={useNotes} />
            Use notes
          </label>
          <label class="chat-opt-row" class:disabled={!wikipediaEnabled} title="Search the indexed Wikipedia catalogue and inject relevant articles as context">
            <input type="checkbox" bind:checked={useWiki} disabled={!wikipediaEnabled} />
            Use wiki
          </label>
          <label class="chat-opt-row" title="Search scanned files (added in Settings → File Scanner) and inject relevant excerpts as context">
            <input type="checkbox" bind:checked={useFiles} />
            Use files
          </label>
          <label class="chat-opt-row" class:disabled={!activeView} title="Include the current board or table view state as context for the LLM">
            <input type="checkbox" bind:checked={useViewContext} disabled={!activeView} />
            Use view
          </label>
          <label class="chat-opt-row" title="Include a feature guide describing Grimoire keyboard shortcuts, view types, and commands">
            <input type="checkbox" bind:checked={useFeatureGuide} />
            Use guide
          </label>
        </div>
      {/if}
    </div>
    {#if onClose}
      <button class="chat-close-btn" onclick={onClose} aria-label="Close chat">✕</button>
    {/if}
  </div>

  <div class="chat-messages" role="log" aria-live="polite" aria-atomic="false" bind:this={messagesEl}>
    {#each messages as msg, i (i)}
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
      <p class="chat-empty">Consult the grimoire.</p>
    {/each}

    {#if isLoading && (messages.length === 0 || messages[messages.length - 1]?.role !== 'assistant' || messages[messages.length - 1]?.content === '')}
      <div class="chat-message assistant loading">
        <p>Thinking…</p>
      </div>
    {/if}
  </div>

  {#if notesError}
    <p class="chat-error">{notesError}</p>
  {/if}

  {#if error}
    <p class="chat-error">{error}</p>
  {/if}

  {#if sourcesUsed.length > 0 || wikiSourcesUsed.length > 0}
    {#if !isLoading}
    <details class="chat-sources">
      <summary class="chat-sources-summary">Sources ({sourcesUsed.length + wikiSourcesUsed.length})</summary>
      <div class="chat-sources-pills">
        {#each sourcesUsed as title}
          <span class="chat-source-pill">{title}</span>
        {/each}
        {#each wikiSourcesUsed as src}
          <button
            class="chat-source-pill chat-source-wiki"
            onclick={() => onOpenWikipediaArticle?.(src.bundleId, src.articlePath, src.title)}
            title="Open article: {src.title}"
          >W · {src.title}</button>
        {/each}
      </div>
    </details>
    {/if}
  {/if}

  {#if import.meta.env.DEV}
  <details class="debug-search" bind:open={debugOpen}>
    <summary>Debug: raw scores</summary>
    <div class="debug-input-row">
      <input bind:value={debugQuery} placeholder="query…" onkeydown={e => e.key === 'Enter' && runDebugSearch()} />
      <button onclick={runDebugSearch}>Search</button>
    </div>
    {#if debugResults.length > 0}
      <p class="debug-section-label">Notes</p>
      <table class="debug-table">
        <thead><tr><th>dist</th><th>title</th><th>excerpt</th></tr></thead>
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
      <p class="debug-section-label">Wikipedia</p>
      <table class="debug-table">
        <thead><tr><th>dist</th><th>title</th><th>excerpt</th></tr></thead>
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
      aria-label="Message"
      rows="6"
      disabled={isLoading || !llmEnabled}
    ></textarea>
    <button onclick={send} disabled={isLoading || !input.trim() || !llmEnabled} aria-busy={isLoading}>Send</button>
  </div>
</aside>
