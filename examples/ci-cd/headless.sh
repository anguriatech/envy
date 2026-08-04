#!/usr/bin/env bash
# =============================================================================
# ci-cd / headless — the exact loop a pipeline runs
#
#   init → set → encrypt (headless) → status JSON gate → diff gate → run
#
# Headless contract (see examples/README.md):
#   - ENVY_PASSPHRASE: ephemeral keyring fallback
#   - ENVY_PASSPHRASE_DEVELOPMENT: envelope passphrase for encrypt/decrypt
#   - stdin from /dev/null so no prompt can ever block a pipeline
# =============================================================================
set -euo pipefail

ENVY="${ENVY_BIN:-envy}"
export ENVY_PASSPHRASE="${ENVY_PASSPHRASE:-ephemeral-cicd-key}"
export ENVY_PASSPHRASE_DEVELOPMENT="${ENVY_PASSPHRASE_DEVELOPMENT:-cicd-dummy-pass}"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cd "$WORKDIR"

echo "== init"
"$ENVY" init

echo "== set"
"$ENVY" set PIPELINE_TOKEN=dummy_pipeline_token
"$ENVY" set PIPELINE_REGION=eu-dummy-1

echo "== encrypt (headless)"
"$ENVY" encrypt -e development < /dev/null

echo "== status --format json (in_sync gate)"
STATUS="$("$ENVY" status --format json)"
echo "$STATUS"
echo "$STATUS" | jq -e '.environments[] | select(.name == "development" and .status == "in_sync")' > /dev/null

echo "== diff gate (0 = in sync)"
"$ENVY" diff -e development

echo "== run"
"$ENVY" run -e development -- sh -c 'echo "pipeline token: ${PIPELINE_TOKEN}"'

echo "ci-cd: OK"
