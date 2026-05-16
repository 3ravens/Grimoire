// Templates store — template list and CRUD operations.
// updateTemplate bumps dbKey so the caller can remount DatabaseView.

import { invoke } from '@tauri-apps/api/core';

export function createTemplates() {
  /** @type {{ id: number, name: string, title: string, content: string }[]} */
  let templates        = $state([]);
  let selectedTemplateId = $state(-1); // -1 = Blank (built-in default)
  let templatesOpen    = $state(false);
  let templateModalOpen = $state(false);
  /** @type {{ id: number, name: string, title: string, content: string } | null} */
  let editingTemplate  = $state(null); // template object being edited, or null
  let dbKey            = $state(0);    // bump to force DatabaseView remount after sync

  async function loadTemplates() {
    try {
      templates = await invoke('list_templates');
    } catch {
      // Non-fatal — picker just shows nothing
    }
  }

  // Throws on failure so TemplateModal can display the error.
  /** @param {string} name @param {string} title @param {string} content @param {any[]} properties */
  async function saveTemplate(name, title, content, properties) {
    await invoke('create_template', { name, title, content, properties });
    await loadTemplates();
    templateModalOpen = false;
  }

  // Throws on failure so TemplateModal can display the error.
  /** @param {string} name @param {string} title @param {string} content @param {any[]} properties */
  async function updateTemplate(name, title, content, properties) {
    const savedId = editingTemplate?.id ?? -1;
    await invoke('update_template', { id: savedId, name, title, content, properties });
    await loadTemplates();
    editingTemplate = null;
    try {
      await invoke('sync_template_to_notes', { templateId: savedId });
      dbKey += 1;
    } catch { /* non-fatal */ }
  }

  /** @param {number} id @param {(e: unknown) => void} showError */
  async function deleteTemplate(id, showError) {
    try {
      await invoke('delete_template', { id });
      await loadTemplates();
      if (selectedTemplateId === id) selectedTemplateId = -1;
    } catch (e) {
      showError(e);
    }
  }

  return {
    get templates() { return templates; },
    get selectedTemplateId() { return selectedTemplateId; },
    set selectedTemplateId(v) { selectedTemplateId = v; },
    get templatesOpen() { return templatesOpen; },
    set templatesOpen(v) { templatesOpen = v; },
    get templateModalOpen() { return templateModalOpen; },
    set templateModalOpen(v) { templateModalOpen = v; },
    get editingTemplate() { return editingTemplate; },
    set editingTemplate(v) { editingTemplate = v; },
    get dbKey() { return dbKey; },
    loadTemplates,
    saveTemplate,
    updateTemplate,
    deleteTemplate,
  };
}
