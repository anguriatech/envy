# Envy

> **Zero-friction local dev. Secure GitOps. No plaintext secrets — ever.**

Envy is a local-first, encrypted environment variable manager built for individual developers and teams. Instead of scattering secrets across plaintext `.env` files that get accidentally committed, shared over Slack, or left on disk unprotected, Envy stores every secret in an AES-256-GCM encrypted vault on your machine, unlocked by a master key held exclusively in your OS credential store.

When you need your secrets, `envy run` injects them directly into your process's memory — no files written, no exports, no leaks. When you need to share them with your team, `envy encrypt` seals them into a single committed file that only the right passphrase can open.

---

## Key Features

- **Encrypted local vault** — SQLCipher-encrypted SQLite (`~/.envy/vault.db`). Each secret value is additionally encrypted with AES-256-GCM before it even reaches the database.
- **OS Keyring integration** — The vault master key lives in macOS Keychain, Windows Credential Manager, or Linux Secret Service. It never touches the filesystem.
- **Seamless process injection** — `envy run -- npm start` decrypts secrets and injects them as environment variables into the child process. Your application code doesn't change.
- **Multi-environment support** — Manage `development`, `staging`, and `production` secrets side-by-side within the same project.
- **GitOps team sync** — `envy encrypt` seals your vault into a single `envy.enc` artifact you can safely commit to Git. `envy decrypt` restores secrets after a pull.
- **Smart Merge** — Seal environments independently with separate passphrases. Envy merges new envelopes into an existing `envy.enc` without disturbing untouched environments — zero Git conflicts.
- **Progressive Disclosure** — Each environment can have its own passphrase. A developer with the dev key imports `development`; `production` is listed as gracefully skipped. No error, no alarm.
- **Sync Status dashboard** — `envy status` gives an instant, read-only overview of every environment's sync state relative to `envy.enc`. No passphrase required.
- **CI/CD headless mode** — Set `ENVY_PASSPHRASE_<ENV>` in your pipeline. `envy decrypt` detects it automatically — no interactive prompts, no code changes.
- **Diceware passphrase generation** — Envy suggests a cryptographically strong, human-memorable passphrase when you seal your vault. You can accept it or type your own.
- **Legacy migration** — `envy migrate .env` imports an existing dotenv file in one step.
- **Shell completions** — Tab-complete every command and flag in bash, zsh, fish, and PowerShell.
- **Single binary, zero runtime dependencies** — One statically-compiled Rust binary. No Node.js, Python, or Docker required.

---

## Installation

### macOS & Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/anguriatech/envy/releases/latest/download/envy-installer.sh | sh
```

### Windows (PowerShell)

```powershell
irm https://github.com/anguriatech/envy/releases/latest/download/envy-installer.ps1 | iex
```

### Build from source (requires Rust 1.85+)

```bash
git clone https://github.com/anguriatech/envy.git
cd envy
cargo install --path .
```

---

## Quickstart

```bash
# 1. Initialise — creates envy.toml (safe to commit)
cd my-project
envy init

# 2. Store secrets
envy set DATABASE_URL=postgres://localhost/myapp
envy set API_KEY=sk_live_abc123

# 3. Run your app with secrets injected
envy run -- npm run dev

# 4. Inspect your vault (key names only — values are never printed by default)
envy list
envy get DATABASE_URL
```

> **Tip:** `development` is the default environment. All commands accept `-e ENV` to target a different one.

---

## Shell Autocompletion

Enable tab-completion for all commands and flags in your shell:

```bash
# bash
envy completions bash >> ~/.bash_completion

# zsh (reload your shell after)
envy completions zsh > ~/.zfunc/_envy

# fish
envy completions fish > ~/.config/fish/completions/envy.fish

