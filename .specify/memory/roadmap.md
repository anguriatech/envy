# 🚀 Envy: Product Roadmap & North Star

**Mission:** Eradicate plaintext `.env` files from software development by establishing a new secure, centralized, and deterministic standard.

## 🌟 The Definitive Vision (North Star)
Create the ultimate tool for environment variable and secret management. It must become as fundamental to a developer's workflow as Git. The transition between local development, Continuous Integration (CI/CD), and production must be seamless, secure by default, and absolutely frictionless.

### Core Tenets
1. **Security by Default:** Zero plaintext files on the project's hard drive.
2. **Determinism:** 100% predictable and reproducible behavior across any machine.
3. **Ergonomics (DX):** Must be faster and more intuitive to use than manually creating a `.env` file.
4. **Zero Dependencies:** Distributed as a single, statically linked compiled binary (Rust). No Node.js, Python, or Docker required to run it.

---

## 🗺️ Product Roadmap

### Phase 1: The "Single-Player" MVP (Months 1 - 3)
*Goal: Make an individual developer prefer Envy on their local machine over a traditional `.env` file.*

**Key Milestones:**
- **Core Architecture:** Define the encrypted local storage Vault (`~/.envy/vault.db`).
- **Robust Encryption:** Implement native industry-standard encryption (`rusqlite` with `bundled-sqlcipher` for AES-256).
- **Master Key Integration:** Securely fetch the master vault key from the OS Credential Manager using the `keyring` crate.
- **Context Resolution:** Create a lightweight `envy.toml` manifest file upon initialization to link a local directory to its Vault `project_id`.
- **Basic CRUD Management:** Commands to initialize projects and manage variables (`init`, `set`, `get`, `rm`, `list`).
- **The Magic Wrapper:** Implement the `run` command to inject variables directly into memory and spawn child processes (e.g., `envy run -- npm run dev`).
- **Multi-Environment Support:** Manage basic contexts (`development`, `staging`, `production`) within the same local project.

### Phase 2: Collaboration & CI/CD (Months 6 - 12)
*Goal: Scale from an individual to a small team, solving secret sharing and automated deployments.*

**Key Milestones:**
- **GitOps Approach (V1):** Generate repository-specific encrypted artifacts (`envy.enc`) that *are* safe to commit to Git. The artifact is the source of truth for team collaboration.
- **`envy encrypt` / `envy enc`:** Seals the local vault into an `envy.enc` artifact. All environments are packed into a single encrypted bundle, ready to be committed to the repository.
- **`envy decrypt` / `envy dec`:** Unseals an `envy.enc` artifact back into the local vault. This is the command developers run after `git pull` to sync secrets from the repository.
- **Progressive Disclosure (Environment-Level Keys):** The encryption model supports two operational modes without changing the user-facing commands:
  - *Startup Mode (Default):* A single shared team key encrypts and decrypts all environments at once. Zero friction for small teams — share one key via your password manager (e.g., Bitwarden) and everyone is fully synced.
  - *Multi-key Mode (Optional):* Individual environments (e.g., `production`) can be locked with separate keys. When a developer runs `envy decrypt` with the dev key, Envy imports `development` and `staging` and **gracefully skips** `production` without throwing an error. Least-privilege access enforced by default.
- **Headless CI/CD Support:** Allow `run` to execute in GitHub Actions/GitLab CI by reading a key (`ENVY_MASTER_KEY`) from the runner's secret store to decrypt the `envy.enc` artifact on the fly.
- **Strict Validation:** The CLI must fail-fast before executing the child app if it detects missing mandatory variables for the selected environment.

### Phase 3: Ecosystem & GUI (Years 2 - 3)
*Goal: Bring Envy to non-terminal users and expand the ecosystem.*

**Key Milestones:**
- **Official VS Code Extension:** Visual secret management — browse environments, set/get/delete secrets, and run commands without touching the terminal.
- **First-Class Integrations:** Native support or official guides in popular frameworks (Next.js, Vite, NestJS) and infra tools (Terraform, Kubernetes).
- **Cloud Sync (Optional):** Native plugins to sync with popular secret backends (Vercel Envs, Railway), acting as a unified local interface.

---

## 🛠️ Interface Design (MVP CLI Commands)

To guarantee the best Developer Experience (DX), syntax must be intuitive.

### 1. Initialization
```bash
# Initializes Envy in the current directory.
# Creates a lightweight manifest file (no secrets) named `envy.toml` containing the project UUID.
envy init
```

### 2. Variable Management (CRUD)
```bash
# Adds or updates a variable in the default environment (development)
envy set STRIPE_KEY=sk_test_12345

# Adds a variable to a specific environment (short flag: -e)
envy set DATABASE_URL=postgres://prod-db -e production

# Lists all variables for the current environment (alias: ls)
envy list
envy ls                              # short alias

# Lists variables for a specific environment
envy ls -e staging

# Displays the exact, decrypted value of a variable
envy get STRIPE_KEY
envy get STRIPE_KEY -e production    # from a specific environment

# Deletes a variable (alias: remove)
envy rm STRIPE_KEY
envy remove STRIPE_KEY               # long alias

# Migrates an existing plaintext .env file into the encrypted vault
envy migrate .env
envy migrate .env -e staging         # migrate into a specific environment
```

### 3. Execution (The Wrapper)
```bash
# Injects the 'development' variables into memory and spawns the child process
envy run -- npm run dev

# Injects the 'staging' variables into memory and spawns the child process (short flag: -e)
envy run -e staging -- python main.py

# Works with any command after the -- separator
envy run -e production -- ./server --port 8080
```

### 4. GitOps & Synchronization (Phase 2 Preview)
```bash
# Seals the entire local vault into an encrypted artifact safe to commit to Git.
# All environments are packed into envy.enc with the shared team key.
envy encrypt
envy enc                             # short alias

# Seal only a specific environment into the artifact
envy enc -e staging

# Unseals envy.enc back into the local vault after a git pull.
# Startup Mode: one key decrypts all environments at once.
envy decrypt
envy dec                             # short alias

# Multi-key mode: run with the dev passphrase — imports development/staging,
# gracefully skips production (locked with a different passphrase, no error thrown).
ENVY_PASSPHRASE=$DEV_TEAM_KEY envy dec

# Headless CI/CD: decrypt in a GitHub Actions runner using a repo secret
ENVY_MASTER_KEY=${{ secrets.ENVY_KEY }} envy dec && envy run -e production -- ./deploy.sh
```

---

## 🧱 Definitive Technology Stack
- **Language:** **Rust**. Chosen for memory safety, sub-millisecond startup times (crucial for a wrapper), and single-binary distribution.
- **CLI Framework:** `clap` (derive API).
- **Local Storage (The Vault):** **SQLite Encrypted (SQLCipher)** integrated via the `rusqlite` crate (using the `bundled-sqlcipher` feature). Ensures ACID transactions, fast lookups, and relational structure for future extensibility.
- **Master Key Management:** The `keyring` crate to natively interact with macOS Keychain / Windows Credential Manager / Linux Secret Service.
- **Architecture:** Strictly adhere to the 4-layer modularity defined in the Constitution (UI/CLI -> Core/Business Logic -> Cryptography / Database).
- **Cryptography (Defense in Depth):** `RustCrypto` ecosystem (AES-256-GCM). Secrets are individually encrypted per row inside the database, ensuring zero plaintext exposure even if the database file is compromised.
