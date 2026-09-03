# PRODUCT.md — envy

## Product

envy is a terminal-first secrets manager. Its unique mechanism in one sentence: a
SQLCipher-encrypted local vault combined with a sealed GitOps artifact (`envy.enc`) makes
**the commit the distribution** — secrets never leave the machine in plaintext, yet every
teammate gets them by pulling the repo and running one command.

## Audience

A developer mid-task, hands on the keyboard, living in vim/terminal muscle memory. They
open the TUI when they need a credential NOW (to paste into a config, a deploy, a debug
session) or when they are about to commit secret changes to share with the team. They
already know lazygit, btop, k9s, and vim; they expect panel focus, contextual key legends,
and a command mode.

## Visitor mode

Operate. The visitor completes tasks: find → reveal/copy → change → seal → verify.
Scanability, legible state, and native terminal affordances outrank expression. Brand
lives in precise details, not decoration.

## What the TUI must prove

The TUI is not a viewer bolted onto the CLI; it is the fastest path through the secret
lifecycle. If the CLI does something interactively better than the TUI, the TUI fails.
Success: a user never needs to run `envy <subcommand>` interactively.

## Brand commitments (non-negotiable)

- The bluish-purple gradient ENVY identity (spec FR-004) stays. Refinement, not replacement.
- Keyboard-only; no mouse support; no key remapping.
- Security: zeroized secret buffers, masked by default, passphrases never logged.
- Quality gates: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` green.
- CLI subcommand output stays byte-identical (FR-003).

## Anti-goals

No mouse, no theming customization, no async runtime, no remote vault/network features,
no batch operations, no keybinding remapping, no TUI for `envy run`.

## Language

All code, comments, docs, and UI copy in English.
