<script>
  /**
   * Reusable Table of Contents component.
   *
   * Props:
   *   headings       — Array<{ id: string, text: string, level: number }>
   *   onHeadingClick — (id: string) => void
   *
   * This component is intentionally generic. It is used by WikipediaReader
   * and will be used by the Phase 3 "Table of contents for notes" feature.
   */

  let { headings = [], onHeadingClick } = $props();

  /** Client-only unique id (Svelte does not ship `useId` on our toolchain). */
  const tocListId = `toc-list-${Math.random().toString(36).slice(2, 11)}`;

  // Collapse state — open by default when headings exist
  let collapsed = $state(false);
</script>

{#if headings.length > 0}
  <nav class="toc" aria-label="Table of contents">
    <div class="toc-header">
      <span class="toc-title">Contents</span>
      <button
        class="toc-toggle"
        onclick={() => (collapsed = !collapsed)}
        aria-expanded={!collapsed}
        aria-controls={tocListId}
        title={collapsed ? 'Expand contents' : 'Collapse contents'}
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
    overflow-y: auto;
    border-right: 1px solid var(--border);
    padding: 12px 0;
    font-family: var(--sans);
    font-size: 13px;
    background: var(--bg-secondary);
  }

  .toc-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px 6px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 6px;
  }

  .toc-title {
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.05em;
  }

  .toc-toggle {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0 4px;
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
