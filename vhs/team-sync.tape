# envy team-sync — Dev A seals envy.enc, Dev B restores it on a fresh machine.
# Rendered to docs/assets/team-sync.gif by CI (vhs-demos job) on master push.

Require envy

Output docs/assets/team-sync.gif

Set Shell "bash"
Set FontSize 16
Set Width 1000
Set Height 600
Set TypingSpeed 70ms
Set Theme "Catppuccin Mocha"

# Headless, deterministic vault — ephemeral fallback key, dummy values only.
Env ENVY_PASSPHRASE "ephemeral-teamsync-key"
Env ENVY_PASSPHRASE_DEVELOPMENT "teamsync-dummy-pass"

# ── Dev A: seal the artifact ───────────────────────────────────────────────
Type "cd $(mktemp -d) && mkdir -p dev-a && cd dev-a"
Enter
Sleep 1s
Type "envy init"
Enter
Sleep 2s
Type "envy set TEAM_TOKEN=dummy_team_token"
Enter
Sleep 1s
Type "envy set TEAM_DSN=dummy://team.example.invalid/db"
Enter
Sleep 1s
Type "envy encrypt -e development < /dev/null"
Enter
Sleep 3s

# The artifact is pure ciphertext — commit it, post it, it is safe.
Type "cat envy.enc"
Enter
Sleep 3s

# ── Dev B: restore on a fresh machine (the "git pull") ──────────────────────
Type "cd .. && mkdir -p dev-b && cd dev-b"
Enter
Sleep 1s
Type "cp ../dev-a/envy.enc . && envy init"
Enter
Sleep 2s
Type "envy decrypt < /dev/null"
Enter
Sleep 3s

# Verify the round-trip, then use the secret.
Type "envy get TEAM_TOKEN"
Enter
Sleep 2s
Type "envy run -- bash -c 'echo token: $TEAM_TOKEN'"
Enter
Sleep 3s

Sleep 2s
