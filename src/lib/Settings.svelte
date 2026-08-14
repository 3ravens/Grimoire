<script>
  import { focusTrap } from './utils/focusTrap.js';

  import SettingsLLM from './settings/SettingsLLM.svelte';
  import SettingsHardware from './settings/SettingsHardware.svelte';
  import SettingsAppearance from './settings/SettingsAppearance.svelte';
  import SettingsSecurity from './settings/SettingsSecurity.svelte';
  import SettingsData from './settings/SettingsData.svelte';
  import SettingsPrivacy from './settings/SettingsPrivacy.svelte';
  import SettingsKeybinds from './settings/SettingsKeybinds.svelte';
  import SettingsDeveloper from './settings/SettingsDeveloper.svelte';
  import SettingsWikipedia from './settings/SettingsWikipedia.svelte';
  import SettingsFileScanner from './settings/SettingsFileScanner.svelte';
  import SettingsHelp from './settings/SettingsHelp.svelte';
  import PrivacyPolicy from './PrivacyPolicy.svelte';

  let {
    onClose,
    onReplayTour = () => {},
    /** When set, switch to this section once when the overlay opens (then cleared via callback). */
    initialSection = null,
    onInitialSectionConsumed = () => {},
    vaultHasPassword = false,
    onSetVaultPassword = () => {},
    onChangeVaultPassword = () => {},
    onRemoveVaultPassword = () => {},
    onLockVault = () => {},
    keepInMemory = false,
    onKeepInMemoryChange = () => {},
    accent = 'default',
    onAccentChange = () => {},
    theme = 'system',
    onThemeChange = () => {},
    dateFormat = 'DD-MM-YYYY',
    onDateFormatChange = () => {},
    devNativeContextMenu = false,
    onDevNativeContextMenuChange = () => {},
    llmEnabled = false,
    onHardwareChange = () => {},
    wikipediaEnabled = false,
    onWikipediaEnabledChange = () => {},
  } = $props();

  const isDev = import.meta.env.DEV;

  let activeSection = $state('llm');
  let showPrivacyPolicy = $state(false);

  function openPrivacyPolicy() {
    activeSection = 'privacy';
    showPrivacyPolicy = true;
  }

  const sections = $derived([
    { id: 'llm',        label: 'LLM' },
    { id: 'hardware',   label: 'Hardware' },
    { id: 'appearance', label: 'Appearance' },
    { id: 'security',   label: 'Security' },
    { id: 'privacy',    label: 'Privacy' },
    { id: 'data',       label: 'Data' },
    { id: 'wikipedia',    label: 'Wikipedia' },
    { id: 'file_scanner', label: 'File Scanner' },
    { id: 'keybinds',   label: 'Keybinds' },
    { id: 'help',       label: 'Help' },
    ...(isDev ? [{ id: 'developer', label: 'Developer' }] : []),
  ]);

  $effect(() => {
    const s = initialSection;
    const list = sections;
    if (s && list.some((x) => x.id === s)) {
      activeSection = s;
      onInitialSectionConsumed();
    }
  });
</script>

<div class="settings-overlay" use:focusTrap>
  <div class="settings-overlay-main" inert={showPrivacyPolicy}>
    <div class="settings-header">
      <span class="settings-title">Settings</span>
      <button class="settings-close" onclick={onClose}>✕ Close</button>
    </div>

    <div class="settings-body">
      <nav class="settings-nav">
        {#each sections as s}
          <button
            class="settings-nav-item"
            class:active={activeSection === s.id}
            onclick={() => (activeSection = s.id)}
          >
            {s.label}
          </button>
        {/each}
      </nav>

      <div class="settings-content">
        {#if activeSection === 'llm'}
          <SettingsLLM
            {keepInMemory}
            {onKeepInMemoryChange}
          />
        {:else if activeSection === 'hardware'}
          <SettingsHardware {llmEnabled} {onHardwareChange} />
        {:else if activeSection === 'appearance'}
          <SettingsAppearance {theme} {onThemeChange} {accent} {onAccentChange} {dateFormat} {onDateFormatChange} />
        {:else if activeSection === 'security'}
          <SettingsSecurity {vaultHasPassword} {onSetVaultPassword} {onChangeVaultPassword} {onRemoveVaultPassword} {onLockVault} />
        {:else if activeSection === 'data'}
          <SettingsData />
        {:else if activeSection === 'privacy'}
          <SettingsPrivacy onOpenPrivacyPolicy={openPrivacyPolicy} />
        {:else if activeSection === 'wikipedia'}
          <SettingsWikipedia {wikipediaEnabled} {onWikipediaEnabledChange} />
        {:else if activeSection === 'file_scanner'}
          <SettingsFileScanner />
        {:else if activeSection === 'keybinds'}
          <SettingsKeybinds />
        {:else if activeSection === 'help'}
          <SettingsHelp onReplayTour={onReplayTour} />
        {:else if activeSection === 'developer'}
          <SettingsDeveloper {devNativeContextMenu} {onDevNativeContextMenuChange} />
        {/if}
      </div>
    </div>
  </div>

  {#if showPrivacyPolicy}
    <PrivacyPolicy onDismiss={() => (showPrivacyPolicy = false)} />
  {/if}
</div>

<style>
  @import './styles/settings.css';

  .settings-overlay-main {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
</style>
