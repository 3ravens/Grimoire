<script>
  import { coalesceSingleLineChangeHunks } from './utils/diff.js';

  let {
    hunks = [],
    instruction = '',
    readonly = false,
    /** When true with `readonly`, render body in two columns (revision | current). */
    sideBySide = false,
    headerTitle = 'Compare to current',
    onAcceptAll,
    onRejectAll,
    onAcceptHunk,
    onRejectHunk,
    onRefineHunk,
    acceptedIndices = [],
    rejectedIndices = [],
  } = $props();

  /** Line hunks, or coalesced `modified` hunks with inline segments when readonly. */
  const displayHunks = $derived(
    readonly ? coalesceSingleLineChangeHunks(hunks) : hunks,
  );

  const displayHeaderTitle = $derived(
    readonly ? headerTitle : 'Suggested improvements',
  );

  const historySideBySide = $derived(readonly && sideBySide);

  const changeHunks = $derived(
    displayHunks.map((h, i) => ({ ...h, idx: i })).filter(h => h.type !== 'unchanged'),
  );

  const activeHunks = $derived(
    changeHunks.filter(h => !acceptedIndices.includes(h.idx) && !rejectedIndices.includes(h.idx)),
  );
</script>

<div
  class="diff-view"
  aria-label={readonly
    ? 'Differences between current note and selected revision'
    : 'Suggested improvements diff'}
