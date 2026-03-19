# envy Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-03-19

## Active Technologies

- Rust stable (MSRV to be pinned in `Cargo.toml` `rust-version`) + `rusqlite` (features: `bundled-sqlcipher`), `uuid` (features: `v4`), `keyring`, `clap` (features: `derive`), `thiserror` (001-vault-db-schema)
- `aes-gcm` (AES-256-GCM AEAD encryption), `zeroize` (features: `derive`, memory zeroing on drop) (002-crypto-layer)
- `toml = "0.8"` (manifest parsing), `serde` (features: `derive`, serialisation) (003-core-logic)
- `dirs = "5"` (cross-platform home directory resolution for vault path `~/.envy/vault.db`) (004-cli-interface)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test
cargo clippy -- -D warnings
cargo audit

## Code Style

Rust stable (MSRV to be pinned in `Cargo.toml` `rust-version`): Follow standard conventions

## Recent Changes

- 001-vault-db-schema: Added Rust stable (MSRV to be pinned in `Cargo.toml` `rust-version`) + `rusqlite` (features: `bundled-sqlcipher`), `uuid` (features: `v4`), `keyring`
- 002-crypto-layer: Added `aes-gcm` (AES-256-GCM AEAD), `zeroize` (features: `derive`); implemented `src/crypto/` with `encrypt`, `decrypt`, `EncryptedSecret`, `get_or_create_master_key`
- 003-core-logic: Added `toml`, `serde` (features: `derive`); implemented `src/core/` with `find_manifest`, `create_manifest`, `set_secret`, `get_secret`, `list_secret_keys`, `delete_secret`, `get_env_secrets`
- 004-cli-interface: Added `dirs = "5"`; implemented `src/cli/` with 7 subcommands (`init`, `set`, `get`, `list`/`ls`, `rm`/`remove`, `run`, `migrate`), `CliError` enum (6 variants), exit-code table (0/1/2/3/4/127), and `pub fn run() -> i32` dispatch; added 7 end-to-end integration tests in `tests/cli_integration.rs` (all `#[ignore]` — require live OS keyring)

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
