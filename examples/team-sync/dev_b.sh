#!/usr/bin/env bash
# =============================================================================
# team-sync / dev_b — the new teammate's side
#
#   (receive envy.enc) → decrypt -e development (headless) → verify → run
#
# ARTIFACT_IN must point at the envy.enc produced by dev_a.sh (the analogue
# of `git pull` on a repo that committed the artifact).
# =============================================================================
set -euo pipefail

ENVY="${ENVY_BIN:-envy}"
export ENVY_PASSPHRASE="${ENVY_PASSPHRASE:-ephemeral-teamsync-key}"
export ENVY_PASSPHRASE_DEVELOPMENT="${ENVY_PASSPHRASE_DEVELOPMENT:-teamsync-dummy-pass}"

ARTIFACT_IN="${ARTIFACT_IN:-./envy.enc}"
[ -f "$ARTIFACT_IN" ] || { echo "error: artifact not found at '$ARTIFACT_IN' (run dev_a.sh first)"; exit 1; }

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cd "$WORKDIR"

echo "== dev_b: receive artifact"
cp "$ARTIFACT_IN" envy.enc

echo "== dev_b: init (fresh machine)"
"$ENVY" init

echo "== dev_b: decrypt (headless)"
"$ENVY" decrypt < /dev/null

echo "== dev_b: verify round-trip"
"$ENVY" get TEAM_TOKEN

echo "== dev_b: run with injected secret"
"$ENVY" run -- sh -c 'echo "injected: ${TEAM_TOKEN}"'

echo "team-sync: OK"
