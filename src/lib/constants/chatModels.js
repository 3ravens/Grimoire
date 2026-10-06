import { t } from '../i18n/t.js';

/**
 * Curated Ollama chat models shown in Settings and the chat panel.
 *
 * `statsShort` / `statsDetail` describe typical **default** tags from the Ollama
 * library (disk size and class vary by quantization and exact tag).
 *
 * @typedef {{ value: string, label: string, statsShort: string, statsDetail: string }} CuratedChatModel
 */

/** @type {{ value: string, catalogKey: string }[]} */
const CURATED_CHAT_MODEL_META = [
  { value: 'llama3.2', catalogKey: 'llama32' },
  { value: 'phi3', catalogKey: 'phi3' },
  { value: 'gemma2:2b', catalogKey: 'gemma2' },
  { value: 'mistral', catalogKey: 'mistral' },
  { value: 'codellama', catalogKey: 'codellama' },
  { value: 'llama3.1:8b', catalogKey: 'llama318b' },
  { value: 'qwen2.5:14b', catalogKey: 'qwen14b' },
  { value: 'llama3:70b', catalogKey: 'llama70b' },
];

/** @type {CuratedChatModel[]} */
export const CURATED_CHAT_MODELS = CURATED_CHAT_MODEL_META.map((m) => ({
  value: m.value,
  label: t(`chatModels.${m.catalogKey}.label`),
  statsShort: t(`chatModels.${m.catalogKey}.statsShort`),
  statsDetail: t(`chatModels.${m.catalogKey}.statsDetail`),
}));

/** Default chat model id (first curated preset). */
export const DEFAULT_CHAT_MODEL = CURATED_CHAT_MODELS[0]?.value ?? 'llama3.2';

/** Curated embedding models (Settings → LLM). Not shown in the chat model picker. */
export const CURATED_EMBEDDING_MODELS = [
  { value: 'nomic-embed-text', label: t('chatModels.embedNomic') },
  { value: 'mxbai-embed-large', label: t('chatModels.embedMxbai') },
];

/**
 * True if this Ollama id is an embedding / embed API model, not a chat model.
 * @param {string} ollamaModelName
 */
export function isEmbeddingModelId(ollamaModelName) {
  const id = String(ollamaModelName).trim().toLowerCase();
  for (const row of CURATED_EMBEDDING_MODELS) {
    const v = row.value.toLowerCase();
    if (id === v || id.startsWith(`${v}:`)) return true;
  }
  if (id.includes('-embed')) return true;
  if (id.includes('embeddinggemma')) return true;
  return false;
}

/** @param {string} modelId */
export function findCuratedChatModel(modelId) {
  const id = String(modelId).trim().toLowerCase();
  return CURATED_CHAT_MODELS.find(
    (c) => id === c.value.toLowerCase() || id.startsWith(`${c.value.toLowerCase()}:`),
  );
}

/** Installed model name is listed under "Other local" when not covered by a curated id. */
export function isExtraInstalledModel(installedName, curated = CURATED_CHAT_MODELS) {
  return !curated.some(
    (p) => installedName === p.value || installedName.startsWith(`${p.value}:`),
  );
}

/** Tier for unknown / non-curated ids (aligned with `chatModelHardware.js` heuristics). */
const STATS_TIER_BY_VALUE = {
  'llama3.2': 'medium',
  phi3: 'light',
  'gemma2:2b': 'light',
  mistral: 'medium',
  codellama: 'heavy',
  'llama3.1:8b': 'medium',
  'qwen2.5:14b': 'heavy',
  'llama3:70b': 'xlarge',
};

function tierStatsFallback(tier) {
  return {
    statsShort: t(`chatModels.tier.${tier}.statsShort`),
    statsDetail: t(`chatModels.tier.${tier}.statsDetail`),
  };
}

/**
 * @param {string} modelId
 * @returns {'light' | 'medium' | 'heavy' | 'xlarge'}
 */
function inferStatsTierForCustom(modelId) {
  const raw = String(modelId).trim().toLowerCase();
  if (!raw) return 'medium';

  const curated = CURATED_CHAT_MODELS.find(
    (c) => raw === c.value.toLowerCase() || raw.startsWith(`${c.value.toLowerCase()}:`),
  );
  if (curated && STATS_TIER_BY_VALUE[curated.value]) {
    return STATS_TIER_BY_VALUE[curated.value];
  }

  const b = raw;
  if (/\b(70|72|65|40)b\b/.test(b) || /:70/.test(b) || /:65/.test(b) || /:40/.test(b)) return 'xlarge';
  if (/\b(34|33|32|30|22|20|13|14|15)b\b/.test(b)) return 'heavy';
  if (/\b(8|9|7)b\b/.test(b)) return 'medium';
  if (/\b(1|2|3|4)b\b/.test(b) || /\b(tiny|mini|small)\b/.test(b)) return 'light';

  return 'medium';
}

/**
 * Stats lines for any model id (curated row or heuristic for custom / local tags).
 * @param {string} modelId
 */
export function statsForAnyModelId(modelId) {
  const row = findCuratedChatModel(modelId);
  if (row) {
    return { statsShort: row.statsShort, statsDetail: row.statsDetail };
  }
  const tier = inferStatsTierForCustom(modelId);
  return tierStatsFallback(tier);
}
