#!/usr/bin/env bash
# =============================================================================
# e2e_devops_scenarios.sh — Multi-user DevOps integration tests for Envy
#
# Simulates real-world scenarios using isolated temp directories:
#   1. Standard Team Sync (Dev A → Dev B)
#   2. Progressive Disclosure (dev-pass / prod-pass, junior reads dev only)
#   3. CI/CD Headless Pipeline (stdin from /dev/null)
#   4. Malicious Actor — AES-GCM tampering detection
#   5. Machine-Readable Output Formats (--format flag)
#   6. Multi-Env Headless Encryption with Smart Merge (FR-001, FR-005, SC-002)
#   8. Sync Status Command (FR-001–FR-008, US1–US4)
#   9. Pre-Encrypt Secret Diff (011-envy-diff)
#
# Requirements:
#   - `envy` binary built or passed via ENVY_BIN env var
#   - `jq` available (for scenario 2 JSON merge and scenario 4 tampering)
#
# Usage:
#   chmod +x tests/e2e_devops_scenarios.sh
#   ENVY_BIN=./target/release/envy ./tests/e2e_devops_scenarios.sh
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

ENVY="${ENVY_BIN:-./target/release/envy}"
PASS=0
FAIL=0
TOTAL=0

# Colours (disable if not a terminal)
if [[ -t 1 ]]; then
  GREEN='\033[0;32m'
  RED='\033[0;31m'
  YELLOW='\033[1;33m'
  CYAN='\033[0;36m'
  BOLD='\033[1m'
  DIM='\033[2m'
  RESET='\033[0m'
else
  GREEN='' RED='' YELLOW='' CYAN='' BOLD='' DIM='' RESET=''
fi

# ---------------------------------------------------------------------------
# Test harness utilities
# ---------------------------------------------------------------------------

section() {
  echo ""
  echo -e "${CYAN}${BOLD}═══════════════════════════════════════════════════════════════${RESET}"
  echo -e "${CYAN}${BOLD}  $1${RESET}"
  echo -e "${CYAN}${BOLD}═══════════════════════════════════════════════════════════════${RESET}"
  echo ""
}

assert_eq() {
  TOTAL=$((TOTAL + 1))
  local label="$1" expected="$2" actual="$3"
  if [[ "$expected" == "$actual" ]]; then
    echo -e "  ${GREEN}✓${RESET} ${label}"
    PASS=$((PASS + 1))
  else
    echo -e "  ${RED}✗${RESET} ${label}"
    echo -e "    ${DIM}expected: ${expected}${RESET}"
    echo -e "    ${DIM}actual:   ${actual}${RESET}"
    FAIL=$((FAIL + 1))
  fi
}

assert_contains() {
  TOTAL=$((TOTAL + 1))
  local label="$1" substring="$2" haystack="$3"
  if [[ "$haystack" == *"$substring"* ]]; then
    echo -e "  ${GREEN}✓${RESET} ${label}"
    PASS=$((PASS + 1))
  else
    echo -e "  ${RED}✗${RESET} ${label}"
    echo -e "    ${DIM}expected to contain: ${substring}${RESET}"
    echo -e "    ${DIM}got: ${haystack:0:200}${RESET}"
    FAIL=$((FAIL + 1))
  fi
}

assert_not_contains() {
  TOTAL=$((TOTAL + 1))
  local label="$1" substring="$2" haystack="$3"
  if [[ "$haystack" != *"$substring"* ]]; then
    echo -e "  ${GREEN}✓${RESET} ${label}"
    PASS=$((PASS + 1))
  else
    echo -e "  ${RED}✗${RESET} ${label}"
    echo -e "    ${DIM}must NOT contain: ${substring}${RESET}"
    FAIL=$((FAIL + 1))
  fi
}

assert_file_exists() {
  TOTAL=$((TOTAL + 1))
  local label="$1" path="$2"
  if [[ -f "$path" ]]; then
    echo -e "  ${GREEN}✓${RESET} ${label}"
    PASS=$((PASS + 1))
  else
    echo -e "  ${RED}✗${RESET} ${label}"
    echo -e "    ${DIM}file does not exist: ${path}${RESET}"
    FAIL=$((FAIL + 1))
  fi
}

# ---------------------------------------------------------------------------
# Helper: create an isolated envy project.
#
# Initialises a fresh envy project in DIR. envy.enc is always written
# alongside envy.toml in the same directory (the project root).
#
# Usage: init_project <dir>
#   Sets PROJECT_DIR and ARTIFACT_PATH for the caller.
# ---------------------------------------------------------------------------
init_project() {
  local dir="$1"
  PROJECT_DIR="$dir"
  ARTIFACT_PATH="$dir/envy.enc"
  mkdir -p "$PROJECT_DIR"
  (cd "$PROJECT_DIR" && "$ENVY" init)
}

# ---------------------------------------------------------------------------
# Pre-flight checks
# ---------------------------------------------------------------------------

echo -e "${BOLD}Envy DevOps Scenario Tests${RESET}"
echo -e "${DIM}$(date -Iseconds)${RESET}"
echo ""

