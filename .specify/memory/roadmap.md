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
- **Secure Export/Import:** Commands to export an encrypted environment payload (`envy export`) to share securely via Slack/email.
- **GitOps Approach (V1):** Generate repository-specific encrypted files (e.g., `envy.enc`) that *are* safe to commit to Git.
- **Headless CI/CD Support:** Allow `run` to execute in GitHub Actions/GitLab CI by reading a single master key (`ENVY_MASTER_KEY`) from the runner's environment to decrypt the rest on the fly.
- **Strict Validation:** The CLI must fail-fast before executing the child app if it detects missing mandatory variables for the selected environment.
- **Project Sync Key:** Establish a single-token architecture. Teams share one master key via their password manager (e.g., Bitwarden) to seamlessly decrypt the `envy.enc` file upon `git pull` via the `envy import` command.

### Phase 3: The Industry Standard (Years 2 - 3)
*Goal: Massive adoption, enterprise support, and mature ecosystem.*

**Key Milestones:**
- **Access Control (RBAC):** Granular permissions for large teams (e.g., Devs only read `staging`, DevOps read `production`).
- **Audit & Logs:** Immutable record of secret access for SOC2/ISO compliance.
- **Cloud Sync (Optional but powerful):** Native plugins to sync with AWS Secrets Manager, HashiCorp Vault, Vercel Envs, acting as a unified interface.
- **First-Class Integrations:** Native support or official guides in popular frameworks (Next.js, Vite, NestJS) and infra tools (Terraform, Kubernetes).

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

---

## 🧱 Definitive Technology Stack
- **Language:** **Rust**. Chosen for memory safety, sub-millisecond startup times (crucial for a wrapper), and single-binary distribution.
- **CLI Framework:** `clap` (derive API).
- **Local Storage (The Vault):** **SQLite Encrypted (SQLCipher)** integrated via the `rusqlite` crate (using the `bundled-sqlcipher` feature). Ensures ACID transactions, fast lookups, and relational structure for future RBAC/Audit logs.
- **Master Key Management:** The `keyring` crate to natively interact with macOS Keychain / Windows Credential Manager / Linux Secret Service.
- **Architecture:** Strictly adhere to the 4-layer modularity defined in the Constitution (UI/CLI -> Core/Business Logic -> Cryptography / Database).
- **Cryptography (Defense in Depth):** `RustCrypto` ecosystem (AES-256-GCM). Secrets are individually encrypted per row inside the database, ensuring zero plaintext exposure even if the database file is compromised.
