# Internal API Contract: `core::sync::rotate_env`

**Feature**: 012-cli-rotate
**Date**: 2026-06-10
**Type**: Rust function signature contract (internal, not exposed to users)

## Signature

```rust
pub fn rotate_env(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    artifact: &mut SyncArtifact,
    env_name: &str,
    current_passphrase: &str,
    new_passphrase: &str,
) -> Result<(), SyncError>
```

## Pre-conditions

- `env_name` is a valid, lowercased environment name (the schema enforces `name = lower(name)`).
- `current_passphrase` and `new_passphrase` are non-empty, non-whitespace strings.
- `artifact` is a valid `SyncArtifact` (the `version` field equals `ARTIFACT_VERSION`).

## Post-conditions (success)

- `artifact.environments[env_name]` is replaced with a fresh `EncryptedEnvelope` whose plaintext is the secrets currently in the vault for `env_name`.
- All other entries in `artifact.environments` are byte-identical to their prior state.
- The vault's `sync_markers` table has a fresh row for `env_name` (written by the reused `seal_env` helper).
- The vault's `secrets` table is unchanged.

## Post-conditions (failure)

- `artifact` is unchanged (the function returns early before any modification).
- The vault is unchanged (the function does not touch it on failure).

## Errors

| `SyncError` variant | When | Maps to `CliError` |
|---------------------|------|---------------------|
| `SyncError::Artifact(ArtifactError::DecryptionFailed)` | `current_passphrase` does not unseal the existing envelope. | `CliError::PassphraseInput` (exit 2) |
| `SyncError::Artifact(ArtifactError::WeakPassphrase)` | `new_passphrase` is empty or whitespace-only. | `CliError::PassphraseInput` (exit 2) |
| `SyncError::Artifact(ArtifactError::MalformedArtifact("env not in artifact"))` | `env_name` is not in `artifact.environments`. (Note: this reuses an existing variant with a different message; alternative is to add a new variant — see "open questions" below.) | `CliError::EnvNotFound` (exit 3) |
| `SyncError::VaultError(_)` | Reading secrets from the vault failed. | `CliError::VaultOpen` (exit 4) |
| `SyncError::Io(_)` | Atomic write of the artifact failed. | `CliError::VaultOpen` (exit 4) |

## Algorithm

```
1. Validate new_passphrase is non-empty, non-whitespace. Else return WeakPassphrase.
2. Look up artifact.environments[env_name]. If absent, return MalformedArtifact("env not in artifact").
3. Verify current_passphrase via check_envelope_passphrase(current_passphrase, env_name, &existing).
   If false, return DecryptionFailed.
4. Read secrets from the vault via get_env_secrets (delegated to seal_env).
5. Call seal_env(vault, master_key, project_id, env_name, new_passphrase) — this:
   a. Validates new_passphrase is non-empty (redundant but cheap).
   b. Reads secrets from the vault.
   c. Builds a fresh ArtifactPayload and calls seal_envelope (new nonce, new KDF salt).
   d. Writes a fresh sync_marker row.
   e. Returns the new EncryptedEnvelope.
6. Insert the new envelope into artifact.environments, replacing the old one.
7. Return Ok(()).
```

The atomic write of `envy.enc` is performed by the caller (`cmd_rotate`), NOT by `rotate_env` itself. `rotate_env` mutates the in-memory `SyncArtifact` only; persistence is the caller's responsibility. This keeps `rotate_env` testable without filesystem side-effects.

## Why this signature

- `&mut SyncArtifact` (not `&SyncArtifact`) because the function mutates the envelope in place. Callers that want to keep the original can clone the artifact before calling.
- `&str` for the passphrases (not `Zeroizing<String>`) because the core layer does not own secret-bearing state; the CLI layer wraps the passphrases in `Zeroizing` and passes views. The `&str` views are constructed on demand and never outlive the `Zeroizing` binding in the caller's scope.
- `Result<(), SyncError>` (not a custom error type) because the error cases all map cleanly to existing `SyncError` variants.

## Open questions

None. All three clarifications provided by the user resolved the design questions.