if [[ ! -x "$ENVY" ]]; then
  echo -e "${RED}error: envy binary not found at '${ENVY}'${RESET}"
  echo "Build it first:  cargo build --release"
  echo "Or specify it:   ENVY_BIN=/path/to/envy $0"
  exit 1
fi
echo -e "  ${GREEN}✓${RESET} envy binary: ${DIM}${ENVY}${RESET}"

if ! command -v jq &>/dev/null; then
  echo -e "${RED}error: jq is required for scenario 2 & 4${RESET}"
  exit 1
fi
echo -e "  ${GREEN}✓${RESET} jq:          ${DIM}$(which jq)${RESET}"

ENVY="$(realpath "$ENVY")"

# macOS does not ship GNU coreutils `timeout`; provide a portable fallback.
if ! command -v timeout &>/dev/null; then
  timeout() {
    local secs="$1"; shift
    "$@" &
    local pid=$!
    ( sleep "$secs" && kill -TERM "$pid" 2>/dev/null ) &
    local watcher=$!
    wait "$pid" 2>/dev/null
    local rc=$?
    kill -TERM "$watcher" 2>/dev/null
    wait "$watcher" 2>/dev/null
    return $rc
  }
fi

WORKSPACE="$(mktemp -d)"
trap 'rm -rf "$WORKSPACE"' EXIT
echo -e "  ${GREEN}✓${RESET} workspace:   ${DIM}${WORKSPACE}${RESET}"

# =============================================================================
# SCENARIO 1 — Standard Team Sync (Dev A → Dev B)
# =============================================================================

section "Scenario 1 — Standard Team Sync (Dev A → Dev B)"

# ---------- Dev A ----------
echo -e "${YELLOW}  [Dev A] Initialising project and setting secrets...${RESET}"
init_project "$WORKSPACE/s1-deva"
DEV_A_DIR="$PROJECT_DIR"
DEV_A_ENC="$ARTIFACT_PATH"

(cd "$DEV_A_DIR" && "$ENVY" set "API_KEY=sk_test_abc123")
(cd "$DEV_A_DIR" && "$ENVY" set "DATABASE_URL=postgres://dev:pass@localhost/myapp")
(cd "$DEV_A_DIR" && "$ENVY" set "JWT_SECRET=super-secret-jwt-token-value")

echo -e "${YELLOW}  [Dev A] Encrypting vault → envy.enc...${RESET}"
(cd "$DEV_A_DIR" && ENVY_PASSPHRASE="team-shared-passphrase" "$ENVY" encrypt)

assert_file_exists "envy.enc created by Dev A" "$DEV_A_ENC"

ENC_CONTENT="$(cat "$DEV_A_ENC")"
assert_not_contains "API_KEY value not in plaintext" "sk_test_abc123" "$ENC_CONTENT"
assert_not_contains "DATABASE_URL value not in plaintext" "postgres://dev" "$ENC_CONTENT"

# ---------- Dev B ----------
echo -e "${YELLOW}  [Dev B] Simulating 'git pull' — new project + copied artifact...${RESET}"
init_project "$WORKSPACE/s1-devb"
DEV_B_DIR="$PROJECT_DIR"
DEV_B_ENC="$ARTIFACT_PATH"
cp "$DEV_A_ENC" "$DEV_B_ENC"

echo -e "${YELLOW}  [Dev B] Decrypting envy.enc into local vault...${RESET}"
DEC_EXIT=0
(cd "$DEV_B_DIR" && ENVY_PASSPHRASE="team-shared-passphrase" "$ENVY" decrypt) || DEC_EXIT=$?
assert_eq "decrypt exits 0" "0" "$DEC_EXIT"

echo -e "${YELLOW}  [Dev B] Verifying secrets match Dev A's originals...${RESET}"
VAL_API="$(cd "$DEV_B_DIR" && "$ENVY" get API_KEY)"
VAL_DB="$(cd "$DEV_B_DIR" && "$ENVY" get DATABASE_URL)"
VAL_JWT="$(cd "$DEV_B_DIR" && "$ENVY" get JWT_SECRET)"

assert_eq "API_KEY matches" "sk_test_abc123" "$VAL_API"
assert_eq "DATABASE_URL matches" "postgres://dev:pass@localhost/myapp" "$VAL_DB"
assert_eq "JWT_SECRET matches" "super-secret-jwt-token-value" "$VAL_JWT"

# =============================================================================
# SCENARIO 2 — Progressive Disclosure (Enterprise)
# =============================================================================

section "Scenario 2 — Progressive Disclosure (Enterprise)"

# ---------- Lead sets up the multi-env vault ----------
echo -e "${YELLOW}  [Lead] Setting up dev + prod secrets...${RESET}"
init_project "$WORKSPACE/s2-lead"
LEAD_DIR="$PROJECT_DIR"
LEAD_ENC="$ARTIFACT_PATH"

