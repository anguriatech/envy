#!/usr/bin/env bash
# =============================================================================
# team-sync / dev_a — the sealer's side
#
#   init → set → encrypt -e development (headless)
#
# Writes envy.enc (safe to commit — pure ciphertext) into the directory
# given by ARTIFACT_OUT when set, otherwise leaves it in the temp project.
# =============================================================================
set -euo pipefail

ENVY="${ENVY_BIN:-envy}"
export ENVY_PASSPHRASE="${ENVY_PASSPHRASE:-ephemeral-teamsync-key}"
export ENVY_PASSPHRASE_DEVELOPMENT="${ENVY_PASSPHRASE_DEVELOPMENT:-teamsync-dummy-pass}"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cd "$WORKDIR"

echo "== dev_a: init"
"$ENVY" init

echo "== dev_a: set"
"$ENVY" set TEAM_TOKEN=dummy_team_token
"$ENVY" set TEAM_DSN=dummy://team.example.invalid/db

echo "== dev_a: encrypt (headless)"
"$ENVY" encrypt -e development < /dev/null

[ -f envy.enc ] || { echo "error: envy.enc not produced"; exit 1; }
if [[ -n "${ARTIFACT_OUT:-}" ]]; then
  mkdir -p "$ARTIFACT_OUT"
  cp envy.enc "$ARTIFACT_OUT/"
fi
echo "dev_a: OK"
