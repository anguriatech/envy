# envy Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-03-22

## Active Technologies

- Rust stable (edition 2024, MSRV 1.85) + `rusqlite` (features: `bundled-sqlcipher`), `uuid` (features: `v4`), `keyring`, `clap` (features: `derive`), `thiserror` (001-vault-db-schema)
- `aes-gcm` (AES-256-GCM AEAD encryption), `zeroize` (features: `derive`, memory zeroing on drop) (002-crypto-layer)
- `toml = "0.8"` (manifest parsing), `serde` (features: `derive`, serialisation) (003-core-logic)
- `dirs = "5"` (cross-platform home directory resolution for vault path `~/.envy/vault.db`) (004-cli-interface)
- `argon2 = "0.5"` (Argon2id KDF for passphrase-based key derivation), `serde_json = "1"` (envy.enc JSON serialization), `base64ct = { version = "1", features = ["alloc"] }` (constant-time Base64 encoding) (005-gitops-sync-artifact)

## Project Structure

```text
src/
  crypto/
    artifact.rs   — ArtifactError, SyncArtifact, EncryptedEnvelope, KdfParams, ArtifactPayload, derive_key, seal_envelope, unseal_envelope
  core/
    sync.rs       — SyncError, UnsealResult, seal_artifact, unseal_artifact, write_artifact, read_artifact
tests/
  sync_artifact.rs — e2e integration tests for envy.enc pipeline
```

## Commands

cargo test
cargo clippy -- -D warnings
cargo audit

## Code Style

Rust stable (edition 2024, MSRV 1.85): Follow standard conventions

## Recent Changes

- 001-vault-db-schema: Added `rusqlite` (features: `bundled-sqlcipher`), `uuid` (features: `v4`), `keyring`
- 002-crypto-layer: Added `aes-gcm` (AES-256-GCM AEAD), `zeroize` (features: `derive`); implemented `src/crypto/` with `encrypt`, `decrypt`, `EncryptedSecret`, `get_or_create_master_key`
- 003-core-logic: Added `toml`, `serde` (features: `derive`); implemented `src/core/` with `find_manifest`, `create_manifest`, `set_secret`, `get_secret`, `list_secret_keys`, `delete_secret`, `get_env_secrets`
- 004-cli-interface: Added `dirs = "5"`; implemented `src/cli/` with 7 subcommands (`init`, `set`, `get`, `list`/`ls`, `rm`/`remove`, `run`, `migrate`), `CliError` enum, exit-code table (0/1/2/3/4/127), and `pub fn run() -> i32` dispatch
- 005-gitops-sync-artifact: Added `argon2 = "0.5"`, `serde_json = "1"`, `base64ct = "1"`; implemented `src/crypto/artifact.rs` (Argon2id KDF + AES-256-GCM envelope crypto) and `src/core/sync.rs` (seal/unseal/write/read artifact orchestration); Progressive Disclosure — unseal skips inaccessible envs gracefully

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