(cd "$LEAD_DIR" && "$ENVY" set "API_KEY=dev-api-key" -e development)
(cd "$LEAD_DIR" && "$ENVY" set "DB_HOST=dev.db.internal" -e development)
(cd "$LEAD_DIR" && "$ENVY" set "API_KEY=prod-api-key-CONFIDENTIAL" -e production)
(cd "$LEAD_DIR" && "$ENVY" set "DB_HOST=prod.db.internal" -e production)
(cd "$LEAD_DIR" && "$ENVY" set "MASTER_KEY=root-master-key" -e production)

echo -e "${YELLOW}  [Lead] Encrypting development with 'dev-pass'...${RESET}"
(cd "$LEAD_DIR" && ENVY_PASSPHRASE="dev-pass" "$ENVY" enc -e development)
DEV_ARTIFACT="$(cat "$LEAD_ENC")"

echo -e "${YELLOW}  [Lead] Encrypting production with 'prod-pass'...${RESET}"
(cd "$LEAD_DIR" && ENVY_PASSPHRASE="prod-pass" "$ENVY" enc -e production)
PROD_ARTIFACT="$(cat "$LEAD_ENC")"

echo -e "${YELLOW}  [Lead] Merging both artifacts (jq) → multi-key envy.enc...${RESET}"
# Each enc call sealed one env. Merge them into a single artifact.
# Use temp files instead of process substitution — Git Bash on Windows does
# not support /proc-based <(...) syntax.
_DEV_TMP=$(mktemp)
_PROD_TMP=$(mktemp)
echo "$DEV_ARTIFACT" > "$_DEV_TMP"
echo "$PROD_ARTIFACT" > "$_PROD_TMP"
jq -s '
  {
    version: .[0].version,
    environments: (.[0].environments * .[1].environments)
  }
' "$_DEV_TMP" "$_PROD_TMP" > "$LEAD_ENC"
rm -f "$_DEV_TMP" "$_PROD_TMP"

MERGED="$(cat "$LEAD_ENC")"
assert_contains "Merged artifact has 'development'" '"development"' "$MERGED"
assert_contains "Merged artifact has 'production'" '"production"' "$MERGED"

# ---------- Junior pulls with dev-pass only ----------
echo -e "${YELLOW}  [Junior] Pulling artifact + decrypt with dev-pass...${RESET}"
init_project "$WORKSPACE/s2-junior"
JUNIOR_DIR="$PROJECT_DIR"
JUNIOR_ENC="$ARTIFACT_PATH"
cp "$LEAD_ENC" "$JUNIOR_ENC"

JUNIOR_OUTPUT=""
JUNIOR_EXIT=0
JUNIOR_OUTPUT="$(cd "$JUNIOR_DIR" && ENVY_PASSPHRASE="dev-pass" "$ENVY" decrypt 2>&1)" || JUNIOR_EXIT=$?

assert_eq "Junior decrypt exits 0 (partial access)" "0" "$JUNIOR_EXIT"
assert_contains "Output shows development imported" "development" "$JUNIOR_OUTPUT"
assert_contains "Output shows production skipped" "skipped" "$JUNIOR_OUTPUT"

echo -e "${YELLOW}  [Junior] Verifying dev secrets loaded...${RESET}"
JUNIOR_API="$(cd "$JUNIOR_DIR" && "$ENVY" get API_KEY -e development)"
assert_eq "Junior sees dev API_KEY" "dev-api-key" "$JUNIOR_API"

JUNIOR_DB="$(cd "$JUNIOR_DIR" && "$ENVY" get DB_HOST -e development)"
assert_eq "Junior sees dev DB_HOST" "dev.db.internal" "$JUNIOR_DB"

echo -e "${YELLOW}  [Junior] Verifying prod secrets NOT accessible...${RESET}"
PROD_EXIT=0
(cd "$JUNIOR_DIR" && "$ENVY" get API_KEY -e production 2>/dev/null) || PROD_EXIT=$?
assert_eq "Junior cannot access prod API_KEY (exit ≠ 0)" "1" "$PROD_EXIT"

# =============================================================================
# SCENARIO 3 — CI/CD Headless Pipeline
# =============================================================================

section "Scenario 3 — CI/CD Headless Pipeline"

echo -e "${YELLOW}  [CI] Setting up project and seeding secrets...${RESET}"
init_project "$WORKSPACE/s3-ci"
CI_DIR="$PROJECT_DIR"
CI_ENC="$ARTIFACT_PATH"

(cd "$CI_DIR" && "$ENVY" set "DEPLOY_TOKEN=ghp_abc123tokenvalue")
(cd "$CI_DIR" && "$ENVY" set "AWS_SECRET=wJalrXUt123secretkey")

echo -e "${YELLOW}  [CI] Encrypting with ENVY_PASSPHRASE (headless, stdin=/dev/null)...${RESET}"
ENC_CI_EXIT=0
(cd "$CI_DIR" && ENVY_PASSPHRASE="ci-pipeline-secret" \
  timeout 30 "$ENVY" encrypt < /dev/null 2>&1) || ENC_CI_EXIT=$?
assert_eq "Headless encrypt exits 0" "0" "$ENC_CI_EXIT"
assert_file_exists "envy.enc created headlessly" "$CI_ENC"

