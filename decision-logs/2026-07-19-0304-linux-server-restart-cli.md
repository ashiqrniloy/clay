---
date: 2026-07-19 03:04
status: approved
decision_about: "Linux server restart CLI"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Add a Linux server restart CLI command

## Decision

Clay provides `clay restart`, also available as `cargo run -- restart`, to replace the default Linux background server and exit after readiness verification. Restart targets only server processes running the current Clay executable against the default endpoint; it does not stop GUI clients, custom-endpoint servers, or isolated smoke servers.

## Context

Bare `cargo run` leaves its auto-started server running after the GUI closes. Developers previously needed an error-prone `pkill` plus optional Unix-socket cleanup before testing a rebuilt server.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: “I would also create a clay restart command to the CLI and a Cargo run option for this already. Do this implementation.”

## Alternatives Considered

1. **Keep documented `pkill` commands** — simple, but depends on an external command and broad process-name matching.
2. **Add an IPC shutdown protocol message** — portable, but expands protocol and shutdown authority solely for a development lifecycle command.
3. **Use a PID file** — portable in principle, but adds persistent lifecycle state, stale-PID validation, and another file requiring secure ownership handling.

## Rationale and Evidence

Linux `/proc` provides enough process identity to match the current executable, `server` subcommand, and default endpoint without a shell or new dependency. Clay already has shell-free background startup and bounded handshake retry in `src/main.rs`; restart reuses both. `SIGTERM` gets a two-second bound before `SIGKILL`. Server startup already safely removes stale Unix socket nodes while refusing non-socket paths in `src/server/mod.rs::remove_stale_socket`.

A live test ran `cargo run -- restart` twice and observed different server PIDs, successful stop/start diagnostics, and handshake readiness both times. Linux `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` pass.

## References

- `src/main.rs` — CLI parsing, Linux process matching/signaling, background startup, and readiness verification.
- `src/server/mod.rs::remove_stale_socket` — safe stale endpoint cleanup.
- `README.md` — primary command and manual fallback.
- `docs/development/launch-and-gui-smoke.md` — launch-mode behavior.
- `docs/wiki/modules/server-ipc-skeleton.md` — implementation flow and boundaries.

## Consequences

- Linux developers can reliably restart a rebuilt default server with one command.
- Existing clients disconnect and must reconnect; restart intentionally opens no GUI.
- Windows and macOS return a clear unsupported error until a native process-lifecycle design is implemented.
- If restart later becomes a user-facing cross-platform service control, revisit an authenticated/local control protocol or platform service manager integration.
