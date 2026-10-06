<script>
  import { t } from './i18n/t.js';
  import SearchIcon from './icons/SearchIcon.svelte';
  import GraphIcon from './icons/GraphIcon.svelte';
  import CalendarIcon from './icons/CalendarIcon.svelte';
  import DailyNoteIcon from './icons/DailyNoteIcon.svelte';
  import QuickSwitcherIcon from './icons/QuickSwitcherIcon.svelte';
  import DocumentIcon from './icons/DocumentIcon.svelte';
  import VaultLockIcon from './icons/VaultLockIcon.svelte';
  import SettingsIcon from './icons/SettingsIcon.svelte';
  import HelpIcon from './icons/HelpIcon.svelte';
  import DocsIcon from './icons/DocsIcon.svelte';
  import BugIcon from './icons/BugIcon.svelte';

  const {
    searchActive      = false,
    showLock          = false,
    wikipediaEnabled  = false,
    updateAvailable   = false,
    onSearch,
    onGraph,
    onCalendar,
    onDailyNote,
    onQuickSwitcher,
    onWikipedia,
    onLock,
    onSettings,
    onHelp,
    onDocs,
    onReportBug,
    reportBugBusy = false,
  } = $props();
</script>

<nav class="activity-bar">
  <!-- ── Top group ──────────────────────────────────────────────────────── -->
  <div class="activity-bar-top">
    <button
      class="activity-bar-btn"
      class:active={searchActive}
      onclick={onSearch}
      title={t('activityBar.searchTitle')}
      aria-label={t('activityBar.searchAria')}
      aria-current={searchActive ? 'page' : undefined}
      data-tour="search"
    >
      <SearchIcon size={16} />
    </button>

    <button
      class="activity-bar-btn"
      onclick={onGraph}
      title={t('activityBar.graphTitle')}
      aria-label={t('activityBar.graphAria')}
    >
      <GraphIcon size={16} />
    </button>

    <button
      class="activity-bar-btn"
      onclick={onCalendar}
      title={t('activityBar.calendarTitle')}
      aria-label={t('activityBar.calendarAria')}
    >
      <CalendarIcon size={16} />
    </button>

    <!-- New Daily Note: calendar icon with a + in the body -->
    <button
      class="activity-bar-btn"
      onclick={onDailyNote}
      title={t('activityBar.dailyNoteTitle')}
      aria-label={t('activityBar.dailyNoteAria')}
    >
      <DailyNoteIcon size={16} />
    </button>

    <!-- Quick Switcher: list with magnifying glass -->
    <button
      class="activity-bar-btn"
      onclick={onQuickSwitcher}
      title={t('activityBar.quickSwitcherTitle')}
      aria-label={t('activityBar.quickSwitcherAria')}
    >
      <QuickSwitcherIcon size={16} />
    </button>

    {#if wikipediaEnabled}
      <!-- Wikipedia article search -->
      <button
        class="activity-bar-btn"
        onclick={onWikipedia}
        title={t('activityBar.wikipediaTitle')}
        aria-label={t('activityBar.wikipediaAria')}
      >
        <DocumentIcon size={16} />
      </button>
    {/if}

    <!-- Pinned actions placeholder (deferred) -->
    <div class="activity-bar-separator"></div>
    <span class="activity-bar-section-label">{t('activityBar.pinned')}</span>
  </div>

  <!-- ── Bottom group ───────────────────────────────────────────────────── -->
  <div class="activity-bar-bottom">
    <div class="activity-bar-separator"></div>

    {#if showLock}
      <button
        class="activity-bar-btn"
        onclick={onLock}
        title={t('activityBar.lockVaultTitle')}
        aria-label={t('activityBar.lockVaultAria')}
      >
        <VaultLockIcon size={16} />
      </button>
    {/if}

    <button
      class="activity-bar-btn"
      class:has-badge={updateAvailable}
      onclick={onSettings}
      title={updateAvailable ? t('activityBar.settingsUpdateTitle') : t('activityBar.settingsTitle')}
      aria-label={updateAvailable ? t('activityBar.settingsUpdateAria') : t('activityBar.settingsAria')}
      data-tour="settings"
    >
      <SettingsIcon size={16} />
      {#if updateAvailable}
        <span class="activity-bar-badge" aria-hidden="true"></span>
      {/if}
    </button>

    <button
      class="activity-bar-btn"
      onclick={onHelp}
      title={t('activityBar.helpTitle')}
      aria-label={t('activityBar.helpAria')}
    >
      <HelpIcon size={16} />
    </button>

    <button
      class="activity-bar-btn"
      onclick={onDocs}
      title={t('activityBar.docsTitle')}
      aria-label={t('activityBar.docsAria')}
    >
      <DocsIcon size={16} />
    </button>

    <button
      class="activity-bar-btn"
      onclick={onReportBug}
      disabled={reportBugBusy}
      title={t('activityBar.bugTitle')}
      aria-label={t('activityBar.bugAria')}
    >
      <BugIcon size={16} />
    </button>
  </div>
</nav>