echo -e "${YELLOW}  [CI] Decrypting headlessly (stdin=/dev/null, timeout=30s)...${RESET}"
CI_OUTPUT=""
CI_EXIT=0
CI_OUTPUT="$(cd "$CI_DIR" && ENVY_PASSPHRASE="ci-pipeline-secret" \
  timeout 30 "$ENVY" decrypt < /dev/null 2>&1)" || CI_EXIT=$?

assert_eq "Headless decrypt exits 0 (no hang)" "0" "$CI_EXIT"
assert_contains "CI decrypt output shows import" "Imported" "$CI_OUTPUT"

echo -e "${YELLOW}  [CI] Verifying secrets available post-decrypt...${RESET}"
CI_TOKEN="$(cd "$CI_DIR" && "$ENVY" get DEPLOY_TOKEN)"
CI_AWS="$(cd "$CI_DIR" && "$ENVY" get AWS_SECRET)"
assert_eq "DEPLOY_TOKEN matches" "ghp_abc123tokenvalue" "$CI_TOKEN"
assert_eq "AWS_SECRET matches" "wJalrXUt123secretkey" "$CI_AWS"

# =============================================================================
# SCENARIO 4 — Malicious Actor (Tampering / Integrity Check)
# =============================================================================

section "Scenario 4 — Malicious Actor (Tampering / Integrity)"

echo -e "${YELLOW}  [Victim] Setting up legitimate vault...${RESET}"
init_project "$WORKSPACE/s4-tamper"
TAMPER_DIR="$PROJECT_DIR"
TAMPER_ENC="$ARTIFACT_PATH"

(cd "$TAMPER_DIR" && "$ENVY" set "PAYMENT_KEY=sk_live_real_key")
(cd "$TAMPER_DIR" && "$ENVY" set "DB_PASSWORD=super-secret-db-pass")

echo -e "${YELLOW}  [Victim] Encrypting vault...${RESET}"
(cd "$TAMPER_DIR" && ENVY_PASSPHRASE="victim-pass" "$ENVY" encrypt)
assert_file_exists "envy.enc created" "$TAMPER_ENC"

# ---- Sub-test 4a: Ciphertext tampering ----
echo ""
echo -e "${YELLOW}  [Attacker] Flipping a byte in ciphertext...${RESET}"
ORIGINAL_CT="$(jq -r '.environments.development.ciphertext' "$TAMPER_ENC")"

# Flip the first character of the base64 string
if [[ "${ORIGINAL_CT:0:1}" == "A" ]]; then
  TAMPERED_CHAR="B"
else
  TAMPERED_CHAR="A"
fi
TAMPERED_CT="${TAMPERED_CHAR}${ORIGINAL_CT:1}"

jq --arg ct "$TAMPERED_CT" \
  '.environments.development.ciphertext = $ct' \
  "$TAMPER_ENC" > "$TAMPER_ENC.tmp"
mv "$TAMPER_ENC.tmp" "$TAMPER_ENC"

# Clear the vault so we can verify nothing gets upserted from tampered data
(cd "$TAMPER_DIR" && "$ENVY" rm PAYMENT_KEY 2>/dev/null) || true
(cd "$TAMPER_DIR" && "$ENVY" rm DB_PASSWORD 2>/dev/null) || true

echo -e "${YELLOW}  [Victim] Decrypting tampered artifact...${RESET}"
TAMPER_OUTPUT=""
TAMPER_EXIT=0
TAMPER_OUTPUT="$(cd "$TAMPER_DIR" && ENVY_PASSPHRASE="victim-pass" "$ENVY" decrypt 2>&1)" \
  || TAMPER_EXIT=$?

assert_eq "Ciphertext-tampered decrypt exits non-zero" "1" "$TAMPER_EXIT"
assert_contains "Output confirms authentication failure" "no environments could be decrypted" "$TAMPER_OUTPUT"

echo -e "${YELLOW}  [Victim] Verifying vault NOT polluted with garbage...${RESET}"
GARBAGE1_EXIT=0
(cd "$TAMPER_DIR" && "$ENVY" get PAYMENT_KEY 2>/dev/null) || GARBAGE1_EXIT=$?
assert_eq "PAYMENT_KEY not in vault after tampering" "1" "$GARBAGE1_EXIT"

GARBAGE2_EXIT=0
(cd "$TAMPER_DIR" && "$ENVY" get DB_PASSWORD 2>/dev/null) || GARBAGE2_EXIT=$?
assert_eq "DB_PASSWORD not in vault after tampering" "1" "$GARBAGE2_EXIT"

# ---- Sub-test 4b: Nonce tampering ----
echo ""
echo -e "${YELLOW}  [Attacker 2] Flipping a byte in nonce field...${RESET}"

# Re-create a clean artifact
(cd "$TAMPER_DIR" && "$ENVY" set "PAYMENT_KEY=sk_live_real_key")
(cd "$TAMPER_DIR" && ENVY_PASSPHRASE="victim-pass" "$ENVY" encrypt)

ORIGINAL_NONCE="$(jq -r '.environments.development.nonce' "$TAMPER_ENC")"
if [[ "${ORIGINAL_NONCE:0:1}" == "A" ]]; then
  NONCE_CHAR="B"
