// Copyright (C) 2026 Wim Palland
// This file is part of Grimoire — licensed under GPL-3.0 or later.

import { invoke } from '@tauri-apps/api/core';
import { marked } from 'marked';
import { sanitizeNoteHtml } from './sanitize.js';

/** Maximum nesting depth for `![[title]]` embeds (inclusive of first expansion). */
export const TRANSCLUSION_MAX_DEPTH = 5;

/**
 * @param {string} s
 */
function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/**
 * Advance past a fenced ``` block starting at `startIdx` (index of the first `).
 * @param {string} source
 * @param {number} startIdx
 */
function skipFencedCodeBlock(source, startIdx) {
  if (source.slice(startIdx, startIdx + 3) !== '```') return startIdx;
  let i = startIdx + 3;
  while (i < source.length && source[i] !== '\n') i++;
  if (i < source.length) i++;

  while (i < source.length) {
    if (source[i] === '`' && source.slice(i, i + 3) === '```') {
      let end = i + 3;
      while (end < source.length && source[end] !== '\n') end++;
      if (end < source.length) end++;
      return end;
    }
    i++;
  }
  return source.length;
}

/**
 * Split markdown into alternating plain segments and embed placeholders.
 * `![[...]]` inside fenced code blocks is left as plain text.
 *
 * @param {string} source
 * @returns {{ type: 'text' | 'embed', value: string }[]}
 */
export function splitMarkdownByEmbeds(source) {
  const parts = [];
  let i = 0;
  while (i < source.length) {
    if (source.slice(i, i + 3) === '```') {
      const after = skipFencedCodeBlock(source, i);
      parts.push({ type: 'text', value: source.slice(i, after) });
      i = after;
      continue;
    }

    const embedStart = source.indexOf('![[', i);
    if (embedStart === -1) {
      parts.push({ type: 'text', value: source.slice(i) });
      break;
    }
    if (embedStart > i) {
      parts.push({ type: 'text', value: source.slice(i, embedStart) });
    }
    const close = source.indexOf(']]', embedStart + 3);
    if (close === -1) {
      parts.push({ type: 'text', value: source.slice(embedStart) });
      break;
    }
    const title = source.slice(embedStart + 3, close).trim();
    parts.push({ type: 'embed', value: title });
    i = close + 2;
  }
  return parts;
}

/**
 * @typedef {{ found: boolean, id?: number | null, locked: boolean, content: string }} EmbedResolve
 */

/**
 * Render markdown with `![[note title]]` transclusions expanded (read-only embeds).
 *
 * @param {string} markdown
 * @param {{ rootNoteId?: number | null }} [opts]
 * @returns {Promise<string>} HTML (same safety model as `marked.parse`).
 */
export async function renderTransclusionMarkdownToHtml(markdown, opts = {}) {
  const rootId = opts.rootNoteId ?? null;
  const initialStack = rootId != null ? [rootId] : [];
  /** @type {Map<string, EmbedResolve>} */
  const memo = new Map();

  async function ensureResolved(titles) {
    const need = titles.filter((t) => t && !memo.has(t));
    if (need.length === 0) return;
    try {
      /** @type {Record<string, EmbedResolve>} */
      const batch = await invoke('resolve_note_embed_batch', { titles: need });
      for (const t of need) {
        const row = batch[t];
        memo.set(t, row ?? { found: false, locked: false, content: '' });
      }
    } catch {
      for (const t of need) {
        memo.set(t, { found: false, locked: false, content: '' });
      }
    }
  }

  /**
   * @param {string} md
   * @param {number[]} stack Note ids in the inclusion chain
   * @param {number} depth 0 = root body
   */
  async function renderChunk(md, stack, depth) {
    if (depth > TRANSCLUSION_MAX_DEPTH) {
      return '<p class="note-embed-stub note-embed-stub--depth">Embedded note omitted (maximum depth reached)</p>';
    }

    const parts = splitMarkdownByEmbeds(md);
    const embedKeys = [
      ...new Set(
        parts
          .filter((p) => p.type === 'embed')
          .map((p) => p.value.trim())
          .filter(Boolean),
      ),
    ];
    await ensureResolved(embedKeys);

    const out = [];
    for (const part of parts) {
      if (part.type === 'text') {
        out.push(sanitizeNoteHtml(marked.parse(part.value)));
        continue;
      }

      const title = part.value.trim();
      if (!title) {
        out.push(
          '<p class="note-embed-stub note-embed-stub--missing"><span class="note-embed-stub-label">Note not found:</span> <em>(empty title)</em></p>',
        );
        continue;
      }

      const r = memo.get(title) ?? { found: false, locked: false, content: '' };
      if (!r.found) {
        out.push(
          `<p class="note-embed-stub note-embed-stub--missing"><span class="note-embed-stub-label">Note not found:</span> <em>${escapeHtml(title)}</em></p>`,
        );
        continue;
      }
      if (r.locked) {
        out.push(
          `<p class="note-embed-stub note-embed-stub--locked"><span class="note-embed-stub-label">Locked note</span> (unlock the folder to view): <em>${escapeHtml(title)}</em></p>`,
        );
        continue;
      }

      const id = r.id ?? null;
      if (id != null && stack.includes(id)) {
        out.push(
          '<p class="note-embed-stub note-embed-stub--circular">Embedded note omitted (circular reference)</p>',
        );
        continue;
      }

      const inner = await renderChunk(
        r.content ?? '',
        id != null ? [...stack, id] : stack,
        depth + 1,
      );
      const idAttr = id != null ? ` data-embed-note-id="${id}"` : '';
      out.push(
        `<div class="note-embed"${idAttr}><div class="note-embed-border"><div class="note-embed-inner read-mode-content">${inner}</div></div></div>`,
      );
    }
    return out.join('');
  }

  return renderChunk(markdown ?? '', initialStack, 0);
}
