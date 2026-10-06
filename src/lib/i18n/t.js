import { en } from './en.js';

const pluralRules = new Intl.PluralRules('en');

/**
 * @param {Record<string, unknown>} root
 * @param {string} key
 * @returns {unknown}
 */
function lookup(root, key) {
  const parts = key.split('.');
  let cur = /** @type {unknown} */ (root);
  for (const part of parts) {
    if (cur == null || typeof cur !== 'object') return undefined;
    cur = /** @type {Record<string, unknown>} */ (cur)[part];
  }
  return cur;
}

/**
 * @param {string} template
 * @param {Record<string, unknown>} [params]
 */
function interpolate(template, params) {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name) => {
    if (!Object.prototype.hasOwnProperty.call(params, name)) return match;
    const v = params[name];
    return v == null ? '' : String(v);
  });
}

/**
 * Resolve a dotted catalog key to an English string.
 * @param {string} key
 * @param {Record<string, unknown>} [params]
 * @returns {string}
 */
export function t(key, params) {
  const value = lookup(en, key);
  if (typeof value !== 'string') {
    if (import.meta.env?.DEV) {
      console.warn(`[i18n] missing key: ${key}`);
    }
    return key;
  }
  return interpolate(value, params);
}

/**
 * Plural-aware lookup. Catalog entry must be `{ one: string, other: string }`
 * (and optionally `zero` / `few` / `many` for future locales).
 * @param {string} key
 * @param {number} count
 * @param {Record<string, unknown>} [params]
 * @returns {string}
 */
export function tp(key, count, params) {
  const value = lookup(en, key);
  if (value == null || typeof value !== 'object') {
    if (import.meta.env?.DEV) {
      console.warn(`[i18n] missing plural key: ${key}`);
    }
    return key;
  }
  const forms = /** @type {Record<string, string>} */ (value);
  const category = pluralRules.select(count);
  const template = forms[category] ?? forms.other ?? forms.one;
  if (typeof template !== 'string') {
    if (import.meta.env?.DEV) {
      console.warn(`[i18n] incomplete plural key: ${key}`);
    }
    return key;
  }
  return interpolate(template, { count, ...params });
}

/**
 * Split a message on `{slot}` markers for rich-text rendering.
 * Text parts are `{ type: 'text', value }` or `{ type: 'slot', name, value? }`.
 * Slot `value` comes from `params[name]` when provided as a string; otherwise
 * the caller wraps the slot in markup using only the name.
 * @param {string} key
 * @param {Record<string, unknown>} [params]
 * @returns {Array<{ type: 'text', value: string } | { type: 'slot', name: string, value?: string }>}
 */
export function tParts(key, params) {
  const template = t(key, undefined);
  if (template === key && lookup(en, key) === undefined) {
    return [{ type: 'text', value: key }];
  }
  /** @type {Array<{ type: 'text', value: string } | { type: 'slot', name: string, value?: string }>} */
  const parts = [];
  const re = /\{(\w+)\}/g;
  let last = 0;
  let m;
  while ((m = re.exec(template)) !== null) {
    if (m.index > last) {
      parts.push({ type: 'text', value: template.slice(last, m.index) });
    }
    const name = m[1];
    const raw = params && Object.prototype.hasOwnProperty.call(params, name)
      ? params[name]
      : undefined;
    if (raw != null && typeof raw !== 'object') {
      parts.push({ type: 'slot', name, value: String(raw) });
    } else {
      parts.push({ type: 'slot', name });
    }
    last = m.index + m[0].length;
  }
  if (last < template.length) {
    parts.push({ type: 'text', value: template.slice(last) });
  }
  return parts;
}