else
  NONCE_CHAR="A"
fi
TAMPERED_NONCE="${NONCE_CHAR}${ORIGINAL_NONCE:1}"

jq --arg n "$TAMPERED_NONCE" \
  '.environments.development.nonce = $n' \
  "$TAMPER_ENC" > "$TAMPER_ENC.tmp"
mv "$TAMPER_ENC.tmp" "$TAMPER_ENC"

(cd "$TAMPER_DIR" && "$ENVY" rm PAYMENT_KEY 2>/dev/null) || true

NONCE_OUTPUT=""
NONCE_EXIT=0
NONCE_OUTPUT="$(cd "$TAMPER_DIR" && ENVY_PASSPHRASE="victim-pass" "$ENVY" decrypt 2>&1)" \
  || NONCE_EXIT=$?

assert_eq "Nonce-tampered decrypt exits non-zero" "1" "$NONCE_EXIT"
assert_contains "Nonce tampering confirms auth failure" "no environments could be decrypted" "$NONCE_OUTPUT"

GARBAGE3_EXIT=0
(cd "$TAMPER_DIR" && "$ENVY" get PAYMENT_KEY 2>/dev/null) || GARBAGE3_EXIT=$?
assert_eq "PAYMENT_KEY not in vault after nonce tampering" "1" "$GARBAGE3_EXIT"

# =============================================================================
# SCENARIO 5 — Machine-Readable Output Formats (--format flag)
# =============================================================================

section "Scenario 5 — Machine-Readable Output Formats"

echo -e "${YELLOW}  [Dev] Setting up project and seeding secrets...${RESET}"
init_project "$WORKSPACE/s5-formats"
FMT_DIR="$PROJECT_DIR"

(cd "$FMT_DIR" && "$ENVY" set "API_KEY=abc123" -e development)
(cd "$FMT_DIR" && "$ENVY" set "DB_PASS=s3cr3t" -e development)

echo -e "${YELLOW}  [Dev] Testing envy list --format json...${RESET}"
LIST_JSON=""
LIST_JSON_EXIT=0
LIST_JSON="$(cd "$FMT_DIR" && "$ENVY" list -e development --format json 2>&1)" || LIST_JSON_EXIT=$?
assert_eq "list --format json exits 0" "0" "$LIST_JSON_EXIT"
assert_contains "list json contains API_KEY" "API_KEY" "$LIST_JSON"
assert_contains "list json contains abc123" "abc123" "$LIST_JSON"
assert_contains "list json contains DB_PASS" "DB_PASS" "$LIST_JSON"

echo -e "${YELLOW}  [Dev] Testing envy export -e development --format shell...${RESET}"
EXPORT_SHELL=""
EXPORT_SHELL_EXIT=0
EXPORT_SHELL="$(cd "$FMT_DIR" && "$ENVY" export -e development --format shell 2>&1)" || EXPORT_SHELL_EXIT=$?
assert_eq "export --format shell exits 0" "0" "$EXPORT_SHELL_EXIT"
assert_contains "export shell contains export API_KEY" "export API_KEY=" "$EXPORT_SHELL"
assert_contains "export shell contains abc123" "abc123" "$EXPORT_SHELL"
assert_contains "export shell contains export DB_PASS" "export DB_PASS=" "$EXPORT_SHELL"

echo -e "${YELLOW}  [Dev] Testing envy export default (dotenv)...${RESET}"
EXPORT_DOTENV=""
EXPORT_DOTENV_EXIT=0
EXPORT_DOTENV="$(cd "$FMT_DIR" && "$ENVY" export -e development 2>&1)" || EXPORT_DOTENV_EXIT=$?
assert_eq "export default dotenv exits 0" "0" "$EXPORT_DOTENV_EXIT"
assert_contains "export dotenv contains API_KEY=abc123" "API_KEY=abc123" "$EXPORT_DOTENV"
assert_contains "export dotenv contains DB_PASS=s3cr3t" "DB_PASS=s3cr3t" "$EXPORT_DOTENV"

# =============================================================================
# SCENARIO 6 — Multi-Env Headless Encryption with Smart Merge (T033)
#
# Verifies:
#   - ENVY_PASSPHRASE_DEVELOPMENT seals only development (FR-001)
#   - ENVY_PASSPHRASE_PRODUCTION seals only production (FR-002)
#   - Both envs coexist in envy.enc after two separate headless runs (FR-005)
#   - A pre-existing third envelope (staging) is preserved unchanged (SC-002)
# =============================================================================

section "Scenario 6 — Multi-Env Headless Encryption with Smart Merge"

echo -e "${YELLOW}  [S6] Setting up project with three environments...${RESET}"
init_project "$WORKSPACE/s6-multienv"
S6_DIR="$PROJECT_DIR"
S6_ENC="$ARTIFACT_PATH"

# Seed all three environments.
(cd "$S6_DIR" && "$ENVY" set "DEPLOY_TOKEN=ghp_s6dev" -e development)
(cd "$S6_DIR" && "$ENVY" set "DB_URL=postgres://prod" -e production)
(cd "$S6_DIR" && "$ENVY" set "STAGING_KEY=stg123" -e staging)

