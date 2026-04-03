# Output first, then ALL Set directives, then Require (rule: config-settings-order)
Output docs/assets/demo.gif

Set Shell         bash
Set Width         1100
Set Height        520
Set FontFamily    "Fira Code"
Set FontSize      18
Set TypingSpeed   65ms
Set Theme         "Catppuccin Mocha"
Set WindowBar     Colorful
Set WindowBarSize 40
Set BorderRadius  12
Set Margin        20
Set MarginFill    "#4b4b65"
Set Padding       20
Set Framerate     50
Set LoopOffset    80%

# Require AFTER all Set directives (rule: deps-require-order)
Require envy

# Hidden setup: clean env, fixed PS1 with proper \[ \] escaping (fixes line-wrap calc)
Hide
Type `export PS1=$'\[\e[1;35m\]❯ \[\e[1;36m\]~/my-project \[\e[1;33m\]$\[\e[0m\] '`
Enter
Type "rm -rf /tmp/envy-demo && mkdir -p /tmp/envy-demo && cd /tmp/envy-demo"
Enter
Type "envy init"
Enter
Sleep 1s
Type "clear"
Enter
Show

# Step 1 — Store production secrets
Type "envy set STRIPE_SECRET_KEY=sk_live_51Nx... -e production"
Sleep 300ms
Enter
Sleep 1.2s

Type "envy set DATABASE_URL=postgres://user:pass@db.prod.com/app -e production"
Sleep 300ms
Enter
Sleep 1.2s

# Step 2 — Status before: shows "Modified"
Type "envy status"
Sleep 300ms
Enter
Sleep 4s

# Step 3 — Diff: audit the exact changes
Type "envy diff -e production"
Sleep 300ms
Enter
Sleep 4s

# Step 4 — Encrypt with interactive passphrase
Type "envy encrypt -e production"
Sleep 300ms
Enter
Sleep 1.5s
Type "my-secret"
Sleep 300ms
Enter
Sleep 1s
Type "my-secret"
Sleep 300ms
Enter
Sleep 2.5s

# Step 5 — Status after: proves production is now "In Sync"
Type "envy status"
Sleep 300ms
Enter

# Final hold — last frame must be readable (rule: timing-final-sleep)
Sleep 6s
