# envy Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-03-23 (006-cli-sync-commands complete)

## Active Technologies
- GitHub Actions YAML; Rust 1.85 (stable) as toolchain installed in CI + `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `shogo82148/actions-setup-perl@v1` (007-ci-smoke-workflows)
- N/A — workflows are stateless; no persistent state between jobs (007-ci-smoke-workflows)

- Rust stable (edition 2024, MSRV 1.85) + `rusqlite` (features: `bundled-sqlcipher`), `uuid` (features: `v4`), `keyring`, `clap` (features: `derive`), `thiserror` (001-vault-db-schema)
- `aes-gcm` (AES-256-GCM AEAD encryption), `zeroize` (features: `derive`, memory zeroing on drop) (002-crypto-layer)
- `toml = "0.8"` (manifest parsing), `serde` (features: `derive`, serialisation) (003-core-logic)
- `dirs = "5"` (cross-platform home directory resolution for vault path `~/.envy/vault.db`) (004-cli-interface)
- `argon2 = "0.5"` (Argon2id KDF for passphrase-based key derivation), `serde_json = "1"` (envy.enc JSON serialization), `base64ct = { version = "1", features = ["alloc"] }` (constant-time Base64 encoding) (005-gitops-sync-artifact)
- `dialoguer = "0.11"` (hidden passphrase prompt + double-entry confirmation; `console` comes transitively for coloured TTY output) (006-cli-sync-commands)

## Project Structure

```text
src/
  crypto/
    artifact.rs   — ArtifactError, SyncArtifact, EncryptedEnvelope, KdfParams, ArtifactPayload, derive_key, seal_envelope, unseal_envelope
  core/
    sync.rs       — SyncError, UnsealResult, seal_artifact, unseal_artifact, write_artifact, read_artifact
  cli/
    mod.rs        — Commands enum (Init, Set, Get, List, Rm, Run, Migrate, Encrypt, Decrypt), run()
    commands.rs   — cmd_* handlers (pub(super))
    error.rs      — CliError, exit-code mappers
tests/
  sync_artifact.rs — e2e integration tests for envy.enc pipeline
  cli_integration.rs — CLI integration tests (requires OS keyring; ignored in CI)
.github/
  workflows/
    release.yml      — cargo-dist managed release workflow (allow-dirty: ["ci"])
    ci.yml           — 3-OS matrix CI (ubuntu/macos/windows); quality gate (fmt/clippy/audit) + cargo test + E2E script; Perl on Windows, dbus/gnome-keyring on Linux
    smoke-test.yml   — post-release smoke test; installs via official installers (no Rust toolchain); full round-trip with envy.enc placement assertion
```

## Commands

cargo test
cargo clippy -- -D warnings
cargo audit

## Code Style

Rust stable (edition 2024, MSRV 1.85): Follow standard conventions

## Recent Changes
- 007-ci-smoke-workflows: Added GitHub Actions YAML; Rust 1.85 (stable) as toolchain installed in CI + `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `shogo82148/actions-setup-perl@v1`

- 001-vault-db-schema: Added `rusqlite` (features: `bundled-sqlcipher`), `uuid` (features: `v4`), `keyring`
- 002-crypto-layer: Added `aes-gcm` (AES-256-GCM AEAD), `zeroize` (features: `derive`); implemented `src/crypto/` with `encrypt`, `decrypt`, `EncryptedSecret`, `get_or_create_master_key`

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
