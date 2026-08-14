<script>
  import { onMount, tick } from 'svelte';
  import { focusTrap } from './utils/focusTrap.js';
  import {
    FIRST_START_TOUR_STEPS,
    computeCalloutPosition,
    computeSpotlightHole,
  } from './utils/firstStartTour.js';

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
      Could not highlight this part of the interface. You can skip the tour or try the next step.
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
        Step {stepIndex + 1} of {total}
      </p>
      <h2 class="first-start-tour-title" id="fst-title">{step.title}</h2>
      <p class="first-start-tour-body" id="fst-body">{step.body}</p>
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
          Skip tour
        </button>
        <span class="first-start-tour-spacer"></span>
        {#if stepIndex > 0}
          <button
            type="button"
            class="first-start-tour-btn secondary"
            onclick={onBack}
            disabled={persistBusy}
          >
            Back
          </button>
        {/if}
        <button
          type="button"
          class="first-start-tour-btn primary"
          onclick={isLast ? onComplete : onNext}
          disabled={persistBusy}
        >
          {#if persistBusy}
            Saving…
          {:else if isLast}
            Done
          {:else}
            Next
          {/if}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  @import './styles/first-start-tour.css';
</style>
