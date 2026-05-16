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

  let { onDismiss } = $props();

  function handleBackdropKeydown(e) {
    if (e.key === 'Escape') onDismiss();
  }

  let dismissBtn = $state(null);
  $effect(() => {
    dismissBtn?.focus();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="backdrop"
  onclick={onDismiss}
  onkeydown={handleBackdropKeydown}
  role="dialog"
  aria-modal="true"
  aria-labelledby="privacy-egg-title"
  tabindex="-1"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" use:focusTrap onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <h2 id="privacy-egg-title" class="modal-title">Privacy policy</h2>
      <button type="button" class="close-btn" onclick={onDismiss} aria-label="Close">✕</button>
    </div>

    <div class="modal-body">
      <p class="policy-line"><strong>Data we track:</strong></p>
      <ul class="policy-list">
        <li>nothing</li>
      </ul>
    </div>

    <div class="modal-actions">
      <button type="button" class="btn-dismiss" bind:this={dismissBtn} onclick={onDismiss}>Got it</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 500;
  }

  .modal {
    background: var(--bg2);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 20px 24px;
    width: min(360px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .modal-title {
    font: 600 14px/1 var(--sans);
    color: var(--text-h);
    margin: 0;
  }

  .close-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text);
    opacity: 0.45;
    font-size: 12px;
    padding: 2px 4px;
    line-height: 1;
  }

  .close-btn:hover {
    opacity: 1;
  }

  .modal-body {
    font: 13px/1.55 var(--sans);
    color: var(--text);
  }

  .policy-line {
    margin: 0 0 4px;
  }

  .policy-list {
    margin: 0 0 0 1.1em;
    padding: 0;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 2px;
  }

  .btn-dismiss {
    padding: 6px 16px;
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 4px;
    font: 13px var(--sans);
    cursor: pointer;
    transition: opacity 0.1s;
  }

  .btn-dismiss:hover {
    opacity: 0.9;
  }
</style>
