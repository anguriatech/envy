# Feature Specification: Interactive TUI (ratatui + crossterm)

**Feature Branch**: `016-interactive-tui`  
**Created**: 2026-08-10  
**Status**: Draft  
**Input**: User description: "Full-screen TUI for Envy — pixel-art ENVY banner with bluish-purple gradient, project/environment sidebar, secrets table with mask/search/edit, lock/unlock, GitOps sync, triggered only when `envy` runs bare in an interactive terminal, silent otherwise, with zeroization guarantees."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Launch the interactive TUI (Priority: P1)

A developer types bare `envy` in an interactive terminal and is taken to a full-screen interface showing their vault at a glance: a pixel-art "ENVY" banner, vault lock state, active environment, project/environment sidebar, and a secrets table.

**Why this priority**: This is the entry point of the entire feature — without it nothing else is reachable. It must land first so every subsequent story has a surface to attach to.

**Independent Test**: Can be fully tested by running `envy` with no arguments against a TTY; the full-screen interface must render and quit cleanly with `Q`.

**Acceptance Scenarios**:

1. **Given** an interactive terminal (TTY) and an initialized vault, **When** the user runs bare `envy`, **Then** a full-screen TUI renders with the ENVY banner, vault status, and secret list; the terminal is restored to its previous state after quitting with `Q`.
2. **Given** the TUI running, **When** the user presses `B`, **Then** the banner toggles between the full gradient banner and a single-line compact header, giving the table more space.

---

### User Story 2 - Silent execution everywhere else (Priority: P1)

Every existing way of invoking `envy` — subcommands, piped output, `envy run -- cmd` — behaves exactly as before: no TUI, no ASCII banner, no ANSI escapes, byte-identical output. Bare `envy` with non-TTY stdout prints help to stderr and exits 0.

**Why this priority**: Regression risk is the top concern — the TUI must never corrupt scripted output, CI logs, or the `run` command's child process output. This story is the safety guarantee that everything else can build on.

**Independent Test**: Can be fully tested by running the entire existing CLI test-suite and E2E scenarios unchanged, plus a new integration test asserting bare piped `envy` produces help without ANSI escapes.

**Acceptance Scenarios**:

1. **Given** a non-interactive invocation (stdout piped or redirected), **When** the user runs bare `envy`, **Then** help text is printed to stderr, the exit code is 0, and stdout contains zero ANSI escape sequences.
2. **Given** any existing subcommand (e.g. `envy status`, `envy list`, `envy run -- cmd`), **When** it runs with stdout piped, **Then** its output is byte-identical to the pre-feature output (no banner, no color when `NO_COLOR`).
3. **Given** the `NO_COLOR` environment variable set, **When** the TUI renders, **Then** the banner and UI use no color codes and remain readable.

---

### User Story 3 - Navigate projects and environments (Priority: P1)

Inside the TUI, a developer picks a project from the left sidebar, then an environment under it (e.g. `development`, `staging`, `production`). The main panel updates to show that environment's secrets with masked values; the header shows the active environment and vault status (`[Locked]` / `[Unlocked]`).

**Why this priority**: This is the core navigation skeleton — sidebar, panel, and status bar — without which secrets cannot be viewed. It precedes editing because editing targets a specific environment.

**Independent Test**: Can be fully tested by opening the TUI against a vault with at least two projects and selecting each project/environment, verifying the table contents change accordingly.

**Acceptance Scenarios**:

1. **Given** a vault with multiple projects, **When** the user navigates the sidebar with arrow keys and selects a project, **Then** that project's environments are listed and its first environment's secrets appear in the main panel.
2. **Given** an environment selected, **When** the user moves the selection, **Then** the header's active-environment indicator reflects the selection.
3. **Given** an empty vault (no projects), **When** the TUI starts, **Then** it renders a clear empty state and a hint to run `envy init`, without crashing.

---

### User Story 4 - Find, reveal, and manage secrets (Priority: P1)

