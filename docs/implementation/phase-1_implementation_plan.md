# Phase 1: Cargo Workspace Initialization & Version Algebra

Phase 1 establishes the Rust codebase foundation, sets up Cargo configuration and core dependencies, and implements the fundamental version algebra subsystem (`ContributorId`, `Revision`, `Version`, `CausalRelation`, `join`, and Snap total ordering) with strict validation and exhaustive Level 1 test coverage.

## Objectives & Invariants

- Dependencies added: `serde` (with derive), `serde_json`, `base64`, `is-terminal`, `httparse`, and `proptest` (dev-dependency).
- `Ord` and `PartialOrd` on `Version`: `Version` implements `Ord` using **Snap total order** (which is a total order that agrees with `PartialEq`). For partial causal order, explicit methods (`causal_cmp`, `is_before`, `is_after`, `is_concurrent`) are provided to prevent conflation between causal precedence and Snap total ordering.

## Proposed Changes

### Cargo & Project Configuration
- **`Cargo.toml`**:
  - Define binary package `snap` (Rust 2021 edition).
  - Declare dependencies:
    - `serde = { version = "1.0", features = ["derive"] }`
    - `serde_json = "1.0"`
    - `base64 = "0.22"`
    - `is-terminal = "0.4"`
    - `httparse = "1.9"`
  - Declare dev-dependencies:
    - `proptest = "1.5"`
- **`src/main.rs`**:
  - Provide initial binary entrypoint skeleton matching `snap` CLI requirements (`--version`).

### Core Domain Subsystem
- **`src/core/mod.rs`**:
  - Export `version` module and re-export public types (`ContributorId`, `Version`, `CausalRelation`, `MAX_REVISION`, errors).
- **`src/core/version.rs`**:
  - **`ContributorId`**: Enforce all SPEC.md §3.1 requirements (pure ASCII, single `@`, non-empty parts, no control chars, whitespace, `,`, `(`, `)`, or `->`, at most 254 bytes, exact casing preserved).
  - **`Revision`**: Positive integer range `1..=9007199254740991` (`MAX_SAFE_INTEGER`), rejecting leading zeroes, negative signs, non-digits, and overflow.
  - **`Version`**: Vector clock backed by `BTreeMap<ContributorId, u64>`, canonical string syntax `()`, JSON `[["author", rev], ...]`, 4-way causal comparison (`Equal`, `Before`, `After`, `Concurrent`), element-wise join, and Snap total order.

### Tests
- **Unit Tests**: Scenarios A.1, A.2, A.3, A.4 from `test-scenarios.md`.
- **Property-based Tests**: Algebraic causal order axioms, Snap order totality, consistency between causal and Snap order, and join properties.
