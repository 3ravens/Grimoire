import { formatAppError } from '../i18n/formatAppError.js';

/**
 * Error banner state — owns the error message shown in the top banner
 * and the showError helper that maps AppError.kind to human-readable messages.
 */
export function createErrorService() {
  let errorMsg = $state("");
  /** @type {ReturnType<typeof setTimeout> | null} */
  let clearTimer = null;

  /** @param {unknown} e */
  function showError(e) {
    if (clearTimer != null) clearTimeout(clearTimer);

    errorMsg = formatAppError(e);

    clearTimer = setTimeout(() => {
      errorMsg = "";
      clearTimer = null;
    }, 4000);
  }

  return {
    get errorMsg() { return errorMsg; },
    showError,
  };
}