A developer finds a secret quickly with search-as-you-type, reveals a single value with `SPACE` (masked again on selection change), and edits, creates, or deletes secrets with `E`/`N`/`D` popups — all without leaving the TUI. Secret values are never echoed to the terminal in clear during input.

**Why this priority**: This is the day-to-day value of the tool — the reason a user would open the TUI instead of the plain CLI. It delivers the complete management loop in one place.

**Independent Test**: Can be fully tested by: searching for a substring that filters the table, revealing and hiding values, and performing create → edit → delete round-trip on a secret, then verifying the change through the plain CLI (`envy get`).

**Acceptance Scenarios**:

1. **Given** the secrets table, **When** the user types in the search box, **Then** the table filters to secret keys containing the query (case-insensitive, live) and a "no matches" state renders for zero results.
2. **Given** a selected secret row, **When** the user presses `SPACE`, **Then** the value toggles between `********` masking and clear text; the value re-masks automatically when the selection moves.
3. **Given** the `E` (edit) / `N` (new) / `D` (delete) popups, **When** the user completes the flow with confirmation, **Then** the vault is updated and the table refreshes; the change is visible via `envy get`/`envy list` outside the TUI.
4. **Given** secret input in an edit/new popup, **When** the user types, **Then** the value is entered masked by default and never rendered in clear.

---

### User Story 5 - Lock, unlock, and sync from the TUI (Priority: P2)

A developer locks the vault with `L` (vault closed, key wiped from TUI state, header shows `[Locked]`), unlocks with `U` (master key re-fetched from the OS keyring, vault reopened), and triggers a GitOps sync with `S`, which shows a working indicator in the status bar while it runs.

**Why this priority**: Lock/unlock and sync are important daily operations but lower risk than the core view/edit flows — they exercise existing, well-tested core code paths behind a thin TUI binding.

**Independent Test**: Can be fully tested by locking and unlocking (asserting the header state and that secrets re-appear after unlock) and running a sync that updates `envy.enc` on disk, verifiable with `envy status`.

**Acceptance Scenarios**:

1. **Given** an unlocked vault in the TUI, **When** the user presses `L`, **Then** the header shows `[Locked]`, secret values are no longer accessible, and the master key is wiped from memory.
2. **Given** a locked vault, **When** the user presses `U` and the keyring holds the master key, **Then** the vault reopens and the header shows `[Unlocked]`.
3. **Given** an unlocked vault with an existing `envy.enc` artifact, **When** the user presses `S`, **Then** a working indicator appears, the sync completes, and `envy status` outside the TUI reports the artifact in sync.

---

### User Story 6 - Security and robustness guarantees (Priority: P2)

A user can quit or crash the TUI without leaking secrets: all decrypted values live in zeroized memory and are wiped on exit, the vault connection is closed cleanly, and the terminal (alternate screen + raw mode) is always restored — including on panic or unexpected termination.

**Why this priority**: Secret-manager trust depends on this. It is deliberately implemented alongside the interactive stories, not after, so no version ships without the guarantee — but it is P2 because the functionality works without it being visible to the user.

**Independent Test**: Can be fully tested by: unit tests asserting zeroization of value buffers on drop, a vault-close assertion on TUI exit, and a terminal-restore test on normal quit and forced termination paths.

**Acceptance Scenarios**:

1. **Given** the TUI running with revealed secret values, **When** the user quits with `Q` or the process is terminated unexpectedly, **Then** all decrypted value buffers are zeroized and the vault is closed.
2. **Given** a forced termination mid-session, **When** the process ends, **Then** the terminal is restored to its pre-TUI state (shell visible, cursor restored) via a defensive restore path.

---

### User Story 7 - Documentation (Priority: P3)

A new user or contributor finds an "Interactive TUI" section in the README explaining what the TUI is, how to launch it, its hotkeys, and the silent-execution rules; the feature keeps a plan under `docs/features/016-interactive-tui/` following repo convention.

