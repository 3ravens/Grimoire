import { t } from './t.js';

/**
 * Map an AppError (or unknown) to a user-facing English banner string.
 * Catalogues only the frontend-owned kind wrappers; Rust `message` stays as-is.
 * @param {unknown} e
 * @returns {string}
 */
export function formatAppError(e) {
  const kind = /** @type {any} */ (e)?.kind;
  const msg = /** @type {any} */ (e)?.message ?? String(e);

  if (kind === 'OllamaUnavailable') {
    return t('errors.ollamaUnavailable', { msg });
  }
  if (kind === 'EmbeddingFailed') {
    return t('errors.embeddingFailed', { msg });
  }
  if (kind === 'NotFound') {
    return t('errors.notFound', { msg });
  }
  if (kind === 'Auth') {
    return t('errors.auth', { msg });
  }
  if (kind) {
    return msg;
  }
  return String(e);
}
