/**
 * Curated Ollama chat models shown in Settings and the chat panel.
 *
 * `statsShort` / `statsDetail` describe typical **default** tags from the Ollama
 * library (disk size and class vary by quantization and exact tag).
 *
 * @typedef {{ value: string, label: string, statsShort: string, statsDetail: string }} CuratedChatModel
 */

/** @type {CuratedChatModel[]} */
export const CURATED_CHAT_MODELS = [
  {
    value: 'llama3.2',
    label: 'llama3.2 · general (default)',
    statsShort: '~3B · ~2 GB disk',
    statsDetail:
      'Typical default ~3B class, ~2 GB on disk. Use 8 GB+ RAM; ~6 GB+ GPU VRAM for responsive GPU inference. Exact size depends on tag (e.g. :1b vs :3b).',
  },
  {
    value: 'phi3',
    label: 'phi3 · lightweight',
    statsShort: '~4B · ~2.3 GB disk',
    statsDetail:
      'Microsoft Phi-3 family; default tags are often ~3.8–4B and ~2–2.5 GB on disk. Runs on CPU with 8 GB+ RAM; GPU optional.',
  },
  {
    value: 'gemma2:2b',
    label: 'gemma2:2b · lightweight',
    statsShort: '2B · ~1.6 GB disk',
    statsDetail:
      'Small Gemma 2 checkpoint; ~1.5–2 GB on disk. Suitable for modest RAM; GPU optional.',
  },
  {
    value: 'mistral',
    label: 'mistral · general',
    statsShort: '~7B · ~4.1 GB disk',
    statsDetail:
      'Often Mistral 7B by default (~4 GB on disk). Prefer 12 GB+ RAM; ~6 GB+ VRAM for smooth GPU chat.',
  },
  {
    value: 'codellama',
    label: 'codellama · programming',
    statsShort: '~7–13B · ~4–8 GB disk',
    statsDetail:
      'Code Llama sizes depend on tag (7b / 13b / 34b). Default is often 7B (~4 GB). Larger tags need much more disk and RAM.',
  },
  {
    value: 'llama3.1:8b',
    label: 'llama3.1:8b · general',
    statsShort: '~8B · ~4.9 GB disk',
    statsDetail:
      'Typical Q4-class weights often use **~5–7 GB VRAM**, leaving headroom on a **12 GB** GPU for the OS and compositor. On-disk size is commonly ~4.8–5.2 GB; exact tag and quant change both.',
  },
  {
    value: 'qwen2.5:14b',
    label: 'qwen2.5:14b · general',
    statsShort: '~14B · ~9 GB disk',
    statsDetail:
      'Common Q4-class pulls are often **~8–10 GB VRAM**, which fits a **16 GB** card with margin for other GPU work. Disk is often ~8–9 GB depending on quantization.',
  },
  {
    value: 'llama3:70b',
    label: 'llama3:70b · high quality (GPU)',
    statsShort: '70B · ~40 GB disk',
    statsDetail:
      'Very large weights (~40 GB on disk for common quants). Expect 48 GB+ RAM and 24 GB+ VRAM for practical GPU use; many setups use heavy quant or CPU offload.',
  },
];

/** Default chat model id (first curated preset). */
export const DEFAULT_CHAT_MODEL = CURATED_CHAT_MODELS[0]?.value ?? 'llama3.2';

/** Curated embedding models (Settings → LLM). Not shown in the chat model picker. */
export const CURATED_EMBEDDING_MODELS = [
  { value: 'nomic-embed-text', label: 'nomic-embed-text (default, ~270 MB)' },
  { value: 'mxbai-embed-large', label: 'mxbai-embed-large · higher quality' },
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

const TIER_STATS_FALLBACK = {
  light: {
    statsShort: '~1–4B · ~1–3 GB disk',
    statsDetail:
      'Small models; typical Ollama pulls ~1–3 GB. ~8 GB+ system RAM; GPU optional. Exact size depends on tag and quantization.',
  },
  medium: {
    statsShort: '~7–8B · ~4–5 GB disk',
    statsDetail:
      'Mid-size chat models; pulls often ~4–5 GB. ~12 GB+ RAM and ~6 GB+ VRAM work well for GPU offload. Quant affects footprint.',
  },
  heavy: {
    statsShort: '~13–34B · ~8–20 GB disk',
    statsDetail:
      'Large checkpoints; multi‑GB to tens of GB on disk. Plan ~24 GB+ RAM for comfortable use; VRAM needs scale with quant.',
  },
  xlarge: {
    statsShort: '~40B+ · ~25–45 GB disk',
    statsDetail:
      'Very large models (e.g. 70B class). Tens of GB on disk; needs strong RAM and typically 24 GB+ VRAM or offload strategies.',
  },
};

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
  return { ...TIER_STATS_FALLBACK[tier] };
}
