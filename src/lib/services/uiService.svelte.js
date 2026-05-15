/**
 * UI overlay state — simple boolean toggles for modal/overlay visibility.
 * These are UI-chrome concerns, not domain state.
 */
export function createUiService() {
  let settingsOpen = $state(false);
  /** When opening Settings, jump to this section once (e.g. `wikipedia` after install wizard). */
  let settingsPendingSection = $state(/** @type {string | null} */ (null));
  let quickSwitcherOpen = $state(false);
  let wikiSearchOpen = $state(false);

  return {
    get settingsOpen() { return settingsOpen; },
    set settingsOpen(v) { settingsOpen = v; },
    get settingsPendingSection() { return settingsPendingSection; },
    set settingsPendingSection(v) { settingsPendingSection = v; },
    get quickSwitcherOpen() { return quickSwitcherOpen; },
    set quickSwitcherOpen(v) { quickSwitcherOpen = v; },
    get wikiSearchOpen() { return wikiSearchOpen; },
    set wikiSearchOpen(v) { wikiSearchOpen = v; },
  };
}
