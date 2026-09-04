# envy TUI — interactive workstation console demo
# Rendered to docs/assets/tui-demo.gif by: vhs vhs/tui-demo.tape
#
# IMPORTANT:
# - Uses the locally-compiled binary (target/debug/envy) so the TUI from this
#   branch is available; the system-installed envy may be older.
# - The hidden setup redirects $HOME to a scratch directory, so the demo runs
#   against a THROWAWAY vault — your real ~/.envy/vault.db is never touched
#   and no real project names appear in the GIF.
# - "Fira Code" must be installed locally; the clipboard moment needs a
#   display session (X11/Wayland), otherwise the honest error fallback shows.

# Fail fast if bash is missing (deps-require).
Require bash

Output docs/assets/tui-demo.gif

Set Shell         bash
Set Width         1200
Set Height        700
Set FontFamily    "Fira Code"
Set FontSize      14
Set TypingSpeed   80ms
Set Theme         "Catppuccin Mocha"
Set WindowBar     Colorful
Set WindowBarSize 40
Set BorderRadius  12
Set Margin        20
Set MarginFill    "#1a0933"
Set Padding       16
Set Framerate     30

# Headless passphrases for the seal in hidden setup
Env ENVY_PASSPHRASE_DEVELOPMENT "demo-dev-pass"
Env ENVY_PASSPHRASE_PRODUCTION  "demo-prod-pass"

# ─── Hidden setup: throwaway vault + project + secrets ─────────────────────
Hide
# Isolate $HOME so envy uses a scratch vault (~/.envy under /tmp) — the real
# vault and the OS keyring master key are untouched by the demo content.
Type `export HOME=/tmp/envy-tui-demo-home`
Enter
# Use the branch-compiled binary instead of the system-installed one
Type `export PATH="/home/oriolgv/aaDev/ANGURIA/envy-project/envy/target/debug:$PATH"`
Enter
Type `export PS1=$'\[\e[1;35m\]❯ \[\e[1;36m\]~/my-project \[\e[1;33m\]$\[\e[0m\] '`
Enter
Type "rm -rf /tmp/envy-tui-demo /tmp/envy-tui-demo-home && mkdir -p /tmp/envy-tui-demo /tmp/envy-tui-demo-home && cd /tmp/envy-tui-demo"
Enter
Sleep 500ms
Type "envy init"
Enter
Sleep 1s
# Development secrets (obviously-fake values: sk_test_/demo prefixes)
Type "envy set API_KEY=sk_test_51DemoKeyZx9 -e development"
Enter
Sleep 300ms
Type "envy set DATABASE_URL=postgres://dev:pass@localhost/app -e development"
Enter
Sleep 300ms
Type "envy set REDIS_URL=redis://localhost:6379 -e development"
Enter
Sleep 300ms
# Production secrets
Type "envy set STRIPE_SECRET=sk_test_51ProdDemoQy7 -e production"
Enter
Sleep 300ms
Type "envy set DATABASE_URL=postgres://admin:s3cr3t@db.prod.com/app -e production"
Enter
Sleep 300ms
Type "envy set JWT_SECRET=demo-hmac-sha256-prod-key -e production"
Enter
Sleep 300ms
Type "envy set SENDGRID_KEY=SG.demo_email_key -e production"
Enter
Sleep 300ms
# Seal development so status shows "in sync" vs "never sealed"
Type "envy encrypt -e development < /dev/null"
Enter
Sleep 2s
Type "clear"
Enter
Sleep 500ms
Show

# ━━━ Launch the TUI ━━━
Sleep 300ms
Type "envy"
Enter
Sleep 3s

# ━━━ Brand moment: b expands the gradient banner, then back to the strip ━━━
# (700px render ≈ 31 rows < BANNER_MIN_HEIGHT, so the TUI opens with the
# compact identity strip — ENVY · v0.5.1 · envy-tui-demo — and b expands)
Type "b"
Sleep 2s
Type "b"
Sleep 600ms

# ━━━ Navigate: select development environment ━━━
Down
Sleep 400ms
Enter
Sleep 1.5s

# ━━━ Switch to secrets panel (legend switches with focus) ━━━
Tab
Sleep 1.5s

# ━━━ Details inspector mirrors the selection (right panel) ━━━
Sleep 2s

# ━━━ Reveal a secret value (Space) ━━━
Space
Sleep 2.5s

# ━━━ Copy without revealing the flow: Y copies the selected value ━━━
Type "y"
Sleep 2s

# ━━━ Move down — value auto-masks, selection follows ━━━
Down
Sleep 1.5s

# ━━━ Create a new secret (N): Enter on key jumps to the value field ━━━
Type "n"
Sleep 1s
Type@60ms "NEW_TOKEN"
Sleep 500ms
Enter
Sleep 500ms
Type@60ms "tok_demo_abc123xyz"
Sleep 500ms
Enter
Sleep 2s

# ━━━ Switch to tree, navigate to production ━━━
Tab
Sleep 600ms
Down
Sleep 400ms
Enter
Sleep 1.5s

# ━━━ Show production secrets ━━━
Tab
Sleep 2s

# ━━━ Command palette → project status (dev in sync, prod never sealed) ━━━
Type ":"
Sleep 800ms
Type@60ms "stat"
Sleep 600ms
Enter
Sleep 3s
Escape
Sleep 1s

# ━━━ Seal preview: environments + counts before any passphrase ━━━
Type "s"
Sleep 2.5s
Escape
Sleep 1s

# ━━━ Back to development for the rotate flow ━━━
Tab
Sleep 600ms
Up
Sleep 400ms
Enter
Sleep 1.5s

# ━━━ Rotate passphrase in-TUI (R): current → new → confirm ━━━
Type "r"
Sleep 1.5s
Type@60ms "demo-dev-pass"
Enter
Sleep 1s
Type@60ms "demo-dev-pass-v2"
Enter
Sleep 1s
Type@60ms "demo-dev-pass-v2"
Enter
Sleep 3s

# ━━━ Help popup: grouped sections, scrollable ━━━
Type "?"
Sleep 1.5s
Down
Sleep 250ms
Down
Sleep 250ms
Down
Sleep 250ms
Down
Sleep 250ms
Sleep 1.5s
Escape
Sleep 1s

# ━━━ Quit ━━━
Type "q"
Sleep 2s