>
  <div class="diff-header">
    <span class="diff-header-title">{displayHeaderTitle}</span>
    {#if instruction && !readonly}
      <span class="diff-header-instruction" title={instruction}>"{instruction}"</span>
    {/if}
    {#if !readonly}
      <div class="diff-actions">
        <button class="accept-all" onclick={onAcceptAll}>Accept All</button>
        <button class="reject-all" onclick={onRejectAll}>Reject All</button>
      </div>
    {/if}
  </div>

  {#if activeHunks.length === 0}
    <div class="diff-empty">
      {#if readonly}
        <span class="diff-empty-msg">No differences</span>
        <span class="diff-empty-sub">The current editor text matches this revision.</span>
      {:else}
        <span class="diff-empty-msg">No changes suggested</span>
        <span class="diff-empty-sub">The LLM returned text identical to the original.</span>
      {/if}
    </div>
  {:else if historySideBySide}
    <div class="diff-body diff-body-side-by-side">
      <div class="diff-side-colhead diff-side-l">Revision</div>
      <div class="diff-side-colhead diff-side-r">Current</div>
      {#each displayHunks as hunk, i}
        {#if hunk.type === 'modified'}
          <div class="diff-side-banner">
            <span class="diff-hunk-header-label">Changed</span>
          </div>
          <div
            class="diff-side-cell diff-side-l diff-hunk modified"
            class:accepted={acceptedIndices.includes(i)}
            class:rejected={rejectedIndices.includes(i)}
            aria-label="Revision text"
          >
            <div class="diff-inline-row diff-readonly-cell">
              {#each hunk.oldSegments as seg, si (si)}
                {#if seg.type === 'equal'}
                  <span class="diff-inline-neutral">{seg.text}</span>
                {:else}
                  <span class="diff-inline-remove">{seg.text}</span>
                {/if}
              {/each}
            </div>
          </div>
          <div
            class="diff-side-cell diff-side-r diff-hunk modified"
            class:accepted={acceptedIndices.includes(i)}
            class:rejected={rejectedIndices.includes(i)}
            aria-label="Current text"
          >
            <div class="diff-inline-row diff-readonly-cell">
              {#each hunk.newSegments as seg, si (si)}
                {#if seg.type === 'equal'}
                  <span class="diff-inline-neutral">{seg.text}</span>
                {:else}
                  <span class="diff-inline-add">{seg.text}</span>
                {/if}
              {/each}
            </div>
          </div>
        {:else if hunk.type === 'unchanged'}
          {#each hunk.lines as line}
            <div class="diff-side-cell diff-side-l diff-side-unchanged-line">{line || '\u00A0'}</div>
            <div class="diff-side-cell diff-side-r diff-side-unchanged-line">{line || '\u00A0'}</div>
          {/each}
        {:else if hunk.type === 'remove'}
          <div class="diff-side-banner diff-banner-remove">
            <span class="diff-hunk-header-label">Removed</span>
            <span class="diff-side-banner-count">({hunk.lines.length} line{hunk.lines.length === 1 ? '' : 's'})</span>
          </div>
          {#each hunk.lines as line}
            <div class="diff-side-cell diff-side-l diff-line remove">{line || '\u00A0'}</div>
            <div class="diff-side-cell diff-side-r diff-side-empty" aria-hidden="true">{'\u00A0'}</div>
          {/each}
        {:else if hunk.type === 'add'}
          <div class="diff-side-banner diff-banner-add">
            <span class="diff-hunk-header-label">Added</span>
            <span class="diff-side-banner-count">({hunk.lines.length} line{hunk.lines.length === 1 ? '' : 's'})</span>
          </div>
          {#each hunk.lines as line}
            <div class="diff-side-cell diff-side-l diff-side-empty" aria-hidden="true">{'\u00A0'}</div>
            <div class="diff-side-cell diff-side-r diff-line add">{line || '\u00A0'}</div>
          {/each}
        {/if}
      {/each}
    </div>
  {:else}
    <div class="diff-body">
      {#each displayHunks as hunk, i}
        {#if hunk.type === 'modified'}
          <div
            class="diff-hunk modified"
            class:accepted={acceptedIndices.includes(i)}
            class:rejected={rejectedIndices.includes(i)}
          >
            <div class="diff-hunk-header">
              <span class="diff-hunk-header-label">Changed</span>
            </div>
            <div class="diff-hunk-lines diff-inline-block">
              <div class="diff-inline-row diff-inline-row-old" aria-label="Previous text">
                {#each hunk.oldSegments as seg, si (si)}
                  {#if seg.type === 'equal'}
                    <span class="diff-inline-neutral">{seg.text}</span>
                  {:else}
                    <span class="diff-inline-remove">{seg.text}</span>
                  {/if}
                {/each}
              </div>
              <div class="diff-inline-row diff-inline-row-new" aria-label="New text">
                {#each hunk.newSegments as seg, si (si)}
                  {#if seg.type === 'equal'}
                    <span class="diff-inline-neutral">{seg.text}</span>
                  {:else}
                    <span class="diff-inline-add">{seg.text}</span>
                  {/if}
                {/each}
              </div>
            </div>
          </div>
        {:else}
          <div
            class="diff-hunk {hunk.type}"
            class:unchanged={hunk.type === 'unchanged'}
            class:accepted={acceptedIndices.includes(i)}
            class:rejected={rejectedIndices.includes(i)}
          >
            {#if hunk.type !== 'unchanged'}
              <div class="diff-hunk-header">
                <span class="diff-hunk-header-label">
                  {hunk.type === 'add' ? '+ Added' : '- Removed'}
                </span>
                <span>({hunk.lines.length} line{hunk.lines.length === 1 ? '' : 's'})</span>
                {#if !readonly}
                  <div class="diff-hunk-actions">
                    {#if acceptedIndices.includes(i)}
                      <span class="accepted-label">Accepted</span>
                    {:else if rejectedIndices.includes(i)}
                      <span class="rejected-label">Rejected</span>
                    {:else}
                      <button class="accept" onclick={() => onAcceptHunk?.(i)}>Accept</button>
                      <button class="reject" onclick={() => onRejectHunk?.(i)}>Reject</button>
                      <button class="refine" onclick={(e) => {
                        const rect = e.currentTarget.getBoundingClientRect();
                        onRefineHunk?.(i, rect.left, rect.bottom);
                      }}>Refine</button>
                    {/if}
                  </div>
                {/if}
              </div>
            {/if}
            <div class="diff-hunk-lines">
              {#each hunk.lines as line}
                <div class="diff-line {hunk.type}">{line || '\u00A0'}</div>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  @import './styles/diff-view.css';
</style>
