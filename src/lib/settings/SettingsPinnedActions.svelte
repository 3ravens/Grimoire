<script>
  import { getContext, tick } from 'svelte';
  import { t } from '../i18n/t.js';
  import { COMMANDS, getCommand } from '../commands/commandRegistry.js';

  /** @type {{
   *   orderedIds: string[],
   *   loaded: boolean,
   *   loadFailed: boolean,
   *   loadError: string | null,
   *   isPinned: (id: string) => boolean,
   *   toggle: (id: string) => Promise<void>,
   *   unpin: (id: string) => Promise<void>,
   *   setOrder: (ids: string[]) => Promise<void>,
   *   moveUp: (id: string) => Promise<void>,
   *   moveDown: (id: string) => Promise<void>,
   *   resetToEmpty: () => Promise<void>,
   * }} */
  const pins = getContext('pins');
  /** @type {{ isAvailable: (id: string) => boolean }} */
  const commands = getContext('commands');

  let query = $state('');
  let liveMessage = $state('');
  /** @type {string | null} */
  let dragId = $state(null);

  const filteredCommands = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return COMMANDS;
    return COMMANDS.filter((c) => t(c.labelKey).toLowerCase().includes(q));
  });

  /** @param {string} id */
  function labelFor(id) {
    const cmd = getCommand(id);
    if (cmd) return t(cmd.labelKey);
    return t('settings.pinnedActions.unknownAction', { id });
  }

  /** @param {string} id */
  function canPin(id) {
    if (!pins.loaded || pins.loadFailed) return false;
    if (pins.isPinned(id)) return true;
    return commands.isAvailable(id);
  }

  /** @param {string} id */
  async function onToggle(id) {
    if (!canPin(id) && !pins.isPinned(id)) return;
    await pins.toggle(id);
  }

  /** @param {string} id @param {HTMLElement | null} focusEl */
  async function onMoveUp(id, focusEl) {
    const i = pins.orderedIds.indexOf(id);
    if (i <= 0) {
      liveMessage = t('settings.pinnedActions.alreadyFirst');
      return;
    }
    await pins.moveUp(id);
    liveMessage = t('settings.pinnedActions.movedAnnounce', {
      label: labelFor(id),
      position: i,
    });
    await tick();
    focusEl?.focus();
  }

  /** @param {string} id @param {HTMLElement | null} focusEl */
  async function onMoveDown(id, focusEl) {
    const i = pins.orderedIds.indexOf(id);
    if (i < 0 || i >= pins.orderedIds.length - 1) {
      liveMessage = t('settings.pinnedActions.alreadyLast');
      return;
    }
    await pins.moveDown(id);
    liveMessage = t('settings.pinnedActions.movedAnnounce', {
      label: labelFor(id),
      position: i + 2,
    });
    await tick();
    focusEl?.focus();
  }

  /** @param {DragEvent} e @param {string} id */
  function onDragStart(e, id) {
    dragId = id;
    e.dataTransfer?.setData('text/plain', id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }

  /** @param {DragEvent} e @param {string} targetId */
  async function onDrop(e, targetId) {
    e.preventDefault();
    const from = dragId ?? e.dataTransfer?.getData('text/plain');
    dragId = null;
    if (!from || from === targetId) return;
    const next = pins.orderedIds.slice();
    const fromIdx = next.indexOf(from);
    const toIdx = next.indexOf(targetId);
    if (fromIdx < 0 || toIdx < 0) return;
    next.splice(fromIdx, 1);
    next.splice(toIdx, 0, from);
    await pins.setOrder(next);
  }

  function onDragOver(e) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
  }
</script>

<h3>{t('settings.pinnedActions.title')}</h3>
<p class="settings-notice">{t('settings.pinnedActions.notice')}</p>

