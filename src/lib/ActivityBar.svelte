<!-- Copyright (C) 2026 Wim Palland
This file is part of Grimoire — licensed under GPL-3.0 or later. -->

<script>
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
      title="Search (Ctrl+F)"
      aria-label="Search"
      aria-current={searchActive ? 'page' : undefined}
    >
      <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
        <circle cx="6.5" cy="6.5" r="4.5"/>
        <line x1="10" y1="10" x2="13.5" y2="13.5"/>
      </svg>
    </button>

    <button
      class="activity-bar-btn"
      onclick={onGraph}
      title="Graph"
      aria-label="Graph"
    >
      <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
        <circle cx="7.5" cy="2" r="1.5"/>
        <circle cx="2" cy="12" r="1.5"/>
        <circle cx="13" cy="12" r="1.5"/>
        <line x1="7.5" y1="3.5" x2="2.7" y2="10.5"/>
        <line x1="7.5" y1="3.5" x2="12.3" y2="10.5"/>
        <line x1="3.5" y1="12" x2="11.5" y2="12"/>
      </svg>
    </button>

    <button
      class="activity-bar-btn"
      onclick={onCalendar}
      title="Calendar"
      aria-label="Calendar"
    >
      <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="1.5" y="2.5" width="12" height="11" rx="1"/>
        <line x1="1.5" y1="6" x2="13.5" y2="6"/>
        <line x1="5" y1="1.5" x2="5" y2="3.5"/>
        <line x1="10" y1="1.5" x2="10" y2="3.5"/>
      </svg>
    </button>

    <!-- New Daily Note: calendar icon with a + in the body -->
    <button
      class="activity-bar-btn"
      onclick={onDailyNote}
      title="New Daily Note"
      aria-label="New Daily Note"
    >
      <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="1.5" y="2.5" width="12" height="11" rx="1"/>
        <line x1="1.5" y1="6" x2="13.5" y2="6"/>
        <line x1="5" y1="1.5" x2="5" y2="3.5"/>
        <line x1="10" y1="1.5" x2="10" y2="3.5"/>
        <line x1="7.5" y1="8.5" x2="7.5" y2="11.5"/>
        <line x1="6" y1="10" x2="9" y2="10"/>
      </svg>
    </button>

    <!-- Quick Switcher: list with magnifying glass -->
    <button
      class="activity-bar-btn"
      onclick={onQuickSwitcher}
      title="Quick Switcher (Ctrl+P)"
      aria-label="Quick Switcher"
    >
      <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
        <line x1="2" y1="4" x2="8" y2="4"/>
        <line x1="2" y1="7.5" x2="6.5" y2="7.5"/>
        <line x1="2" y1="11" x2="5.5" y2="11"/>
        <circle cx="11" cy="10" r="2.5"/>
        <line x1="12.8" y1="11.8" x2="14" y2="13"/>
      </svg>
    </button>

    {#if wikipediaEnabled}
      <!-- Wikipedia article search -->
      <button
        class="activity-bar-btn"
        onclick={onWikipedia}
        title="Search Wikipedia"
        aria-label="Search Wikipedia"
      >
        <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <rect x="1.5" y="1.5" width="12" height="12" rx="1"/>
          <line x1="4" y1="4.5" x2="11" y2="4.5"/>
          <line x1="4" y1="7.5" x2="11" y2="7.5"/>
          <line x1="4" y1="10.5" x2="8" y2="10.5"/>
        </svg>
      </button>
    {/if}

    <!-- Pinned actions placeholder (deferred) -->
    <div class="activity-bar-separator"></div>
    <span class="activity-bar-section-label">PINNED</span>
  </div>

  <!-- ── Bottom group ───────────────────────────────────────────────────── -->
  <div class="activity-bar-bottom">
    <div class="activity-bar-separator"></div>

    {#if showLock}
      <button
        class="activity-bar-btn"
        onclick={onLock}
        title="Lock vault (Ctrl+Shift+L)"
        aria-label="Lock vault"
      >
        <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="7" width="9" height="7" rx="1"/>
          <path d="M5 7V5a2.5 2.5 0 0 1 5 0v2"/>
          <circle cx="7.5" cy="10.5" r="0.75" fill="currentColor" stroke="none"/>
        </svg>
      </button>
    {/if}

    <button
      class="activity-bar-btn"
      class:has-badge={updateAvailable}
      onclick={onSettings}
      title={updateAvailable ? "Settings — update available" : "Settings"}
      aria-label={updateAvailable ? "Settings, update available" : "Settings"}
    >
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 0 0 2.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 0 0 1.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 0 0-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 0 0-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 0 0-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 0 0-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 0 0 1.066-2.573c-.94-1.543.826-3.31 2.37-2.37c1 .608 2.296.07 2.572-1.065z"/>
        <circle cx="12" cy="12" r="3"/>
      </svg>
      {#if updateAvailable}
        <span class="activity-bar-badge" aria-hidden="true"></span>
      {/if}
    </button>

    <button
      class="activity-bar-btn"
      onclick={onHelp}
      title="Help"
      aria-label="Help"
    >
      <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="7.5" cy="7.5" r="6"/>
        <path d="M5.5 5.5a2 2 0 1 1 2 2c0 1-0.5 1.5-0.5 2"/>
        <circle cx="7.5" cy="11.5" r="0.6" fill="currentColor" stroke="none"/>
      </svg>
    </button>

    <button
      class="activity-bar-btn"
      onclick={onDocs}
      title="Documentation"
      aria-label="Documentation"
    >
      <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M3.5 2.5h4a2 2 0 0 1 2 2v8.5H4a1 1 0 0 1-1-1v-8a1 1 0 0 1 .5-.87z"/>
        <path d="M7.5 2.5H11a1.5 1.5 0 0 1 1.5 1.5v8.5"/>
        <line x1="5.5" y1="5.5" x2="8.5" y2="5.5"/>
        <line x1="5.5" y1="7.5" x2="8.5" y2="7.5"/>
        <line x1="5.5" y1="9.5" x2="7.5" y2="9.5"/>
      </svg>
    </button>

    <button
      class="activity-bar-btn"
      onclick={onReportBug}
      disabled={reportBugBusy}
      title="Report a bug"
      aria-label="Report a bug"
    >
      <svg width="16" height="16" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M7.5 2.5v-1M5.2 3.2L4.5 2M9.8 3.2l.7-.8" />
        <ellipse cx="7.5" cy="8.5" rx="2.3" ry="3.3" />
        <path d="M5.2 6.2L3 5.5M9.8 6.2L12 5.5M5 8.5H2.5M10 8.5h2.5M5.2 10.8L3 11.5M9.8 10.8L12 11.5" />
      </svg>
    </button>
  </div>
</nav>
