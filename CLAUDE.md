# envy Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-03-28 (011-envy-diff complete)

## Active Technologies
- GitHub Actions YAML; Rust 1.85 (stable) as toolchain installed in CI + `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `shogo82148/actions-setup-perl@v1` (007-ci-smoke-workflows)
- N/A — workflows are stateless; no persistent state between jobs (007-ci-smoke-workflows)
- Rust stable (edition 2024, MSRV 1.85) + `clap` (derive API, `ValueEnum`), `serde` + `serde_json` (already present), `thiserror` (008-output-formats)
- SQLite via `rusqlite` with `bundled-sqlcipher-vendored-openssl` (existing vault, read-only for this feature) (008-output-formats)
- Rust stable (edition 2024, MSRV 1.85) + `rand = "0.8"` (new direct dep; already in `Cargo.lock` transitively), `dialoguer = "0.11"` (already present; `MultiSelect` + `Confirm` used), `serde_json` (already present) (009-multi-env-encrypt)
- Rust stable (edition 2024, MSRV 1.85) + `rusqlite` (bundled-sqlcipher), `clap` (derive), `serde_json`, `comfy-table = "7"` *(new)* (010-status-command)
- SQLite via `bundled-sqlcipher-vendored-openssl` (existing vault) + `sync_markers` table (new, V2 schema) (010-status-command)
- Rust stable (edition 2024, MSRV 1.85) + `clap` (derive), `serde_json`, `zeroize`, `comfy-table` (unused for diff — ANSI codes instead) (011-envy-diff)
- SQLite via `rusqlite` with `bundled-sqlcipher-vendored-openssl` (read-only for this feature) (011-envy-diff)

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
    diff.rs       — ChangeType, DiffEntry, DiffReport, compute_diff (pure diff logic, no I/O)
    sync.rs       — SyncError, UnsealResult, seal_artifact, unseal_artifact, write_artifact, read_artifact
  core/
    status.rs     — SyncStatus, StatusRow, derive_sync_status, get_status_report
  cli/
    mod.rs        — Commands enum (Init, Set, Get, List, Rm, Run, Migrate, Encrypt, Decrypt, Export, Diff, Status), run()
    commands.rs   — cmd_* handlers (pub(super)); cmd_diff, cmd_status, render_diff_table, write_diff_json, is_color_enabled, colorize
    error.rs      — CliError (incl. EnvNotFound, ArtifactUnreadable), exit-code mappers
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
- 011-envy-diff: Complete — `envy diff` / `envy df` command with table + JSON output; `--reveal` flag for values; ANSI colored output; `ChangeType`/`DiffEntry`/`DiffReport` in `src/core/diff.rs`; `EnvNotFound`/`ArtifactUnreadable` error variants; E2E Scenario 9 added; zero new crates
- 010-status-command: Complete — `envy status` / `envy st` command with table + JSON output; `sync_markers` V2 schema; `seal_env` writes sync marker; `comfy-table = "7"` added; E2E Scenario 8 added
- 010-status-command: Added Rust stable (edition 2024, MSRV 1.85) + `rusqlite` (bundled-sqlcipher), `clap` (derive), `serde_json`, `comfy-table = "7"` *(new)*


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
