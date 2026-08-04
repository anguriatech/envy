# envy ci-cd — headless pipeline: encrypt → status JSON gate → diff gate → run.
# Rendered to docs/assets/ci-cd.gif by CI (vhs-demos job) on master push.

Require envy
Require jq

Output docs/assets/ci-cd.gif

Set Shell "bash"
Set FontSize 16
Set Width 1000
Set Height 600
Set TypingSpeed 70ms
Set Theme "Catppuccin Mocha"

# Headless, deterministic vault — ephemeral fallback key, dummy values only.
# stdin is closed with < /dev/null so no prompt can block a pipeline.
Env ENVY_PASSPHRASE "ephemeral-cicd-key"
Env ENVY_PASSPHRASE_DEVELOPMENT "cicd-dummy-pass"
# jq filter value — avoids embedded double quotes in the typed command.
Env GATE_STATUS "in_sync"

Type "cd $(mktemp -d) && mkdir -p pipeline && cd pipeline"
Enter
Sleep 1s
Type "envy init"
Enter
Sleep 2s
Type "envy set PIPELINE_TOKEN=dummy_pipeline_token"
Enter
Sleep 1s

# Seal headlessly — no prompts in CI.
Type "envy encrypt -e development < /dev/null"
Enter
Sleep 3s

# Machine-readable status gate: JSON, no passphrase, no decryption.
Type "envy status --format json"
Enter
Sleep 3s

# Gate: the environment must be in_sync before deploying.
Type "envy status --format json | jq -e '.environments[] | select(.status == env.GATE_STATUS)' > /dev/null && echo gate: in_sync"
Enter
Sleep 3s

# Diff gate: exit 0 = vault and artifact match, 1 = drift, 2+ = error.
Type "envy diff -e development"
Enter
Sleep 3s
Type "echo diff gate exit: $?"
Enter
Sleep 2s

# Deploy with injected secrets.
Type "envy run -e development -- bash -c 'echo pipeline token: $PIPELINE_TOKEN'"
Enter
Sleep 3s

Sleep 2s
