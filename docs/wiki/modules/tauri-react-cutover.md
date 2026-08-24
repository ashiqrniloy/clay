# Tauri/React Desktop Cutover

Plan 097 Phase 12 makes Tauri/React Clay's only desktop client while retaining the standalone Rust server.

## Ownership

- `src/main.rs` and `src/launch.rs` route `clay` and `clay client` to `clay-desktop`; `clay server` remains explicit.
- `src-tauri/` owns desktop lifecycle, narrow commands/channels, native dialogs, packaging, and process supervision.
- `frontend/src/` owns React shell, CodeMirror rendering/input, tabs/splits, SDUI projection, settings, and AG-UI Chat presentation.
- `src/server/`, `src/protocol/`, `src/packages/`, and `deno_core` retain canonical state, authority, package execution, and validation.

## Removed implementation

Phase 12 deletes Masonry widgets, native editor/shell/driver modules, native clipboard code, local AccessKit/Masonry patches, and native-only tests/benchmarks. Renderer-neutral Rust modules remain only where server validation or Tauri transport uses them: optimistic edit queue/connection state, theme and package validation, protocol color/style projection, and legacy `layout.json` parsing.

`tests/documentation_coverage.rs::removed_native_client_modules_cannot_return` pins removed paths. `Cargo.toml` and `Cargo.lock` contain no Masonry, Vello, Parley, winit, AccessKit, or local native UI patch dependency.

## Compatibility and authority

No dual-client feature flag exists. Packages still receive only bounded inert manifests, SDUI trees, and typed command intents. Tauri/webview internals are absent from Clay JS APIs. Ordinary CodeMirror typing paints locally before bounded asynchronous IPC; server/package/agent work cannot enter that path.

Legacy `layout.json` remains readable through renderer-neutral schema code. Historical plans and wiki pages describing Masonry are retained as historical records; this page and current development docs describe production ownership.

## Verification

Blocking Linux gates:

```text
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
npm --prefix frontend run format:check
npm --prefix frontend run lint
npm --prefix frontend run typecheck
npm --prefix frontend test
npm --prefix frontend run build
npm --prefix frontend run check:budget
npm --prefix clay-agent test
scripts/security-audit.sh
scripts/package-smoke.sh
```

Parity evidence lives in `docs/development/tauri-react-parity-ledger.{md,json}`. Default-launch tests live in `src/main.rs` and `src/launch.rs`; frontend and bridge suites cover replacement behavior.

## Related

- [Desktop Typed Bridge](desktop-typed-bridge.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- [Desktop Release Hardening](desktop-release-hardening.md)
- [Tauri/React parity ledger](../../development/tauri-react-parity-ledger.md)