**Why this priority**: Documentation has no functional risk and only helps after the feature exists; lowest priority while remaining required before merge.

**Independent Test**: Can be fully tested by a reviewer following the README's TUI section to launch the TUI and use every documented hotkey successfully.

**Acceptance Scenarios**:

1. **Given** the README, **When** a reader opens the Interactive TUI section, **Then** it documents: launch conditions (bare `envy` + TTY), the full hotkey table, and the silent-execution rules.
2. **Given** the repo, **When** a maintainer looks for the feature plan, **Then** `docs/features/016-interactive-tui/` exists with plan content following the repo convention.

---

### Edge Cases

- **Empty vault**: no projects exist — TUI shows an empty state with a hint to run `envy init`; no crash, no blank render.
- **Empty environment**: selected environment has no secrets — table shows an empty state with a hint to press `N`.
- **Keyring unavailable at unlock**: re-fetch of the master key fails — TUI shows an error in the status bar and stays locked, without crashing.
- **Tiny terminal**: width/height too small for the banner or table — layout degrades gracefully (compact header, truncation) instead of panicking.
- **No search matches**: filter yields zero rows — a "no matches" state renders; clearing the search restores the full list.
- **Delete last secret**: deleting the only secret in an environment — confirmation popup, table empties gracefully, vault stays consistent.
- **Lock while a popup is open**: the lock action is ignored while a popup is active — state machine stays consistent, no deadlock.
- **Non-ASCII values**: secret values with unicode/newlines — masked rendering and editing tolerate them without corruption.
- **Exit during a sync operation**: user quits while the working indicator is shown — the operation completes or is aborted cleanly, vault is still closed properly.
- **Panic mid-render**: unexpected internal error — the defensive restore path puts the terminal back in a usable state and the zeroized buffers are dropped.
- **Nested project launch**: running bare `envy` below a project root — TUI sync reads and writes artifact beside discovered `envy.toml`.
- **Artifact write failure**: sync write/rename fails — status reports failure and sync marker remains unchanged.
- **Second instance**: running `envy` bare while another instance holds the vault — error surfaces in the status bar, TUI remains usable.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Running `envy` with no subcommand and an interactive stdout MUST launch the full-screen TUI.
- **FR-002**: Running `envy` with no subcommand and a non-interactive stdout MUST print help to stderr and exit 0, with no TUI and no ANSI escape sequences emitted.
- **FR-003**: All existing subcommands MUST behave byte-identically to the pre-feature version — no banner, no TUI, no output changes.
- **FR-004**: The TUI header MUST display a block-style Unicode ASCII "ENVY" banner (characters such as `█`, `╔`, `╚`) with a vertical gradient using bluish-purple/violet/indigo shades (bright periwinkle `#8A2BE2` / `#7B68EE` / `#9370DB` at the top down to deep indigo `#1A0933` / `#0D0221` at the base), rendered in TrueColor with explicit ANSI-256 fallback (detected via terminal capabilities; ratatui does not map RGB to 256-color automatically).
- **FR-005**: The banner MUST be compactable: a toggle (`B`) switches between the full gradient banner and a single-line compact header to free table space.
- **FR-006**: The header MUST show the vault lock state (`[Locked]` / `[Unlocked]`) and the active environment indicator.
- **FR-007**: The TUI MUST render a left sidebar listing projects, with the selected project's environments listed beneath it, navigable via keyboard.
- **FR-008**: The main panel MUST render the selected environment's secrets as a table: key, masked value (`********`), and metadata such as last-updated time.
- **FR-009**: Secret values MUST be masked by default in the table.
- **FR-010**: The TUI MUST provide a live search box filtering secret keys by case-insensitive substring; a "no matches" state renders when the filter yields zero rows.
- **FR-011**: `SPACE` MUST toggle clear-text visibility for the selected row only, and the value MUST re-mask when the selection changes.
- **FR-012**: The TUI MUST support creating (`N`), editing (`E`), and deleting (`D`, with confirmation) secrets from popups with a text input; secret values entered in popups MUST be masked by default, with an explicit reveal toggle (e.g. `Ctrl+R`) while typing.
- **FR-013**: `L` MUST lock the vault: close the vault connection, wipe the master key and all decrypted secret values from TUI state, and clear the secrets table (cached project/environment structure is retained — it is not secret). `U` MUST unlock it by re-fetching the master key, reopening the vault, and reloading the secrets table.
- **FR-014**: `S` MUST trigger a GitOps sync (seal/unseal of the `envy.enc` artifact) showing a working indicator in the status bar while it runs. Passphrases per environment MUST be resolved in this order: (a) `ENVY_PASSPHRASE_<ENV>` / `ENVY_PASSPHRASE` environment variables when set (headless mode, no UI); (b) otherwise a masked passphrase input popup inside the TUI for each environment being sealed.
- **FR-015**: The bottom status bar MUST show the hotkey hints: `[Q] Quit [B] Banner [F] Find [SPACE] Unmask [E] Edit [N] New [D] Delete [S] Sync [L] Lock [U] Unlock`.
- **FR-016**: All decrypted secret values in TUI state MUST be held in zeroized buffers that are wiped on exit or unexpected termination.
- **FR-017**: The terminal (alternate screen + raw mode) MUST be restored on every exit path, including panic and unexpected termination.
- **FR-018**: The vault MUST be closed cleanly when the TUI exits.
- **FR-019**: `NO_COLOR` MUST be honored: with it set, the TUI renders without color codes.
- **FR-020**: The feature MUST include unit tests for banner gradient interpolation, search filtering, mask/unmask state, and lock/unlock transitions, and an integration test asserting bare `envy` with piped stdout prints help with no ANSI escapes.
- **FR-021**: The README MUST gain an "Interactive TUI" section (launch conditions, hotkey table, silent-execution rules) and the repo MUST include the feature plan under `docs/features/016-interactive-tui/`.
- **FR-022**: TUI sync MUST resolve `envy.enc` from the discovered manifest directory, not process cwd, so nested-project and subdirectory launches target the same artifact as plain CLI commands.
- **FR-023**: TUI sync MUST update sync markers only after artifact write succeeds; failed writes MUST NOT report the environment as in sync.
- **FR-024**: The active search query MUST be visible while search is active, and remain visible when filtered results are shown.
- **FR-025**: The secrets table MUST display each secret's last-updated timestamp. Revealed values MUST be borrowed directly from `Zeroizing<String>` state for rendering, with no additional plaintext copy in TUI state.
- **FR-026**: The TUI MUST provide a searchable project picker, opened with `P`, so large project lists do not overwhelm the main sidebar.
- **FR-027**: `X` MUST open a destructive project-delete confirmation showing project name, environment count, and secret count; deletion MUST require typing the exact project name.
- **FR-028**: Confirmed project deletion MUST cascade through environments and secrets using the core/database deletion path, then select a valid remaining project and refresh the UI.
- **FR-029**: `?` MUST open a help popup documenting navigation, project selection/deletion, secret CRUD, sync, lock/unlock, and quit controls.
- **FR-030**: The sidebar MUST show every registered project as a navigable tree; `↑`/`↓` moves through projects and expanded environments, `Enter`/`→` expands or selects, and `←` collapses.
- **FR-031**: Each visible environment MUST show sync state (`InSync`, modified/needs refresh, or never sealed) and the active environment MUST be visually distinct.
- **FR-032**: `T` MUST show a status report for the active project, including secret counts, seal state, and stale-secret/refresh information.
- **FR-033**: `G` MUST show a key-level diff for the active environment between vault and `envy.enc`, without revealing secret values by default.
- **FR-034**: `Y` MUST unseal/import the active environment from `envy.enc` into the vault using the same passphrase resolution and masked popup rules as sync.
- **FR-035**: Secret timestamps MUST be human-readable in the table; raw Unix timestamps MUST NOT be shown to users.
- **FR-036**: The project tree and project picker MUST use stateful scrolling so the highlighted entry remains visible in arbitrarily long lists.
- **FR-037**: Selecting a project MUST preserve the tree cursor on that project; loading its environments MUST NOT reset navigation to the first row.
- **FR-038**: The footer MUST remain short and readable on narrow terminals, pointing users to `?` for complete help instead of listing every command.
- **FR-039**: The help popup MUST use readable grouped sections with aligned controls and descriptions; project-picker help MUST explain search, selection, and closing.

