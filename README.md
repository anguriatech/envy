# Envy

**Eradicate plaintext `.env` files from your workflow.**

Envy is a local-first, encrypted environment variable manager for developers. Instead of scattering secrets across plaintext `.env` files that get accidentally committed, shared over Slack, or left on disk unprotected, Envy stores every secret in an AES-256-GCM encrypted vault on your machine, unlocked by a master key held exclusively in your OS credential store.

When you need your secrets, `envy run` injects them directly into your process's memory — no files written, no exports, no leaks. When you need to share them with your team, `envy encrypt` seals them into a single committed file that only the right passphrase can open.

## Key Features

- **Encrypted local vault** — Secrets are stored in a SQLCipher-encrypted SQLite database (`~/.envy/vault.db`). Each secret value is individually encrypted with AES-256-GCM before it even reaches the database.
- **OS Keyring integration** — The vault master key lives in your OS credential manager (macOS Keychain, Windows Credential Manager, Linux Secret Service). It never touches the filesystem.
- **Seamless process injection** — `envy run -- npm start` decrypts secrets and injects them as environment variables into the child process. Your application code doesn't change at all.
- **Multi-environment support** — Manage `development`, `staging`, and `production` secrets side-by-side within the same project.
- **GitOps team sync** — `envy encrypt` seals your entire vault into a single `envy.enc` artifact you can safely commit to Git. `envy decrypt` restores secrets after a pull. One passphrase, one file, zero plaintext.
- **Progressive Disclosure** — Seal each environment with a different passphrase. A developer with the dev key imports `development` and sees `production` listed as gracefully skipped — no error, no alarm, no support ticket.
- **CI/CD headless mode** — Set `ENVY_PASSPHRASE` in your pipeline secrets. `envy decrypt` detects it and skips the interactive prompt entirely, with no code changes needed.
- **Legacy migration** — Import an existing `.env` file with a single command: `envy migrate .env`. Then delete the plaintext file forever.
- **UNIX-friendly** — `envy get KEY` outputs the raw value to stdout with no labels, so it works seamlessly in shell pipelines.
- **Single binary, zero runtime dependencies** — Distributed as one statically-compiled Rust binary. No Node.js, Python, or Docker required.

## Installation

Envy ships as a native, statically-compiled binary with zero runtime dependencies — no Node.js, Python, or Docker required. The installer auto-detects your CPU architecture and fetches the optimised binary for your machine (Intel, Apple Silicon, ARM64 Linux, or x64 Windows).

### macOS & Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/anguriatech/envy/releases/latest/download/envy-installer.sh | sh
```

The installer places the binary in your Cargo bin directory and configures your `PATH` automatically. Open a new terminal and `envy` is ready.

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

## CLI Command Reference

Every command at a glance.

| Command | Alias | Description | Example |
|---------|-------|-------------|---------|
| `envy init` | — | Create `envy.toml` in the current directory, linking it to a vault UUID | `envy init` |
| `envy set KEY=VALUE` | — | Store a secret in the vault (default env: `development`) | `envy set API_KEY=sk_live_abc123` |
| `envy set KEY=VALUE -e ENV` | — | Store a secret in a specific environment | `envy set DB_URL=postgres://prod -e production` |
| `envy get KEY` | — | Print a single decrypted value to stdout (no label) | `envy get API_KEY` |
| `envy get KEY -e ENV` | — | Print a value from a specific environment | `envy get DB_URL -e production` |
| `envy list` | `envy ls` | List all key names in the vault (never values) | `envy list` |
| `envy list -e ENV` | `envy ls -e ENV` | List key names for a specific environment | `envy ls -e staging` |
| `envy rm KEY` | `envy remove KEY` | Delete a secret from the vault | `envy rm API_KEY` |
| `envy run -- CMD` | — | Inject secrets into a child process and run it | `envy run -- npm run dev` |
| `envy run -e ENV -- CMD` | — | Inject secrets from a specific environment | `envy run -e production -- ./deploy.sh` |
| `envy migrate FILE` | — | Import all `KEY=VALUE` pairs from a plaintext file | `envy migrate .env` |
| `envy migrate FILE -e ENV` | — | Import into a specific environment | `envy migrate .env.staging -e staging` |
| `envy encrypt` | `envy enc` | Seal all environments into `envy.enc` | `envy encrypt` |
| `envy encrypt -e ENV` | `envy enc -e ENV` | Seal a single environment into `envy.enc` | `envy enc -e production` |
| `envy decrypt` | `envy dec` | Unseal `envy.enc` and restore secrets to the local vault | `envy decrypt` |

---

## Workflow 1: Local Development (Single-Player)

The core loop for an individual developer. After installation, `envy` is on your `PATH` and ready immediately — no shell restart needed.

```bash
# 1. Initialise Envy in your project directory
cd my-project
envy init
# => Creates envy.toml (safe to commit — contains only a project UUID)

# 2. Store your secrets
envy set DATABASE_URL=postgres://localhost/myapp
envy set API_KEY=sk_live_abc123
envy set STRIPE_KEY=sk_live_xyz789

# 3. Run your app with secrets injected into the process
envy run -- npm run dev
# => DATABASE_URL, API_KEY, and STRIPE_KEY are available as env vars in the child process.
# => Nothing is written to disk. No exports. No shell history leakage.

# 4. Inspect your vault
envy list               # key names only — values are never printed
envy get DATABASE_URL   # decrypt and print a single value when you need it

# 5. Manage multiple environments side-by-side
envy set DATABASE_URL=postgres://staging-host/myapp -e staging
envy set DATABASE_URL=postgres://prod-host/myapp -e production

envy run -e staging -- python manage.py migrate
envy run -e production -- ./scripts/deploy.sh

envy list -e production   # inspect production keys without touching staging
envy rm OLD_KEY -e staging
```

