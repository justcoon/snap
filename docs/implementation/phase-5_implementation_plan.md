# Implementation Plan - Phase 5: Configuration & Core Local Commands

Phase 5 implements configuration management and the core local command suite (`init`, `config`, `status`, `log`, `commit`), connecting the core domain engine and filesystem layer to the CLI runner.

## User Review Required

> [!IMPORTANT]
> - **CLI Grammar & Error Channels:** Commands enforce exact grammar rules; unknown commands, misplaced options, extra positional parameters, or missing required arguments output `snap: invalid command or arguments\n` on stderr and exit with code 1.
> - **Discovery of Repository Root:** Commands requiring a repository traverse from cwd upwards through parent directories looking for `.snap/repository.json`. If no repository is found, stderr outputs `snap: not a Snap repository\n` with exit code 1.
> - **Configuration Precedence:** Local `.snap/config.json` takes precedence over `$HOME/.snapconfig.json`. If neither provides a contributor ID, authoring commands (`commit`, `revert`) abort with `snap: contributor.id is required; configure it locally or globally\n`.
> - **YAML Suite Notice (`tests/03-configuration.yaml`):** In step 12 (`global-fallback`), line 70 contains an apparent typo `text: '{"contributor":{"id":"global@example.com"}}}}'` (four closing braces instead of two). Per `AGENTS.md`, since `SPEC.md §8` defines valid configuration as ordinary JSON and mandates that malformed files are errors, we correct line 70 to `{"contributor":{"id":"global@example.com"}}` so the test functions as intended.

## Proposed Changes

### Configuration Subsystem (`rust/src/config/`)

#### [NEW] [`model.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/config/model.rs)
- Strongly typed `SnapConfig` and `ContributorConfig`:
  ```json
  {"contributor":{"id":"alice@example.com"}}
  ```
- Strict JSON validation (rejection of duplicate keys, unknown fields, and invalid IDs).

#### [NEW] [`mod.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/config/mod.rs)
- `resolve_contributor_id(repo_root: Option<&Path>) -> Result<Option<ContributorId>, ConfigError>`
- `write_config(path: &Path, id: &ContributorId) -> Result<(), ConfigError>`
- `ConfigError` with clear error strings matching SPEC requirements.

---

### CLI & Presentation Subsystem (`rust/src/cli/`)

#### [NEW] [`args.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/cli/args.rs)
- Strict argument grammar parser without loose external frameworks:
  - `init [path]`
  - `config [--global] contributor.id <id>`
  - `status`
  - `log`
  - `commit <message>`
  - `--version`
- Rejects any extra arguments, duplicate flags, or misplaced options.

#### [NEW] [`commands.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/cli/commands.rs)
- `find_repository_root() -> Result<PathBuf, CliError>`: Walks up parent directories looking for `.snap/repository.json`.
- `cmd_init(path: Option<String>) -> Result<(), CliError>`: Creates `.snap/` and initial repository, outputs `()\n`.
- `cmd_config(is_global: bool, key: &str, value: &str) -> Result<(), CliError>`: Validates ID and writes local/global config.
- `cmd_status() -> Result<(), CliError>`: Compares current tree to working tree, printing version and sorted change list.
- `cmd_log() -> Result<(), CliError>`: Prints patches in reverse canonical integration order with escaped message characters (`\\`, `\t`, `\n`).
- `cmd_commit(message: String) -> Result<(), CliError>`: Validates message and contributor, diffs working tree, generates new patch, updates repository atomically, and prints new version.

#### [NEW] [`mod.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/cli/mod.rs)
- CLI dispatch handler and entry point.

---

### Main Entrypoint (`rust/src/main.rs`)

#### [MODIFY] [`main.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/main.rs)
- Wire up `cli::dispatch(std::env::args())` to route commands.
- Map domain errors to exit code 1 with single-line `snap: <error>` formatting on stderr, exit code 0 for success, exit code 2 for unexpected internal panics.

---

## Verification Plan

### Automated Tests
1. **Compilation & Static Checks:**
   ```bash
   cd rust
   cargo check
   cargo clippy --all-targets -- -D warnings
   cargo fmt --check
   ```
2. **In-Process Unit Tests (`cargo test`):**
   - Config model serialization / deserialization tests.
   - CLI argument parsing tests (valid commands and grammar errors).
   - Commit authoring with text and binary files.
   - Status output generation and log message escaping.
3. **Canonical YAML Acceptance Suite:**
   ```bash
   ./verify --lang rust --filter 01-init
   ./verify --lang rust --filter 02-init-paths
   ./verify --lang rust --filter 03-configuration
   ./verify --lang rust --filter 04-commit-status-log
   ./verify --lang rust --filter 14-cli-errors
   ./verify --lang rust --filter 24-cli-grammar-matrix
   ```
4. **Subprocess / Binary Check:**
   ```bash
   ./run --lang rust --version
   ```
