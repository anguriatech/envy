# Interactive TUI Plan

Implementation plan: `specs/016-interactive-tui/plan.md`.

The TUI uses ratatui 0.29 and crossterm 0.28 to preserve Envy's Rust 1.85 MSRV. Bare
interactive `envy` launches the terminal interface; piped bare invocations print help and
existing subcommands remain unchanged. Secrets are masked by default and stored in
zeroizing buffers while displayed values are managed through the existing core layer. Sync
resolves artifacts beside the discovered manifest, commits markers after atomic writes, and
renders visible search state plus last-updated metadata.
Project/environment navigation uses an explicit tree model; `T`, `G`, and `Y` expose status,
diff, and artifact import operations without revealing values by default.
