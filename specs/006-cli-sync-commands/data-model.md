# Data Model: CLI Sync Commands (encrypt / decrypt)

**Feature**: 006-cli-sync-commands
**Date**: 2026-03-22

This feature introduces no new database entities — all persistent data is handled by the existing vault and `envy.enc` artifact format (defined in feature 005). This document focuses on the in-memory data flow and the new CLI-layer types.

---

## In-Memory Data Flow

### `encrypt` command

```
Terminal/ENVY_PASSPHRASE
        │
        ▼ Zeroizing<String>         passphrase (never touches disk)
┌─────────────────┐
│   cmd_encrypt   │ ← vault: &Vault, master_key: &[u8;32], project_id: &ProjectId
│ (CLI layer)     │ ← env_filter: Option<String>  (from -e flag)
└────────┬────────┘
         │ seal_artifact(vault, master_key, project_id, passphrase, envs)
         ▼
┌────────────────┐
│  core::sync    │   reads secrets → encrypts per env → returns SyncArtifact
└────────┬───────┘
         │ write_artifact(artifact, path)
         ▼
    envy.enc (disk)
```

### `decrypt` command

```
Terminal/ENVY_PASSPHRASE
        │
        ▼ Zeroizing<String>
┌─────────────────┐
│   cmd_decrypt   │ ← vault: &Vault, master_key: &[u8;32], project_id: &ProjectId
│ (CLI layer)     │
└────────┬────────┘
         │ read_artifact(path)
         ▼
┌────────────────┐
│  core::sync    │   parse envy.enc → unseal envs → UnsealResult
└────────┬───────┘
         │ for each env in UnsealResult.imported:
         │   set_secret(vault, master_key, project_id, env, key, value)
         ▼
    vault (secrets upserted)
```

---

## New CLI-Layer Types

### `CliError` additions

```rust
/// Terminal passphrase read failed (IO error, Ctrl-C, or confirmation mismatch).
PassphraseInput(String),

/// `decrypt` completed but zero environments were imported.
/// The caller should suggest checking the passphrase.
NothingImported,
```

### `Commands` additions

```rust
/// Seal the vault into an `envy.enc` artifact.
#[command(alias = "enc")]
Encrypt {
    /// Seal only this environment (default: all environments).
    #[arg(short = 'e', long = "env", value_name = "ENV")]
    env: Option<String>,
},

/// Unseal `envy.enc` and upsert secrets into the local vault.
#[command(alias = "dec")]
Decrypt,
```

---

## Data Flow Rules

| Constraint | Enforced by |
|------------|-------------|
| Passphrase is `Zeroizing<String>` from the moment it leaves terminal/env | `cmd_encrypt`, `cmd_decrypt` handlers |
| Passphrase is NEVER logged, printed, or stored | CLI handler must not print passphrase |
| `envy.enc` path is always `<manifest_dir>/envy.enc` | `cmd_encrypt` and `cmd_decrypt` derive path from `find_manifest` |
| Vault is not written during `decrypt` if zero envs are imported | `cmd_decrypt` checks `result.imported.is_empty()` before any `set_secret` |
| No partial vault writes on error mid-import | Each `set_secret` is independent; any failure is logged as a warning, import continues |
| `UnsealResult.imported` values are `Zeroizing<String>` — zeroed after import | Core layer guarantees; CLI layer drops `result` at end of function |

---

## Output Format Data

### `encrypt` success

```
Sealed 2 environment(s) into envy.enc:
  ✓  development   (3 secrets)
  ✓  staging       (2 secrets)
```

- Green `✓` per environment line.
- Final line: path to written artifact.

### `decrypt` success (all imported)

```
Decrypted envy.enc — 2 environment(s) imported:
  ✓  development   (3 secrets upserted)
  ✓  staging       (2 secrets upserted)
```

### `decrypt` partial (Progressive Disclosure)

```
Decrypted envy.enc — 1 environment(s) imported:
  ✓  development   (3 secrets upserted)
  ⚠  production    skipped (wrong passphrase or different key)
```

- Green `✓` for imported.
- Yellow dim `⚠` for skipped.
- Exit code: **0** (partial success is not an error).

### `decrypt` nothing imported

```
error: no environments could be decrypted — check your passphrase
```

- Exit code: **1**.

---

## Secrets-in-Transit Lifecycle

```
passphrase string
   └── Zeroizing::new(string)       ← immediately on terminal read or env var read
         └── &str ref passed to seal_artifact / unseal_artifact
               └── core zeroes its own copies via ArtifactPayload
                     └── Zeroizing<String> dropped at end of cmd_* function scope
```

Secret values from `UnsealResult.imported` follow the same pattern — they are `Zeroizing<String>` and are passed directly to `set_secret` without buffering.
