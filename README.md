# Envy

**Eradicate plaintext `.env` files from your workflow.**

Envy is a local-first, encrypted environment variable manager for developers. Instead of scattering secrets across plaintext `.env` files that get accidentally committed, shared over Slack, or left on disk unprotected, Envy stores every secret in an AES-256-GCM encrypted vault on your machine, unlocked by a master key held exclusively in your OS credential store.

When you need your secrets, `envy run` injects them directly into your process's memory. No files written. No exports. No leaks. When you need to share them with your team, `envy encrypt` seals them into a single committed file that only the right passphrase can open.

## Key Features

- **Encrypted local vault** — Secrets are stored in a SQLCipher-encrypted SQLite database (`~/.envy/vault.db`). Each secret value is individually encrypted with AES-256-GCM before it even reaches the database.
- **OS Keyring integration** — The vault master key lives in your OS credential manager (macOS Keychain, Windows Credential Manager, Linux Secret Service). It never touches the filesystem.
- **Seamless process injection** — `envy run -- npm start` decrypts secrets and injects them as environment variables into the child process. Your application code doesn't change at all.
- **Multi-environment support** — Manage `development`, `staging`, and `production` secrets side-by-side within the same project.
- **GitOps team sync** — `envy encrypt` seals your entire vault into a single `envy.enc` artifact you can safely commit to Git. `envy decrypt` restores secrets after a pull. One passphrase, one file, zero plaintext.
- **Progressive Disclosure** — Enterprise teams can seal each environment with a different passphrase. A developer with the dev key imports `development` and sees `production` listed as gracefully skipped — no error, no alarm, no support ticket.
- **CI/CD headless mode** — Set `ENVY_PASSPHRASE` in your pipeline secrets. `envy decrypt` detects it and skips the interactive prompt entirely, with no code changes needed.
- **Legacy migration** — Import an existing `.env` file with a single command: `envy migrate .env`. Then delete the plaintext file forever.
- **UNIX-friendly** — `envy get KEY` outputs the raw value to stdout with no labels, so it works seamlessly in shell pipelines.
- **Single binary, zero runtime dependencies** — Distributed as one statically-compiled Rust binary. No Node.js, Python, or Docker required.

## Installation

### From source (requires Rust 1.85+)

```bash
git clone https://github.com/<your-username>/envy.git
cd envy
cargo install --path .
```

### From Git directly

```bash
cargo install --git https://github.com/<your-username>/envy.git
```

## Quickstart

```bash
# 1. Initialise Envy in your project
cd my-project
envy init
# => Creates envy.toml (safe to commit — contains only a project UUID)

# 2. Store some secrets
envy set DATABASE_URL=postgres://localhost/myapp
envy set API_KEY=sk_live_abc123

# 3. Run your app with secrets injected
envy run -- npm run dev
# => Your app sees DATABASE_URL and API_KEY as environment variables

# 4. Manage secrets
envy list                    # Show key names (never values)
envy get API_KEY             # Print a single decrypted value
envy rm API_KEY              # Delete a secret

# 5. Migrate from a legacy .env file
envy migrate .env            # Import all KEY=VALUE pairs into the vault
rm .env                      # Delete the plaintext file
```

### Multi-environment usage

```bash
envy set DATABASE_URL=postgres://staging-db -e staging
envy run -e staging -- python manage.py migrate
envy list -e production
```

---

## GitOps & Team Sync

Envy's sync workflow lets your team share secrets through Git, with zero plaintext ever committed.

### Sealing the vault

```bash
envy encrypt
# Enter passphrase: ········
# Confirm passphrase: ········

# Sealed 2 environment(s) → envy.enc
#   ✓  development   (3 secrets)
#   ✓  production    (1 secret)

git add envy.enc
git commit -m "chore: update encrypted secrets"
git push
```

The `envy.enc` file is pure ciphertext. No secret values, no key names, no project identifiers appear anywhere in the file. It is safe for public repositories.

Use the alias `envy enc` for brevity. Seal a single environment with the `-e` flag:

```bash
envy enc -e staging    # seals only staging into envy.enc
```

