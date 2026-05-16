// Copyright (C) 2026 Wim Palland — see App.svelte for license header.

/**
 * Svelte action: focuses and selects the text of a newly-mounted input element.
 * Usage:  <input use:autofocus />
 * @param {HTMLInputElement} node
 */
export function autofocus(node) {
  requestAnimationFrame(() => { node.focus(); node.select(); });
}
