<script>
  import { tick } from 'svelte';

  let { x = 0, y = 0, label = 'What should the LLM improve?', onSend, onCancel } = $props();

  let instruction = $state('');

  let textareaEl = $state(null);

  $effect(() => {
    if (textareaEl) {
      tick().then(() => textareaEl.focus());
    }
  });

  function handleKeydown(e) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
    if (e.key === 'Escape') {
      onCancel();
    }
  }

  async function submit() {
    const val = instruction.trim();
    if (!val) return;
    onSend(val);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="improve-backdrop" onmousedown={onCancel}></div>

<div class="improve-popover" style="left: {Math.min(x, window.innerWidth - 440)}px; top: {Math.min(y, window.innerHeight - 200)}px;">
  <span class="improve-popover-label">{label}</span>
  <textarea
    bind:this={textareaEl}
    bind:value={instruction}
    onkeydown={handleKeydown}
    placeholder='E.g. "Fix grammar and spelling", "Make it more concise", "Restructure into bullet points"'
    aria-label="Improvement instruction"
  ></textarea>
  <div class="improve-popover-actions">
    <button onclick={onCancel}>Cancel</button>
    <button class="primary" disabled={!instruction.trim()} onclick={submit}>Improve</button>
  </div>
</div>

<style>
  @import './styles/improve-popover.css';
</style>