### Restoring secrets

After a pull, any teammate with the passphrase runs:

```bash
git pull
envy decrypt
# Enter passphrase: ········

# Imported 2 environment(s) from envy.enc
#   ✓  development   (3 secrets upserted)
#   ✓  production    (1 secret upserted)
```

The alias `envy dec` works identically.

### Progressive Disclosure

Enterprise teams can seal each environment with a **different passphrase**, giving developers exactly the access they need — nothing more. When a developer decrypts with their dev key, environments sealed with a different key are listed as skipped, not as errors. The command exits 0 and their vault is fully up to date for every environment they have access to.

```bash
envy decrypt
# Enter passphrase: ········   (dev key — production uses a separate prod key)

# Imported 1 environment(s) from envy.enc
#   ✓  development   (3 secrets upserted)
#   ⚠  production    skipped — different passphrase or key
```

The `⚠` line is informational. Exit code is `0`. The production contents of the local vault are untouched.

---

## CI/CD (Headless)

For automated pipelines, set `ENVY_PASSPHRASE` as a secret in your CI/CD provider. Envy checks for this variable before showing any terminal prompt, so the entire decrypt flow runs silently.

### GitHub Actions

```yaml
# .github/workflows/deploy.yml
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install envy
        run: cargo install --git https://github.com/<your-username>/envy.git

      - name: Decrypt secrets
        env:
          ENVY_PASSPHRASE: ${{ secrets.ENVY_PASSPHRASE }}
        run: envy decrypt

      - name: Deploy
        run: envy run -e production -- ./scripts/deploy.sh
```

### Vercel / Railway / any platform with secret injection

```bash
# Set in your platform's secret manager:
ENVY_PASSPHRASE=your-shared-team-passphrase

# In your build script:
envy decrypt
envy run -- node server.js
```

**Security note**: `ENVY_PASSPHRASE` is the shared artifact password — distinct from the vault master key, which is managed by the OS keyring and never set via environment variable. Whitespace-only values are treated as unset and cause the command to fail cleanly rather than decrypt with an empty key.

---

## How It Works

```
Local development:

  envy.toml            ~/.envy/vault.db              OS Keyring
  (project UUID)  -->  (SQLCipher encrypted DB)  <--  (master key)
                       AES-256-GCM per-secret


Team sync via Git:

  ~/.envy/vault.db   --[envy encrypt]--->  envy.enc (Argon2id + AES-256-GCM)
                                                |
                                           git commit
                                                |
                     <--[envy decrypt]---  envy.enc
```

1. `envy init` creates a lightweight `envy.toml` manifest linking your project directory to a UUID in the vault.
2. Secrets are encrypted with AES-256-GCM using the vault master key, then stored in the encrypted SQLite database.
3. The master key itself is held by your OS credential manager — never written to any file.
4. `envy run` decrypts secrets in-memory and passes them directly to the child process via `std::process::Command::envs()`.
5. `envy encrypt` re-encrypts each environment's secrets with a user-provided passphrase (Argon2id key derivation + AES-256-GCM) and writes the result to `envy.enc`.
6. `envy decrypt` reads `envy.enc`, derives the key from the passphrase, and upserts every decrypted secret back into the local vault.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0    | Success; or partial decrypt (≥ 1 environment imported, some skipped) |
| 1    | Not found (manifest, secret, `envy.enc`); or zero environments imported |
| 2    | Invalid input (key name, assignment format, empty passphrase) |
| 3    | Initialisation conflict |
| 4    | Vault / crypto failure; malformed `envy.enc`; unsupported version |
| 127  | Child binary not found (`envy run`) |
| N    | Child process exit code (proxied by `envy run`) |

---

## Roadmap

Envy has completed **Phase 1** (encrypted local vault) and **Phase 2** (GitOps sync & CI/CD). Here's what's next:

**Phase 3 — Enterprise**: Role-based access control (RBAC), audit logging for SOC2/ISO compliance, mandatory-variable validation before process launch, and native integrations with AWS Secrets Manager, HashiCorp Vault, and popular frameworks.
