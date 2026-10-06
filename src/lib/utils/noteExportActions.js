import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import {
  buildStandaloneReadModeHtml,
  printStandaloneHtml,
} from './noteExportHtml.js';
import { t } from '../i18n/t.js';

/**
 * Use live editor text when this note is the active tab; otherwise the list row snapshot.
 * @param {{ activeNote: { id: number } | null, editorTitle: string, editorContent: string }} ns
 * @param {{ id: number, title: string, content: string }} note
 */
export function resolveExportPayload(ns, note) {
  if (ns.activeNote?.id === note.id) {
    return { title: ns.editorTitle, body: ns.editorContent };
  }
  return { title: note.title, body: note.content };
}

/** Safe default filename segment from a note title (mirrors Rust sanitise loosely). */
export function sanitiseExportBasename(title) {
  const s = String(title ?? '')
    .replace(/[/\\:*?"<>|]/g, '-')
    .trim()
    .replace(/\s+/g, ' ');
  return s || t('export.defaultBasename');
}

/**
 * @param {{ noteId: number, title: string, body: string, onError: (e: unknown) => void }} opts
 */
export async function exportNoteMarkdown(opts) {
  const { noteId, title, body, onError } = opts;
  try {
    const base = sanitiseExportBasename(title);
    const path = await save({
      title: t('export.markdownTitle'),
      defaultPath: `${base}.md`,
      filters: [{ name: t('export.filterMarkdown'), extensions: ['md'] }],
    });
    if (!path) return;
    await invoke('export_single_note_markdown', {
      noteId,
      destPath: path,
      markdown: body,
    });
  } catch (e) {
    onError?.(e);
  }
}

/**
 * @param {{ noteId: number, title: string, body: string, onError: (e: unknown) => void }} opts
 */
export async function exportNoteHtml(opts) {
  const { noteId, title, body, onError } = opts;
  try {
    const html = await buildStandaloneReadModeHtml(title, body, { rootNoteId: noteId });
    const base = sanitiseExportBasename(title);
    const path = await save({
      title: t('export.htmlTitle'),
      defaultPath: `${base}.html`,
      filters: [{ name: t('export.filterHtml'), extensions: ['html', 'htm'] }],
    });
    if (!path) return;
    await invoke('save_note_html_export', {
      noteId,
      destPath: path,
      html,
    });
  } catch (e) {
    onError?.(e);
  }
}

/**
 * @param {{ noteId: number, title: string, body: string, onError: (e: unknown) => void }} opts
 */
export async function exportNotePdfPrint(opts) {
  const { noteId, title, body, onError } = opts;
  try {
    await invoke('log_note_export_pdf_print', { noteId });
    const html = await buildStandaloneReadModeHtml(title, body, { rootNoteId: noteId });
    printStandaloneHtml(html, sanitiseExportBasename(title));
  } catch (e) {
    onError?.(e);
  }
}