# PowerShell
envy completions powershell >> $PROFILE
```

---

## Command Reference

| Command | Aliases | Description |
|---------|---------|-------------|
| `envy init` | — | Create `envy.toml`, register project in vault |
| `envy set KEY=VALUE [-e ENV]` | — | Store or update a secret |
| `envy get KEY [-e ENV]` | — | Print a single decrypted value to stdout |
| `envy list [-e ENV]` | `ls` | List all key names (never values) |
| `envy rm KEY [-e ENV]` | `remove`, `unset` | Delete a secret |
| `envy run [-e ENV] -- CMD` | — | Inject secrets and run a child process |
| `envy migrate FILE [-e ENV]` | — | Import all `KEY=VALUE` pairs from a dotenv file |
| `envy encrypt [-e ENV]` | `enc` | Seal vault into `envy.enc` |
| `envy decrypt` | `dec` | Unseal `envy.enc` and restore secrets |
| `envy export [-e ENV]` | — | Print all secrets to stdout (dotenv / JSON / shell) |
| `envy status` | `st` | Show sync status dashboard |

---

## Sync Status Dashboard

`envy status` gives you an instant, read-only snapshot of every environment's sync state — no passphrase, no decryption.

```
$ envy status

╔═══════════════╦═════════╦══════════════════╦═══════════════╗
║ Environment   ║ Secrets ║ Last Modified    ║ Status        ║
╠═══════════════╬═════════╬══════════════════╬═══════════════╣
║ development   ║ 4       ║ 2 minutes ago    ║ ⚠ Modified    ║
║ production    ║ 3       ║ 3 days ago       ║ ✓ In Sync     ║
║ staging       ║ 2       ║ 1 week ago       ║ ✗ Never Sealed║
╚═══════════════╩═════════╩══════════════════╩═══════════════╝

Artifact: ./envy.enc  (last written: 3 days ago)
  Sealed environments: production
```

### Sync States

| Status | Meaning |
|--------|---------|
| ✓ **In Sync** | All secrets were last modified before (or at) the last `envy encrypt`. The vault and `envy.enc` match. |
| ⚠ **Modified** | At least one secret was changed after the last `envy encrypt`. Re-run `envy encrypt` to bring the artifact up to date. |
| ✗ **Never Sealed** | This environment has never been encrypted. Run `envy encrypt -e <env>` to seal it. |

Use `--format json` for machine-readable output in CI/CD pipelines:

```bash
envy status --format json
```

```json
{
  "environments": [
    { "name": "development", "secret_count": 4, "last_modified_at": "2026-03-25T10:30:00Z", "status": "modified" },
    { "name": "production",  "secret_count": 3, "last_modified_at": "2026-03-22T08:00:00Z", "status": "in_sync"  }
  ],
  "artifact": {
    "found": true,
    "path": "./envy.enc",
    "last_modified_at": "2026-03-22T08:00:00Z",
    "environments": ["production"]
  }
}
```

---

## Team Sync via Git

### Sealing the vault

```bash
envy encrypt
# Envy suggests a Diceware passphrase:
#   Suggested passphrase: correct-horse-battery-staple
#   Use this passphrase? [Y/n]

# Sealed 2 environment(s) → envy.enc
#   ✓  development   (4 secrets)
#   ✓  production    (3 secrets)

git add envy.enc envy.toml
git commit -m "chore: update encrypted secrets"
git push
```

`envy.enc` is pure ciphertext — no key names, no values, no project identifiers. It is safe to commit to a public repository.

### Restoring secrets (after a pull)

```bash
git pull
envy decrypt
# Enter passphrase: ········
# Imported 2 environment(s) from envy.enc
#   ✓  development   (4 secrets upserted)
#   ✓  production    (3 secrets upserted)
```

---

## Multi-Environment Encryption & Smart Merge

Seal each environment with its own passphrase to enforce least-privilege access. Envy uses **Smart Merge**: when you seal a single environment, the existing envelopes for all other environments are preserved untouched — zero Git conflicts.

```bash
# Seal development with the shared dev passphrase
envy enc -e development

# Seal production with the restricted prod passphrase
envy enc -e production

