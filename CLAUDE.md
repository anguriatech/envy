# envy Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-03-19

## Active Technologies

- Rust stable (MSRV to be pinned in `Cargo.toml` `rust-version`) + `rusqlite` (features: `bundled-sqlcipher`), `uuid` (features: `v4`), `keyring`, `clap` (features: `derive`), `thiserror` (001-vault-db-schema)
- `aes-gcm` (AES-256-GCM AEAD encryption), `zeroize` (features: `derive`, memory zeroing on drop) (002-crypto-layer)

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

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
