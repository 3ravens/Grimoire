import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

let pullInFlight = false;

export function isPullInFlight() {
  return pullInFlight;
}

export async function checkChatModelInstalled(model) {
  return invoke('ollama_model_installed', { model: String(model).trim() });
}

export async function saveChatModelSetting(model) {
  await invoke('set_setting', { key: 'chat_model', value: String(model).trim() });
}

/** Delete a locally installed Ollama model (resolved like install checks). */
export async function deleteOllamaModel(model) {
  return invoke('delete_ollama_model', { model: String(model).trim() });
}

/** @param {(payload: Record<string, unknown>) => void} [onProgress] */
export async function pullChatModel(model, onProgress) {
  if (pullInFlight) {
    throw new Error('Another model download is already in progress.');
  }
  let unlisten = null;
  unlisten = await listen('ollama:pull_progress', (e) => onProgress?.(e.payload));
  pullInFlight = true;
  try {
    await invoke('pull_ollama_model', { model: String(model).trim() });
  } finally {
    try {
      unlisten?.();
    } catch {
      /* ignore */
    }
    pullInFlight = false;
  }
}
