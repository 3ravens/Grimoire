import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { computeDiff, applyAcceptedHunks } from '../utils/diff.js';

/**
 * @typedef {{ type: 'unchanged' | 'add' | 'remove', lines: string[] }} DiffHunk
 * @typedef {{ status: string, instruction: string, improvedText: string, hunks: DiffHunk[], originalContent: string, acceptedIndices: number[], rejectedIndices: number[] }} ImproveState
 * @typedef {{ status: string, hunkIndex: number | null, x: number, y: number }} RefineState
 * @typedef {{ id: any, title?: string, [k: string]: any }} ActiveNote
 */

/** @type {ImproveState} */
const IDLE_IMPROVE = Object.freeze({
  status: 'idle', instruction: '', improvedText: '',
  hunks: [], originalContent: '', acceptedIndices: [], rejectedIndices: [],
});
/** @type {RefineState} */
const IDLE_REFINE = Object.freeze({ status: 'idle', hunkIndex: null, x: 0, y: 0 });

/**
 * @param {{
 *   onError: (e: unknown) => void,
 *   getEditorContent: () => string,
 *   setEditorContent: (v: string) => void,
 *   markDirty: () => void,
 *   getActiveNote: () => ActiveNote | null,
 * }} opts
 */
export function createImproveService({ onError, getEditorContent, setEditorContent, markDirty, getActiveNote }) {
  /** @type {ImproveState} */
  let improveState = $state({ ...IDLE_IMPROVE });
  /** @type {RefineState} */
  let refineState  = $state({ ...IDLE_REFINE });

  // Accumulated tokens live outside improveState/refineState so the Tauri
  // event listener always closes over the latest value via a local variable.
  let improveAccumulated = $state('');
  let refineAccumulated  = '';

  /** @type {import('@tauri-apps/api/event').UnlistenFn | null} */
  let improveUnlisten = null;
  /** @type {import('@tauri-apps/api/event').UnlistenFn | null} */
  let refineUnlisten  = null;

  // ── helpers ──────────────────────────────────────────────────────────────

  /** @param {DiffHunk[]} hunks */
  function allChangedIndices(hunks) {
    return hunks.map((h, i) => h.type !== 'unchanged' ? i : -1).filter(i => i !== -1);
  }

  /**
   * @param {DiffHunk[]} hunks
   * @param {number} index
   */
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

  /** @param {ImproveState} state */
  function applyAndClose(state) {
    const { hunks, originalContent, improvedText, acceptedIndices: accepted, rejectedIndices } = state;
    const allChanged = allChangedIndices(hunks);
    const undecided  = allChanged.filter(i => !accepted.includes(i) && !rejectedIndices.includes(i));
    const finalAccepted = new Set([...accepted, ...undecided]);
    if (finalAccepted.size > 0) {
      if (finalAccepted.size === allChanged.length && rejectedIndices.length === 0 && accepted.length === 0) {
        setEditorContent(improvedText);
      } else {
        setEditorContent(applyAcceptedHunks(hunks, finalAccepted, originalContent, improvedText));
      }
      markDirty();
    }
    improveState = { ...IDLE_IMPROVE };
  }

  async function loadModelSettings() {
    const model        = await invoke('get_setting', { key: 'chat_model' }) || 'llama3.2';
    const temperature  = await invoke('get_setting', { key: 'chat_temperature' });
    const topP         = await invoke('get_setting', { key: 'chat_top_p' });
    const topK         = await invoke('get_setting', { key: 'chat_top_k' });
    const repeatPenalty = await invoke('get_setting', { key: 'chat_repeat_penalty' });
    const numCtx       = await invoke('get_setting', { key: 'chat_num_ctx' });
    return {
      model,
      temperature:   temperature   !== '' ? Number(temperature)   : 0.8,
      topP:          topP          !== '' ? Number(topP)          : 0.9,
      topK:          topK          !== '' ? Number(topK)          : 40,
      repeatPenalty: repeatPenalty !== '' ? Number(repeatPenalty) : 1.1,
      numCtx:        numCtx        !== '' ? Number(numCtx)        : 8192,
    };
  }

  // ── public API ────────────────────────────────────────────────────────────

  function startImprove() {
    if (improveState.status !== 'idle') return;
    improveState = { ...improveState, status: 'prompt' };
  }

  /** Cancel any in-progress improve/refine when the user navigates away. */
  function cancelIfActive() {
    if (improveState.status !== 'idle') {
      if (improveUnlisten) { improveUnlisten(); improveUnlisten = null; }
      improveState = { ...IDLE_IMPROVE };
    }
    if (refineState.status !== 'idle') {
      if (refineUnlisten) { refineUnlisten(); refineUnlisten = null; }
      refineState = { ...IDLE_REFINE };
    }
  }

  /** @param {string} instruction */
  async function handleImproveStart(instruction) {
    if (!getActiveNote()) return;
    improveAccumulated = '';
    const content = getEditorContent();
    improveState = { status: 'streaming', instruction, improvedText: '', hunks: [], originalContent: content, acceptedIndices: [], rejectedIndices: [] };

    const unlistenP = listen('note:improve-token', (event) => {
      const token = /** @type {string} */ (event.payload);
      improveAccumulated += token;
      improveState = { ...improveState, improvedText: improveAccumulated };
    });
    const unlisten = await unlistenP;
    improveUnlisten = unlisten;

    try {
      const activeNote = getActiveNote();
      const settings   = await loadModelSettings();
      await invoke('suggest_note_improvement', {
        ...settings,
        noteContent: content,
        instruction,
        noteId:    activeNote?.id    ?? null,
        noteTitle: activeNote?.title ?? null,
      });
      unlisten();
      improveUnlisten = null;
      const hunks = computeDiff(improveState.originalContent, improveAccumulated);
      improveState = { ...improveState, status: 'diff', hunks, acceptedIndices: [], rejectedIndices: [] };
    } catch (e) {
      unlisten();
      improveUnlisten = null;
      onError(e);
      improveState = { ...IDLE_IMPROVE };
    }
  }

  function handleImproveAcceptAll() {
    applyAndClose(improveState);
  }

  function handleImproveRejectAll() {
    improveState = { ...IDLE_IMPROVE };
  }

  /** @param {number} hunkIndex */
  function handleImproveAcceptHunk(hunkIndex) {
    const hunk = improveState.hunks[hunkIndex];
    if (!hunk || hunk.type === 'unchanged') return;
    if (improveState.acceptedIndices.includes(hunkIndex)) return;
    const pair = pairedHunkIndex(improveState.hunks, hunkIndex);
    const indices = pair !== -1 && !improveState.acceptedIndices.includes(pair) && !improveState.rejectedIndices.includes(pair)
      ? [hunkIndex, pair] : [hunkIndex];
    const newAccepted = [...improveState.acceptedIndices, ...indices];
    const allChanged  = allChangedIndices(improveState.hunks);
    const allDecided  = allChanged.every(i => newAccepted.includes(i) || improveState.rejectedIndices.includes(i));
    if (allDecided) {
      applyAndClose({ ...improveState, acceptedIndices: newAccepted });
    } else {
      improveState = { ...improveState, acceptedIndices: newAccepted };
    }
  }

  /** @param {number} hunkIndex */
  function handleImproveRejectHunk(hunkIndex) {
    const hunk = improveState.hunks[hunkIndex];
    if (!hunk || hunk.type === 'unchanged') return;
    if (improveState.rejectedIndices.includes(hunkIndex)) return;
    const pair = pairedHunkIndex(improveState.hunks, hunkIndex);
    const indices = pair !== -1 && !improveState.rejectedIndices.includes(pair) && !improveState.acceptedIndices.includes(pair)
      ? [hunkIndex, pair] : [hunkIndex];
    const newRejected = [...improveState.rejectedIndices, ...indices];
    const allChanged  = allChangedIndices(improveState.hunks);
    const allDecided  = allChanged.every(i => improveState.acceptedIndices.includes(i) || newRejected.includes(i));
    if (allDecided) {
      applyAndClose({ ...improveState, rejectedIndices: newRejected });
    } else {
      improveState = { ...improveState, rejectedIndices: newRejected };
    }
  }

  /**
   * @param {number} hunkIndex
   * @param {number} x
   * @param {number} y
   */
  function handleRefineHunk(hunkIndex, x, y) {
    refineState = { status: 'prompt', hunkIndex, x, y };
  }

  function handleRefineCancel() {
    if (refineUnlisten) { refineUnlisten(); refineUnlisten = null; }
    refineState = { ...IDLE_REFINE };
  }

  /** @param {string} instruction */
  async function handleRefineSend(instruction) {
    const { hunkIndex } = refineState;
    if (hunkIndex === null) return;
    /** @type {number} */
    const idx = hunkIndex;
    const hunks = improveState.hunks;
    const hunk  = hunks[idx];
    if (!hunk) return;

    // Prefer the original (remove) lines so the LLM works on the pre-improve text.
    let hunkContent;
    if (hunk.type === 'add') {
      const removePair = pairedHunkIndex(hunks, idx);
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
      const settings = await loadModelSettings();
      await invoke('suggest_hunk_refinement', { ...settings, hunkContent, instruction });
      unlisten();
      refineUnlisten = null;

      const addIndex = hunk.type === 'add' ? idx : pairedHunkIndex(hunks, idx);
      if (addIndex !== -1) {
        const newHunks = hunks.map((h, i) =>
          i === addIndex ? { ...h, lines: refineAccumulated.split('\n') } : h
        );
        improveState = { ...improveState, hunks: newHunks };
      }
    } catch (e) {
      unlisten();
      refineUnlisten = null;
      onError(e);
    }

    refineState = { ...IDLE_REFINE };
  }

  return {
    get improveState()    { return improveState; },
    set improveState(v)   { improveState = v; },
    get refineState()     { return refineState; },
    set refineState(v)    { refineState = v; },
    startImprove,
    cancelIfActive,
    handleImproveStart,
    handleImproveAcceptAll,
    handleImproveRejectAll,
    handleImproveAcceptHunk,
    handleImproveRejectHunk,
    handleRefineHunk,
    handleRefineSend,
    handleRefineCancel,
  };
}
