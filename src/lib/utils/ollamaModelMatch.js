/**
 * Match Ollama installed names to requested ids (mirrors `ollama_models.rs`).
 * @param {string} s
 * @returns {[string, string | null]}
 */
export function splitOllamaModelName(s) {
  const t = String(s).trim();
  if (!t) return ['', null];
  const i = t.indexOf(':');
  if (i === -1) return [t, null];
  const tag = t.slice(i + 1);
  return tag ? [t.slice(0, i), tag] : [t, null];
}

/**
 * @param {string} installedFull e.g. `phi3:latest`
 * @param {string} requested e.g. `phi3` or `phi3:latest`
 */
export function ollamaInstalledMatchesRequest(installedFull, requested) {
  const req = String(requested).trim();
  if (!req) return false;
  if (installedFull === req) return true;
  const [reqBase, reqTag] = splitOllamaModelName(req);
  const [insBase, insTag] = splitOllamaModelName(installedFull);
  if (reqBase !== insBase) return false;
  if (reqTag == null) return true;
  return insTag === reqTag;
}

/**
 * First full installed name that satisfies `requested`, or null.
 * @param {string} requested
 * @param {string[]} installed
 */
export function firstInstalledFullName(requested, installed) {
  for (const full of installed) {
    if (ollamaInstalledMatchesRequest(full, requested)) return full;
  }
  return null;
}
