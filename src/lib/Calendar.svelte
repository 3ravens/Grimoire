<script>
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import { getContext } from 'svelte';

  const settings = getContext('settings');

  /**
   * @type {{
   *   onSelectNote: (note: object) => void,
   *   onRefresh: () => void,
   *   onSelectFolder: (id: number) => void,
   *   dateFormat: string
   * }}
   */
  let { onSelectNote, onRefresh, onSelectFolder, dateFormat = 'DD-MM-YYYY' } = $props();

  // ── Constants ──────────────────────────────────────────────────────────────

  const DOW_LABELS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

  const MONTH_NAMES = [
    'January', 'February', 'March', 'April', 'May', 'June',
    'July', 'August', 'September', 'October', 'November', 'December',
  ];

  const MONTH_SHORT = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

  // Labels for the heatmap day-of-week axis (Mon = row 0).
  // Only Mon, Wed, Fri are shown to avoid crowding at 14px cell height.
  const DOW_AXIS = ['Mo', '', 'We', '', 'Fr', '', ''];

  // ── State ──────────────────────────────────────────────────────────────────

  /** @type {'calendar' | 'heatmap'} */
  let subView = $state('calendar');

  const nowObj = new Date();
  let viewYear  = $state(nowObj.getFullYear());
  let viewMonth = $state(nowObj.getMonth()); // 0-indexed

  /** @type {Array<{date: string, created: number, modified: number}>} */
  let heatmapData = $state([]);

  /** @type {string | null} */
  let selectedHeatmapDay = $state(null);

  /** @type {object[]} */
  let dayNotes = $state([]);

  /** All notes in the vault — fetched independently of the current sidebar selection. */
  let allNotes = $state([]);

  // Tooltip state — single floating tooltip driven by mouse position.
  let tooltipText = $state('');
  let tooltipX = $state(0);
  let tooltipY = $state(0);
  let tooltipVisible = $state(false);

  function showTooltip(e, text) {
    tooltipText = text;
    tooltipX = e.clientX;
    tooltipY = e.clientY;
    tooltipVisible = true;
  }

  function moveTooltip(e) {
    tooltipX = e.clientX;
    tooltipY = e.clientY;
  }

  function hideTooltip() {
    tooltipVisible = false;
  }

  // ── Date helpers ───────────────────────────────────────────────────────────

  /** Convert a Date object to a YYYY-MM-DD string in local time. */
  function toISODate(d) {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
  }

  /** Format a YYYY-MM-DD string for display using the user's dateFormat preference. */
  function formatDateStr(iso) {
    const [y, m, d] = iso.split('-');
    if (dateFormat === 'DD-MM-YYYY') return `${d}-${m}-${y}`;
    if (dateFormat === 'MM-DD-YYYY') return `${m}-${d}-${y}`;
    return iso;
  }

  const todayISO = toISODate(new Date());

  // ── Calendar keyboard navigation ───────────────────────────────────────────

  // The day number (1–31) currently focused via keyboard in the monthly calendar.
  let calendarFocusedDay = $state(null);

  let calGridEl = $state(null);

  const daysInCurrentMonth = $derived(new Date(viewYear, viewMonth + 1, 0).getDate());

  async function handleCalGridKeydown(e) {
    if (!['ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) return;
    e.preventDefault();

    if (calendarFocusedDay === null) {
      // First key: land on today if in this month, else day 1.
      const today = new Date();
      calendarFocusedDay =
        today.getFullYear() === viewYear && today.getMonth() === viewMonth
          ? today.getDate()
          : 1;
      await tick();
      calGridEl?.querySelector(`[data-day="${calendarFocusedDay}"]`)?.focus();
      return;
    }

    if (e.key === 'Enter' || e.key === ' ') {
      openDay(calendarFocusedDay);
      return;
    }

    let next = calendarFocusedDay;
    if (e.key === 'ArrowRight') next = Math.min(next + 1, daysInCurrentMonth);
    else if (e.key === 'ArrowLeft') next = Math.max(next - 1, 1);
    else if (e.key === 'ArrowDown') next = Math.min(next + 7, daysInCurrentMonth);
    else if (e.key === 'ArrowUp') next = Math.max(next - 7, 1);

    calendarFocusedDay = next;
    await tick();
    calGridEl?.querySelector(`[data-day="${next}"]`)?.focus();
  }

  // ── Heatmap keyboard navigation ────────────────────────────────────────────

  /** ISO date string focused via keyboard in the activity heatmap. */
  let heatmapFocusedIso = $state(null);
  let heatGridEl = $state(null);

  function heatmapCoordsForIso(iso) {
    for (let w = 0; w < heatmapWeeks.length; w++) {
      for (let d = 0; d < 7; d++) {
        const cell = heatmapWeeks[w][d];
        if (cell && cell.date === iso) return { w, d };
      }
    }
    return null;
  }

  function heatmapCellAt(w, d) {
    const week = heatmapWeeks[w];
    if (!week) return null;
    return week[d] ?? null;
  }

  function initialHeatmapFocusIso() {
    const todayCoords = heatmapCoordsForIso(todayISO);
    if (todayCoords && heatmapCellAt(todayCoords.w, todayCoords.d)) return todayISO;
    for (let w = heatmapWeeks.length - 1; w >= 0; w--) {
      for (let d = 6; d >= 0; d--) {
        const cell = heatmapCellAt(w, d);
        if (cell && cell.total > 0) return cell.date;
      }
    }
    for (let w = heatmapWeeks.length - 1; w >= 0; w--) {
      for (let d = 6; d >= 0; d--) {
        const cell = heatmapCellAt(w, d);
        if (cell) return cell.date;
      }
    }
    return null;
  }

  async function focusHeatmapCell(iso) {
    heatmapFocusedIso = iso;
    await tick();
    heatGridEl?.querySelector(`[data-heat-date="${iso}"]`)?.focus();
  }

  async function handleHeatGridKeydown(e) {
    if (!['ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) return;
    e.preventDefault();

    if (heatmapFocusedIso === null) {
      const iso = initialHeatmapFocusIso();
      if (iso) await focusHeatmapCell(iso);
      return;
    }

    if (e.key === 'Enter' || e.key === ' ') {
      const coords = heatmapCoordsForIso(heatmapFocusedIso);
      if (!coords) return;
      const cell = heatmapCellAt(coords.w, coords.d);
      if (cell && cell.total > 0) selectHeatmapDay(cell.date);
      return;
    }

    const coords = heatmapCoordsForIso(heatmapFocusedIso);
    if (!coords) return;
    let { w, d } = coords;
    if (e.key === 'ArrowRight') w += 1;
    else if (e.key === 'ArrowLeft') w -= 1;
    else if (e.key === 'ArrowDown') d += 1;
    else if (e.key === 'ArrowUp') d -= 1;

    for (let i = 0; i < 400; i++) {
      const cell = heatmapCellAt(w, d);
      if (cell) {
        await focusHeatmapCell(cell.date);
        return;
      }
      if (e.key === 'ArrowRight') w += 1;
      else if (e.key === 'ArrowLeft') w -= 1;
      else if (e.key === 'ArrowDown') d += 1;
      else if (e.key === 'ArrowUp') d -= 1;
      if (w < 0 || w >= heatmapWeeks.length || d < 0 || d > 6) return;
    }
  }

  // ── Calendar grid derived values ───────────────────────────────────────────

  /**
   * Set of ISO date strings that have a daily note.
   * Daily notes are identified by a title matching any supported date format.
   * The Set contains ISO (YYYY-MM-DD) strings so comparisons with cellISO() work
   * regardless of which format the note was stored in.
   */
  function titleToISO(title) {
    // Already ISO: YYYY-MM-DD
    if (/^\d{4}-\d{2}-\d{2}$/.test(title)) return title;
    const [a, b, c] = title.split('-');
    if (dateFormat === 'DD-MM-YYYY') return `${c}-${b}-${a}`;
    if (dateFormat === 'MM-DD-YYYY') return `${c}-${a}-${b}`;
    return title;
  }

  let dailyNoteDates = $derived(
    new Set(
      allNotes
        .filter(n => !n.locked && /^\d{2,4}-\d{2}-\d{2,4}$/.test(n.title))
        .map(n => titleToISO(n.title))
    )
  );

  let monthLabel = $derived(`${MONTH_NAMES[viewMonth]} ${viewYear}`);

  /**
   * Flat array of cells for the calendar grid.
   * null = leading empty cell; number = day-of-month.
   */
  let calendarCells = $derived.by(() => {
    const firstDay = new Date(viewYear, viewMonth, 1);
    const daysInMonth = new Date(viewYear, viewMonth + 1, 0).getDate();
    // Convert JS day (0=Sun) to Mon-first offset (0=Mon ... 6=Sun)
    const startOffset = (firstDay.getDay() + 6) % 7;
    const cells = [];
    for (let i = 0; i < startOffset; i++) cells.push(null);
    for (let d = 1; d <= daysInMonth; d++) cells.push(d);
    return cells;
  });

  function cellISO(day) {
    return `${viewYear}-${String(viewMonth + 1).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
  }

  function isCellToday(day) {
    return cellISO(day) === todayISO;
  }

  function isCellFuture(day) {
    const t = new Date();
    t.setHours(0, 0, 0, 0);
    return new Date(viewYear, viewMonth, day) > t;
  }

  // ── Heatmap derived values ─────────────────────────────────────────────────

  /**
   * 2-D grid of heatmap cells: weeks[col][row].
   * col 0 = oldest week (leftmost), col 52 = current week (rightmost).
   * row 0 = Monday, row 6 = Sunday.
   * A cell is null when the date is in the future, otherwise { date, created, modified, total, level }.
   */
  let heatmapWeeks = $derived.by(() => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);

    // Monday of current week
    const dow = today.getDay(); // 0=Sun ... 6=Sat
    const daysToMon = dow === 0 ? 6 : dow - 1;
    const currentMonday = new Date(today);
    currentMonday.setDate(today.getDate() - daysToMon);

    // Start: Monday 52 weeks ago
    const startMonday = new Date(currentMonday);
    startMonday.setDate(startMonday.getDate() - 52 * 7);

    const activityMap = new Map(heatmapData.map(d => [d.date, d]));
    const maxTotal = heatmapData.reduce((m, d) => Math.max(m, d.created + d.modified), 0);

    const weeks = [];
    for (let w = 0; w < 53; w++) {
      const week = [];
      for (let d = 0; d < 7; d++) {
        const cellDate = new Date(startMonday);
        cellDate.setDate(startMonday.getDate() + w * 7 + d);
        if (cellDate > today) {
          week.push(null);
        } else {
          const iso = toISODate(cellDate);
          const act = activityMap.get(iso);
          const total = (act?.created ?? 0) + (act?.modified ?? 0);
          week.push({
            date: iso,
            created: act?.created ?? 0,
            modified: act?.modified ?? 0,
            total,
            level: heatLevel(total, maxTotal),
          });
        }
      }
      weeks.push(week);
    }
    return weeks;
  });

  /**
   * Bucket total activity into one of 5 heat levels (0–4) relative to the
   * maximum activity found in the dataset. This mirrors GitHub's approach
   * so the chart always has visual contrast regardless of absolute counts.
   */
  function heatLevel(total, maxTotal) {
    if (total === 0 || maxTotal === 0) return 0;
    const r = total / maxTotal;
    if (r <= 0.25) return 1;
    if (r <= 0.5)  return 2;
    if (r <= 0.75) return 3;
    return 4;
  }

  /**
   * Month labels positioned above the heatmap grid.
   * Each label records the column index where a new month starts.
   */
  let monthLabels = $derived.by(() => {
    const labels = [];
    let lastMonth = -1;
    heatmapWeeks.forEach((week, colIdx) => {
      const first = week.find(c => c !== null);
      if (!first) return;
      const m = new Date(first.date).getMonth();
      if (m !== lastMonth) {
        lastMonth = m;
        labels.push({ col: colIdx, label: MONTH_SHORT[m] });
      }
    });
    return labels;
  });

  // ── Effects ────────────────────────────────────────────────────────────────

  $effect(() => {
    // Load data when the component mounts.
    const now = new Date();
    viewYear = now.getFullYear();
    viewMonth = now.getMonth();
    selectedHeatmapDay = null;
    dayNotes = [];
    invoke('get_activity_heatmap')
      .then(data => (heatmapData = data))
      .catch(() => {});
    invoke('list_notes', { all: true })
      .then(data => (allNotes = data))
      .catch(() => {});
  });

  // ── Navigation ─────────────────────────────────────────────────────────────

  function prevYear() { viewYear--; }
  function nextYear() { viewYear++; }

  function goToday() {
    const now = new Date();
    viewYear = now.getFullYear();
    viewMonth = now.getMonth();
  }

  function prevMonth() {
    if (viewMonth === 0) { viewMonth = 11; viewYear--; }
    else viewMonth--;
  }

  function nextMonth() {
    if (viewMonth === 11) { viewMonth = 0; viewYear++; }
    else viewMonth++;
  }

  // ── Actions ────────────────────────────────────────────────────────────────

  async function openDay(day) {
    if (isCellFuture(day)) return;
    try {
      const note = await invoke('get_or_create_daily_note', { dateStr: cellISO(day), dateFormat });
      await onRefresh();
      if (settings.llmEnabled) {
        invoke('index_note', {
          noteId: note.id,
          title: note.title,
          content: note.content ?? '',
        }).catch(() => {});
      }
      // Select the Daily Notes folder in the sidebar so the note is visible.
      if (note.folder_id != null) onSelectFolder(note.folder_id);
      onSelectNote(note);
    } catch (e) {
      console.error('Calendar: failed to open daily note', e);
    }
  }

  async function selectHeatmapDay(iso) {
    selectedHeatmapDay = iso;
    try {
      dayNotes = await invoke('get_notes_for_day', { dateStr: iso });
    } catch (e) {
      dayNotes = [];
    }
  }
</script>

<div class="calendar-overlay" role="region" aria-label="Calendar">

  <!-- ── Header ───────────────────────────────────────────────────────────── -->
  <div class="cal-header">
    <div class="cal-tabs" role="tablist">
      <button
        class="cal-tab"
        class:active={subView === 'calendar'}
        role="tab"
        id="cal-tab-calendar"
        aria-selected={subView === 'calendar'}
        aria-controls="cal-panel-calendar"
        onclick={() => (subView = 'calendar')}
      >Calendar</button>
      <button
        class="cal-tab"
        class:active={subView === 'heatmap'}
        role="tab"
        id="cal-tab-heatmap"
        aria-selected={subView === 'heatmap'}
        aria-controls="cal-panel-heatmap"
        onclick={() => (subView = 'heatmap')}
      >Activity</button>
    </div>
    <!-- Close button is provided by the tab-fullview-close button in App.svelte -->
  </div>

  <!-- ── Body ─────────────────────────────────────────────────────────────── -->
  <div class="cal-body">

    {#if subView === 'calendar'}
      <!-- ── Monthly calendar grid ──────────────────────────────────────── -->
      <div class="cal-view" id="cal-panel-calendar" role="tabpanel" aria-labelledby="cal-tab-calendar">
        <div class="cal-month-nav">
          <button class="cal-nav-btn" onclick={prevYear} aria-label="Previous year">«</button>
          <button class="cal-nav-btn" onclick={prevMonth} aria-label="Previous month">‹</button>
          <span class="cal-month-label">{monthLabel}</span>
          <button class="cal-nav-btn" onclick={nextMonth} aria-label="Next month">›</button>
          <button class="cal-nav-btn" onclick={nextYear} aria-label="Next year">»</button>
          <button class="cal-today-btn" onclick={goToday} aria-label="Go to today">Today</button>
        </div>

        <div class="cal-grid" role="grid" aria-label={monthLabel} tabindex="-1" bind:this={calGridEl} onkeydown={handleCalGridKeydown}>
          <!-- Day-of-week header row -->
          {#each DOW_LABELS as label}
            <div class="cal-dow" role="columnheader">{label}</div>
          {/each}

          <!-- Day cells -->
          {#each calendarCells as cell}
            {#if cell === null}
              <div class="cal-cell cal-empty" role="gridcell" aria-hidden="true"></div>
            {:else}
              <button
                class="cal-cell cal-day"
                class:today={isCellToday(cell)}
                class:future={isCellFuture(cell)}
                role="gridcell"
                data-day={cell}
                tabindex={calendarFocusedDay === cell ? 0 : (calendarFocusedDay === null && isCellToday(cell) ? 0 : -1)}
                aria-selected={calendarFocusedDay === cell}
                aria-label="{cellISO(cell)}{isCellToday(cell) ? ' (today)' : ''}{dailyNoteDates.has(cellISO(cell)) ? ', has note' : ''}"
                onclick={() => { calendarFocusedDay = cell; openDay(cell); }}
              >
                <span class="cal-day-num">{cell}</span>
                {#if dailyNoteDates.has(cellISO(cell))}
                  <span class="cal-dot" aria-hidden="true"></span>
                {/if}
              </button>
            {/if}
          {/each}
        </div>

        <p class="cal-hint">Click a day to open or create a daily note.</p>
      </div>

    {:else}
      <!-- ── Activity heatmap ──────────────────────────────────────────── -->
      <div class="heat-view" id="cal-panel-heatmap" role="tabpanel" aria-labelledby="cal-tab-heatmap">

        <!-- Left: heatmap + legend -->
        <div class="heat-left">
          <div class="heat-layout">

            <!-- Day-of-week axis labels -->
            <div class="heat-dow-axis" aria-hidden="true">
              <div class="heat-dow-month-spacer"></div>
              {#each DOW_AXIS as label}
                <div class="heat-dow-label">{label}</div>
              {/each}
            </div>

            <!-- Months row + grid columns -->
            <div class="heat-right">
              <!-- Month labels aligned to column positions -->
              <div class="heat-months" aria-hidden="true">
                {#each heatmapWeeks as _week, wi}
                  <div class="heat-col-label">
                    {#each monthLabels as ml}
                      {#if ml.col === wi}{ml.label}{/if}
                    {/each}
                  </div>
                {/each}
              </div>

              <!-- 53 × 7 grid -->
              <div
                class="heat-grid"
                role="grid"
                aria-label="Activity heatmap"
                tabindex="-1"
                bind:this={heatGridEl}
                onkeydown={handleHeatGridKeydown}
              >
                {#each heatmapWeeks as week, _wi}
                  <div class="heat-col" role="row">
                    {#each week as cell}
                      {#if cell === null}
                        <div class="heat-cell heat-empty" role="gridcell"></div>
                      {:else}
                        <button
                          class="heat-cell heat-{cell.level}"
                          class:heat-clickable={cell.total > 0}
                          class:heat-selected={cell.date === selectedHeatmapDay}
                          role="gridcell"
                          tabindex={cell.date === heatmapFocusedIso ? 0 : -1}
                          data-heat-date={cell.date}
                          aria-label="{cell.date}: {cell.created} created, {cell.modified} modified"
                          onclick={() => {
                            if (cell.total > 0) selectHeatmapDay(cell.date);
                            heatmapFocusedIso = cell.date;
                          }}
                          onmouseenter={cell.total > 0 ? (e) => showTooltip(e, `${formatDateStr(cell.date)}\n${cell.created} created \u00b7 ${cell.modified} modified`) : null}
                          onmousemove={cell.total > 0 ? moveTooltip : null}
                          onmouseleave={cell.total > 0 ? hideTooltip : null}
                        >
                        </button>
                      {/if}
                    {/each}
                  </div>
                {/each}
              </div>

              <!-- Legend -->
              <div class="heat-legend" aria-hidden="true">
                <span>Less</span>
                <div class="heat-cell heat-0"></div>
                <div class="heat-cell heat-1"></div>
                <div class="heat-cell heat-2"></div>
                <div class="heat-cell heat-3"></div>
                <div class="heat-cell heat-4"></div>
                <span>More</span>
              </div>
            </div>

          </div>
        </div>

        <!-- JS-driven floating tooltip (escapes all overflow contexts) -->
        {#if tooltipVisible}
          <div
            class="heat-tooltip-float"
            style="left: {tooltipX}px; top: {tooltipY}px;"
            aria-hidden="true"
          >{tooltipText}</div>
        {/if}

        <!-- Detail panel below the heatmap -->
        <div class="heat-sidebar" role="region" aria-label="Day detail" aria-live="polite">
          {#if selectedHeatmapDay}
            <div class="heat-detail-header">
              <span class="heat-detail-date">{formatDateStr(selectedHeatmapDay)}</span>
              <button class="heat-detail-close" aria-label="Close day detail" onclick={() => { selectedHeatmapDay = null; dayNotes = []; }}>✕</button>
            </div>
            {#if dayNotes.length === 0}
              <p class="heat-detail-empty">No notes found for this day.</p>
            {:else}
              <div class="heat-detail-list" role="list">
                {#each dayNotes as note}
                  <button
                    class="heat-note-pill"
                    onclick={() => { onSelectNote(note); }}
                  >{note.locked ? '🔒 Locked note' : note.title}</button>
                {/each}
              </div>
            {/if}
          {:else}
            <p class="heat-sidebar-empty">Select a day on the heatmap to see its notes.</p>
          {/if}
        </div>

      </div>
    {/if}

  </div>
</div>

<style>
  @import './styles/calendar.css';
</style>