### Key Entities *(include if feature involves data)*

- **Project**: A registered project in the vault (from `envy init`); the top level of the sidebar tree.
- **Environment**: A named environment under a project (e.g. `development`, `staging`, `production`); the second level of the sidebar and the unit the secrets table shows.
- **Secret entry**: A key/value pair in an environment; rendered masked or clear in the table, editable via popups, held in zeroized buffers.
- **Vault state**: `[Locked]` / `[Unlocked]` — whether the vault connection is open and the master key is held in TUI state.
- **Search query**: The live filter text applied to secret keys.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of existing subcommand outputs are byte-identical after the feature (regression suite + E2E scenarios green, zero output diffs).
- **SC-002**: Bare `envy` with piped stdout prints help to stderr with exit code 0 and zero ANSI escape sequences on stdout (integration-tested).
- **SC-003**: The TUI launches to a usable state in under 1 second on a standard terminal after vault unlock.
- **SC-004**: Every documented hotkey performs its documented action — verified by automated state-level tests (filter, mask/unmask, lock/unlock, CRUD) plus a reviewer following the README.
- **SC-005**: Zero plaintext secret values appear in process listings, logs, crash output, or terminal scrollback during any TUI session (masked-by-default + zeroized buffers, verified by tests).
- **SC-006**: The terminal is restored to its pre-TUI state on normal quit and on forced/panic termination (restore path covered by tests).
- **SC-007**: `cargo test` and `cargo clippy -- -D warnings` pass in CI without regressions.