# Step 1: seal only staging first (gives us a pre-existing third envelope).
echo -e "${YELLOW}  [S6] Step 1: Sealing staging as pre-existing envelope...${RESET}"
S6_STAGE_EXIT=0
(cd "$S6_DIR" && ENVY_PASSPHRASE_STAGING="stg-pass" \
  "$ENVY" encrypt -e staging < /dev/null 2>&1) || S6_STAGE_EXIT=$?
assert_eq "Headless seal staging exits 0" "0" "$S6_STAGE_EXIT"

# Capture the staging envelope bytes before the multi-env run.
S6_STAGING_BEFORE=""
S6_STAGING_BEFORE="$(jq -r '.environments.staging' "$S6_ENC" 2>/dev/null || echo "MISSING")"

# Step 2: seal development with its own passphrase (smart merge adds it).
echo -e "${YELLOW}  [S6] Step 2: Sealing development with per-env passphrase...${RESET}"
S6_DEV_EXIT=0
(cd "$S6_DIR" && ENVY_PASSPHRASE_DEVELOPMENT="dev-secret" \
  "$ENVY" encrypt -e development < /dev/null 2>&1) || S6_DEV_EXIT=$?
assert_eq "Headless seal development exits 0" "0" "$S6_DEV_EXIT"

# Step 3: seal production with its own passphrase (smart merge adds it).
echo -e "${YELLOW}  [S6] Step 3: Sealing production with per-env passphrase...${RESET}"
S6_PROD_EXIT=0
(cd "$S6_DIR" && ENVY_PASSPHRASE_PRODUCTION="prod-secret" \
  "$ENVY" encrypt -e production < /dev/null 2>&1) || S6_PROD_EXIT=$?
assert_eq "Headless seal production exits 0" "0" "$S6_PROD_EXIT"

# Step 4: verify envy.enc contains all three envelopes.
echo -e "${YELLOW}  [S6] Step 4: Verifying all three envelopes coexist...${RESET}"
S6_CONTENT="$(cat "$S6_ENC")"
assert_contains "envy.enc has development" '"development"' "$S6_CONTENT"
assert_contains "envy.enc has production"  '"production"'  "$S6_CONTENT"
assert_contains "envy.enc has staging"     '"staging"'     "$S6_CONTENT"

# Step 5: verify staging envelope bytes are unchanged (SC-002).
echo -e "${YELLOW}  [S6] Step 5: Verifying staging was not re-sealed (SC-002)...${RESET}"
S6_STAGING_AFTER=""
S6_STAGING_AFTER="$(jq -r '.environments.staging' "$S6_ENC" 2>/dev/null || echo "MISSING")"
assert_eq "staging envelope unchanged after smart merge" "$S6_STAGING_BEFORE" "$S6_STAGING_AFTER"

# Step 6: decrypt development with its passphrase and verify the secret.
echo -e "${YELLOW}  [S6] Step 6: Decrypting development and verifying secret...${RESET}"
S6_DEC_EXIT=0
(cd "$S6_DIR" && ENVY_PASSPHRASE="dev-secret" \
  "$ENVY" decrypt < /dev/null 2>&1) || S6_DEC_EXIT=$?
assert_eq "Headless decrypt (dev-pass) exits 0" "0" "$S6_DEC_EXIT"
S6_TOKEN="$(cd "$S6_DIR" && "$ENVY" get DEPLOY_TOKEN -e development)"
assert_eq "DEPLOY_TOKEN matches after decrypt" "ghp_s6dev" "$S6_TOKEN"

# =============================================================================
# SCENARIO 8 — Sync Status Command (010-status-command)
#
# Verifies:
#   - US1: `envy status` exits 0 and shows "Never Sealed" before any encrypt
#   - US2: `envy status` shows "In Sync" after encrypt and "Modified" after set
#   - US3: `envy status --format json` exits 0 and returns valid JSON
#   - US4: Artifact metadata (sealed envs, mtime) appears in table and JSON
# =============================================================================

section "Scenario 8 — Sync Status Command"

echo -e "${YELLOW}  [S8] Setting up project...${RESET}"
init_project "$WORKSPACE/s8-status"
S8_DIR="$PROJECT_DIR"
S8_ENC="$ARTIFACT_PATH"

(cd "$S8_DIR" && "$ENVY" set "API_KEY=abc" -e development)

# ── US1: status before any encrypt shows Never Sealed ────────────────────────
echo -e "${YELLOW}  [S8] US1: status before encrypt exits 0 and shows Never Sealed...${RESET}"
S8_STATUS1_EXIT=0
S8_STATUS1="$(cd "$S8_DIR" && "$ENVY" status 2>&1)" || S8_STATUS1_EXIT=$?
assert_eq   "status before encrypt exits 0"           "0" "$S8_STATUS1_EXIT"
assert_contains "status before encrypt shows Never Sealed" "Never Sealed" "$S8_STATUS1"

