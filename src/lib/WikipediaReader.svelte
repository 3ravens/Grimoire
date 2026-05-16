<script>
  import { invoke } from '@tauri-apps/api/core';
  import TableOfContents from './TableOfContents.svelte';

  /**
   * Wikipedia article reader.
   *
   * Props:
   *   bundleId          — String  — the ZIM bundle ID
   *   articlePath       — String  — the ZIM entry path (e.g. "A/Photosynthesis")
   *   bundleName        — String  — human-readable bundle label, shown in search bar hint
   *   onArticleNavigate — (bundleId, articlePath, title) => void
   *                       Called when the reader navigates internally (cross-link or
   *                       search). The parent calls tabService.updateWikipediaTab so
   *                       the tab label stays in sync without opening a new tab.
   *   onOpenArticle     — (bundleId, articlePath, title) => void
   *                       Called for cross-link destinations that resolve to a
   *                       *different* bundle than the current one. The parent should
   *                       open a new tab.
   */

  let {
    bundleId,
    articlePath,
    bundleName = '',
    onArticleNavigate,
    onOpenArticle,
    onClose = null,
  } = $props();

  // ── Article state ──────────────────────────────────────────────────────────
  let articleHtml   = $state('');
  let articleTitle  = $state('');
  let isLoading     = $state(false);
  let loadError     = $state('');

  // ── Navigation stack (back/forward within this tab) ────────────────────────
  /** @type {{ bundleId: string, articlePath: string, title: string }[]} */
  let history    = $state([]);
  let historyIdx = $state(-1);

  const canGoBack    = $derived(historyIdx > 0);
  const canGoForward = $derived(historyIdx < history.length - 1);

  // ── Link not-installed banner ──────────────────────────────────────────────
  let notInstalledBanner = $state('');

  // ── Search bar ────────────────────────────────────────────────────────────
  let searchQuery       = $state('');
  let searchResults     = $state([]);
  let searchDropdownEl  = $state(null);
  let searchInputEl     = $state(null);
  let searchDebounce    = null;
  let searchOpen        = $state(false);
  /** Ignore stale `suggest_wikipedia_articles` responses when the query changes. */
  let searchSuggestSeq  = 0;

  // ── Highlights ─────────────────────────────────────────────────────────────
  /** @type {{ id: number, highlighted_text: string, context_before: string|null, context_after: string|null, status: string }[]} */
  let highlights = $state([]);

  // Floating highlight toolbar state
  let highlightToolbar = $state({ visible: false, x: 0, y: 0, selectedText: '', contextBefore: '', contextAfter: '' });

  // ── DOM refs ───────────────────────────────────────────────────────────────
  let articleEl = $state(null);
  let headings  = $state([]);

  // Plain (non-reactive) flag: set to true before calling onArticleNavigate in
  // the link-click handler so the prop-change $effect below skips its reload
  // and doesn't clobber the history that the handler just pushed.
  let _suppressNextPropEffect = false;

  // ── Load article on prop changes ───────────────────────────────────────────
  $effect(() => {
    if (bundleId && articlePath) {
      if (_suppressNextPropEffect) { _suppressNextPropEffect = false; return; }
      loadArticle(bundleId, articlePath, true);
    }
  });

  // ── Inject HTML into the article element whenever articleHtml changes ──────
  // We can't use {@html} reactively here because we need a DOM ref to also
  // run extractHeadings/injectHighlights after the HTML is set.
  $effect(() => {
    if (articleEl && articleHtml) {
      articleEl.innerHTML = articleHtml;
      extractHeadings();
      injectHighlights();
      // load images asynchronously after innerHTML is settled
      const bid = history[historyIdx]?.bundleId ?? bundleId;
      loadImages(bid);
    }
  });

  // skipHistory = true is used by back/forward navigation, which already manages historyIdx.
  async function loadArticle(bid, apath, isPropsChange = false, skipHistory = false) {
    isLoading = true;
    loadError = '';
    notInstalledBanner = '';
    highlightToolbar = { visible: false, x: 0, y: 0, selectedText: '', contextBefore: '', contextAfter: '' };

    try {
      const [articleResult, loadedHighlights] = await Promise.all([
        invoke('read_wikipedia_article_html', { bundleId: bid, articlePath: apath }),
        invoke('load_wikipedia_highlights', { bundleId: bid, articlePath: apath }),
      ]);

      articleHtml  = articleResult.html;
      articleTitle = articleResult.title;
      highlights   = loadedHighlights;

      if (!skipHistory) {
        if (isPropsChange) {
          history    = [{ bundleId: bid, articlePath: apath, title: articleResult.title }];
          historyIdx = 0;
        } else {
          const entry = { bundleId: bid, articlePath: apath, title: articleResult.title };
          history    = [...history.slice(0, historyIdx + 1), entry];
          historyIdx = history.length - 1;
        }
      }
    } catch (e) {
      loadError = e?.message ?? String(e);
    } finally {
      isLoading = false;
    }
  }

  function navigateBack() {
    if (!canGoBack) return;
    historyIdx -= 1;
    const entry = history[historyIdx];
    _suppressNextPropEffect = true;
    onArticleNavigate?.(entry.bundleId, entry.articlePath, entry.title);
    loadArticle(entry.bundleId, entry.articlePath, false, true);
  }

  function navigateForward() {
    if (!canGoForward) return;
    historyIdx += 1;
    const entry = history[historyIdx];
    _suppressNextPropEffect = true;
    onArticleNavigate?.(entry.bundleId, entry.articlePath, entry.title);
    loadArticle(entry.bundleId, entry.articlePath, false, true);
  }

  // ── Heading extraction ─────────────────────────────────────────────────────
  function extractHeadings() {
    if (!articleEl) return;
    const els = articleEl.querySelectorAll('h1, h2, h3, h4');
    const out = [];
    let idCounter = 0;
    for (const el of els) {
      // Ensure every heading has an id we can scroll to
      if (!el.id) {
        el.id = `wiki-h-${++idCounter}`;
      }
      const level = parseInt(el.tagName[1], 10);
      // Strip edit-section links from the text
      const text = el.textContent.replace(/\[edit\]/g, '').trim();
      if (text) out.push({ id: el.id, text, level });
    }
    headings = out;
  }

  function scrollToHeading(id) {
    if (!articleEl) return;
    const el = articleEl.querySelector(`#${CSS.escape(id)}`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  // ── Image loading ──────────────────────────────────────────────────────────
  async function loadImages(bid) {
    if (!articleEl) return;
    const imgs = Array.from(articleEl.querySelectorAll('img[src]'));
    await Promise.all(imgs.map(async (img) => {
      const src = img.getAttribute('src');
      if (!src || src.startsWith('data:')) return;
      // Normalise ZIM image paths
      const cleanSrc = src
        .replace(/^\.\.\//, '')
        .replace(/^\//, '');
      try {
        const dataUri = await invoke('serve_wikipedia_image', {
          bundleId: bid,
          imagePath: cleanSrc,
        });
        if (dataUri) {
          img.src = dataUri;
        } else {
          img.style.display = 'none';
        }
      } catch {
        img.style.display = 'none';
      }
    }));
  }

  // ── Highlight injection ────────────────────────────────────────────────────
  function injectHighlights() {
    if (!articleEl || highlights.length === 0) return;

    const walker = document.createTreeWalker(articleEl, NodeFilter.SHOW_TEXT);
    const textNodes = [];
    let node;
    while ((node = walker.nextNode())) {
      textNodes.push(node);
    }

    for (const hl of highlights) {
      const needle = hl.highlighted_text;
      if (!needle) continue;

      for (const tn of textNodes) {
        const idx = tn.nodeValue.indexOf(needle);
        if (idx === -1) continue;

        // Verify context anchoring if available (best-effort — don't reject if missing)
        if (hl.context_before || hl.context_after) {
          const fullText = articleEl.textContent;
          const anchor   = (hl.context_before || '') + needle + (hl.context_after || '');
          if (anchor && !fullText.includes(anchor)) {
            // Context mismatch — render as orphaned
            break;
          }
        }

        const before = tn.nodeValue.slice(0, idx);
        const after  = tn.nodeValue.slice(idx + needle.length);

        const mark = document.createElement('mark');
        mark.className = hl.status === 'orphaned' ? 'wiki-highlight wiki-highlight-orphaned' : 'wiki-highlight';
        mark.dataset.highlightId = String(hl.id);
        if (hl.status === 'orphaned') {
          mark.title = "This highlight's source text may have changed after a sync.";
        }
        mark.textContent = needle;

        const fragment = document.createDocumentFragment();
        if (before) fragment.appendChild(document.createTextNode(before));
        fragment.appendChild(mark);
        if (after) fragment.appendChild(document.createTextNode(after));

        tn.parentNode.replaceChild(fragment, tn);
        break; // Only inject once per highlight
      }
    }
  }

  // ── Cross-link and highlight click handling ───────────────────────────────
  async function handleArticleClick(e) {
    // Clicking a highlight mark removes it
    const mark = e.target.closest('mark[data-highlight-id]');
    if (mark) {
      e.preventDefault();
      e.stopPropagation();
      removeHighlight(parseInt(mark.dataset.highlightId, 10));
      return;
    }

    const link = e.target.closest('a');
    if (!link) return;

    const href = link.getAttribute('href');
    const wikiPath = link.dataset.wikiPath;
    if (!href) return;

    if (link.dataset.external) {
      e.preventDefault();
      return; // tooltip already visible on hover
    }

    // Let bare fragment-only links scroll natively (no wiki path set means it's
    // a real in-page anchor, not a rewritten internal wiki link)
    if (href === '#' && !wikiPath) return;

    if (!wikiPath) return;

    e.preventDefault();

    // Strip fragment for ZIM lookup, keep for scroll-to after load
    const hashIdx = wikiPath.indexOf('#');
    const pathOnly = hashIdx !== -1 ? wikiPath.slice(0, hashIdx) : wikiPath;

    if (!pathOnly) return;

    try {
      const result = await invoke('resolve_wikipedia_link', {
        currentBundleId: bundleId,
        articlePath: pathOnly,
      });

      if (result) {
        if (result.bundle_id !== bundleId) {
          // Different bundle — open in a new tab
          onOpenArticle?.(result.bundle_id, result.article_path, result.title);
        } else {
          // Suppress the prop-change $effect so it doesn't reset history.
          _suppressNextPropEffect = true;
          onArticleNavigate?.(result.bundle_id, result.article_path, result.title);
          await loadArticle(result.bundle_id, result.article_path);
        }
      } else {
        notInstalledBanner = `This article isn't available in your installed Wikipedia bundles. Install additional bundles from Settings → Wikipedia.`;
      }
    } catch (err) {
      notInstalledBanner = `Could not resolve link: ${err}`;
    }
  }

  // ── Text selection → highlight toolbar ────────────────────────────────────
  function handleArticleMouseUp(e) {
    // Don't show the toolbar if clicking on a highlight's mark (that's a remove action)
    if (e.target.classList.contains('wiki-highlight')) return;

    const sel = window.getSelection();
    const text = sel?.toString().trim();
    if (!text || text.length < 2) {
      highlightToolbar = { ...highlightToolbar, visible: false };
      return;
    }

    // Capture surrounding context from the selection's text node
    let contextBefore = '';
    let contextAfter  = '';
    try {
      const range = sel.getRangeAt(0);
      const container = range.startContainer;
      if (container.nodeType === Node.TEXT_NODE) {
        const full = container.nodeValue;
        contextBefore = full.slice(Math.max(0, range.startOffset - 100), range.startOffset);
        contextAfter  = full.slice(range.endOffset, range.endOffset + 100);
      }
    } catch { /* ignore */ }

    const rect = sel.getRangeAt(0).getBoundingClientRect();
    highlightToolbar = {
      visible:       true,
      x:             rect.left + rect.width / 2,
      y:             rect.top - 8,
      selectedText:  text,
      contextBefore,
      contextAfter,
    };
  }

  async function saveHighlight() {
    const { selectedText, contextBefore, contextAfter } = highlightToolbar;
    highlightToolbar = { ...highlightToolbar, visible: false };
    window.getSelection()?.removeAllRanges();
    try {
      const id = await invoke('save_wikipedia_highlight', {
        bundleId,
        articlePath,
        highlightedText: selectedText,
        contextBefore,
        contextAfter,
      });
      highlights = [...highlights, {
        id, highlighted_text: selectedText,
        context_before: contextBefore, context_after: contextAfter,
        status: 'active',
      }];
      // Re-inject to wrap the new highlight
      injectHighlights();
    } catch (err) {
      console.error('Failed to save highlight:', err);
    }
  }

  async function removeHighlight(highlightId) {
    try {
      await invoke('delete_wikipedia_highlight', { id: highlightId });
      highlights = highlights.filter(h => h.id !== highlightId);
      // Remove the mark element from DOM
      const mark = articleEl?.querySelector(`mark[data-highlight-id="${highlightId}"]`);
      if (mark) {
        const text = document.createTextNode(mark.textContent);
        mark.parentNode.replaceChild(text, mark);
      }
    } catch (err) {
      console.error('Failed to remove highlight:', err);
    }
  }

  // ── Search bar ────────────────────────────────────────────────────────────
  let searchError = $state('');

  function onSearchInput() {
    clearTimeout(searchDebounce);
    searchError = '';
    const q = searchQuery.trim();
    if (!q) { searchResults = []; searchOpen = false; return; }
    searchOpen = true; // keep dropdown open to show loading state
    const localSeq = ++searchSuggestSeq;
    searchDebounce = setTimeout(async () => {
      try {
        const next = await invoke('suggest_wikipedia_articles', { bundleId, query: q });
        if (localSeq !== searchSuggestSeq) return;
        searchResults = next;
        if (searchResults.length === 0) searchError = 'No results found.';
      } catch (err) {
        if (localSeq !== searchSuggestSeq) return;
        console.error('Wikipedia suggest failed:', err);
        searchResults = [];
        searchError = typeof err === 'string' ? err : 'Search failed.';
      }
    }, 300);
  }

  function onSearchKeydown(e) {
    if (e.key === 'Escape') { searchQuery = ''; searchResults = []; searchOpen = false; searchError = ''; }
  }

  async function selectSearchResult(result) {
    _suppressNextPropEffect = true;
    searchQuery  = '';
    searchResults = [];
    searchOpen   = false;
    searchError  = '';
    onArticleNavigate?.(bundleId, result.path, result.title);
    await loadArticle(bundleId, result.path);
  }

  function dismissHighlightToolbar(e) {
    // Dismiss toolbar when clicking outside it
    if (highlightToolbar.visible && !e.target.closest('.wiki-highlight-toolbar')) {
      highlightToolbar = { ...highlightToolbar, visible: false };
    }
    // Dismiss search dropdown when clicking outside it
    if (searchOpen && !e.target.closest('.wiki-search-area')) {
      searchOpen = false;
      searchError = '';
    }
  }
</script>

<svelte:window onclick={dismissHighlightToolbar} />

<div class="wiki-reader" role="main">
  <!-- Top bar: back/forward + search + title + close -->
  <div class="wiki-topbar">
    <div class="wiki-nav-btns">
      <button
        class="wiki-nav-btn"
        onclick={navigateBack}
        disabled={!canGoBack}
        aria-label="Back"
        title="Back"
      >←</button>
      <button
        class="wiki-nav-btn"
        onclick={navigateForward}
        disabled={!canGoForward}
        aria-label="Forward"
        title="Forward"
      >→</button>
    </div>

    <div class="wiki-search-area">
      <input
        class="wiki-search-input"
        type="search"
        placeholder="Search articles…"
        bind:value={searchQuery}
        oninput={onSearchInput}
        onkeydown={onSearchKeydown}
        aria-label="Search Wikipedia articles"
        aria-autocomplete="list"
        bind:this={searchInputEl}
      />
      {#if bundleName}
        <span class="wiki-search-bundle-label" title={bundleName}>Searching: {bundleName}</span>
      {/if}
      {#if searchOpen && searchResults.length > 0}
        <ul class="wiki-search-dropdown" role="listbox" bind:this={searchDropdownEl}>
          {#each searchResults as result}
            <li role="option" aria-selected="false">
              <button class="wiki-search-result" onclick={() => selectSearchResult(result)}>
                {result.title}
              </button>
            </li>
          {/each}
        </ul>
      {:else if searchOpen && searchError}
        <div class="wiki-search-dropdown wiki-search-message">{searchError}</div>
      {:else if searchOpen}
        <div class="wiki-search-dropdown wiki-search-message">Searching…</div>
      {/if}
    </div>

    {#if articleTitle}
      <span class="wiki-article-title-header" title={articleTitle}>{articleTitle}</span>
    {/if}

    {#if onClose}
      <button class="wiki-close-btn" onclick={onClose} aria-label="Close article" title="Close">✕</button>
    {/if}
  </div>

  <!-- Not-installed banner -->
  {#if notInstalledBanner}
    <div class="wiki-banner wiki-banner-info" role="alert">
      {notInstalledBanner}
      <button class="wiki-banner-dismiss" onclick={() => (notInstalledBanner = '')} aria-label="Dismiss">✕</button>
    </div>
  {/if}

  <!-- Main content area: ToC + article -->
  <div class="wiki-content-area">
    <TableOfContents {headings} onHeadingClick={scrollToHeading} />

    <div class="wiki-article-container">
      {#if isLoading}
        <div class="wiki-loading" aria-live="polite">Loading…</div>
      {:else if loadError}
        <div class="wiki-error" role="alert">{loadError}</div>
      {:else}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <article
          class="wiki-article-body"
          bind:this={articleEl}
          onclick={handleArticleClick}
          onmouseup={handleArticleMouseUp}
          aria-label={articleTitle || 'Wikipedia article'}
          role="region"
        >
          <!-- Article HTML injected here by $effect after load -->
        </article>
      {/if}
    </div>
  </div>
</div>

<!-- Floating highlight toolbar -->
{#if highlightToolbar.visible}
  <div
    class="wiki-highlight-toolbar"
    style="left: {highlightToolbar.x}px; top: {highlightToolbar.y}px; transform: translateX(-50%);"
    role="toolbar"
    aria-label="Highlight selection"
  >
    <button onclick={saveHighlight} aria-label="Highlight selected text">Highlight</button>
  </div>
{/if}

<style>
  @import './styles/wikipedia-view.css';
</style>
