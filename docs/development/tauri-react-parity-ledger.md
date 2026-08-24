# Tauri/React Migration Parity Ledger

Phase 1–12 artifact of `plans/097-Tauri-React-Architecture-Migration.md`. It
records the former native Masonry baseline, maps every capability to its
Tauri/React owner, and certifies the final cutover.

- Deterministic data: [`tauri-react-parity-ledger.json`](tauri-react-parity-ledger.json)
  (validated by `tests/documentation_coverage.rs`).
- Decision source:
  `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`.

## Scope captured at freeze time (2026-08-23)

| Dimension | Count | Source |
|---|---|---|
| Capabilities | 18 | this ledger |
| Manual test steps | 541 | `test-plan/01` … `test-plan/14` |
| Public Clay JS API IDs (`registry_public`) | 130 | `docs/reference/clay-js-api/api-inventory.toml` |
| Protocol message families | 26 client + 40 server + agent inventory | `src/protocol/mod.rs`, `src/protocol/agent.rs` |

The ledger distinguishes implemented behavior from unfinished historical
roadmap work: only surfaces that exist in today's tests, docs, and source have
rows. Unfinished roadmap items stay in history and are not parity targets.

## Row schema

Every capability row contains:

- `capability_id`: stable dotted ID (`area.name`).
- `current_owner` / `current_tests`: native files and suites that pin the
  behavior today.
- `manual_steps` / `public_apis` / `protocol_messages`: exact references that
  must each appear in exactly one row (coverage test enforces this).
- `target_owner` / `target_tests`: where the Tauri/React implementation and its
  checks will live.
- `migration_phase`: plan phase (2–12) that owns the port.
- `status`: `pending` → `ported` → `verified`, or `approved-removed`.

## Completion rules

1. A row moves to `ported` only when the target implementation exists behind
   the Phase 3 bridge and current automated suites still pass against the
   unchanged server.
2. A row moves to `verified` only with named evidence in both fields:
   - `verified_automated`: target test names/suites that passed on Linux.
   - `verified_manual`: executed `test-plan` module steps with recorded results.
   The status test fails a `verified` row missing either field.
3. `approved-removed` requires `removal_reference` pointing at an approved
   decision or plan section; it is used when parity means deliberate removal
   (for example Windows-only steps before a Windows port exists).
4. Phase 12 closes the ledger: every row is `verified` or `approved-removed`
   before native deletion, and no budget is silently removed or raised.

## Final cutover (Phase 12)

All 18 capability rows are verified. Tauri/React is the only production desktop
client; `clay server` remains standalone. Masonry widgets, native editor/shell/
driver code, native UI patches, and native-only tests/benchmarks are deleted.
Renderer-neutral server models retained under `src/client`, `src/editor`, and
`src/shell` support protocol transport, theme resolution, package validation,
and legacy `layout.json` reading only.

`tests/documentation_coverage.rs::removed_native_client_modules_cannot_return`
blocks reintroduction of removed native paths and local UI patches. Default
launch tests pin `clay` and `clay client` to `clay-desktop`; standalone server
and smoke routes remain explicit.

## Baseline validation record (pre-migration)

Recorded pre-freeze blockers (blockers, not waivers):

1. AT-SPI/accesskit_consumer `TreeUpdate` panic risk on focused dirty-pane
   close paths (Plan 086 crash; Phase 26 regression tests added, live AT-SPI
   remains a release blocker until the Tauri client replaces the AccessKit
   plumbing).
2. `cargo test --all-targets` FAIL
   `packages::bundled::tests::bundled_extension_points_match_real_contributions`
   — “@clay/chat extension point chat.entrySurface declares scope
   `chat.entry` that names no real contribution of the package”
   (`src/packages/bundled.rs:432`).
3. `cargo test --test security` FAIL
   `rust_visibility_api_mapping::phase24_5_command_centre_sessions_are_not_a_package_programmatic_surface`
   — `open_command_centre_session(` call-site count 4 vs expected 3 in
   `src/server/connection/menus.rs` + `runtime.rs`.
4. The historical Control Center test timeout (>60 s) from the pre-freeze
   audit did not reproduce in the 2026-08-23 baseline run; treated as resolved
   unless it recurs.

Exact baseline command results:

### Baseline run 2026-08-23

| Gate | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo check --all-targets` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo test --all-targets --no-fail-fast` | 2 FAIL of 9 targets: lib + security failures listed above; 7 targets ok (1675 passed / 1 failed / 2 ignored in lib; 137 passed / 1 failed / 2 ignored in security) |
| `cargo audit` | No vulnerabilities. 3 allowed unmaintained-dependency warnings: RUSTSEC-2025-0141 (bincode), RUSTSEC-2024-0436 (paste), RUSTSEC-2026-0192 (ttf-parser). The rkyv 0.8.x advisories recorded pre-freeze are resolved at rkyv 0.8.17 |
| Node (`clay-agent`) `npm test` | PASS (8/8) |
| Generated-registry / doc-coverage checks | PASS (included in the protocol suite: primitives docs, Clay JS registry freshness, manual-smoke docs) |
| Review harness (`scripts/capture-ui-review.sh`) | NOT RE-RUN at freeze (requires desktop session); latest retained record is the 2026-08-21 Phase 28.7 recapture under `code-reviews/screenshots/` |

## Final Phase 12 validation record (2026-08-24)

| Gate | Result |
|---|---|
| Rust fmt/check/clippy | PASS |
| `cargo test --all-targets --no-fail-fast` | PASS: 1117 lib, 30 presentation, 184 protocol, 68 runtime, 130 security, launch tests, and server benchmarks |
| Frontend typecheck/test/build | PASS: 96 Vitest tests |
| Frontend bundle budgets | PASS: shell 160.4/180 kB gzip; total 342.9/400 kB gzip |
| `npm --prefix clay-agent test` | PASS: 8/8 |
| `scripts/security-audit.sh` | PASS: no vulnerabilities; 19 explicitly allowed transitive warnings |
| `scripts/package-smoke.sh` | PASS |
| Native source/dependency absence guard | PASS |
