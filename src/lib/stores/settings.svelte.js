// Settings store — all user preferences and theme state.
// Uses Svelte 5 runes so this works in .svelte.js files outside of components.

export function createSettings() {
  let keepModelInMemory = $state(localStorage.getItem('keepModelInMemory') === 'true');
  let accent = $state(localStorage.getItem('accent') ?? 'default');
  let theme  = $state(localStorage.getItem('theme')  ?? 'system');
  let dailyNoteFormat = $state(localStorage.getItem('dailyNoteFormat') ?? 'DD-MM-YYYY');
  let devNativeContextMenu = $state(
    import.meta.env.DEV ? (localStorage.getItem('devNativeContextMenu') === 'true') : false
  );

  // Hardware capability — loaded non-blocking on startup.
  // 'embeddingOnly' is the safe default until the first response arrives.
  let hwCapability    = $state('embeddingOnly');
  let llmForceEnabled = $state(false);
  /** Last `get_hardware_info` payload (flattened); used for chat-model heuristics. */
  let hardwareReport = $state(/** @type {Record<string, unknown> | null} */ (null));
  const llmEnabled = $derived(hwCapability === 'full' || llmForceEnabled);

  // Wikipedia local knowledge source toggle.
  // Loaded from the 'wikipedia_enabled' setting in SQLite on startup.
  let wikipediaEnabled = $state(false);

  // Persist preferences to localStorage and apply theme to the DOM.
  $effect(() => {
    localStorage.setItem('keepModelInMemory', String(keepModelInMemory));
  });

  $effect(() => {
    localStorage.setItem('dailyNoteFormat', dailyNoteFormat);
  });

  $effect(() => {
    localStorage.setItem('accent', accent);
    localStorage.setItem('theme',  theme);

    const root = document.documentElement;
    if (accent === 'red') {
      root.removeAttribute('data-accent');
    } else {
      root.setAttribute('data-accent', accent);
    }
    if (theme === 'system') {
      root.removeAttribute('data-theme');
    } else {
      root.setAttribute('data-theme', theme);
    }
  });

  $effect(() => {
    if (import.meta.env.DEV) {
      localStorage.setItem('devNativeContextMenu', String(devNativeContextMenu));
    }
  });

  return {
    get keepModelInMemory() { return keepModelInMemory; },
    set keepModelInMemory(v) { keepModelInMemory = v; },
    get accent() { return accent; },
    set accent(v) { accent = v; },
    get theme() { return theme; },
    set theme(v) { theme = v; },
    get dailyNoteFormat() { return dailyNoteFormat; },
    set dailyNoteFormat(v) { dailyNoteFormat = v; },
    get devNativeContextMenu() { return devNativeContextMenu; },
    set devNativeContextMenu(v) { devNativeContextMenu = v; },
    get hwCapability() { return hwCapability; },
    set hwCapability(v) { hwCapability = v; },
    get llmForceEnabled() { return llmForceEnabled; },
    set llmForceEnabled(v) { llmForceEnabled = v; },
    get hardwareReport() { return hardwareReport; },
    set hardwareReport(v) { hardwareReport = v; },
    get llmEnabled() { return llmEnabled; },
    get wikipediaEnabled() { return wikipediaEnabled; },
    set wikipediaEnabled(v) { wikipediaEnabled = v; },
  };
}
