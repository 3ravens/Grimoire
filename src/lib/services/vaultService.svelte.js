import { invoke } from "@tauri-apps/api/core";

/**
 * @param {{
 *   onError?: (e: unknown) => void,
 *   ns: any,
 *   ts: any,
 *   fs: any,
 * }} opts
 */
export function createVaultService({ onError, ns, ts, fs }) {
  let lockCheckDone = $state(false);
  let vaultLocked = $state(false);
  let vaultHasPassword = $state(false);
  let vaultPwModal = $state(null);

  async function checkLockState() {
    try {
      const [locked, hasPw] = await Promise.all([
        invoke("is_vault_locked"),
        invoke("vault_has_password"),
      ]);
      vaultLocked = locked;
      vaultHasPassword = hasPw;
    } catch (err) {
      vaultLocked = true;
      onError?.(err);
    }
    lockCheckDone = true;
  }

  /** @param {() => Promise<void>} loadDataFn */
  async function onVaultUnlocked(loadDataFn) {
    vaultLocked = false;
    await loadDataFn?.();
  }

  async function lockVault() {
    if (!vaultHasPassword) return;
    try {
      await invoke("lock_vault");
      vaultLocked = true;
      // Clear cross-service state so no unencrypted data lingers in the UI.
      ns?.clearActiveNote();
      if (ns) {
        ns.isDirty = false;
        ns.notes = [];
        ns.allTags = [];
      }
      if (ts) {
        ts.tabs = [];
        ts.activeTabId = null;
      }
      if (fs) {
        fs.folders = [];
      }
    } catch (e) {
      onError?.(e);
    }
  }

  /** @param {string} password */
  async function handleVaultPwSubmit(password) {
    if (vaultPwModal === "set" || vaultPwModal === "change") {
      await invoke("set_vault_password", { password });
      vaultHasPassword = true;
      vaultPwModal = null;
      try {
        await invoke("reindex_all");
      } catch (e) {
        onError?.(e);
      }
    } else if (vaultPwModal === "remove") {
      await invoke("remove_vault_password", { password });
      vaultHasPassword = false;
      vaultPwModal = null;
      try {
        await invoke("reindex_all");
      } catch (e) {
        onError?.(e);
      }
    }
    return true;
  }

  function clearState() {
    vaultLocked = true;
    vaultPwModal = null;
  }

  return {
    get lockCheckDone() {
      return lockCheckDone;
    },
    set lockCheckDone(v) {
      lockCheckDone = v;
    },
    get vaultLocked() {
      return vaultLocked;
    },
    set vaultLocked(v) {
      vaultLocked = v;
    },
    get vaultHasPassword() {
      return vaultHasPassword;
    },
    set vaultHasPassword(v) {
      vaultHasPassword = v;
    },
    get vaultPwModal() {
      return vaultPwModal;
    },
    set vaultPwModal(v) {
      vaultPwModal = v;
    },
    checkLockState,
    onVaultUnlocked,
    lockVault,
    handleVaultPwSubmit,
    clearState,
  };
}
