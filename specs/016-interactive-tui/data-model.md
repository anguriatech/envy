# Data Model: Interactive TUI (016)

**Date**: 2026-08-10 | **Branch**: `016-interactive-tui`

Derived from spec Key Entities (spec.md). Entities below are **TUI state** — they mirror
vault data via `core/` reads and add presentation state. No schema changes: the vault
(`vault.db`) is untouched by this feature.

## Entities

### Project (mirror, read-only)
- **Source**: `core` → `db::list_projects()`.
- **Fields**: `id: ProjectId`, `name: String`.
- **Rules**: non-secret; cached in state at startup and after unlock; refreshes on project switch.

### Environment (mirror, read-only)
- **Source**: `core` → `db::list_environments(project_id)`.
- **Fields**: `id: EnvId`, `name: String`.
- **Rules**: non-secret; the unit of the secrets table; one environment is active at a time.

### SecretEntry (decrypted, zeroized)
- **Source**: `core` → `list_secrets_with_metadata(vault, key, project_id, env_name)`.
- **Fields**: `key: String` (not zeroized — key names are not secret values, but are cleared
  on lock together with the table), `value: Zeroizing<String>`, `updated_at: i64` (Unix seconds).
- **Rules**:
  - value MUST live in `Zeroizing<String>` (FR-016);
  - masked by default (FR-009); visibility is per-selected-row and reset on selection change (FR-011);
  - cleared entirely on lock (FR-013).

### VaultState — state machine
```
Unlocked ⇄ Locked
```
- **Unlocked**: vault connection open, master key held (transiently, in `Zeroizing<[u8;32]>`), secrets loaded.
- **Locked**: `vault.close()` done, key dropped, values cleared; cached Project/Environment structure retained (R-007).
- **Transitions**:
  - `Unlocked --L--> Locked`: close vault, drop key, clear `SecretEntry` list.
  - `Locked --U--> Unlocked`: `get_or_create_master_key()` → `Vault::open(vault_path, key)` → reload projects/envs/secrets. Keyring failure → stay `Locked`, status-bar error.
  - Both: exit → `vault.close()` (FR-018), state dropped → Zeroizing wipes (FR-016).

### SearchQuery
- **Fields**: `text: String`.
- **Rules**: live filter on secret keys, case-insensitive substring (FR-010); empty query = full table; zero matches renders "no matches" state.

### PopupState (editor)
- **Variants**: `NewSecret`, `EditSecret(SecretEntry)`, `DeleteConfirm(SecretEntry)`, `Passphrase(env: String)`, `ConfirmImport(env: String)` (FR-043), `ConfirmSeal(project, [(env, count)])` (FR-054), `Rotate(env, stage: Current | New | Confirm, buffers)` (FR-055).
- **Project actions**: `ProjectPicker(query: String)`, `DeleteProject(name: String, confirmation: String, counts)` and `Help { scroll }` are non-secret UI popups.
- **Read-only operational popups**: `Status(text, scroll)`, `Diff(text, scroll)` and passphrase-scoped decrypt/import actions expose no secret values by default; long text popups scroll with ↑↓/j/k (FR-041).
- **Fields**: `key_input: String`, `value_input: Zeroizing<String>`, `revealed: bool` (Ctrl+R toggle, FR-012 amendment).
- **Rules**: input is masked by default; Enter confirms, Esc cancels; popup open → global hotkeys `L`/`S`/`Q` are ignored (edge case: "Lock while a popup is open"); paste works in every text field (FR-046); cancelling the sync passphrase popup aborts the remaining queue (FR-049).

### CommandPalette (FR-053)
- **Fields**: `query: String`, `index: usize`; actions are a static table of `{id, label}` matched by case-insensitive substring.
- **Rules**: `:` opens/closes; Enter executes the selected action through the same handler as its hotkey; the palette is not a `Popup` so popups can stay open underneath is false — palette and popups are mutually exclusive modes.

### Inspector (FR-050/FR-051)
- **Fields**: `artifact_path: String` (compact display label, non-secret).
- **Rules**: mirrors the focused panel's selection; read-only; hidden under 100 columns.

### Clipboard (FR-057)
- **Fields**: one worker thread owning a single `arboard::Clipboard`; commands over an mpsc channel with a per-copy reply.
- **Rules**: copy never renders the value in clear; clipboard is cleared 30s after the most recent copy; failures surface as status-bar errors, never fatal.

### SyncState
- **Fields**: `phase: Idle | ResolvingPassphrase(env) | Working | Done(Ok | Err(String))`.
- **Rules**: passphrase resolution per env (env var → popup, R-006); `Working` shows status-bar indicator (FR-014); `Done` clears after next key press or refresh.

### BannerState
- **Fields**: `compact: bool`.
- **Rules**: `B` toggles full gradient banner ↔ single-line compact header (FR-005).

## Validation rules (from spec FRs)

- Secret key: non-empty, no `=` — enforced by `core::set_secret` (`InvalidSecretKey`); TUI surfaces the error in the status bar.
- Passphrase: non-empty (core enforces `WeakPassphrase`); mismatched envelope → error + hint to `envy rotate` (same message as CLI).
- Empty environment on sync: skipped (0 secrets), like `cmd_encrypt` (R-006).
