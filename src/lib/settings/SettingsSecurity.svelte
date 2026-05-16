<script>
  let {
    vaultHasPassword = false,
    onSetVaultPassword = () => {},
    onChangeVaultPassword = () => {},
    onRemoveVaultPassword = () => {},
    onLockVault = () => {},
  } = $props();
</script>

<h3>Security</h3>

<div class="setting-row">
  <div class="setting-label">
    <span class="setting-name">Vault password</span>
    <span class="setting-desc">
      Password-protects the entire vault. Contents are encrypted at rest using a key
      derived from the password via Argon2id. The password is never stored — if you
      forget it, your notes cannot be recovered.
    </span>
  </div>
  <div class="vault-actions">
    {#if !vaultHasPassword}
      <button class="settings-action-btn" onclick={onSetVaultPassword}>Set password</button>
    {:else}
      <button class="settings-action-btn" onclick={onChangeVaultPassword}>Change password</button>
      <button class="settings-action-btn" onclick={onRemoveVaultPassword}>Remove password</button>
      <button class="settings-action-btn danger" onclick={onLockVault}>Lock vault now</button>
    {/if}
  </div>
</div>
