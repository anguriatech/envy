# Research: Multi-Environment Encryption and Smart Merging

**Feature**: 009-multi-env-encrypt
**Date**: 2026-03-25

---

## Decision 1 — Diceware word list source and embedding strategy

**Decision**: Embed the EFF Large Wordlist (`eff_large_wordlist.txt`, 7776 words) as a compile-time asset via `include_str!("../../data/eff-wordlist.txt")` inside `src/crypto/diceware.rs`.

**Rationale**: No runtime file I/O, no network access, no additional build-time complexity. The EFF Large Wordlist is the community standard for Diceware (used by 1Password, Bitwarden, Signal). At ~90 KiB it adds negligible binary size. Embedding at compile time satisfies Constitution Principle II (determinism — the word list cannot change between invocations).

**Alternatives considered**:
- Ship as a runtime data file: rejected — requires knowing install path, breaks `cargo install`, fails on some platforms.
- Generate random strings instead of words: rejected — words are more memorable and the whole point of Diceware is memorability.
- Use a third-party `diceware` crate: none with sufficient maintenance on crates.io; the implementation is trivial (parse 5-digit → word map, pick 4 via CSPRNG).

---

## Decision 2 — RNG for Diceware word selection

**Decision**: Add `rand = "0.8"` as a direct dependency. Use `rand::rngs::OsRng` via `rand::seq::SliceRandom::choose` to pick words from the parsed wordlist slice.

**Rationale**: `rand 0.8.5` is already in `Cargo.lock` (transitively via `aes-gcm` → `rand_core`). Making it a direct dep pins the version explicitly and enables the `SliceRandom` trait needed for ergonomic slice sampling. `OsRng` already used in `artifact.rs` for nonce/salt generation satisfies Constitution Principle I (CSPRNG, never `thread_rng` or seeded RNG).

**Alternatives considered**:
- Use raw `OsRng::fill_bytes` + modulo arithmetic to index into the wordlist: works but produces modulo bias for non-power-of-2 wordlist sizes (7776 = 6^5, evenly divides a 32-bit range only approximately). `SliceRandom::choose` uses rejection sampling to eliminate bias.
- `getrandom` crate directly: lower-level, no `SliceRandom`, more code to write.

---

## Decision 3 — Smart merge: where the merge logic lives

**Decision**: The merge logic lives in `cmd_encrypt` (CLI layer). `cmd_encrypt` reads the existing `SyncArtifact` (or creates a new empty one), updates only the target environment envelopes, then calls a new core function `core::write_artifact_atomic`.

**Rationale**: The merge is a coordination concern that requires knowing both which envs to update (a CLI decision, driven by user selection or env-vars) and the existing artifact. Placing it in the CLI layer avoids adding a mutable-update signature to the Core layer that would need to know about "which envs are selected" — a UI concept. The Core layer exposes `seal_env` (seals one env → returns one `EncryptedEnvelope`) and `write_artifact_atomic` (writes atomically). The CLI composes them.

**Alternatives considered**:
- New `core::update_artifact(existing, envs, passphrases)` function: adds a HashMap/Vec of passphrases to the Core signature, coupling the Core to the passphrase-per-env resolution logic that belongs to the CLI layer.
- Replace `SyncArtifact.environments` in-place using `BTreeMap::insert`: that's what `cmd_encrypt` does after getting each `EncryptedEnvelope` back from `core::seal_env`. Clean and direct.

---

## Decision 4 — Atomic write implementation

**Decision**: Refactor `write_artifact` in `src/core/sync.rs` into `write_artifact_atomic`. The implementation writes JSON to `envy.enc.tmp` (sibling of the target path), then calls `std::fs::rename`. Keep the old `write_artifact` as a private alias for backward compatibility within the existing tests, or update them directly.

**Rationale**: `fs::rename` is atomic on all POSIX filesystems when source and destination are on the same device (guaranteed since both are in the same directory). On Windows, `fs::rename` is also atomic as of Windows Vista+ when using `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`. A crash after `fs::write(tmp)` but before `rename` leaves the old `envy.enc` intact (FR-006, SC-003).

**Cross-device case**: `fs::rename` fails with `EXDEV` if the temp file and destination are on different filesystems. Since the temp file is `envy.enc.tmp` in the same directory as `envy.enc`, cross-device failure is structurally impossible in normal usage. Error is surfaced as `SyncError::Io` with a clear message (FR-013 analogue for writes).

