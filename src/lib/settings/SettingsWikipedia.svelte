<!-- Copyright (C) 2026 Wim Palland

This file is part of Grimoire.

Grimoire is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

Grimoire is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with Grimoire. If not, see <https://www.gnu.org/licenses/>. -->

<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { onMount, onDestroy } from 'svelte';
  import ConfirmModal from '../ConfirmModal.svelte';

  let {
    wikipediaEnabled = false,
    onWikipediaEnabledChange = () => {},
  } = $props();

  // ── State ────────────────────────────────────────────────────────────────

  let bundles         = $state([]);
  let catalogueItems  = $state([]);
  let catalogueSearch   = $state('');
  let loadingCatalogue  = $state(false);
  let catalogueError    = $state('');
  let storagePath       = $state('');

  const filteredCatalogue = $derived(
    catalogueSearch.trim()
      ? catalogueItems.filter((i) =>
          (i.title || i.name).toLowerCase().includes(catalogueSearch.toLowerCase())
        )
      : catalogueItems
  );

  // Per-bundle indexing progress: bundle_id → { indexed, scanned, total, done, error }
  let progress = $state({});

  // { startTime: ms, startScan: number } recorded on the first non-zero scan event.
  // Rate = (current_scanned - startScan) / elapsed, which avoids inflating the rate
  // with entries that were already scanned before we started the timer.
  let indexingStarts = $state({});

  // Download progress: bundle_id → { downloaded_bytes, total_bytes }
  let downloadProgress = $state({});

  // Global bulk re-index status for installed bundles.
  let reindexAll = $state({ running: false, done: 0, total: 0, error: '' });

  // Confirm modal state
  let confirmModal = $state(null); // { message, onConfirm }

  // ── Lifecycle ─────────────────────────────────────────────────────────────

  let unlistenIndex    = null;
  let unlistenDownload = null;

  onMount(async () => {
    await loadBundles();
    storagePath = await invoke('get_setting', { key: 'wikipedia_storage_path' }).catch(() => '');

    unlistenIndex = await listen('wikipedia:index-progress', (ev) => {
      const {
        bundle_id,
        indexed,
        scanned,
        total,
        article_count,
        permanently_skipped,
        batch_splits,
        single_fallbacks,
        done,
        error,
      } = ev.payload;

      // Record the moment we first see real scan progress so we can derive rate.
      if ((scanned ?? 0) > 0 && !indexingStarts[bundle_id]) {
        indexingStarts = { ...indexingStarts, [bundle_id]: { startTime: Date.now(), startScan: scanned ?? 0 } };
      }

      progress = {
        ...progress,
        [bundle_id]: {
          indexed: indexed ?? 0,
          scanned: scanned ?? 0,
          total: total ?? 0,
          article_count: article_count ?? null,
          permanently_skipped: permanently_skipped ?? 0,
          batch_splits: batch_splits ?? 0,
          single_fallbacks: single_fallbacks ?? 0,
          done: !!done,
          error: error ?? null,
        },
      };

      if (done) {
        const next = { ...indexingStarts };
        delete next[bundle_id];
        indexingStarts = next;
        loadBundles();
      }
    });

    unlistenDownload = await listen('wikipedia:download-progress', (ev) => {
      const { bundle_id, downloaded_bytes, total_bytes } = ev.payload;
      downloadProgress = {
        ...downloadProgress,
        [bundle_id]: { downloaded_bytes, total_bytes },
      };
    });
  });

  onDestroy(() => {
    unlistenIndex?.();
    unlistenDownload?.();
  });

  // ── Helpers ───────────────────────────────────────────────────────────────

  async function loadBundles() {
    const raw = await invoke('list_wikipedia_bundles').catch(() => []);
    // If the app was restarted mid-index, the DB still shows 'indexing'.
    // Reset those to 'none' so the user can restart them.
    for (const b of raw) {
      if (b.indexing_state === 'indexing' && !progress[b.id]) {
        await invoke('set_bundle_indexing_state', { bundleId: b.id, state: 'none' }).catch(() => {});
        b.indexing_state = 'none';
      }
    }
    bundles = raw;
  }

  async function pickStorageFolder() {
    const selected = await openDialog({ directory: true, multiple: false, title: 'Select Wikipedia storage folder' });
    if (selected) {
      storagePath = selected;
      await saveStoragePath();
    }
  }

  function installedIds() {
    return new Set(bundles.map((b) => b.id));
  }

  function fmt(bytes) {
    if (!bytes) return '—';
    const gb = bytes / 1e9;
    if (gb >= 1) return `${gb.toFixed(1)} GB`;
    const mb = bytes / 1e6;
    return `${mb.toFixed(0)} MB`;
  }

  /** Format a duration in seconds as a human-readable string, e.g. "4h 12m", "3m 8s". */
  function fmtEta(secs) {
    if (!isFinite(secs) || secs < 0) return null;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${s}s`;
    return `${s}s`;
  }

  /**
   * Linear indexing progress is driven by the ZIM scan (scanned / total entries).
   * Article embed count lags behind that because many ZIM paths are skipped
   * (redirects, non-HTML, templates, short stubs, …). ETA uses scan rate, so
   * % and the bar must use the same basis or they disagree near the end.
   */
  function scanProgressFraction(p) {
    const t = p?.total ?? 0;
    const s = p?.scanned ?? 0;
    if (t <= 0) return null;
    return Math.max(0, Math.min(1, s / t));
  }

  function stateLabel(bundle) {
    const p = progress[bundle.id];
    if (p && !p.done) {
      const frac = scanProgressFraction(p);
      const pct = frac != null ? `${Math.floor(frac * 100)}%` : '';

      let etaPart = '';
      const start = indexingStarts[bundle.id];
      if (start && p.scanned > start.startScan && p.total > 0 && p.scanned < p.total) {
        const elapsedSec = (Date.now() - start.startTime) / 1000;
        const rate = (p.scanned - start.startScan) / elapsedSec;
        const remainingSec = (p.total - p.scanned) / rate;
        if (elapsedSec > 5 && remainingSec > 5) {
          const formatted = fmtEta(remainingSec);
          if (formatted) etaPart = ` · ~${formatted} left`;
        }
      }

      const entriesPart =
        p.total > 0
          ? ` · ${(p.scanned ?? 0).toLocaleString()} / ${p.total.toLocaleString()} ZIM entries`
          : '';
      // Article total in the bundle header is catalogue metadata; embedded count
      // can differ (filters, skips). Show live embedded count only here.
      const articlesPart = ` · ${(p.indexed ?? 0).toLocaleString()} articles embedded`;
      const pctSeg = pct ? ` ${pct}` : '';
      return `Indexing…${pctSeg}${articlesPart}${entriesPart}${etaPart}`;
    }
    if (bundle.indexing_state === 'done') {
      const p = progress[bundle.id];
      const skipped = p?.permanently_skipped ?? 0;
      if (p?.done && skipped > 0) {
        return `Indexed (${skipped} items skipped after retries)`;
      }
      return 'Indexed';
    }
    if (bundle.indexing_state === 'error') return 'Error';
    if (bundle.indexing_state === 'indexing') return 'Indexing…';
    return 'Not indexed';
  }

  function performanceWarning(bundle) {
    const p = progress[bundle.id];
    if (!p || p.done) return '';
    const splits = p.batch_splits ?? 0;
    const singles = p.single_fallbacks ?? 0;
    if (singles > 0) {
      return `Performance warning: ${singles} single-item embed fallback${singles === 1 ? '' : 's'} detected.`;
    }
    if (splits >= 10) {
      return `Performance warning: batch requests were split ${splits} times.`;
    }
    return '';
  }

  function progressBarPercent(bundle) {
    const p = progress[bundle.id];
    if (!p) return 0;
    const frac = scanProgressFraction(p);
    if (frac != null) {
      return frac * 100;
    }
    const totalArticles = p.article_count || bundle.article_count || 0;
    if (totalArticles > 0) {
      return ((p.indexed ?? 0) / totalArticles) * 100;
    }
    return 0;
  }

  function hasActiveBundleIndexing() {
    return bundles.some((bundle) => {
      const p = progress[bundle.id];
      return bundle.indexing_state === 'indexing' || (p && !p.done);
    });
  }

  // ── Actions ───────────────────────────────────────────────────────────────

  async function toggleEnabled(v) {
    await invoke('set_setting', { key: 'wikipedia_enabled', value: v ? 'true' : 'false' }).catch(() => {});
    onWikipediaEnabledChange(v);
  }

  async function saveStoragePath() {
    await invoke('set_setting', { key: 'wikipedia_storage_path', value: storagePath }).catch(() => {});
  }

  async function fetchCatalogue() {
    loadingCatalogue = true;
    catalogueError = '';
    try {
      const conn = await invoke('check_wikipedia_connectivity');
      if (!conn.online) {
        catalogueError = conn.message || 'No internet connection';
        return;
      }
      catalogueItems = await invoke('fetch_wikipedia_catalogue');
    } catch (e) {
      catalogueError = e?.message ?? String(e);
    } finally {
      loadingCatalogue = false;
    }
  }

  async function startDownload(item) {
    if (!storagePath) {
      catalogueError = 'Set a storage path before downloading.';
      return;
    }
    catalogueError = '';
    try {
      const conn = await invoke('check_wikipedia_connectivity');
      if (!conn.online) {
        catalogueError = conn.message || 'No internet connection';
        return;
      }
      await invoke('check_wikipedia_download_preflight', {
        destDir: storagePath,
        expectedSizeBytes: item.size_bytes ?? null,
      });
    } catch (e) {
      catalogueError = e?.message ?? String(e);
      return;
    }

    downloadProgress = {
      ...downloadProgress,
      [item.id]: { downloaded_bytes: 0, total_bytes: item.size_bytes },
    };
    try {
      const path = await invoke('download_wikipedia_bundle', {
        bundleId: item.id,
        bundleName: item.name,
        bundleTitle: item.title,
        downloadUrl: item.download_url,
        destDir: storagePath,
        expectedSizeBytes: item.size_bytes,
        articleCount: item.article_count ?? null,
      });
      await loadBundles();
      // Clear download progress on completion.
      const next = { ...downloadProgress };
      delete next[item.id];
      downloadProgress = next;
      // Auto-index after download completes.
      const bundle = bundles.find((b) => b.id === item.id);
      if (bundle) {
        await startIndexing(bundle);
      }
    } catch (e) {
      catalogueError = `Download failed: ${e?.message ?? e}`;
      const next = { ...downloadProgress };
      delete next[item.id];
      downloadProgress = next;
    }
  }

  async function startIndexing(bundle, reset = false) {
    progress = {
      ...progress,
      [bundle.id]: { indexed: 0, total: 0, done: false, error: null },
    };
    try {
      await invoke('index_wikipedia_bundle', { bundleId: bundle.id, reset });
      return true;
    } catch (e) {
      // Progress event with error will have already arrived via the event listener.
      return false;
    }
  }

  async function stopIndexing(bundle) {
    try {
      await invoke('cancel_wikipedia_indexing', { bundleId: bundle.id });
      // Mark local progress as finished immediately; backend emits a final
      // done event as well, but this keeps the UI responsive.
      const existing = progress[bundle.id] || {};
      progress = { ...progress, [bundle.id]: { ...existing, done: true, error: null } };
      await loadBundles();
    } catch (e) {
      catalogueError = e?.message ?? String(e);
    }
  }

  async function reindexAllBundles() {
    if (!bundles.length || hasActiveBundleIndexing()) {
      return;
    }

    reindexAll = { running: true, done: 0, total: bundles.length, error: '' };

    for (const bundle of bundles) {
      const ok = await startIndexing(bundle, true);
      if (!ok && !reindexAll.error) {
        reindexAll = { ...reindexAll, error: `Some bundles failed to index. Check each row for details.` };
      }
      reindexAll = { ...reindexAll, done: reindexAll.done + 1 };
    }

    await loadBundles();
    reindexAll = { ...reindexAll, running: false };
  }

  function confirmRemove(bundle) {
    confirmModal = {
      message: `Remove "${bundle.title || bundle.name}"? This will delete the index. The .zim file will NOT be deleted from disk.`,
      onConfirm: () => removeBundle(bundle, false),
    };
  }

  function confirmRemoveWithFile(bundle) {
    confirmModal = {
      message: `Remove "${bundle.title || bundle.name}" AND delete the .zim file from disk? This cannot be undone.`,
      onConfirm: () => removeBundle(bundle, true),
    };
  }

  async function removeBundle(bundle, deleteFile) {
    confirmModal = null;
    await invoke('remove_wikipedia_bundle', { bundleId: bundle.id, deleteFile }).catch(() => {});
    await loadBundles();
  }
</script>

<h3>Wikipedia</h3>
<p class="settings-notice">
  Download Kiwix Wikipedia bundles (nopic flavour) to enable Wikipedia as a local knowledge source
  for the AI assistant. Nothing is sent to the internet — all indexing and search runs on-device.
</p>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Enable Wikipedia search</span>
    <span class="setting-desc">When enabled, relevant Wikipedia articles are included in AI chat context.</span>
  </div>
  <label class="toggle">
    <input
      type="checkbox"
      checked={wikipediaEnabled}
      onchange={(e) => toggleEnabled(e.currentTarget.checked)}
    />
    <span class="toggle-label">{wikipediaEnabled ? 'On' : 'Off'}</span>
  </label>
</div>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Storage path</span>
    <span class="setting-desc">Directory where .zim files will be downloaded and read from.</span>
  </div>
  <div class="wiki-path-row">
    <span class="wiki-path-display" class:wiki-path-placeholder={!storagePath}>
      {storagePath || 'No folder selected'}
    </span>
    <button class="wiki-btn" onclick={pickStorageFolder}>Browse…</button>
  </div>
</div>

<!-- Installed bundles -->
{#if bundles.length > 0}
  <h4 class="settings-subsection">Installed bundles</h4>
  <div class="wiki-bulk-actions">
    <button class="wiki-btn" onclick={reindexAllBundles} disabled={reindexAll.running || hasActiveBundleIndexing()}>
      {reindexAll.running ? `Re-indexing ${reindexAll.done}/${reindexAll.total}…` : 'Re-index all bundles'}
    </button>
    {#if reindexAll.error}
      <span class="wiki-error">{reindexAll.error}</span>
    {/if}
  </div>
  <div class="wiki-bundle-list">
    {#each bundles as bundle (bundle.id)}
      {@const p = progress[bundle.id]}
      <div class="wiki-bundle-row">
        <div class="wiki-bundle-info">
          <span class="wiki-bundle-title">{bundle.title || bundle.name}</span>
          <span class="wiki-bundle-meta">
            {bundle.article_count ? bundle.article_count.toLocaleString() + ' articles · ' : ''}
            {fmt(bundle.size_bytes)}
          </span>
          <span class="wiki-bundle-state" class:state-done={bundle.indexing_state === 'done'} class:state-error={bundle.indexing_state === 'error'}>
            {stateLabel(bundle)}
          </span>
          {#if performanceWarning(bundle)}
            <span class="wiki-bundle-warning">{performanceWarning(bundle)}</span>
          {/if}
          {#if p && !p.done && ((p.article_count || bundle.article_count || 0) > 0 || p.total > 0)}
            <div class="wiki-progress-bar">
              <div class="wiki-progress-fill" style="width: {Math.max(0, Math.min(100, progressBarPercent(bundle))).toFixed(1)}%"></div>
            </div>
          {/if}
        </div>
        <div class="wiki-bundle-actions">
          {#if bundle.indexing_state !== 'indexing' && !(p && !p.done)}
            <button class="wiki-btn" onclick={() => startIndexing(bundle, bundle.indexing_state === 'done' || bundle.indexing_state === 'error')}>
              {bundle.indexing_state === 'done' ? 'Re-index' : 'Index'}
            </button>
          {:else}
            <button class="wiki-btn" onclick={() => stopIndexing(bundle)}>
              Stop
            </button>
          {/if}
          <button class="wiki-btn wiki-btn-danger" onclick={() => confirmRemove(bundle)}>Remove</button>
          <button class="wiki-btn wiki-btn-danger" onclick={() => confirmRemoveWithFile(bundle)}>Remove + delete file</button>
        </div>
      </div>
    {/each}
  </div>
{/if}

<!-- Catalogue -->
<h4 class="settings-subsection">Download bundles</h4>
<p class="settings-notice">
  Fetches the Kiwix catalogue to find available Wikipedia bundles. This is the only outbound
  network request Grimoire makes for the Wikipedia feature.
</p>
<button class="wiki-btn" onclick={fetchCatalogue} disabled={loadingCatalogue}>
  {loadingCatalogue ? 'Fetching…' : 'Fetch catalogue'}
</button>
{#if catalogueError}
  <p class="wiki-error">{catalogueError}</p>
{/if}

{#if catalogueItems.length > 0}
  <div class="wiki-catalogue-search-row">
    <input
      class="wiki-catalogue-search"
      type="search"
      placeholder="Filter bundles…"
      bind:value={catalogueSearch}
    />
    <span class="wiki-catalogue-count">
      {filteredCatalogue.length} of {catalogueItems.length}
    </span>
  </div>
  <div class="wiki-catalogue">
    {#each filteredCatalogue as item (item.id)}
      {@const isInstalled = installedIds().has(item.id)}
      {@const dl = downloadProgress[item.id]}
      <div class="wiki-catalogue-row">
        <div class="wiki-bundle-info">
          <span class="wiki-bundle-title">{item.title || item.name}</span>
          <span class="wiki-bundle-meta">
            {item.article_count ? item.article_count.toLocaleString() + ' articles · ' : ''}{fmt(item.size_bytes)}
          </span>
          {#if dl}
            <span class="wiki-bundle-state">
              Downloading… {fmt(dl.downloaded_bytes)}{dl.total_bytes ? ' / ' + fmt(dl.total_bytes) : ''}
            </span>
            {#if dl.total_bytes}
              <div class="wiki-progress-bar">
                <div class="wiki-progress-fill" style="width: {Math.min(100, (dl.downloaded_bytes / dl.total_bytes) * 100).toFixed(1)}%"></div>
              </div>
            {/if}
          {/if}
        </div>
        <div class="wiki-bundle-actions">
          {#if isInstalled}
            <span class="wiki-installed-badge">Installed</span>
          {:else if !item.download_url}
            <span class="wiki-bundle-meta">No download available</span>
          {:else if !dl}
            <button class="wiki-btn" onclick={() => startDownload(item)}>Download</button>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{/if}

{#if confirmModal}
  <ConfirmModal
    message={confirmModal.message}
    onConfirm={confirmModal.onConfirm}
    onCancel={() => (confirmModal = null)}
  />
{/if}

<style>
  @import '../styles/settings-wikipedia.css';
</style>
