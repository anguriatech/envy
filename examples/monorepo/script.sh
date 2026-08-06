#!/usr/bin/env bash
# =============================================================================
# monorepo — nested projects with independent secrets (feature 014)
#
# Recreates in a fresh temp dir:
#   repo/envy.toml          (team-wide project)
#   repo/apps/app-a/        (own project, own UUID)
#   repo/apps/app-b/        (own project, own UUID)
#
# Verifies each nested project resolves ONLY its own secrets.
# =============================================================================
set -euo pipefail

ENVY="${ENVY_BIN:-envy}"
export ENVY_PASSPHRASE="${ENVY_PASSPHRASE:-ephemeral-monorepo-key}"
export ENVY_PASSPHRASE_DEVELOPMENT="${ENVY_PASSPHRASE_DEVELOPMENT:-monorepo-dummy-pass}"

ROOT="$(mktemp -d)"
trap 'rm -rf "$ROOT"' EXIT

echo "== repo root"
mkdir -p "$ROOT/apps/app-a" "$ROOT/apps/app-b"
(cd "$ROOT" && "$ENVY" init)
(cd "$ROOT" && "$ENVY" set TEAM_FLAG=root_shared_dummy)

echo "== app-a (nested)"
(cd "$ROOT/apps/app-a" && "$ENVY" init)
(cd "$ROOT/apps/app-a" && "$ENVY" set APP_A_TOKEN=dummy_a_only)

echo "== app-b (nested)"
(cd "$ROOT/apps/app-b" && "$ENVY" init)
(cd "$ROOT/apps/app-b" && "$ENVY" set APP_B_TOKEN=dummy_b_only)

echo "== verify independence (closest envy.toml wins)"
LIST_A="$(cd "$ROOT/apps/app-a" && "$ENVY" list)"
LIST_B="$(cd "$ROOT/apps/app-b" && "$ENVY" list)"
LIST_ROOT="$(cd "$ROOT" && "$ENVY" list)"
echo "app-a: $LIST_A"
echo "app-b: $LIST_B"
echo "root : $LIST_ROOT"

echo "$LIST_A" | grep -q '^APP_A_TOKEN$' || { echo "error: app-a missing its token"; exit 1; }
echo "$LIST_A" | grep -q '^APP_B_TOKEN$' && { echo "error: app-a sees app-b's secret"; exit 1; }
echo "$LIST_B" | grep -q '^APP_B_TOKEN$' || { echo "error: app-b missing its token"; exit 1; }
echo "$LIST_B" | grep -q '^APP_A_TOKEN$' && { echo "error: app-b sees app-a's secret"; exit 1; }
echo "$LIST_ROOT" | grep -q '^TEAM_FLAG$' || { echo "error: root missing team secret"; exit 1; }

echo "monorepo: OK"
