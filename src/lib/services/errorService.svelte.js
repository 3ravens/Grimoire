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

    const kind = /** @type {any} */ (e)?.kind;
    const msg = /** @type {any} */ (e)?.message ?? String(e);

    if (kind === "OllamaUnavailable") {
      errorMsg = `${msg} — Make sure Ollama is running: ollama serve`;
    } else if (kind === "EmbeddingFailed") {
      errorMsg = `${msg} — Check that your embedding model is pulled (ollama pull <model>)`;
    } else if (kind === "NotFound") {
      errorMsg = `Not found: ${msg}`;
    } else if (kind === "Auth") {
      errorMsg = `Authentication error: ${msg}`;
    } else if (kind) {
      errorMsg = msg;
    } else {
      errorMsg = String(e);
    }

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
