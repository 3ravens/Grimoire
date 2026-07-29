import { CURATED_CHAT_MODELS } from '../constants/chatModels.js';

/**
 * Curated chat models shown in the installation wizard (~5), by host LLM tier.
 * @param {string | undefined} capability from `get_hardware_info` (`full` | `embeddingOnly` | `insufficient`)
 */
export function wizardCuratedChatModels(capability) {
  const c = String(capability ?? 'embeddingOnly').toLowerCase();
  /** @type {Set<string>} */
  let allow;
  if (c === 'insufficient') {
    allow = new Set(['gemma2:2b', 'phi3', 'llama3.2']);
  } else if (c === 'embeddingonly') {
    allow = new Set(['gemma2:2b', 'phi3', 'llama3.2', 'mistral', 'llama3.1:8b']);
  } else {
    allow = new Set([
      'llama3.2',
      'mistral',
      'phi3',
      'gemma2:2b',
      'llama3.1:8b',
      'codellama',
      'qwen2.5:14b',
      'llama3:70b',
    ]);
  }
  return CURATED_CHAT_MODELS.filter((m) => allow.has(m.value)).slice(0, 8);
}