# Both envelopes now coexist in envy.enc
git add envy.enc && git commit -m "chore: rotate secrets"
```

### Progressive Disclosure

```bash
# Junior developer — has only the dev key
envy decrypt
# Enter passphrase: ········   (dev key)
#
# Imported 1 environment(s) from envy.enc
#   ✓  development   (4 secrets upserted)
#   ⚠  production    skipped — different passphrase or key
```

The `⚠` line is purely informational. Exit code is `0`. Production secrets are untouched.

---

## CI/CD Integration

Set per-environment passphrase variables in your pipeline secrets. Envy checks for `ENVY_PASSPHRASE_<ENV>` (uppercase env name) before showing any terminal prompt.

### GitHub Actions

```yaml
# .github/workflows/deploy.yml
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install envy
        run: |
          curl --proto '=https' --tlsv1.2 -LsSf \
            https://github.com/anguriatech/envy/releases/latest/download/envy-installer.sh | sh

      - name: Decrypt secrets
        env:
          ENVY_PASSPHRASE_DEVELOPMENT: ${{ secrets.ENVY_PASSPHRASE_DEVELOPMENT }}
          ENVY_PASSPHRASE_PRODUCTION:  ${{ secrets.ENVY_PASSPHRASE_PRODUCTION }}
        run: envy decrypt

      - name: Verify sync state
        run: |
          STATUS=$(envy status --format json)
          echo "$STATUS"
          # Fail if production is not in_sync
          echo "$STATUS" | jq -e '.environments[] | select(.name == "production") | .status == "in_sync"'

      - name: Deploy
        run: envy run -e production -- ./scripts/deploy.sh
```

### Using `envy status --format json` as a quality gate

```bash
# Parse sync state in a shell script
STATUS=$(envy status --format json)

if echo "$STATUS" | jq -e '.environments[] | select(.status == "modified")' > /dev/null; then
  echo "ERROR: Some environments have unsealed changes. Run 'envy encrypt' first."
  exit 1
fi
```

---

## Legacy Migration

```bash
# Import every KEY=VALUE line from .env into the development vault
envy migrate .env

# Import staging secrets from a separate file
envy migrate .env.staging -e staging

# Verify, then delete the plaintext files
envy list
rm .env .env.staging
echo '.env*' >> .gitignore
```

---

## How It Works

```
Local development:

  envy.toml            ~/.envy/vault.db              OS Keyring
  (project UUID)  -->  (SQLCipher encrypted DB)  <--  (master key)
                       AES-256-GCM per-secret
                       sync_markers (sealed_at per env)


Team sync via Git:

  ~/.envy/vault.db   --[envy encrypt]--->  envy.enc (Argon2id + AES-256-GCM)
                                                │
                                           git commit
                                                │
                     <--[envy decrypt]---  envy.enc
```

1. `envy init` creates a lightweight `envy.toml` linking your project to a UUID in the vault.
2. Secrets are encrypted with AES-256-GCM using the vault master key, then stored in the encrypted SQLite database.
3. The master key lives in your OS credential manager — never written to any file.
4. `envy run` decrypts secrets in-memory and passes them to the child process via `std::process::Command::envs()`. Nothing is written to disk.
5. `envy encrypt` derives a key from your passphrase (Argon2id), encrypts each environment, and writes `envy.enc`. It also records a `sealed_at` timestamp per environment so `envy status` can report the sync state.
6. `envy decrypt` reads `envy.enc`, derives the key, and upserts every decrypted secret back into the local vault.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success; or partial decrypt (≥ 1 environment imported, some skipped) |
| 1 | Not found (manifest, secret, `envy.enc`); or zero environments imported |
| 2 | Invalid input (key name, assignment format, empty passphrase) |
| 3 | Initialisation conflict |
| 4 | Vault / crypto failure; malformed `envy.enc`; unsupported version |
| 127 | Child binary not found (`envy run`) |
| N | Child process exit code (proxied by `envy run`) |

---

## Roadmap

Envy has completed **Phase 1** (encrypted local vault), **Phase 2** (GitOps sync & CI/CD), and the **Phase 2.x** improvements (output formats, multi-env encrypt, sync status). Here's what's next:

**Phase 3 — Ecosystem & GUI**: An official VS Code Extension to make secret management visual and seamless, without needing to leave the editor.
