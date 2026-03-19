# Envy

**Eradicate plaintext `.env` files from your workflow.**

Envy is a local-first, encrypted environment variable manager for developers. Instead of scattering secrets across plaintext `.env` files that get accidentally committed, shared over Slack, or left on disk unprotected, Envy stores every secret in an AES-256-GCM encrypted vault on your machine, unlocked by a master key held exclusively in your OS credential store.

When you need your secrets, `envy run` injects them directly into your process's memory. No files written. No exports. No leaks.

## Key Features

- **Encrypted local vault** — Secrets are stored in a SQLCipher-encrypted SQLite database (`~/.envy/vault.db`). Each secret value is individually encrypted with AES-256-GCM before it even reaches the database.
- **OS Keyring integration** — The vault master key lives in your OS credential manager (macOS Keychain, Windows Credential Manager, Linux Secret Service). It never touches the filesystem.
- **Seamless process injection** — `envy run -- npm start` decrypts secrets and injects them as environment variables into the child process. Your application code doesn't change at all.
- **Multi-environment support** — Manage `development`, `staging`, and `production` secrets side-by-side within the same project.
- **Legacy migration** — Import an existing `.env` file with a single command: `envy migrate .env`. Then delete the plaintext file forever.
- **UNIX-friendly** — `envy get KEY` outputs the raw value to stdout with no labels, so it works seamlessly in shell pipelines.
- **Single binary, zero runtime dependencies** — Distributed as one statically-compiled Rust binary. No Node.js, Python, or Docker required.

## Installation

### From source (requires Rust 1.85+)

```bash
# Clone and install
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

## How It Works

```
envy.toml            ~/.envy/vault.db              OS Keyring
(project UUID)  -->  (SQLCipher encrypted DB)  <--  (master key)
                     AES-256-GCM per-secret
```

1. `envy init` creates a lightweight `envy.toml` manifest linking your project directory to a UUID in the vault.
2. Secrets are encrypted with AES-256-GCM using the vault master key, then stored in the encrypted SQLite database.
3. The master key itself is held by your OS credential manager — never written to any file.
4. `envy run` decrypts secrets in-memory and passes them directly to the child process via `std::process::Command::envs()`.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Not found (manifest, secret, file) |
| 2    | Invalid input (key name, assignment format) |
| 3    | Initialisation conflict |
| 4    | Vault / crypto failure |
| 127  | Child binary not found (`envy run`) |
| N    | Child process exit code (proxied by `envy run`) |

## Roadmap

Envy is currently in **Phase 1** (single-developer local MVP). Here's what's next:

**Phase 2 — Collaboration & CI/CD**: Encrypted export files (`envy.enc`) safe to commit to Git, headless CI/CD mode via a single `ENVY_MASTER_KEY` environment variable, and mandatory-variable validation before process launch.

**Phase 3 — Enterprise**: Role-based access control (RBAC), audit logging for SOC2/ISO compliance, and native integrations with AWS Secrets Manager, HashiCorp Vault, and popular frameworks.

