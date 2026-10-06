<script>
  import { onMount, tick } from 'svelte';
  import { focusTrap } from './utils/focusTrap.js';
  import {
    FIRST_START_TOUR_STEPS,
    computeCalloutPosition,
    computeSpotlightHole,
  } from './utils/firstStartTour.js';
  import { t } from './i18n/t.js';

  /** @type {{
   *   stepIndex: number,
   *   onNext: () => void,
   *   onBack: () => void,
   *   onComplete: () => void,
   *   persistError?: string,
   *   persistBusy?: boolean,
   * }} */
  let {
    stepIndex = 0,
    onNext,
    onBack,
    onComplete,
    persistError = '',
    persistBusy = false,
  } = $props();

  const step = $derived(FIRST_START_TOUR_STEPS[stepIndex]);
  const isLast = $derived(stepIndex >= FIRST_START_TOUR_STEPS.length - 1);
  const total = FIRST_START_TOUR_STEPS.length;

  /** @type {{ x: number, y: number, width: number, height: number } | null} */
  let hole = $state(null);
  /** @type {{ top: number, left: number, width: number } | null} */
  let calloutPos = $state(null);
  let anchorMissing = $state(false);

  async function measure() {
    await tick();
    await new Promise((r) => requestAnimationFrame(r));

    const current = FIRST_START_TOUR_STEPS[stepIndex];
    if (!current) {
      hole = null;
      calloutPos = null;
      anchorMissing = true;
      return;
    }

    const el = document.querySelector(current.selector);
    if (!(el instanceof HTMLElement)) {
      hole = null;
      calloutPos = null;
      anchorMissing = true;
      return;
    }

    anchorMissing = false;
    el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    const rect = el.getBoundingClientRect();
    const viewport = { width: window.innerWidth, height: window.innerHeight };
    hole = computeSpotlightHole(rect, viewport);
    calloutPos = computeCalloutPosition(rect, viewport);
  }

  $effect(() => {
    stepIndex;
    void measure();
  });

  onMount(() => {
    const onResize = () => {
      void measure();
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  });

  /** @param {KeyboardEvent} e */
  function handleKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onComplete();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="first-start-tour-root" role="presentation" aria-hidden="false">
  {#if hole}
    <div
      class="first-start-tour-spotlight"
      style:left="{hole.x}px"
      style:top="{hole.y}px"
      style:width="{hole.width}px"
      style:height="{hole.height}px"
      aria-hidden="true"
    ></div>
  {/if}

  {#if anchorMissing}
    <div class="first-start-tour-missing" role="status">
      {t('tour.missingAnchor')}
    </div>
  {/if}

  {#if calloutPos && step}
    <div
      class="first-start-tour-callout"
      use:focusTrap
      role="dialog"
      aria-modal="true"
      aria-labelledby="fst-title"
      aria-describedby="fst-body"
      style:top="{calloutPos.top}px"
      style:left="{calloutPos.left}px"
      style:width="{calloutPos.width}px"
    >
      <p class="first-start-tour-step-count" id="fst-step-count">
        {t('tour.stepOf', { n: stepIndex + 1, total })}
      </p>
      <h2 class="first-start-tour-title" id="fst-title">{t(step.titleKey)}</h2>
      <p class="first-start-tour-body" id="fst-body">{t(step.bodyKey)}</p>
      {#if persistError}
        <p class="first-start-tour-error" role="alert">{persistError}</p>
      {/if}
      <div class="first-start-tour-actions">
        <button
          type="button"
          class="first-start-tour-btn secondary"
          onclick={onComplete}
          disabled={persistBusy}
        >
          {t('tour.skipTour')}
        </button>
        <span class="first-start-tour-spacer"></span>
        {#if stepIndex > 0}
          <button
            type="button"
            class="first-start-tour-btn secondary"
            onclick={onBack}
            disabled={persistBusy}
          >
            {t('tour.back')}
          </button>
        {/if}
        <button
          type="button"
          class="first-start-tour-btn primary"
          onclick={isLast ? onComplete : onNext}
          disabled={persistBusy}
        >
          {#if persistBusy}
            {t('tour.saving')}
          {:else if isLast}
            {t('tour.done')}
          {:else}
            {t('tour.next')}
          {/if}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  @import './styles/first-start-tour.css';
</style>
