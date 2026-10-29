import { t } from '../i18n/t.js';
import { toReadablePlainText } from './readableText.js';

/**
 * Fail closed: only an explicit unlocked flag may copy.
 * @param {unknown} locked
 */
export function isNoteBodyReadable(locked) {
  return locked === false;
}

/**
 * Prefer the notes-list row lock flag (refreshed after folder lock) over a
 * possibly stale activeNote. Missing/unknown → locked.
 * @param {{ locked?: unknown } | null | undefined} listNote
 * @param {{ locked?: unknown } | null | undefined} activeNote
 */
export function effectiveCopyLockFlag(listNote, activeNote) {
  if (listNote) return listNote.locked === false ? false : true;
  return activeNote?.locked === false ? false : true;
}

/** @param {string} text */
async function defaultWriteText(text) {
  await navigator.clipboard.writeText(text);
}

/**
 * @param {{
 *   locked: unknown,
 *   body?: string | null,
 *   onError?: (e: unknown) => void,
 *   writeText?: (text: string) => Promise<void>,
 * }} opts
 * @returns {Promise<boolean>}
 */
export async function copyNoteMarkdown(opts) {
  const { locked, body, onError, writeText = defaultWriteText } = opts;
  if (!isNoteBodyReadable(locked)) {
    onError?.(t('notes.copyLockedRefuse'));
    return false;
  }
  try {
    await writeText(String(body ?? ''));
    return true;
  } catch (e) {
    onError?.(e);
    return false;
  }
}

/**
 * @param {{
 *   locked: unknown,
 *   body?: string | null,
 *   onError?: (e: unknown) => void,
 *   writeText?: (text: string) => Promise<void>,
 * }} opts
 * @returns {Promise<boolean>}
 */
export async function copyNotePlainText(opts) {
  const { locked, body, onError, writeText = defaultWriteText } = opts;
  if (!isNoteBodyReadable(locked)) {
    onError?.(t('notes.copyLockedRefuse'));
    return false;
  }
  try {
    await writeText(toReadablePlainText(String(body ?? '')));
    return true;
  } catch (e) {
    onError?.(e);
    return false;
  }
}
