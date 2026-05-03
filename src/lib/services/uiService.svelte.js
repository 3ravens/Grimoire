/**
 * UI overlay state — simple boolean toggles for modal/overlay visibility.
 * These are UI-chrome concerns, not domain state.
 */
export function createUiService() {
  let settingsOpen = $state(false);
  let quickSwitcherOpen = $state(false);
  let wikiSearchOpen = $state(false);

  return {
    get settingsOpen() { return settingsOpen; },
    set settingsOpen(v) { settingsOpen = v; },
    get quickSwitcherOpen() { return quickSwitcherOpen; },
    set quickSwitcherOpen(v) { quickSwitcherOpen = v; },
    get wikiSearchOpen() { return wikiSearchOpen; },
    set wikiSearchOpen(v) { wikiSearchOpen = v; },
  };
}
