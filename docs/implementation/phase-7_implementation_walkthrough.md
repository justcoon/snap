# Phase 7 Implementation Walkthrough: HTTP Server Snapshot & HTTP Client

## 1. Overview
Phase 7 introduces Snap's network transport capabilities:
1. **Read-only frozen HTTP repository snapshot server (`snap --serve [port]`)** binding strictly to loopback `127.0.0.1`, supporting `GET` and `HEAD` for `/repository.json`, snapshot isolation, and graceful shutdown on `SIGINT`/`SIGTERM`.
2. **Synchronous read-only remote HTTP repository client fetcher** enabling `snap diff <old> <new> --repo <url>` and `snap merge <url>` with strict JSON schema parsing and graph invariant validation.

---

## 2. Key Changes Implemented

### HTTP Subsystem (`rust/src/http/`)
- [`rust/src/http/mod.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/http/mod.rs): Facade exposing `serve_repository` and `fetch_repository`.
- [`rust/src/http/server.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/http/server.rs):
  - In-memory snapshotting via `repo.to_json_pretty()`.
  - TCP binding strictly to `127.0.0.1` on requested or default (8765) port.
  - Startup announcement: `http://127.0.0.1:<port>/repository.json\n` written to stdout and flushed.
  - POSIX signal handling using `libc::sigaction` and `libc::poll` loop monitoring `SHUTDOWN: AtomicBool` for graceful termination (exit 0, no stderr).
  - Exact routing: `GET` and `HEAD` on `/repository.json` return 200 OK (`Content-Type: application/json; charset=utf-8`); other methods return 405 Method Not Allowed (`Allow: GET, HEAD`); other paths or query strings return 404 Not Found.
- [`rust/src/http/client.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/http/client.rs):
  - Uses `url::Url` for robust, WHATWG-compliant URL parsing and scheme/host/port extraction.
  - Synchronous TCP connection to remote HTTP server.
  - Generates HTTP/1.1 `GET` request with `Host` and `Connection: close`.
  - Parses HTTP status; raises `snap: HTTP <status>` on non-200 responses.
  - Decodes chunked transfer encoding or standard payloads.
  - Enforces strict JSON verification (`Repository::from_json_slice`) and full graph validation (`validate_repository`).

### CLI Integration
- [`rust/src/cli/args.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/cli/args.rs):
  - Updated `Command::Serve { port: Option<u16> }`.
  - Strict port parsing (`0..=65535`), returning `ParseError::InvalidPort` when invalid.
- [`rust/src/cli/mod.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/cli/mod.rs):
  - Dispatched `Command::Serve` to `cmd_serve(port)`.
- [`rust/src/cli/commands.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/cli/commands.rs):
  - `cmd_serve(port)` locates repository root, validates repository, and launches `http::serve_repository`.
  - `load_remote_repository(source)` detects `http://` / `https://` and delegates to `http::fetch_repository(source)`.
- [`rust/src/main.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/main.rs):
  - Declared `pub mod http;`.
- [`rust/Cargo.toml`](file:///Users/coon/workspace-zv/git/snap/rust/Cargo.toml):
  - Added direct dependency on `libc = "0.2"`.

---

## 3. Plan vs. Implementation Discrepancy Check
- **Planned Scope:**
  - Embedded read-only HTTP server binding to `127.0.0.1` (`snap --serve [port]`).
  - Frozen snapshot isolation and exact endpoint routing for `/repository.json`.
  - Graceful shutdown on `SIGINT`/`SIGTERM` exiting 0.
  - Read-only HTTP repository fetcher for `merge` and `diff`.
  - Unit and acceptance tests for HTTP server and client.
- **Implemented Scope:**
  - All planned components were implemented in `src/http/server.rs`, `src/http/client.rs`, `src/http/mod.rs`, and CLI layers.
  - Signal handling implemented with `libc::sigaction` and non-blocking `libc::poll` event loop.
  - Chunked transfer encoding decoder included in client.
  - Error formatting for JSON syntax errors and unknown fields refined to align with test expectations.
- **Deviations / Adjustments:**
  - None: Implementation strictly adhered to the approved plan.

---

## 4. Verification Results

### Unit & Property Tests
```
running 41 tests
test cli::args::tests::test_parse_version ... ok
test cli::args::tests::test_parse_init ... ok
test cli::args::tests::test_parse_config ... ok
test cli::args::tests::test_parse_commit ... ok
test cli::args::tests::test_parse_serve ... ok
...
test http::client::tests::test_fetch_repository_success_and_error ... ok
test http::server::tests::test_server_endpoint_routing ... ok
...
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

### Static Analysis & Lints
```bash
cargo check                     # PASS (0 errors)
cargo clippy --all-targets      # PASS (0 warnings)
cargo fmt --check               # PASS (code clean)
```

### Acceptance Test Verification
```bash
./verify --lang rust --filter 12-http-server
# ✓ server exposes one immutable repository snapshot and exits on SIGTERM 1151ms
# 1 passed in 1151ms

./verify --lang rust --filter 13-http-client
# ✓ HTTP merge and diff use one exact validated GET without redirects 1240ms
# 1 passed in 1240ms

./verify --lang rust --filter 26-portability-and-failure-safety
# ✓ local exchange preserves text bytes and malformed remotes never mutate 1468ms
# 1 passed in 1468ms
```
