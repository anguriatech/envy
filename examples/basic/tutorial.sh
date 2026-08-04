#!/usr/bin/env bash
# =============================================================================
# basic — local quickstart: init → set → list → run
#
# Headless contract (see examples/README.md):
#   - ENVY_PASSPHRASE: ephemeral keyring fallback (vault works without a daemon)
#   - ENVY_PASSPHRASE_DEVELOPMENT: envelope passphrase for encrypt/decrypt
#   - Only dummy values. Safe to run twice (own temp dir per run).
# =============================================================================
set -euo pipefail

ENVY="${ENVY_BIN:-envy}"
export ENVY_PASSPHRASE="${ENVY_PASSPHRASE:-ephemeral-basic-key}"
export ENVY_PASSPHRASE_DEVELOPMENT="${ENVY_PASSPHRASE_DEVELOPMENT:-basic-dummy-pass}"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cd "$WORKDIR"

echo "== init"
"$ENVY" init

echo "== set"
"$ENVY" set DEMO_TOKEN=dummy_token_123
"$ENVY" set DEMO_URL=https://example.invalid/dummy

echo "== list"
"$ENVY" list

echo "== run"
"$ENVY" run -- sh -c 'echo "injected: ${DEMO_TOKEN}"'

[ -f envy.toml ] || { echo "error: envy.toml missing"; exit 1; }
echo "basic: OK"
