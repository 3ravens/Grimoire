/** @typedef {{ id: string, title: string, body: string, selector: string }} FirstStartTourStep */

export const FIRST_START_TOUR_SETTING_KEY = 'first_start_tour_v1_completed';

/** @type {FirstStartTourStep[]} */
export const FIRST_START_TOUR_STEPS = [
  {
    id: 'folders',
    title: 'Notes and folders',
    body: 'Write Markdown notes, organise them in the folder tree, and use tags plus [[wiki-links]] between ideas.',
    selector: '[data-tour="folders"]',
  },
  {
    id: 'editor',
    title: 'Note editor',
    body: 'Open notes here to write and edit. Use the toolbar for export, read mode, and more.',
    selector: '[data-tour="editor"]',
  },
  {
    id: 'chat',
    title: 'Chat with your vault',
    body: 'The chat sidebar uses a local Ollama model. Grimoire never sends your notes to the cloud — retrieval and inference stay on this machine.',
    selector: '[data-tour="chat"]',
  },
  {
    id: 'search',
    title: 'Search',
    body: 'Use Search (Ctrl+F) for full-text and semantic search across unlocked notes.',
    selector: '[data-tour="search"]',
  },
  {
    id: 'settings',
    title: 'Settings',
    body: 'All models, privacy tools, and optional sources like Wikipedia are configured in Settings — the same place you can change anything later.',
    selector: '[data-tour="settings"]',
  },
];

const CALLOUT_MARGIN = 12;
const CALLOUT_WIDTH = 320;
const CALLOUT_HEIGHT_ESTIMATE = 220;

/**
 * @param {DOMRect} rect
 * @param {{ width: number, height: number }} viewport
 * @param {{ width?: number, height?: number }} [opts]
 */
export function computeSpotlightHole(rect, viewport, opts = {}) {
  const pad = 6;
  return {
    x: Math.max(0, rect.left - pad),
    y: Math.max(0, rect.top - pad),
    width: Math.min(viewport.width, rect.width + pad * 2),
    height: Math.min(viewport.height, rect.height + pad * 2),
  };
}

/**
 * Position the callout near the spotlight hole without covering it.
 * @param {DOMRect} targetRect
 * @param {{ width: number, height: number }} viewport
 * @param {{ width?: number, height?: number }} [opts]
 */
export function computeCalloutPosition(targetRect, viewport, opts = {}) {
  const calloutW = opts.width ?? CALLOUT_WIDTH;
  const calloutH = opts.height ?? CALLOUT_HEIGHT_ESTIMATE;
  const margin = CALLOUT_MARGIN;

  const spaceBelow = viewport.height - targetRect.bottom;
  const spaceAbove = targetRect.top;
  const spaceRight = viewport.width - targetRect.right;
  const spaceLeft = targetRect.left;

  let top;
  let left;

  if (spaceBelow >= calloutH + margin) {
    top = targetRect.bottom + margin;
    left = clamp(targetRect.left, margin, viewport.width - calloutW - margin);
  } else if (spaceAbove >= calloutH + margin) {
    top = targetRect.top - calloutH - margin;
    left = clamp(targetRect.left, margin, viewport.width - calloutW - margin);
  } else if (spaceRight >= calloutW + margin) {
    top = clamp(targetRect.top, margin, viewport.height - calloutH - margin);
    left = targetRect.right + margin;
  } else if (spaceLeft >= calloutW + margin) {
    top = clamp(targetRect.top, margin, viewport.height - calloutH - margin);
    left = targetRect.left - calloutW - margin;
  } else {
    top = clamp(
      viewport.height - calloutH - margin,
      margin,
      viewport.height - calloutH - margin,
    );
    left = clamp(
      (viewport.width - calloutW) / 2,
      margin,
      viewport.width - calloutW - margin,
    );
  }

  return {
    top: Math.round(top),
    left: Math.round(left),
    width: calloutW,
  };
}

/**
 * @param {number} value
 * @param {number} min
 * @param {number} max
 */
export function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}
