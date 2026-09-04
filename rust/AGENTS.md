# Snap — Rust attendee scaffold

Implement the contract in the packaged [`SPEC.md`](../SPEC.md); the language-neutral public
YAML suite under [`tests/`](../tests/) defines the acceptance criteria. Follow the architectural
roadmap in [`plan.md`](../plan.md) and concrete test specifications in [`test-scenarios.md`](../test-scenarios.md).

Use idiomatic, safe Rust (2021 edition). Avoid `unsafe` code and avoid unhandled `unwrap()` / `expect()` calls in fallible domain and CLI paths.

## Setup, build, run, and test

From within the `rust/` directory:

```bash
cargo check                        # fast compilation check
cargo clippy --all-targets         # lint check (including tests)
cargo fmt                          # format code (or cargo fmt --check)
cargo build                        # build debug binary (target/debug/snap)
cargo run -- <arguments>           # run the CLI directly
cargo test                         # run in-memory unit, integration, and property tests
```

From the repository root, run the packaged language-neutral acceptance suite:

```bash
./verify --lang rust
```

To run a specific test case or filter:

```bash
./verify --lang rust --filter 01-init
./verify --lang rust --filter ot-matrix --verbose
```

## Implementation layout & discipline

Keep responsibilities strictly partitioned across modules as outlined in [`plan.md`](../plan.md):

- **Pure Domain (`src/core/`):** Version algebra, canonical token diff, operational transformation (OT), deterministic replay, and repository graph validation. Must be deterministic and independent of filesystem or network I/O.
- **Filesystem & State (`src/fs/`, `src/config/`):** Path normalization, segment prefix-freedom, working-tree scanner (clean/dirty detection, symlink rejection), atomic file materialization, and configuration resolution.
- **HTTP Mode (`src/http/`):** Synchronous, read-only snapshot server on `127.0.0.1` (`GET`/`HEAD` on `/repository.json`) and synchronous remote repository fetcher.
- **CLI & Presentation (`src/cli/`, `src/presentation/`):** Strict positional argument parsing (rejecting misplaced flags or unknown options) and dual-mode formatting (byte-stable plain output vs ANSI SGR terminal styling).

## Anti-Patterns & Idiomatic Rust Guidelines

Avoid common anti-patterns across all modules:

| Anti-Pattern | Why It's Bad | Idiomatic Correction |
| :--- | :--- | :--- |
| **Excessive `.unwrap()` / `.expect()`** | Causes runtime crashes (panics) on unexpected errors. | Propagate errors gracefully using the `?` operator. |
| **Aggressive `.clone()`** | Generates hidden, expensive heap allocations to bypass the borrow checker. | Pass data by reference (`&T`) or slice (`&str`, `&[T]`). |
| **"Stringly-Typed" Everything** | Uses `String` for structured data, inviting invalid states and validation bugs. | Enforce structural validity with strictly typed `enum` or `struct` definitions. |
| **Scattered Magic Literals & Raw Paths** | Creates maintenance friction and risk of subtle typos across modules. | Centralize in canonical constants (`REPOSITORY_FORMAT_VERSION`, `CONTRIBUTOR_ID_KEY`) and path helper functions (`repo_file_path()`, `local_config_path()`). |
| **Ad-Hoc Default Arguments & Loose Options** | Passing ad-hoc optional flags or scattered default constants complicates configuration. | Encapsulate options in a dedicated config struct (e.g., `HttpServerConfig`, `HttpClientConfig`) implementing `Default`. |
| **Monolithic Command Handlers** | Large files containing all command implementations increase coupling and hinder isolation. | Split commands into dedicated sub-modules under `src/cli/commands/` with uniform `cmd_<name>` handlers. |
| **C-Style Index Loops** | Triggers runtime bounds checking on every single iteration. | Use high-performance iterators and `.enumerate()`. |
| **Holding Mutexes Across `.await`** | Freezes asynchronous tasks, leading to runtime deadlocks. | Drop the lock guard explicitly or use block scopes before `.await`. |

## Scope discipline

Snap’s surface is deliberately compact. Do not introduce branches, staging areas, checkout, push, authentication, background daemons, or unresolved conflict markers. Focus complexity entirely on deterministic behavior, exact validation, and passing both the internal test suite and the shared acceptance harness.