## Assumptions & Confirmed Decisions

- **Master key always available at unlock via OS keyring**: `U` re-fetches from the keyring (same source as `envy init`); a keyring failure surfaces as a status-bar error and the TUI stays locked.
- **ratatui 0.30 + crossterm 0.29**: crossterm 0.29 is already in the dependency tree (via dialoguer); ratatui 0.30 is selected so no second crossterm version is introduced.
- **Synchronous operations**: TUI operations run inline in the event loop with a working indicator; core operations are local SQLite (milliseconds) and the Argon2id seal is ~100–500 ms — acceptable freeze. A background thread is a later feature, not part of this spec.
- **Edit/New/Delete are in v1**: the full management loop ships with the initial TUI, not a read-only browser.
- **Banner is compactable**: `B` toggles full gradient banner ↔ single-line compact header.
- **Trigger semantics**: `envy` with no subcommand + interactive stdout → TUI; no subcommand + non-interactive stdout → help to stderr, exit 0. All subcommands (incl. `run`) are untouched.
- **Fixed palette**: the bluish-purple gradient is the only theme; no user theming.
- **Keybinding `U` for unlock** (distinct from `L` for lock).
- **VHS demo tape for the TUI is out of scope** (stretch, not in this spec).

## Non-Goals

- **No async runtime**: no background threads, no tokio/async executor; operations are synchronous.
- **No multi-project tree editing**: sidebar supports selection only; creating/renaming projects and environments stays with the plain CLI (`init`, `migrate`).
- **No TUI for `run`**: `envy run -- <cmd>` remains fully headless and silent.
- **No theming customization**: the fixed bluish-purple palette is the only theme.
- **No remote vault / no network features**.
- **No mouse support**: keyboard-only interaction.
- **No batch operations**: no multi-select delete/edit, no bulk import/export from the TUI.
- **No keybinding remapping**.
