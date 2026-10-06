<script>
  import { focusTrap } from './utils/focusTrap.js';
  import { t } from './i18n/t.js';

  /**
   * @typedef {'confirm' | 'pulling' | 'error'} ModelDownloadPhase
   * @typedef {'downloadMissing' | 'installedRisk'} ConfirmKind
   * @typedef {{ level: 'caution' | 'severe', lines: string[] }} HardwareWarning
   */

  /** @type {{ model: string, phase: ModelDownloadPhase, confirmKind?: ConfirmKind, hardwareWarning?: HardwareWarning | null, statusLine: string, progress: { completed: number, total: number } | null, errorMessage: string, onDownload: () => void, onCancel: () => void }} */
  let {
    model = '',
    phase = 'confirm',
    confirmKind = 'downloadMissing',
    hardwareWarning = null,
    statusLine = '',
    progress = null,
    errorMessage = '',
    onDownload,
    onCancel,
  } = $props();

  function handleKeydown(e) {
    if (e.key === 'Escape' && phase !== 'pulling') onCancel();
  }

  let primaryBtn = $state(null);
  $effect(() => {
    if (phase === 'confirm' && primaryBtn) primaryBtn.focus();
  });

  let pct = $derived.by(() => {
    if (!progress || progress.total <= 0) return null;
    return Math.min(100, Math.round((100 * progress.completed) / progress.total));
  });

  let primaryLabel = $derived(
    confirmKind === 'installedRisk' ? t('views.modelDownloadUse') : t('views.modelDownloadDownload'),
  );

  let confirmTitle = $derived(
    confirmKind === 'installedRisk'
      ? t('views.modelDownloadHwNotice')
      : t('views.modelDownloadNotInstalled'),
  );
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="backdrop"
  onclick={() => {
    if (phase !== 'pulling') onCancel();
  }}
  onkeydown={handleKeydown}
  role="dialog"
  aria-modal="true"
  aria-labelledby="mdl-title"
  tabindex="-1"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" use:focusTrap onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <h2 id="mdl-title" class="modal-title">
        {#if phase === 'error'}
          {t('views.modelDownloadError')}
        {:else if phase === 'pulling'}
          {t('views.modelDownloadPulling')}
        {:else}
          {confirmTitle}
        {/if}
      </h2>
      {#if phase !== 'pulling'}
        <button class="close-btn" onclick={onCancel} aria-label={t('common.close')}>✕</button>
      {/if}
    </div>

    {#if phase === 'confirm'}
      {#if hardwareWarning?.lines?.length}
        <div
          class="modal-hw-warn"
          class:severe={hardwareWarning.level === 'severe'}
          class:caution={hardwareWarning.level === 'caution'}
          role="note"
        >
          <strong>{t('views.modelDownloadHwCheck')}</strong>
          <ul>
            {#each hardwareWarning.lines as line}
              <li>{line}</li>
            {/each}
          </ul>
        </div>
      {/if}

      {#if confirmKind === 'downloadMissing'}
        <p class="modal-message">
          {t('views.modelDownloadMissingBody', { model })}
        </p>
      {:else}
        <p class="modal-message">
          {t('views.modelDownloadRiskBody', { model })}
        </p>
      {/if}
      <div class="modal-actions">
        <button class="btn-primary" bind:this={primaryBtn} onclick={onDownload}>{primaryLabel}</button>
        <button class="btn-cancel" onclick={onCancel}>{t('common.cancel')}</button>
      </div>
    {:else if phase === 'pulling'}
      <p class="modal-message">{t('views.modelDownloadPullingVia', { model })}</p>
      {#if hardwareWarning?.lines?.length}
        <div
          class="modal-hw-warn"
          class:severe={hardwareWarning.level === 'severe'}
          class:caution={hardwareWarning.level === 'caution'}
          role="note"
        >
          <strong>{t('views.modelDownloadReminder')}</strong>
          <ul>
            {#each hardwareWarning.lines as line}
              <li>{line}</li>
            {/each}
          </ul>
        </div>
      {/if}
      {#if statusLine}
        <div class="pull-status" role="status" aria-live="polite">{statusLine}</div>
      {/if}
      {#if pct !== null}
        <div class="progress-wrap">
          <div class="progress-bar" aria-label={t('views.modelDownloadProgressAria')} aria-valuenow={pct} aria-valuemin="0" aria-valuemax="100" role="progressbar">
            <div class="progress-fill" style="width: {pct}%"></div>
          </div>
        </div>
      {/if}
      <div class="modal-actions">
        <button class="btn-cancel" type="button" disabled>{t('views.modelDownloadWait')}</button>
      </div>
    {:else}
      <p class="modal-message">{errorMessage || t('views.modelDownloadFailed')}</p>
      <div class="modal-actions">
        <button class="btn-primary" onclick={onCancel}>{t('common.ok')}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  @import './styles/model-download-modal.css';
</style>
