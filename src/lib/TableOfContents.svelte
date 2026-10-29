<script>
  import { t } from './i18n/t.js';

  /**
   * Reusable Table of Contents component.
   *
   * Props:
   *   headings       — Array<{ id: string, text: string, level: number }>
   *   onHeadingClick — (id: string) => void
   *
   * Used by WikipediaReader and the note editor. Collapse is session-local
   * per mount; open by default when headings exist.
   */

  let { headings = [], onHeadingClick } = $props();

  /** Client-only unique id (Svelte does not ship `useId` on our toolchain). */
  const tocListId = `toc-list-${Math.random().toString(36).slice(2, 11)}`;

  // Open by default; closed = narrow rail that reclaims body width.
  let collapsed = $state(false);

  function toggleCollapsed() {
    collapsed = !collapsed;
  }
</script>

{#if headings.length > 0}
  <nav
    class="toc"
    class:toc-collapsed={collapsed}
    aria-label={t('views.tocAria')}
  >
    <div class="toc-header">
      {#if !collapsed}
        <span class="toc-title">{t('views.tocTitle')}</span>
      {/if}
      <button
        class="toc-toggle"
        type="button"
        onclick={toggleCollapsed}
        aria-expanded={!collapsed}
        aria-controls={tocListId}
        title={collapsed ? t('views.tocExpand') : t('views.tocCollapse')}
        aria-label={collapsed ? t('views.tocExpand') : t('views.tocCollapse')}
      >
        {collapsed ? '▸' : '▾'}
      </button>
    </div>
    {#if !collapsed}
      <ol class="toc-list" id={tocListId} role="list">
        {#each headings as h}
          <li class="toc-item toc-level-{h.level}" role="listitem">
            <button
              class="toc-link"
              type="button"
              onclick={() => onHeadingClick?.(h.id)}
              title={h.text}
            >
              {h.text}
            </button>
          </li>
        {/each}
      </ol>
    {/if}
  </nav>
{/if}

<style>
  .toc {
    flex-shrink: 0;
    width: 220px;
    overflow-x: hidden;
    overflow-y: auto;
    border-right: 1px solid var(--border);
    padding: 12px 0;
    font-family: var(--sans);
    font-size: 13px;
    background: var(--bg-secondary);
    transition: width 0.12s ease;
  }

  .toc.toc-collapsed {
    width: 36px;
    padding: 8px 0;
  }

  .toc-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 4px;
    padding: 0 12px 6px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 6px;
  }

  .toc.toc-collapsed .toc-header {
    flex-direction: column;
    justify-content: flex-start;
    padding: 0;
    border-bottom: none;
    margin-bottom: 0;
  }

  .toc-title {
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.05em;
    min-width: 0;
  }

  .toc-toggle {
    flex-shrink: 0;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    padding: 4px;
    font-size: 12px;
    line-height: 1;
    border-radius: 3px;
  }

  .toc-toggle:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  .toc-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .toc-item {
    margin: 0;
  }

  .toc-link {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    cursor: pointer;
    padding: 4px 12px;
    color: var(--text);
    font-family: var(--sans);
    font-size: 13px;
    line-height: 1.4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-radius: 0;
  }

  .toc-link:hover {
    background: var(--bg-hover);
    color: var(--accent);
  }

  /* Indent by heading level */
  .toc-level-1 .toc-link { padding-left: 12px; font-weight: 600; }
  .toc-level-2 .toc-link { padding-left: 20px; }
  .toc-level-3 .toc-link { padding-left: 28px; color: var(--text-muted); font-size: 12px; }
  .toc-level-4 .toc-link { padding-left: 36px; color: var(--text-muted); font-size: 12px; }
</style>
