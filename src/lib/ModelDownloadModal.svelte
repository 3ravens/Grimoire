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
  import { focusTrap } from './utils/focusTrap.js';

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

  let primaryLabel = $derived(confirmKind === 'installedRisk' ? 'Use this model' : 'Download');

  let confirmTitle = $derived(
    confirmKind === 'installedRisk' ? 'Hardware notice' : 'Model not installed',
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
          Download failed
        {:else if phase === 'pulling'}
          Downloading model
        {:else}
          {confirmTitle}
        {/if}
      </h2>
      {#if phase !== 'pulling'}
        <button class="close-btn" onclick={onCancel} aria-label="Close">✕</button>
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
          <strong>Hardware check</strong>
          <ul>
            {#each hardwareWarning.lines as line}
              <li>{line}</li>
            {/each}
          </ul>
        </div>
      {/if}

      {#if confirmKind === 'downloadMissing'}
        <p class="modal-message">
          <strong>{model}</strong> is not available locally. Ollama can download it from the internet (this may use a lot of disk space and time). Continue?
        </p>
      {:else}
        <p class="modal-message">
          <strong>{model}</strong> is already installed, but it may be a poor match for this machine. You can pick a smaller model in Settings → Hardware if chat is unstable.
        </p>
      {/if}
      <div class="modal-actions">
        <button class="btn-primary" bind:this={primaryBtn} onclick={onDownload}>{primaryLabel}</button>
        <button class="btn-cancel" onclick={onCancel}>Cancel</button>
      </div>
    {:else if phase === 'pulling'}
      <p class="modal-message">Pulling <strong>{model}</strong> via Ollama…</p>
      {#if hardwareWarning?.lines?.length}
        <div
          class="modal-hw-warn"
          class:severe={hardwareWarning.level === 'severe'}
          class:caution={hardwareWarning.level === 'caution'}
          role="note"
        >
          <strong>Reminder</strong>
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
          <div class="progress-bar" aria-label="Model download progress" aria-valuenow={pct} aria-valuemin="0" aria-valuemax="100" role="progressbar">
            <div class="progress-fill" style="width: {pct}%"></div>
          </div>
        </div>
      {/if}
      <div class="modal-actions">
        <button class="btn-cancel" type="button" disabled>Please wait…</button>
      </div>
    {:else}
      <p class="modal-message">{errorMessage || 'The download could not be completed.'}</p>
      <div class="modal-actions">
        <button class="btn-primary" onclick={onCancel}>OK</button>
      </div>
    {/if}
  </div>
</div>

<style>
  @import './styles/model-download-modal.css';
</style>