# ── US2: status after encrypt shows In Sync ──────────────────────────────────
echo -e "${YELLOW}  [S8] US2a: status after encrypt shows In Sync...${RESET}"
S8_ENC_EXIT=0
(cd "$S8_DIR" && ENVY_PASSPHRASE="s8-pass" "$ENVY" encrypt < /dev/null 2>&1) \
  || S8_ENC_EXIT=$?
assert_eq "encrypt exits 0" "0" "$S8_ENC_EXIT"

S8_STATUS2_EXIT=0
S8_STATUS2="$(cd "$S8_DIR" && "$ENVY" status 2>&1)" || S8_STATUS2_EXIT=$?
assert_eq   "status after encrypt exits 0"         "0" "$S8_STATUS2_EXIT"
assert_contains "status after encrypt shows In Sync"  "In Sync"  "$S8_STATUS2"

# ── US2: status after set_secret shows Modified ──────────────────────────────
echo -e "${YELLOW}  [S8] US2b: status after set shows Modified...${RESET}"
sleep 1  # ensure updated_at > sealed_at
(cd "$S8_DIR" && "$ENVY" set "NEW_KEY=xyz" -e development)
S8_STATUS3_EXIT=0
S8_STATUS3="$(cd "$S8_DIR" && "$ENVY" status 2>&1)" || S8_STATUS3_EXIT=$?
assert_eq   "status after set exits 0"         "0" "$S8_STATUS3_EXIT"
assert_contains "status after set shows Modified" "Modified" "$S8_STATUS3"

# ── US3: --format json returns valid JSON with expected shape ─────────────────
echo -e "${YELLOW}  [S8] US3: --format json exits 0 and returns valid JSON...${RESET}"
S8_JSON_EXIT=0
S8_JSON="$(cd "$S8_DIR" && "$ENVY" status --format json 2>&1)" || S8_JSON_EXIT=$?
assert_eq "status --format json exits 0" "0" "$S8_JSON_EXIT"

# Validate JSON structure with jq.
S8_ENV_COUNT="$(echo "$S8_JSON" | jq '.environments | length' 2>/dev/null || echo "INVALID")"
assert_eq "JSON has 1 environment" "1" "$S8_ENV_COUNT"

S8_STATUS_STR="$(echo "$S8_JSON" | jq -r '.environments[0].status' 2>/dev/null || echo "INVALID")"
assert_eq "JSON status is 'modified' after set" "modified" "$S8_STATUS_STR"

S8_ARTIFACT_FOUND="$(echo "$S8_JSON" | jq -r '.artifact.found' 2>/dev/null || echo "INVALID")"
assert_eq "JSON artifact.found is true after encrypt" "true" "$S8_ARTIFACT_FOUND"

# ── US4: table output contains artifact section ───────────────────────────────
echo -e "${YELLOW}  [S8] US4: table output shows artifact section...${RESET}"
assert_contains "table output shows Artifact line" "Artifact:" "$S8_STATUS2"
assert_contains "table output lists development in artifact" "development" "$S8_STATUS2"

# ── US4: JSON output includes artifact environments list ─────────────────────
S8_ART_ENVS="$(echo "$S8_JSON" | jq -r '.artifact.environments | join(",")' 2>/dev/null || echo "INVALID")"
assert_contains "JSON artifact.environments contains development" "development" "$S8_ART_ENVS"

# ── US1: envy st alias works ──────────────────────────────────────────────────
echo -e "${YELLOW}  [S8] US1: 'envy st' alias works...${RESET}"
S8_ALIAS_EXIT=0
(cd "$S8_DIR" && "$ENVY" st 2>&1) || S8_ALIAS_EXIT=$?
assert_eq "envy st alias exits 0" "0" "$S8_ALIAS_EXIT"

# =============================================================================
# Scenario 9: envy diff round-trip (011-envy-diff)
# =============================================================================
section "Scenario 9 — Pre-Encrypt Secret Diff"

S9_DIR="$WORKSPACE/scenario9"

# ── Setup: init, set 3 secrets, encrypt ──────────────────────────────────────
echo -e "${YELLOW}  [S9] Setup: init + set + encrypt...${RESET}"
init_project "$S9_DIR"
(cd "$S9_DIR" && "$ENVY" set 'A=alpha')
(cd "$S9_DIR" && "$ENVY" set 'B=bravo')
(cd "$S9_DIR" && "$ENVY" set 'C=charlie')
(cd "$S9_DIR" && ENVY_PASSPHRASE_DEVELOPMENT='diff-pass' "$ENVY" encrypt -e development < /dev/null)

# ── Drift: add D, modify B, remove C ────────────────────────────────────────
echo -e "${YELLOW}  [S9] Creating drift: add D, modify B, remove C...${RESET}"
(cd "$S9_DIR" && "$ENVY" set 'D=delta')
(cd "$S9_DIR" && "$ENVY" set 'B=bravo2')
(cd "$S9_DIR" && "$ENVY" rm C)