{#if pins.loadFailed}
  <p class="settings-notice pinned-actions-error" role="alert">
    {pins.loadError ?? t('settings.pinnedActions.loadError')}
  </p>
  <button
    type="button"
    class="pinned-actions-toggle"
    onclick={() => {
      if (confirm(t('settings.pinnedActions.resetCorruptConfirm'))) {
        void pins.resetToEmpty();
      }
    }}
  >
    {t('settings.pinnedActions.resetCorrupt')}
  </button>
{:else}
  <div class="pinned-actions-search">
    <label class="sr-only" for="pinned-actions-search">{t('settings.pinnedActions.searchAria')}</label>
    <input
      id="pinned-actions-search"
      type="search"
      placeholder={t('settings.pinnedActions.searchPlaceholder')}
      bind:value={query}
      disabled={!pins.loaded}
    />
  </div>

  <h4 class="pinned-actions-heading">{t('settings.pinnedActions.registryHeading')}</h4>
  <ul class="pinned-actions-registry">
    {#each filteredCommands as cmd (cmd.id)}
      {@const pinned = pins.isPinned(cmd.id)}
      {@const available = commands.isAvailable(cmd.id)}
      {@const Icon = cmd.icon}
      <li class="pinned-actions-row" class:unavailable={!available}>
        <span class="pinned-actions-icon" aria-hidden="true"><Icon size={15} /></span>
        <span class="pinned-actions-label">
          {t(cmd.labelKey)}
          {#if !available}
            <span class="pinned-actions-badge">{t('settings.pinnedActions.unavailable')}</span>
          {/if}
        </span>
        <button
          type="button"
          class="pinned-actions-toggle"
          disabled={!pins.loaded || (!pinned && !available)}
          onclick={() => onToggle(cmd.id)}
        >
          {pinned ? t('settings.pinnedActions.unpin') : t('settings.pinnedActions.pin')}
        </button>
      </li>
    {/each}
  </ul>

  <h4 class="pinned-actions-heading">{t('settings.pinnedActions.pinListHeading')}</h4>
  {#if pins.orderedIds.length === 0}
    <p class="settings-notice">{t('settings.pinnedActions.pinListEmpty')}</p>
  {:else}
    <ul class="pinned-actions-order">
      {#each pins.orderedIds as id, index (id)}
        {@const cmd = getCommand(id)}
        {@const available = cmd ? commands.isAvailable(id) : false}
        {@const Icon = cmd?.icon}
        <li
          class="pinned-actions-row"
          class:unavailable={!available}
          draggable={pins.loaded}
          ondragstart={(e) => onDragStart(e, id)}
          ondragover={onDragOver}
          ondrop={(e) => onDrop(e, id)}
        >
          <span class="pinned-actions-icon" aria-hidden="true">
            {#if Icon}
              <Icon size={15} />
            {/if}
          </span>
          <span class="pinned-actions-label">{labelFor(id)}</span>
          <div class="pinned-actions-order-btns">
            <button
              type="button"
              class="pinned-actions-toggle"
              disabled={!pins.loaded}
              onclick={(e) => onMoveUp(id, e.currentTarget)}
              title={index === 0 ? t('settings.pinnedActions.alreadyFirst') : t('settings.pinnedActions.moveUp')}
            >
              {t('settings.pinnedActions.moveUp')}
            </button>
            <button
              type="button"
              class="pinned-actions-toggle"
              disabled={!pins.loaded}
              onclick={(e) => onMoveDown(id, e.currentTarget)}
              title={index === pins.orderedIds.length - 1
                ? t('settings.pinnedActions.alreadyLast')
                : t('settings.pinnedActions.moveDown')}
            >
              {t('settings.pinnedActions.moveDown')}
            </button>
            <button
              type="button"
              class="pinned-actions-toggle"
              disabled={!pins.loaded}
              onclick={() => pins.unpin(id)}
            >
              {t('settings.pinnedActions.unpin')}
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="sr-only" aria-live="polite">{liveMessage}</div>
{/if}

<style>
  .pinned-actions-search {
    margin-block-end: 12px;
  }

  .pinned-actions-search input {
    width: 100%;
    max-width: 320px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg2);
    color: var(--text-h);
    font: 13px var(--sans);
  }

  .pinned-actions-heading {
    margin: 16px 0 8px;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text);
  }

  .pinned-actions-registry,
  .pinned-actions-order {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-width: 560px;
  }

  .pinned-actions-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 5px;
    background: var(--bg2);
    border: 1px solid var(--border);
  }

  .pinned-actions-row.unavailable {
    opacity: 0.65;
  }

  .pinned-actions-icon {
    display: flex;
    width: 18px;
    justify-content: center;
    color: var(--text);
    flex-shrink: 0;
  }

  .pinned-actions-label {
    flex: 1;
    font-size: 13px;
    color: var(--text-h);
    min-width: 0;
  }

  .pinned-actions-badge {
    margin-inline-start: 6px;
    font-size: 11px;
    color: var(--text);
    opacity: 0.8;
  }

  .pinned-actions-toggle {
    flex-shrink: 0;
    padding: 4px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg3);
    color: var(--text-h);
    font: 12px var(--sans);
    cursor: pointer;
  }

  .pinned-actions-toggle:hover:not(:disabled) {
    border-color: var(--accent);
  }

  .pinned-actions-toggle:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .pinned-actions-order-btns {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    justify-content: flex-end;
  }

  .pinned-actions-error {
    color: var(--danger, #c44);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
