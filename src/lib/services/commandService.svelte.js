import { getCommand } from '../commands/commandRegistry.js';

/**
 * Shared run / availability / active-state for registry commands.
 *
 * @param {{
 *   handlers: Record<string, () => void>,
 *   isAvailable: (id: string) => boolean,
 *   isActive: (id: string) => boolean,
 * }} deps
 */
export function createCommandService(deps) {
  /** @param {string} id */
  function run(id) {
    const fn = deps.handlers[id];
    if (typeof fn === 'function') fn();
  }

  /** @param {string} id */
  function isAvailable(id) {
    const cmd = getCommand(id);
    if (!cmd) return false;
    return deps.isAvailable(id);
  }

  /** @param {string} id */
  function isActive(id) {
    return deps.isActive(id);
  }

  return { run, isAvailable, isActive };
}