> **Note:** `development` is the default environment. All commands accept `-e ENV` to target a different one.

---

## Workflow 2: Legacy Migration

Already have a `.env` file? Migrate it to the vault in one step.

```bash
# Import every KEY=VALUE line from .env into the development vault
envy migrate .env

# Import staging secrets from a separate file
envy migrate .env.staging -e staging

# Verify the import
envy list
envy list -e staging

# Delete the plaintext files — they are no longer needed
rm .env .env.staging

# Add .env* to .gitignore as a permanent safeguard
echo '.env*' >> .gitignore
```

Blank lines and `#` comments in the source file are ignored. Existing keys are overwritten (upsert semantics), so re-running the command is safe.

---

## Workflow 3: Team Sync (Multi-Player via Git)

Share secrets with your team through Git — zero plaintext ever committed.

### Sealing the vault (Lead / secret owner)

```bash
envy encrypt
# Enter passphrase: ········
# Confirm passphrase: ········

# Sealed 2 environment(s) → envy.enc
#   ✓  development   (3 secrets)
#   ✓  production    (1 secret)

git add envy.enc envy.toml
git commit -m "chore: update encrypted secrets"
git push
```

`envy.enc` is pure ciphertext — no key names, no values, no project identifiers. It is safe to commit to a public repository.

### Restoring secrets (Teammate, after a pull)

```bash
git pull
envy decrypt
# Enter passphrase: ········

# Imported 2 environment(s) from envy.enc
#   ✓  development   (3 secrets upserted)
#   ✓  production    (1 secret upserted)
```

That's it. The teammate's local vault is now fully in sync. Use `envy run` as normal.

### Sealing a single environment

```bash
envy enc -e staging    # seals only staging into envy.enc
```

Use `envy enc` and `envy dec` as short aliases for both commands.

---

## Workflow 4: Progressive Disclosure (Environment Passwords)

Seal different environments with different passphrases to enforce least-privilege access without any extra tooling.

### Lead: seal each environment independently

```bash
# Seal development with the shared dev passphrase
envy enc -e development
# Enter passphrase: ········ (dev-team-key)
# Confirm passphrase: ········

# Seal production with the restricted prod passphrase
envy enc -e production
# Enter passphrase: ········ (prod-only-key)
# Confirm passphrase: ········

git add envy.enc
git commit -m "chore: rotate secrets"
git push
```

Both environments are packed into the same `envy.enc` file, each sealed independently.

### Junior developer: decrypt with the dev key

```bash
git pull
envy decrypt
# Enter passphrase: ········   (dev-team-key — doesn't know the prod key)

# Imported 1 environment(s) from envy.enc
#   ✓  development   (3 secrets upserted)
#   ⚠  production    skipped — different passphrase or key
```

The `⚠` line is purely informational. The exit code is `0`. The production contents of the local vault are untouched. The junior developer never sees an error, never files a support ticket, and cannot accidentally import production secrets they shouldn't have.

### Senior developer or DevOps: decrypt with the prod key

```bash
envy decrypt
# Enter passphrase: ········   (prod-only-key)

# Imported 1 environment(s) from envy.enc
#   ✓  production    (1 secret upserted)
#   ⚠  development   skipped — different passphrase or key
```

Run `envy decrypt` twice (once per passphrase) to import all environments in a single checkout.

---

## Workflow 5: CI/CD Pipeline (Headless)

For automated pipelines, set `ENVY_PASSPHRASE` as a secret in your CI/CD provider. Envy checks for this variable before showing any terminal prompt, so the entire decrypt flow runs silently.

> **Security note:** `ENVY_PASSPHRASE` is the shared artifact passphrase — distinct from the vault master key, which is managed by the OS keyring and never set via environment variable. Whitespace-only values are treated as unset and cause the command to fail cleanly rather than decrypt with an empty key.

### GitHub Actions

Use the same installer one-liner to pull the latest pre-built binary — no Rust toolchain required on the runner, and installation takes seconds instead of minutes.

```yaml
# .github/workflows/deploy.yml
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install envy
        run: curl --proto '=https' --tlsv1.2 -LsSf https://github.com/anguriatech/envy/releases/latest/download/envy-installer.sh | sh

      - name: Decrypt secrets
        env:
          ENVY_PASSPHRASE: ${{ secrets.ENVY_PASSPHRASE }}
        run: envy decrypt

      - name: Deploy
        run: envy run -e production -- ./scripts/deploy.sh
```

### Docker

```dockerfile
# Install envy during image build — the installer detects the container's architecture automatically
RUN curl --proto '=https' --tlsv1.2 -LsSf https://github.com/anguriatech/envy/releases/latest/download/envy-installer.sh | sh

# At runtime, pass the passphrase via a secret and decrypt
ENV ENVY_PASSPHRASE=""
RUN envy decrypt && envy run -- node server.js
```

### Vercel / Railway / any platform with secret injection

```bash
# Set in your platform's secret manager:
ENVY_PASSPHRASE=your-shared-team-passphrase

# In your build script:
envy decrypt
envy run -- node server.js
```

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

**Phase 3 — Ecosystem & GUI**: Bringing Envy to non-terminal users, starting with an official VS Code Extension to make secret management visual and seamless.