<script>
  import { focusTrap } from './utils/focusTrap.js';
  /**
   * TemplateModal — modal for creating or editing a user-defined template.
   *
   * Props:
   *   onSave(name, title, content, properties)  — called when the form is submitted
   *   onCancel                                  — called when the modal is dismissed
   *   template                                  — optional; edit mode pre-fills all fields
   */
  import { untrack } from 'svelte';

  let { onSave, onCancel, template = null } = $props();

  // untrack: this modal mounts fresh each open, so capturing the initial prop value is intentional.
  let name    = $state(untrack(() => template?.name    ?? ''));
  let title   = $state(untrack(() => template?.title   ?? ''));
  let content = $state(untrack(() => template?.content ?? ''));
  // Each property spec is { name, type, options } — options is a comma-separated string in the UI,
  // stored as a JSON array string when sent to Rust.
  let templateProps = $state(
    untrack(() =>
      (template?.properties ?? []).map(p => ({
        name: p.name,
        type: p.type,
        options: (() => {
          if (!p.options) return '';
          try {
            const arr = JSON.parse(p.options);
            return Array.isArray(arr) ? arr.join(', ') : String(arr);
          } catch {
            return '';
          }
        })(),
      }))
    )
  );
  let loading = $state(false);
  let error   = $state('');

  const isEditing = $derived(template !== null);

  $effect(() => {
    document.getElementById('tmpl-modal-name')?.focus();
  });

  function addProp() {
    templateProps = [...templateProps, { name: '', type: 'text', options: '' }];
  }

  function removeProp(i) {
    templateProps = templateProps.filter((_, idx) => idx !== i);
  }

  function updateProp(i, field, value) {
    templateProps = templateProps.map((p, idx) =>
      idx === i ? { ...p, [field]: value } : p
    );
  }

  async function submit() {
    if (!name.trim()) return;
    loading = true;
    error = '';
    try {
      // Convert UI comma-separated options back to JSON array strings for Rust.
      const properties = templateProps
        .filter(p => p.name.trim())
        .map(p => ({
          name: p.name.trim(),
          type: p.type,
          options: p.type === 'select' && p.options.trim()
            ? JSON.stringify(p.options.split(',').map(s => s.trim()).filter(Boolean))
            : null,
        }));
      await onSave(name.trim(), title.trim(), content, properties);
    } catch (e) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') onCancel();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" onclick={onCancel} onkeydown={handleKeydown} role="dialog" aria-modal="true" aria-labelledby="tmpl-modal-title" tabindex="-1">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" use:focusTrap onclick={(e) => e.stopPropagation()}>
    <h2 id="tmpl-modal-title" class="modal-title">{isEditing ? 'Edit Template' : 'New Template'}</h2>

    <label class="field">
      <span>Template name</span>
      <input
        id="tmpl-modal-name"
        type="text"
        bind:value={name}
        placeholder="e.g. Project Kickoff"
        disabled={loading}
      />
    </label>

    <label class="field">
      <span>Default note title <span class="optional">(optional)</span></span>
      <input
        type="text"
        bind:value={title}
        placeholder="Leave blank to use template name"
        disabled={loading}
      />
    </label>

    <label class="field">
      <span>Content <span class="optional">(optional)</span></span>
      <textarea
        bind:value={content}
        placeholder="Template body…"
        rows="6"
        disabled={loading}
      ></textarea>
    </label>

    <!-- Property definitions -->
    <div class="props-section">
      <span class="props-section-label">Properties</span>
      {#if templateProps.length > 0}
        <div class="props-list">
          {#each templateProps as prop, i}
            <div class="prop-spec-row">
              <input
                type="text"
                class="prop-spec-input"
                value={prop.name}
                oninput={(e) => updateProp(i, 'name', e.currentTarget.value)}
                placeholder="Name"
                disabled={loading}
              />
              <select
                class="prop-spec-input prop-spec-type"
                value={prop.type}
                onchange={(e) => updateProp(i, 'type', e.currentTarget.value)}
                disabled={loading}
              >
                <option value="text">Text</option>
                <option value="number">Number</option>
                <option value="date">Date</option>
                <option value="boolean">Checkbox</option>
                <option value="select">Select</option>
              </select>
              {#if prop.type === 'select'}
                <input
                  type="text"
                  class="prop-spec-input prop-spec-options"
                  value={prop.options}
                  oninput={(e) => updateProp(i, 'options', e.currentTarget.value)}
                  placeholder="Options (comma-separated)"
                  disabled={loading}
                />
              {/if}
              <button
                class="prop-spec-delete"
                onclick={() => removeProp(i)}
                disabled={loading}
                title="Remove"
                aria-label="Remove property {prop.name || (i + 1)}"
              >✕</button>
            </div>
          {/each}
        </div>
      {/if}
      <button class="prop-spec-add" onclick={addProp} disabled={loading}>+ Add property</button>
    </div>

    {#if error}
      <p class="modal-error">{error}</p>
    {/if}

    <div class="modal-actions">
      <button class="modal-cancel" onclick={onCancel} disabled={loading}>Cancel</button>
      <button
        class="modal-confirm"
        onclick={submit}
        disabled={loading || !name.trim()}
      >
        {loading ? 'Saving…' : (isEditing ? 'Save Changes' : 'Save Template')}
      </button>
    </div>
  </div>
</div>

<style>
  @import './styles/templates.css';
</style>
