<script>
  /**
   * @typedef {{
   *   value: string,
   *   label: string,
   *   statsShort: string,
   *   statsDetail: string,
   *   installedFull?: string | null,
   * }} ChatModelOption
   */

  let {
    /** @type {string} */
    selected = '',
    /** @type {ChatModelOption[]} */
    options = [],
    disabled = false,
    /** @param {string} v */
    onSelect = () => {},
    /** @param {boolean} o */
    onOpenChange = () => {},
    /** @param {ChatModelOption} opt */
    onUninstall = undefined,
    /** When set, uninstall is in progress for this option `value`. */
    uninstallBusyKey = null,
    ariaLabel = 'Chat model',
    /** @type {'chat' | 'settings'} */
    variant = 'chat',
  } = $props();

  let open = $state(false);
  /** @type {HTMLElement | null} */
  let rootEl = $state(null);

  /** @param {ChatModelOption} opt */
  function canUninstall(opt) {
    return !!(opt.installedFull && onUninstall);
  }

  function onWindowClick(/** @type {MouseEvent} */ e) {
    if (!open) return;
    const t = /** @type {Element | null} */ (e.target);
    if (t && rootEl?.contains(t)) return;
    open = false;
    onOpenChange(false);
  }

  function onWindowKeydown(/** @type {KeyboardEvent} */ e) {
    if (e.key === 'Escape' && open) {
      open = false;
      onOpenChange(false);
    }
  }

  function toggle() {
    if (disabled) return;
    const next = !open;
    open = next;
    onOpenChange(next);
  }

  /**
   * @param {string} v
   */
  function pick(v) {
    open = false;
    onOpenChange(false);
    onSelect(v);
  }

  /**
   * @param {MouseEvent} e
   * @param {ChatModelOption} opt
   */
  function onRemoveClick(e, opt) {
    e.preventDefault();
    e.stopPropagation();
    if (disabled || uninstallBusyKey || !onUninstall) return;
    onUninstall(opt);
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={onWindowKeydown} />

<div
  class="chat-model-combobox"
  class:chat-model-combobox--chat={variant === 'chat'}
  class:chat-model-combobox--settings={variant === 'settings'}
  bind:this={rootEl}
>
  <button
    type="button"
    class="chat-model-combobox-trigger"
    class:model-input={variant === 'chat'}
    class:chat-model-combobox-trigger--settings={variant === 'settings'}
    {disabled}
    aria-label={ariaLabel}
    aria-expanded={open}
    aria-haspopup="true"
    onclick={toggle}
  >
    <span class="chat-model-combobox-trigger-text">{selected || '…'}</span>
    <span class="chat-model-combobox-chevron" aria-hidden="true">▾</span>
  </button>
  {#if open && !disabled}
    <div class="chat-model-combobox-menu" aria-label={ariaLabel}>
      {#each options as opt (opt.value)}
        <div
          class="chat-model-combobox-item-row"
          class:active={opt.value === selected}
        >
          <button
            type="button"
            class="chat-model-combobox-item"
            title={opt.statsDetail}
            onclick={() => pick(opt.value)}
          >
            <span class="chat-model-combobox-item-title">{opt.label}</span>
            <span class="chat-model-combobox-item-meta">{opt.statsShort}</span>
          </button>
          {#if canUninstall(opt)}
            <button
              type="button"
              class="chat-model-combobox-remove"
              title="Remove {opt.installedFull} from Ollama"
              aria-label="Remove {opt.installedFull} from Ollama"
              disabled={!!uninstallBusyKey}
              onclick={(e) => onRemoveClick(e, opt)}
            >Remove</button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
