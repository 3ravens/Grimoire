#!/usr/bin/env node
/**
 * Acceptance helper for T-006: flag likely leftover user-facing English
 * string literals in Svelte/JS UI sources.
 *
 * Exclusions (documented allowlist):
 * - *.test.js fixtures
 * - src/lib/llm/prompts.js, src/lib/utils/featureGuide.js (LLM)
 * - src/lib/i18n/en.js (the catalog itself)
 * - Non-UI tokens (CSS, role=, type=, key names used as logic)
 *
 * Usage: node scripts/check-i18n-literals.mjs
 * Exit 0 always; prints suspects for human review (heuristic).
 */
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..');
const SRC = path.join(ROOT, 'src');

const EXCLUDE_FILES = new Set([
  path.join(SRC, 'lib', 'llm', 'prompts.js'),
  path.join(SRC, 'lib', 'utils', 'featureGuide.js'),
  path.join(SRC, 'lib', 'i18n', 'en.js'),
  path.join(SRC, 'lib', 'i18n', 't.test.js'),
]);

const ATTR_RE =
  /\b(?:title|placeholder|aria-label)\s*=\s*(["'])(?!\s*\{)([^"']{2,})\1/g;
const TEXT_HINT_RE =
  />\s*([A-Z][^<{]{2,80}?)\s*</g;

/** @param {string} dir @param {string[]} out */
function walk(dir, out) {
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) walk(p, out);
    else if (/\.(svelte|js)$/.test(ent.name) && !ent.name.endsWith('.test.js')) out.push(p);
  }
}

const files = [];
walk(SRC, files);

let hits = 0;
for (const file of files) {
  if (EXCLUDE_FILES.has(file)) continue;
  if (file.includes(`${path.sep}i18n${path.sep}`) && file.endsWith('en.js')) continue;
  const text = fs.readFileSync(file, 'utf8');
  const rel = path.relative(ROOT, file).replace(/\\/g, '/');

  for (const m of text.matchAll(ATTR_RE)) {
    const val = m[2];
    if (/^(true|false|page|dialog|presentation|tab|tablist|menu|listbox|option|grid|region|status|alert|polite|assertive)$/i.test(val)) continue;
    if (/^[\d\s./\-_:]+$/.test(val)) continue;
    console.log(`${rel}: attr ${m[0].slice(0, 80)}`);
    hits++;
  }
}

console.log(`\nHeuristic attribute-literal hits: ${hits}`);
console.log('Review manually; LLM/user-content/IPC passthrough may still appear.');
