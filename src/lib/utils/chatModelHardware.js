import { CURATED_CHAT_MODELS } from '../constants/chatModels.js';
import { t } from '../i18n/t.js';

/** @typedef {'ok' | 'caution' | 'severe'} HardwareWarnLevel */

/**
 * @typedef {{
 *   ramTotalMb?: number,
 *   capability?: string,
 *   gpus?: Array<{ vramTotalMb?: number | null, isUnifiedMemory?: boolean }>,
 * }} HardwareReportLike
 */

/** Curated model id → coarse resource tier for heuristics only. */
const CURATED_TIER = {
  'llama3.2': 'medium',
  phi3: 'light',
  'gemma2:2b': 'light',
  mistral: 'medium',
  codellama: 'heavy',
  'llama3.1:8b': 'medium',
  'qwen2.5:14b': 'heavy',
  'llama3:70b': 'xlarge',
};

const TIER_RULES = {
  light: {
    minRamMb: 6 * 1024,
    severeRamBelowMb: 4 * 1024,
    minComfortVramMb: 0,
    severeVramBelowMb: 0,
  },
  medium: {
    minRamMb: 12 * 1024,
    severeRamBelowMb: 8 * 1024,
    minComfortVramMb: 6 * 1024,
    severeVramBelowMb: 4 * 1024,
  },
  heavy: {
    minRamMb: 24 * 1024,
    severeRamBelowMb: 12 * 1024,
    minComfortVramMb: 10 * 1024,
    severeVramBelowMb: 6 * 1024,
  },
  xlarge: {
    minRamMb: 48 * 1024,
    severeRamBelowMb: 32 * 1024,
    minComfortVramMb: 24 * 1024,
    severeVramBelowMb: 16 * 1024,
  },
};

/**
 * Infer tier from a free-form Ollama model id (e.g. `mistral:7b-instruct`).
 * @param {string} modelId
 * @returns {'light' | 'medium' | 'heavy' | 'xlarge'}
 */
export function inferChatModelResourceTier(modelId) {
  const raw = String(modelId).trim().toLowerCase();
  if (!raw) return 'medium';

  const curated = CURATED_CHAT_MODELS.find(
    (c) => raw === c.value.toLowerCase() || raw.startsWith(`${c.value.toLowerCase()}:`),
  );
  if (curated && CURATED_TIER[curated.value]) {
    return CURATED_TIER[curated.value];
  }

  const b = raw;
  if (/\b(70|72|65|40)b\b/.test(b) || /:70/.test(b) || /:65/.test(b) || /:40/.test(b)) return 'xlarge';
  if (/\b(34|33|32|30|22|20|13|14|15)b\b/.test(b)) return 'heavy';
  if (/\b(8|9|7)b\b/.test(b)) return 'medium';
  if (/\b(1|2|3|4)b\b/.test(b) || /\b(tiny|mini|small)\b/.test(b)) return 'light';

  return 'medium';
}

/**
 * Largest reported VRAM pool in MB, or null if unknown / no GPUs.
 * @param {HardwareReportLike | null | undefined} report
 */
function maxGpuVramMb(report) {
  const gpus = report?.gpus;
  if (!Array.isArray(gpus) || gpus.length === 0) return null;
  let m = 0;
  let any = false;
  for (const g of gpus) {
    const v = g?.vramTotalMb;
    if (v != null && v > 0) {
      any = true;
      m = Math.max(m, v);
    }
  }
  return any ? m : null;
}

function fmtGbFromMb(mb) {
  if (mb == null || mb <= 0) return t('hardware.unknownRam');
  const gb = mb / 1024;
  return gb >= 10 ? `${Math.round(gb)} GB` : `${gb.toFixed(1)} GB`;
}

/**
 * Heuristic: compare model tier to RAM / VRAM from `get_hardware_info`.
 * @param {string} modelId
 * @param {HardwareReportLike | null | undefined} report
 * @returns {{ level: HardwareWarnLevel, lines: string[] }}
 */
export function assessChatModelHardware(modelId, report) {
  const lines = [];
  if (!report || report.ramTotalMb == null) {
    return { level: 'ok', lines };
  }

  const ram = Number(report.ramTotalMb) || 0;
  const vram = maxGpuVramMb(report);
  const tier = inferChatModelResourceTier(modelId);
  const rules = TIER_RULES[tier];

  let level = /** @type {HardwareWarnLevel} */ ('ok');

  const bump = (next) => {
    if (next === 'severe') level = 'severe';
    else if (next === 'caution' && level !== 'severe') level = 'caution';
  };

  if (ram < rules.severeRamBelowMb) {
    lines.push(
      t('hardware.ramSevere', {
        ram: fmtGbFromMb(ram),
        model: modelId,
        minRam: fmtGbFromMb(rules.minRamMb),
      }),
    );
    bump('severe');
  } else if (ram < rules.minRamMb) {
    lines.push(
      t('hardware.ramCaution', {
        ram: fmtGbFromMb(ram),
        model: modelId,
        minRam: fmtGbFromMb(rules.minRamMb),
      }),
    );
    bump('caution');
  }

  if (rules.minComfortVramMb > 0) {
    if (vram == null) {
      lines.push(t('hardware.noGpu'));
      bump(tier === 'xlarge' || tier === 'heavy' ? 'severe' : 'caution');
    } else if (vram < rules.severeVramBelowMb) {
      lines.push(
        t('hardware.vramSevere', {
          vram: fmtGbFromMb(vram),
          model: modelId,
          minVram: fmtGbFromMb(rules.minComfortVramMb),
        }),
      );
      bump('severe');
    } else if (vram < rules.minComfortVramMb) {
      lines.push(
        t('hardware.vramCaution', {
          vram: fmtGbFromMb(vram),
          model: modelId,
          minVram: fmtGbFromMb(rules.minComfortVramMb),
        }),
      );
      bump('caution');
    }
  }

  if (report.capability === 'embeddingOnly' && (tier === 'heavy' || tier === 'xlarge')) {
    lines.push(t('hardware.embeddingOnly'));
    bump('caution');
  }

  return { level, lines };
}
