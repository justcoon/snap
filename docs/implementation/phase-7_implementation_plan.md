# Phase 7 Implementation Plan: HTTP Server Snapshot & HTTP Client

## 1. Objectives & Scope
Implement Phase 7 of the Snap implementation roadmap according to `plan.md` (§7 Phase 7), `SPEC.md` (§7.9, §9), and `test-scenarios.md` (Domain J).

Phase 7 implements Snap's network layer:
1. **Read-Only Snapshot HTTP Server (`snap --serve [port]`)**:
   - Locates and strictly validates repository snapshot at startup.
   - Preserves snapshot isolation in-memory: subsequent disk mutations to `.snap/repository.json` do not affect served content.
   - Binds strictly to IPv4 loopback `127.0.0.1`.
   - Port defaults to `8765`, while `0` requests an ephemeral OS-assigned port.
   - Prints `http://127.0.0.1:<port>/repository.json\n` to stdout and flushes immediately.
   - Serves exact endpoint routing:
     - `GET /repository.json` -> `200 OK` with `Content-Type: application/json; charset=utf-8` and formatted repository JSON.
     - `HEAD /repository.json` -> `200 OK` with identical headers and zero-length body.
     - `POST` or other methods -> `405 Method Not Allowed` with `Allow: GET, HEAD`.
     - Unknown paths or paths with query strings (e.g. `/repository.json?query=1`, `/invalid`) -> `404 Not Found`.
   - Implements graceful shutdown on `SIGINT` or `SIGTERM`, terminating with exit code 0 and producing no stderr.
2. **Read-Only HTTP Client Resolver**:
   - Recognizes repository operands starting with `http://` or `https://` in `snap merge <repo>` and `snap diff <old> <new> --repo <repo>`.
   - Executes a single synchronous HTTP GET to the exact specified URL.
   - Requires HTTP status `200 OK`; reports HTTP status errors (e.g., `snap: HTTP 302`) without following redirects.
   - Strictly parses and validates the response body as a Snap repository JSON value (`validate_json_strict`, `format: 1`, typed patch validation, and full graph integrity).
   - Rejects malformed JSON with `snap: invalid JSON: ...`.

---

## 2. Technical Architecture & Module Layout

### A. HTTP Subsystem (`rust/src/http/`)
Create a new module `rust/src/http/`:
- `rust/src/http/mod.rs`: Facade re-exporting `serve_repository` and `fetch_repository`.
- `rust/src/http/server.rs`:
  - Uses `std::net::TcpListener` bound to `127.0.0.1:<port>`.
  - Captures frozen `Repository` and its formatted JSON byte buffer at startup.
  - Registers `libc::sigaction` handler for `SIGINT` and `SIGTERM` setting an `AtomicBool` flag `SHUTDOWN`.
  - Event loop uses non-blocking listener with `libc::poll` (or 100ms timeout) checking `SHUTDOWN` flag to ensure immediate response to signals.
  - Parses HTTP requests with `httparse::Request`:
    - Validates method and target path `/repository.json`.
    - Responds with `200`, `404`, or `405` with required headers (`Content-Type`, `Content-Length`, `Allow`, `Connection: close`).
- `rust/src/http/client.rs`:
  - `fetch_repository(url: &str) -> Result<Repository, CliError>`:
    - Parses URL host, port (default 80), and path/query.
    - Connects via `std::net::TcpStream` with timeout.
    - Sends synchronous HTTP/1.1 `GET <path> HTTP/1.1\r\nHost: <host>\r\nConnection: close\r\n\r\n`.
    - Reads response to EOF and parses with `httparse::Response`.
    - Checks HTTP status == 200; on non-200 returns `CliError::Custom(format!("HTTP {status}"))`.
    - Decodes transfer payload (including chunked de-chunking if needed).
    - Deserializes via `Repository::from_json_slice(body)` and validates via `validate_repository(&repo)`.

### B. CLI Grammar & Error Alignment (`rust/src/cli/args.rs` & `rust/src/cli/commands.rs`)
- `rust/src/cli/args.rs`:
  - Update `Command::Serve`:
    ```rust
    Command::Serve { port: Option<u16> }
    ```
  - Parse `--serve`:
    - 0 extra args: `Ok(Command::Serve { port: None })` (defaults to 8765).
    - 1 extra arg: Parse unsigned integer. If purely digits and `<= 65535`, return `Ok(Command::Serve { port: Some(p as u16) })`.
    - If port string is non-numeric or `> 65535`, return `ParseError::InvalidPort(port_str.clone())`.
    - If more than 1 extra arg, return `ParseError::InvalidCommandOrArguments`.
- `rust/src/cli/mod.rs`:
  - Handle `Command::Serve { port }` by calling `cmd_serve(port)`.
  - Map `ParseError::InvalidPort(p)` to `CliError::InvalidPort(p)` which formats as `snap: invalid port: {p}`.
- `rust/src/cli/commands.rs`:
  - Implement `cmd_serve(port_opt: Option<u16>) -> Result<(), CliError>`:
    - Locate root with `find_repository_root()?`.
    - Load and strictly validate local repository.
    - Delegate to `crate::http::serve_repository(&repo, port)`.
  - Update `load_remote_repository(source: &str) -> Result<Repository, CliError>`:
    - If `source.starts_with("http://") || source.starts_with("https://")`:
      Delegate to `crate::http::fetch_repository(source)`.
    - Else continue local filesystem path resolution.

### C. Dependencies & Cargo Configuration (`rust/Cargo.toml`)
- Add `libc = "0.2"` to `[dependencies]` in `rust/Cargo.toml` (already locked in `Cargo.lock` via transitive dependencies).

---

## 3. Strict Quality & Anti-Pattern Compliance
- **Zero Panics:** No `.unwrap()` or `.expect()` in production code; all system calls, I/O operations, and parsing return typed `Result`s.
- **Snapshot Isolation:** The repository JSON buffer is created once at server startup. Any disk changes to `.snap/repository.json` while the server is running are completely ignored.
- **Graceful Shutdown:** `SIGTERM` and `SIGINT` trigger clean loop termination and process exit with code 0.
- **Strict Error Handling:** HTTP non-200 responses return `snap: HTTP <status>`, malformed JSON returns `snap: invalid JSON: <detail>`.

---

## 4. Verification Plan

### Automated In-Process Unit Tests
- `cargo test`:
  - In-process HTTP server startup, binding to port 0, endpoint checking (`GET`, `HEAD`, `POST`, `404`).
  - Snapshot isolation unit test: Start background thread server, mutate file on disk, verify GET still yields initial content.
  - Client URL parsing and GET fetcher tests.

### Shared YAML Acceptance Suite
- Run targeted Phase 7 test suites:
  ```bash
  ./verify --lang rust --filter 12-http-server
  ./verify --lang rust --filter 13-http-client
  ```
- Run full suite:
  ```bash
  ./verify --lang rust
  ```
