## Description

<!-- What does this PR do? Why is it needed? Link any related issues with "Closes #123". -->

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor (no behaviour change)
- [ ] Documentation update
- [ ] CI / tooling

## Checklist

Before requesting a review, confirm each item below.

- [ ] `cargo fmt --check` passes — code is formatted
- [ ] `cargo clippy -- -D warnings` passes — zero warnings
- [ ] `cargo test` passes — all non-ignored tests green
- [ ] `bash tests/e2e_devops_scenarios.sh` passes — full CLI scenarios verified (required for changes to `cli/`, `core/sync.rs`, or `crypto/artifact.rs`)
- [ ] `cargo audit` run — no new advisories introduced
- [ ] Layer architecture respected — CLI calls Core, not DB directly (see [developer-guide.md](../docs/developer-guide.md#4-the-4-layer-architecture))
- [ ] New code has tests (unit or integration)

## Documentation

- [ ] `README.md` updated (if user-facing behaviour changed)
- [ ] `docs/developer-guide.md` updated (if architecture, patterns, or commands changed)
- [ ] No documentation changes needed

## Security considerations

<!-- Does this PR touch cryptographic code, key handling, passphrase input, or secret storage? If so, briefly describe the security implications and how they were addressed. Write "N/A" if not applicable. -->