# ── envy diff (table) — expect exit code 1 ──────────────────────────────────
echo -e "${YELLOW}  [S9] envy diff — expect differences (exit 1)...${RESET}"
S9_DIFF_EXIT=0
S9_DIFF_OUT=$(cd "$S9_DIR" && ENVY_PASSPHRASE_DEVELOPMENT='diff-pass' "$ENVY" diff -e development 2>/dev/null) || S9_DIFF_EXIT=$?
assert_eq "envy diff exits 1 when differences exist" "1" "$S9_DIFF_EXIT"

# ── envy diff --format json — parse with jq ─────────────────────────────────
echo -e "${YELLOW}  [S9] envy diff --format json — verify counts...${RESET}"
S9_JSON_EXIT=0
S9_JSON=$(cd "$S9_DIR" && ENVY_PASSPHRASE_DEVELOPMENT='diff-pass' "$ENVY" diff -e development --format json 2>/dev/null) || S9_JSON_EXIT=$?
assert_eq "envy diff --format json exits 1" "1" "$S9_JSON_EXIT"

S9_ADDED=$(echo "$S9_JSON" | jq '.summary.added')
S9_REMOVED=$(echo "$S9_JSON" | jq '.summary.removed')
S9_MODIFIED=$(echo "$S9_JSON" | jq '.summary.modified')
S9_TOTAL=$(echo "$S9_JSON" | jq '.summary.total')

assert_eq "JSON summary.added == 1" "1" "$S9_ADDED"
assert_eq "JSON summary.removed == 1" "1" "$S9_REMOVED"
assert_eq "JSON summary.modified == 1" "1" "$S9_MODIFIED"
assert_eq "JSON summary.total == 3" "3" "$S9_TOTAL"

# ── Verify no old_value/new_value without --reveal ──────────────────────────
echo -e "${YELLOW}  [S9] No reveal — old_value/new_value must be absent...${RESET}"
S9_HAS_OLD=$(echo "$S9_JSON" | jq '[.changes[] | has("old_value")] | any')
S9_HAS_NEW=$(echo "$S9_JSON" | jq '[.changes[] | has("new_value")] | any')
assert_eq "old_value absent without --reveal" "false" "$S9_HAS_OLD"
assert_eq "new_value absent without --reveal" "false" "$S9_HAS_NEW"

# ── Re-encrypt to bring vault in sync ───────────────────────────────────────
echo -e "${YELLOW}  [S9] Re-encrypt to sync...${RESET}"
(cd "$S9_DIR" && ENVY_PASSPHRASE_DEVELOPMENT='diff-pass' "$ENVY" encrypt -e development < /dev/null)

# ── envy diff — expect exit code 0 (no differences) ────────────────────────
echo -e "${YELLOW}  [S9] envy diff after re-encrypt — expect no differences (exit 0)...${RESET}"
S9_CLEAN_EXIT=0
(cd "$S9_DIR" && ENVY_PASSPHRASE_DEVELOPMENT='diff-pass' "$ENVY" diff -e development 2>/dev/null) || S9_CLEAN_EXIT=$?
assert_eq "envy diff exits 0 when no differences" "0" "$S9_CLEAN_EXIT"

# ── envy diff --format json — empty changes ─────────────────────────────────
echo -e "${YELLOW}  [S9] envy diff --format json after sync — has_differences == false...${RESET}"
S9_CLEAN_JSON=$(cd "$S9_DIR" && ENVY_PASSPHRASE_DEVELOPMENT='diff-pass' "$ENVY" diff -e development --format json 2>/dev/null)
S9_HAS_DIFF=$(echo "$S9_CLEAN_JSON" | jq '.has_differences')
S9_CHANGES_LEN=$(echo "$S9_CLEAN_JSON" | jq '.changes | length')
assert_eq "has_differences is false" "false" "$S9_HAS_DIFF"
assert_eq "changes array is empty" "0" "$S9_CHANGES_LEN"

# ── envy df alias works ────────────────────────────────────────────────────
echo -e "${YELLOW}  [S9] 'envy df' alias works...${RESET}"
S9_DF_EXIT=0
(cd "$S9_DIR" && ENVY_PASSPHRASE_DEVELOPMENT='diff-pass' "$ENVY" df -e development 2>/dev/null) || S9_DF_EXIT=$?
assert_eq "envy df alias exits 0" "0" "$S9_DF_EXIT"

# =============================================================================
# Summary
# =============================================================================

echo ""
echo -e "${CYAN}${BOLD}═══════════════════════════════════════════════════════════════${RESET}"
echo -e "${CYAN}${BOLD}  Summary${RESET}"
echo -e "${CYAN}${BOLD}═══════════════════════════════════════════════════════════════${RESET}"
echo ""

if [[ $FAIL -eq 0 ]]; then
  echo -e "  ${GREEN}${BOLD}ALL ${TOTAL} ASSERTIONS PASSED${RESET}  🎉"
else
  echo -e "  ${GREEN}Passed: ${PASS}/${TOTAL}${RESET}"
  echo -e "  ${RED}Failed: ${FAIL}/${TOTAL}${RESET}"
fi

echo ""
echo -e "${DIM}Workspace cleaned up: ${WORKSPACE}${RESET}"

exit "$FAIL"
