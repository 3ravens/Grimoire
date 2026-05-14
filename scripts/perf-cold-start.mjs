#!/usr/bin/env node
/**
 * Cold-start measurement helper for Grimoire.
 *
 * The app sets `window.__GRIMOIRE_PERF_READY__ = true` in App.svelte after
 * `getCurrentWindow().show()` and a `tick()`, i.e. when the shell is visible.
 *
 * Usage outline:
 *   1. Build a dev binary: `npm run tauri build -- --debug` (or run `tauri dev` in another terminal).
 *   2. Optionally install Playwright: `npm i -D @playwright/test` then `npx playwright install chromium`.
 *   3. Run your automation to open the app and `page.waitForFunction(() => window.__GRIMOIRE_PERF_READY__)`.
 *   4. Record `performance.now()` deltas from process spawn (or from before launch) to that predicate.
 *
 * This script only prints instructions so CI and contributors do not pick up a
 * heavyweight browser dependency by default.
 */

const lines = [
  'Grimoire cold-start benchmark (target: < 2000 ms on reference hardware, Ollama already running)',
  '',
  'Signal: window.__GRIMOIRE_PERF_READY__ === true (see src/App.svelte).',
  '',
  'Suggested Playwright snippet:',
  '  const t0 = Date.now();',
  '  // launch app / attach to WebView2 — platform-specific',
  '  await page.waitForFunction(() => globalThis.__GRIMOIRE_PERF_READY__ === true, { timeout: 15000 });',
  '  console.log("cold_start_ms", Date.now() - t0);',
  '',
  'Run multiple cold starts (median of N ≥ 5) after a full reboot discard run.',
]

console.log(lines.join('\n'))
