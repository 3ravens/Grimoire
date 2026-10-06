<script>
  import { t } from '../i18n/t.js';
</script>

<h3>{t('settings.keybinds.title')}</h3>
<p class="settings-notice">{t('settings.keybinds.notAvailable')}</p>

<table class="keybinds-table">
  <thead>
    <tr>
      <th>{t('settings.keybinds.action')}</th>
      <th>{t('settings.keybinds.shortcut')}</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>{t('settings.keybinds.saveNote')}</td>
      <td><kbd>Ctrl+S</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.lockVault')}</td>
      <td><kbd>Ctrl+Shift+L</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.sendSelectionToChat')}</td>
      <td><kbd>Ctrl+Shift+Enter</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.continueList')}</td>
      <td><kbd>Enter</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.nestList')}</td>
      <td><kbd>Tab</kbd> / <kbd>Shift+Tab</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.toggleChecklist')}</td>
      <td><kbd>Ctrl+Enter</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.cycleTabForward')}</td>
      <td><kbd>Ctrl+Tab</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.cycleTabBackward')}</td>
      <td><kbd>Ctrl+Shift+Tab</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.openNoteNewTab')}</td>
      <td><kbd>Ctrl+Click</kbd></td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.toggleChat')}</td>
      <td>{t('settings.shared.emDash')}</td>
    </tr>
    <tr>
      <td>{t('settings.keybinds.toggleGraph')}</td>
      <td>{t('settings.shared.emDash')}</td>
    </tr>
  </tbody>
</table>