**Alternatives considered**:
- `tempfile::NamedTempFile::persist`: adds a dependency (`tempfile` is already a dev-dependency but not a runtime dependency); overkill for a single-file atomic write.
- Write to OS temp dir then rename: cross-device risk is real in this case; rejected.

---

## Decision 5 — Per-environment passphrase resolution

**Decision**: Add `resolve_passphrase_for_env(env_name: &str, confirm: bool, suggested: Option<&str>) -> Result<Zeroizing<String>, CliError>` in `src/cli/commands.rs`. Priority order:
1. `ENVY_PASSPHRASE_<UPPER_NORMALISED>` (e.g., `my-env` → `ENVY_PASSPHRASE_MY_ENV`)
2. `ENVY_PASSPHRASE` fallback
3. Interactive prompt (with optional Diceware suggestion as default)

**Rationale**: Per-env env vars give CI operators fine-grained control without requiring a `--passphrase` flag (which is prohibited by FR-003). Normalisation (uppercase + hyphen → underscore) follows the convention used by Docker, Heroku, and GitHub Actions for environment-derived variable names. The existing `resolve_passphrase` is kept for `cmd_decrypt` which does not have per-env semantics.

**Normalisation rule**: `env_name.to_uppercase().replace('-', "_")` applied once; result prefixed with `ENVY_PASSPHRASE_`. Special characters other than hyphens (e.g., dots, slashes) are not normalised — environments with such names simply will not match an env var and will fall through to interactive mode.

**Alternatives considered**:
- A single `ENVY_PASSPHRASE` for all envs and a `--env` filter flag: insufficient for multi-env automation with different passphrases per env.
- `--passphrase` CLI flag: explicitly prohibited by FR-003.

---

## Decision 6 — Pre-flight check: where unseal_envelope is called from

**Decision**: Add `core::check_envelope_passphrase(passphrase: &str, env_name: &str, envelope: &EncryptedEnvelope) -> bool` in `src/core/sync.rs`. Returns `true` if the passphrase decrypts the envelope successfully. `cmd_encrypt` calls this before sealing.

**Rationale**: Constitution Principle IV prohibits the CLI layer from calling `crate::crypto` directly (except the two permitted infrastructure exceptions). `unseal_envelope` lives in `crate::crypto::artifact`. Wrapping it in a Core function maintains the layer boundary and gives the check a meaningful, intention-revealing name.

**Alternatives considered**:
- Call `crate::crypto::unseal_envelope` directly from `cmd_encrypt`: violates Principle IV.
- Re-export `unseal_envelope` from `crate::core` and call it from CLI: technically compliant but makes the Core a thin pass-through with no added value; the named wrapper is cleaner.

---

## Decision 7 — Headless mode: which environments are encrypted

**Decision**: In headless mode (at least one `ENVY_PASSPHRASE*` env var is set), `cmd_encrypt` iterates ALL environments from the vault. For each environment, it calls `resolve_passphrase_for_env` — if an env var resolves, that env is encrypted; if neither `ENVY_PASSPHRASE_<ENV>` nor `ENVY_PASSPHRASE` is set, that env is skipped silently.

**Rationale**: Since `ENVY_PASSPHRASE` is always set in typical CI pipelines (as a global fallback), this means "all environments with a resolvable passphrase" effectively means "all environments" when the fallback is set. Operators who want to seal only one env headlessly set only `ENVY_PASSPHRASE_<ENV>` for that env and do NOT set `ENVY_PASSPHRASE`. This matches the spec assumption.

**Headless mode detection**: The system is "in headless mode" if, for any environment in the vault, a passphrase env var resolves. The interactive menu is only shown when NO passphrase env var resolves for ANY environment.

**Alternatives considered**:
- Always show interactive menu even when some env vars are set: surprising in CI where `ENVY_PASSPHRASE` is set globally; confusing behaviour.
- Require explicit `--headless` flag: adds complexity, prohibited by spec design (no new flags).

---

## Decision 8 — `seal_artifact` backward compatibility

**Decision**: Keep the existing `seal_artifact(vault, master_key, project_id, passphrase, envs)` function unchanged in `src/core/sync.rs`. Add a new `seal_env` function alongside it. `cmd_encrypt` switches to using `seal_env` + the merge loop. Existing tests for `seal_artifact` remain valid.

**Rationale**: `seal_artifact` is tested directly and used by `cmd_encrypt`'s current implementation. Replacing it mid-feature risks breaking the test suite during implementation. Adding `seal_env` alongside it avoids a disruptive refactor — `seal_artifact` can be internally refactored to call `seal_env` in a follow-up if desired.
