<script>
  import { t } from '../i18n/t.js';

  let {
    theme = 'system', onThemeChange = () => {},
    accent = 'default', onAccentChange = () => {},
    dateFormat = 'DD-MM-YYYY', onDateFormatChange = () => {},
    readingWpm = 200, onReadingWpmChange = () => {},
  } = $props();

  /** Client-only unique ids (Svelte does not ship `useId` on our toolchain). */
  const themeSelectId = `theme-select-${Math.random().toString(36).slice(2, 11)}`;
  const dateFormatSelectId = `date-format-select-${Math.random().toString(36).slice(2, 11)}`;

  const visibleTheme = $derived(theme === 'bag' ? 'dark' : theme);

  const accentDesc = $derived(
    theme === 'spellbook'
      ? t('settings.appearance.accentDescSpellbook')
      : theme === 'matrix'
        ? t('settings.appearance.accentDescMatrix')
        : theme === 'bag'
          ? t('settings.appearance.accentDescBag')
          : t('settings.appearance.accentDescDefault'),
  );

  function selectStandardAccent(nextAccent) {
    onAccentChange(nextAccent);
    if (theme === 'bag') onThemeChange('dark');
  }

  function selectBlackAndGreyAppearance() {
    onThemeChange('bag');
  }
</script>

<h3>{t('settings.appearance.title')}</h3>

<div class="setting-row">
  <div class="setting-label">
    <label class="setting-name" for={themeSelectId}>{t('settings.appearance.theme')}</label>
    <span class="setting-desc">{t('settings.appearance.themeDesc')}</span>
  </div>
  <select id={themeSelectId} value={visibleTheme} onchange={(e) => onThemeChange(e.currentTarget.value)} aria-label={t('settings.appearance.themeAria')}>
    <option value="system">{t('settings.appearance.themeSystem')}</option>
    <option value="light">{t('settings.appearance.themeLight')}</option>
    <option value="dark">{t('settings.appearance.themeDark')}</option>
    <option value="spellbook">{t('settings.appearance.themeSpellbook')}</option>
    <option value="matrix">{t('settings.appearance.themeMatrix')}</option>
  </select>
</div>

<div class="setting-row" class:faded={theme === 'spellbook' || theme === 'matrix'}>
  <div class="setting-label">
    <span class="setting-name">{t('settings.appearance.accentColour')}</span>
    <span class="setting-desc">{accentDesc}</span>
  </div>
  <div class="accent-swatches" role="group" aria-label={t('settings.appearance.accentGroupAria')}>
    <button
      type="button"
      class="accent-swatch"
      class:active={theme !== 'bag' && accent === 'default'}
      style="--swatch-color: #c8a44e"
      title={t('settings.appearance.accentDefault')}
      aria-label={t('settings.appearance.accentDefaultAria')}
      aria-pressed={theme !== 'bag' && accent === 'default'}
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={() => selectStandardAccent('default')}
    ></button>
    <button
      type="button"
      class="accent-swatch"
      class:active={theme !== 'bag' && accent === 'red'}
      style="--swatch-color: #9b2020"
      title={t('settings.appearance.accentCrimson')}
      aria-label={t('settings.appearance.accentCrimsonAria')}
      aria-pressed={theme !== 'bag' && accent === 'red'}
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={() => selectStandardAccent('red')}
    ></button>
    <button
      type="button"
      class="accent-swatch"
      class:active={theme !== 'bag' && accent === 'cyan'}
      style="--swatch-color: #0c6e7e"
      title={t('settings.appearance.accentCyan')}
      aria-label={t('settings.appearance.accentCyanAria')}
      aria-pressed={theme !== 'bag' && accent === 'cyan'}
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={() => selectStandardAccent('cyan')}
    ></button>
    <button
      type="button"
      class="accent-swatch"
      class:active={theme !== 'bag' && accent === 'green'}
      style="--swatch-color: #256b3a"
      title={t('settings.appearance.accentForest')}
      aria-label={t('settings.appearance.accentForestAria')}
      aria-pressed={theme !== 'bag' && accent === 'green'}
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={() => selectStandardAccent('green')}
    ></button>
    <button
      type="button"
      class="accent-swatch accent-swatch-bag"
      class:active={theme === 'bag'}
      style="--swatch-color: #4a4a4a"
      title={t('settings.appearance.accentBag')}
      aria-label={t('settings.appearance.accentBagAria')}
      aria-pressed={theme === 'bag'}
      disabled={theme === 'spellbook' || theme === 'matrix'}
      onclick={selectBlackAndGreyAppearance}
    ></button>
  </div>
</div>

<div class="setting-row">
  <div class="setting-label">
    <label class="setting-name" for={dateFormatSelectId}>{t('settings.appearance.dateFormat')}</label>
    <span class="setting-desc">{t('settings.appearance.dateFormatDesc')}</span>
  </div>
  <select id={dateFormatSelectId} value={dateFormat} onchange={(e) => onDateFormatChange(e.currentTarget.value)} aria-label={t('settings.appearance.dateFormatAria')}>
    <option value="DD-MM-YYYY">DD-MM-YYYY</option>
    <option value="YYYY-MM-DD">YYYY-MM-DD</option>
    <option value="MM-DD-YYYY">MM-DD-YYYY</option>
  </select>
</div>

<div class="setting-row">
  <div class="setting-label">
    <label class="setting-name" for="reading-wpm-input">{t('settings.appearance.readingWpm')}</label>
    <span class="setting-desc">{t('settings.appearance.readingWpmDesc')}</span>
  </div>
  <input
    id="reading-wpm-input"
    type="number"
    class="setting-num"
    min="50"
    max="600"
    step="1"
    value={readingWpm}
    aria-label={t('settings.appearance.readingWpmAria')}
    onchange={(e) => onReadingWpmChange(e.currentTarget.value)}
  />
</div>
