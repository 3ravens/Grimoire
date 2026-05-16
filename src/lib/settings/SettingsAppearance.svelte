<script>
  let {
    theme = 'system', onThemeChange = () => {},
    accent = 'default', onAccentChange = () => {},
    dateFormat = 'DD-MM-YYYY', onDateFormatChange = () => {},
  } = $props();

  const visibleTheme = $derived(theme === 'bag' ? 'dark' : theme);

  function selectStandardAccent(nextAccent) {
    onAccentChange(nextAccent);
    if (theme === 'bag') onThemeChange('dark');
  }

  function selectBlackAndGreyAppearance() {
    onThemeChange('bag');
  }
</script>

<h3>Appearance</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Theme</span>
    <span class="setting-desc">Controls the base theme mode. "System" follows your OS preference.</span>
  </div>
  <select value={visibleTheme} onchange={(e) => onThemeChange(e.currentTarget.value)}>
    <option value="system">System</option>
    <option value="light">Light</option>
    <option value="dark">Dark</option>
    <option value="spellbook">Spellbook ✦</option>
    <option value="matrix">Matrix ▓</option>
  </select>
</div>

<div class="setting-row" class:faded={theme === 'spellbook' || theme === 'matrix'}>
  <div class="setting-label">
    <span class="setting-name">Accent colour</span>
    <span class="setting-desc">{theme === 'spellbook' ? 'Not available in Spellbook theme — accent is fixed gold.' : theme === 'matrix' ? 'Not available in Matrix theme — accent is fixed green.' : theme === 'bag' ? 'Black and grey uses a monochrome dark palette. Pick another swatch to return to Dark mode.' : 'Changes the highlight colour used across the app. Black and grey is available here as a monochrome palette option.'}</span>
  </div>
  <div class="accent-swatches">
    <button
      class="accent-swatch"
      class:active={theme !== 'bag' && accent === 'default'}
      style="--swatch-color: #c8a44e"
      title="Default"
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={() => selectStandardAccent('default')}
    ></button>
    <button
      class="accent-swatch"
      class:active={theme !== 'bag' && accent === 'red'}
      style="--swatch-color: #9b2020"
      title="Crimson"
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={() => selectStandardAccent('red')}
    ></button>
    <button
      class="accent-swatch"
      class:active={theme !== 'bag' && accent === 'cyan'}
      style="--swatch-color: #0c6e7e"
      title="Cyan"
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={() => selectStandardAccent('cyan')}
    ></button>
    <button
      class="accent-swatch"
      class:active={theme !== 'bag' && accent === 'green'}
      style="--swatch-color: #256b3a"
      title="Forest green"
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={() => selectStandardAccent('green')}
    ></button>
    <button
      class="accent-swatch accent-swatch-bag"
      class:active={theme === 'bag'}
      style="--swatch-color: #4a4a4a"
      title="Black and grey"
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={selectBlackAndGreyAppearance}
    ></button>
  </div>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Date format</span>
    <span class="setting-desc">
      Controls how dates are displayed in the calendar. Notes are always stored with
      ISO 8601 titles (YYYY-MM-DD) — changing this only affects display.
    </span>
  </div>
  <select value={dateFormat} onchange={(e) => onDateFormatChange(e.currentTarget.value)}>
    <option value="DD-MM-YYYY">DD-MM-YYYY</option>
    <option value="YYYY-MM-DD">YYYY-MM-DD</option>
    <option value="MM-DD-YYYY">MM-DD-YYYY</option>
  </select>
</div>
