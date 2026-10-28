/** Private MIME so pin reorder never becomes a folder-move drop. */
export const FOLDER_PIN_MIME = 'application/x-grimoire-folder-pin';

/** @param {DataTransfer | null | undefined} dt */
export function isFolderPinDrag(dt) {
  if (!dt) return false;
  return [...dt.types].includes(FOLDER_PIN_MIME);
}
