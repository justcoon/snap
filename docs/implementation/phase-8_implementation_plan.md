# Phase 8 Implementation Plan: Terminal Presentation Engine & Color Negotiation

## 1. Objectives & Scope
Implement Phase 8 of the Snap implementation roadmap according to `plan.md` (§7 Phase 8), `SPEC.md` (§7.10, §7.11), and `test-scenarios.md` (Domain I).

Phase 8 introduces the terminal presentation engine, bringing human-readable ANSI styling to Snap while strictly preserving deterministic, byte-stable plain output:
1. **Presentation Mode Negotiation**:
   - Inspects `SNAP_COLOR` and `NO_COLOR` environment variables along with TTY status of stdout and stderr (`is-terminal`).
   - `SNAP_COLOR`:
     - Unset or `auto`: Terminal mode independently on stdout or stderr when that stream is a TTY, unless `NO_COLOR` is present (even if empty).
     - `always`: Terminal mode on both streams unconditionally (overrides redirection and `NO_COLOR`).
     - `never`: Plain mode on both streams unconditionally.
     - Any other value: Immediately abort before command execution with plain error `snap: SNAP_COLOR must be auto, always, or never\n` on stderr and exit code 1.
2. **Exact ANSI SGR Formatting**:
   - Helper `S(n, text)` -> `\x1b[<n>m<text>\x1b[0m` for styles: bold `1`, dim `2`, red `31`, green `32`, yellow `33`, magenta `35`, cyan `36`.
   - Successful actions:
     - `init`: `S(32,"✓") + " " + S(1,"Initialized repository") + " " + S(36,version) + "\n"`
     - `commit`: `S(32,"✓") + " " + S(1,"Committed") + " " + S(36,version) + "\n"`
     - `revert`: `S(32,"✓") + " " + S(1,"Reverted") + " " + S(36,version) + "\n"`
     - `merge`: `S(32,"✓") + " " + S(1,"Merged") + " " + S(36,version) + "\n"`
   - `status`:
     - Header: `S(1,"Snap status") + "  " + S(36,version) + "\n\n"`
     - Clean: `"  " + S(32,"✓") + " Working tree clean\n"`
     - Dirty rows: `"  " + S(color,symbol) + " " + path + " " + S(2,"(" + label + ")") + "\n"`
       - Added: `(32, "+", "added")`
       - Deleted: `(31, "−", "deleted")` (using unicode minus `\u{2212}`)
       - Modified: `(33, "~", "modified")`
   - `log`:
     - Per entry: `S(36,"●") + " " + S(1,message) + "\n  " + S(36,version) + " " + S(2,"by") + " " + S(35,author) + "\n"`
     - Double newline between entries.
   - `diff`:
     - Wraps matching lines excluding LF:
       - `--- ` or `+++ `: bold `1`
       - `@@ `: cyan `36`
       - `-`: red `31`
       - `+`: green `32`
       - `\ `: dim `2`
       - `Binary files `: yellow `33`
   - `--version`:
     - `S(1,"snap 1.0.0") + "\n"` (update package version in `Cargo.toml` to `1.0.0` to match golden).
   - Warnings and Errors:
     - Plain warning `warning: <detail>` -> `S(33,"⚠") + " " + S(33,"<detail>") + "\n"`.
     - Plain error `<error>` -> `S(31,"✗ " + <error>) + "\n"`.
   - Invariants:
     - `config` remains completely silent.
     - `--serve` startup URL remains plain.

---

## 2. Technical Architecture & Module Layout

### A. Presentation Subsystem (`rust/src/presentation/`)
- `rust/src/presentation/mod.rs`:
  - Facade exposing `PresentationMode`, `Negotiator`, and presentation formatting utilities.
- `rust/src/presentation/ansi.rs`:
  - `S(code: u8, text: &str) -> String` producing `\x1b[{code}m{text}\x1b[0m`.
  - Constants for symbols: `CHECK = "✓"`, `CIRCLE = "●"`, `MINUS = "−"`, `PLUS = "+"`, `TILDE = "~"`, `WARNING = "⚠"`, `CROSS = "✗"`.
- `rust/src/presentation/formatters.rs`:
  - Formatters for:
    - Success messages (`format_success(label, version, mode)`)
    - Status output (`format_status(version, entries, mode)`)
    - Log output (`format_log(entries, mode)`)
    - Diff output (`format_diff(plain_diff, mode)`)
    - Warning line (`format_warning(detail, mode)`)
    - Error line (`format_error(err, mode)`)

### B. CLI & Main Integration
- `rust/src/main.rs`:
  - Negotiate presentation before running command dispatch.
  - Reject invalid `SNAP_COLOR` immediately with exit code 1.
  - Format errors using presentation mode negotiated for stderr.
- `rust/src/cli/commands.rs`:
  - Pass `PresentationMode` to command handlers or use printer helpers for stdout and stderr.
- `rust/Cargo.toml`:
  - Bump package version from `0.1.0` to `1.0.0`.

---

## 3. Strict Quality & Anti-Pattern Compliance
- **Determinism:** Plain mode output must remain byte-for-byte identical to previous phases.
- **Zero Panics:** All env var checks and string formatting operations must avoid panics.
- **Spec Adherence:** Spaces and formatting match SPEC.md §7.11 and golden tests in `28-terminal-presentation.yaml`.

---

## 4. Verification Plan
- Unit tests in `rust/src/presentation/` for negotiator and formatters.
- Acceptance suite:
  ```bash
  ./verify --lang rust --filter 28-terminal-presentation
  ```
- Full acceptance suite:
  ```bash
  ./verify --lang rust
  ```
