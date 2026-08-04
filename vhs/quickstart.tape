# envy quickstart — the hello-world: init → set → list → run
# Rendered to docs/assets/quickstart.gif by CI (vhs-demos job) on master push.

Require envy

Output docs/assets/quickstart.gif

Set Shell "bash"
Set FontSize 16
Set Width 1000
Set Height 600
Set TypingSpeed 70ms
Set Theme "Catppuccin Mocha"

# Headless, deterministic vault — ephemeral fallback key, dummy values only.
Env ENVY_PASSPHRASE "ephemeral-quickstart-key"
Env ENVY_PASSPHRASE_DEVELOPMENT "quickstart-dummy-pass"

# Fresh workspace for every render — never touches the repo.
Type "cd $(mktemp -d)"
Enter
Sleep 1s

# 1. Init — creates envy.toml (safe to commit), registers the project.
Type "envy init"
Enter
Sleep 2s

# 2. Set two dummy secrets — encrypted with AES-256-GCM in the vault.
Type "envy set STRIPE_KEY=sk_test_dummy123"
Enter
Sleep 1s
Type "envy set DB_URL=postgres://localhost/envy_demo"
Enter
Sleep 1s

# 3. List — key names only, values never printed by default.
Type "envy list"
Enter
Sleep 2s

# 4. Run — secrets injected into the child process, never written to disk.
Type "envy run -- bash -c 'echo stripe key: $STRIPE_KEY'"
Enter
Sleep 3s

Sleep 2s
