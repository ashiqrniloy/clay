# Build and Test

Clay's required development and CI host is Linux. Routine builds use Cargo's normal repository-local `target/` directory; do not set `CARGO_TARGET_DIR` for ordinary verification. A second target tree duplicates large V8 artifacts and defeats incremental reuse.

## Profiles

`Cargo.toml` sets `debug = "line-tables-only"` for `dev` and `test`. Routine binaries retain source/line information for backtraces and breakpoints while omitting full variable/type debug data. For an interactive debugging session that needs local variables, build the relevant target with the opt-in full-DWARF profile:

```bash
cargo test --profile debugging --test runtime --no-run
```

The installed Cargo 1.96.1 documentation defines line tables as its minimal source/line format and confirms that `test` otherwise inherits `dev`. On the verification host, `llvm-symbolizer` resolved a routine test symbol to `tests/suites/protocol.rs:21`; the `debugging` profile built successfully and emitted full parameter/local-variable DWARF. GDB/LLDB were not installed on that host, so interactive stepping was not claimed.

## Integration suites

Cargo auto-discovery is disabled because every top-level `tests/*.rs` file otherwise becomes a separately linked binary. Four explicit roots use plain `#[path] mod` declarations; source files stay independently readable and test names gain only their source-module prefix.

| Suite | Source modules |
|---|---|
| `security` | language-server authority, package conflicts/graph/loading/primitive gate, runtime sandbox, Rust visibility |
| `runtime` | command/completion/intelligence, LSP bridges, parse/runtime reload/update, selected-file smoke, syntax grammar |
| `editor` | decoration transport, editor invariants, Markdown rendering/mode, diagnostics, themes, typography |
| `protocol` | Clay JS docs/facades, smoke/package docs, fixtures/budgets/protocol performance, primitive docs |

Run a suite or one former source harness with a module filter:

```bash
cargo test --test security
cargo test --test security package_loading::
cargo test --test runtime language_intelligence::specific_test_name
cargo test --test protocol primitives_docs::audit_exceptions_are_documented_and_unexpired
```

`integration_suite_inventory_assigns_every_source_once` fails if a top-level integration source is omitted or assigned twice. Before consolidation, Cargo listed 1,782 tests. After removing the new module prefix, the post-change multiset contains the same 1,782 names; the inventory guard adds one new test.

## Required gates

```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo audit
```

The gates are run once serially (never as concurrent Cargo invocations). `scripts/check.sh full` wraps exactly this serial gate under one repo-local lock, and CI invokes that same command; see the supported check wrapper below.

### Bounded configuration and Control Center tests (plan 086 task 7)

Green baseline recorded 2026-08-14 (all serial): `example_configuration_loads_cleanly_and_applies_effects` (0.06 s), `control_center_opens_filters_activates_and_cancels` (0.03 s), `runtime_generation_replacement_cancels_open_control_center` (0.03 s). Each runs under a 5 s whole-workflow `tokio::time::timeout` whose message names pending session/runtime cleanup instead of waiting indefinitely, uses mode-700 hermetic config/workspace roots (never ambient `~/.config/clay`), and asserts cleanup (`drain_bounded` + `close`) plus a sentinel-typography check proving the reloaded generation came from the hermetic root.

The watcher-reload tests in `runtime_generation_tests` share the `wait_until` poll helper (10 ms poll, 5 s bound); on timeout its panic reports the scenario plus the live generation id and diagnostic codes so a stalled watcher points at pending session/runtime-replacement cleanup instead of a bare `Elapsed`. `wait_until_panics_with_scenario_and_server_state_on_timeout` pins the diagnostic message.

Security/adversarial coverage remains in the normal gate: package authority and sandbox tests are under `security`; multi-client, filesystem, and queue tests remain library tests; audit exceptions are checked under `protocol` and by `cargo audit`.

## Tauri/React desktop shell (Plan 097 Phase 2+)

The repository is a Cargo workspace: the root package stays `clay`; the
Tauri v2 shell lives in `src-tauri/` (`clay-desktop`) and the React frontend
in `frontend/`. The existing gates above run across all workspace members,
including `clay-desktop`.

### Linux prerequisites

Ubuntu/Debian CI (see `.github/workflows/ci.yml`):

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev libssl-dev
```

AerynOS (verified working): install the `-devel` counterparts of the
webkigtk/gtk/appindicator/rsvg/dbus stack via moss. `libxdo` and OpenSSL are
NOT required by Clay: the dependency tree contains no `xdo` and uses rustls,
not `openssl-sys` (verify with `grep -i 'xdo\|openssl' Cargo.lock`).

Without the WebKit/GTK/dbus headers, workspace-wide Cargo commands fail inside
`*-sys` build scripts; root-package work is unaffected: scope with `-p clay`.

### Local build and run (no CI needed)

```bash
cd frontend && npm ci && npm run build   # renderer → frontend/dist
cd .. && cargo build -p clay -p clay-desktop
cargo run                  # kill leftover default-endpoint servers, then open GUI
cargo run -- restart       # replace the server, no GUI
cargo run -- client        # extra GUI against a running server
CLAY_SERVER_BIN=/path/to/clay-server target/debug/clay-desktop   # explicit override
```

Debug `clay-desktop` embeds `frontend/dist` (`custom-protocol` is the default feature). Rebuild the desktop crate after `npm run build` so the bundle is current. Hot-reload uses Vite instead: `cd frontend && npm run dev` in one terminal and `cargo tauri dev` in another (`tauri dev` disables `custom-protocol` and loads `http://localhost:1420`).

The desktop shell resolves `clay-server` as `$CLAY_SERVER_BIN` → sibling of
its own executable → `PATH`. If an endpoint already has a live listener
(e.g. you ran `clay server` yourself), the shell reports a typed Connected
status with `pid: null` instead of spawning a second server. Closing the
window kills and reaps only servers it spawned.

### Frontend gates

```bash
cd frontend
npm ci          # deterministic install from package-lock.json
npm run lint    # eslint (typescript-eslint + react-hooks)
npm run format:check
npm test        # vitest (theme adapter, components, shell, bridge)
npm run build   # tsc -b && vite build → dist/
npm run check:budget  # shell gzip ≤ 180 kB; total (incl. editor) ≤ 400 kB
```

### Running the desktop shell

See "Local build and run" above. The desktop binary resolves `clay-server`
as: `$CLAY_SERVER_BIN` → sibling of its own executable → `PATH`; live
endpoints are adopted rather than double-spawned. Dropping the supervisor or
closing the window kills and reaps spawned server processes.

### Phase 2 baselines (2026-08-23, AerynOS verification host)

- Frontend production build (`npm run build`): ~0.6–1.7 s wall;
  `dist/assets/index.js` ~199 kB (~63 kB gzip), CSS 0.94 kB, HTML 0.39 kB.
- Vitest suite: 11 tests, ~0.3 s; desktop crate tests: 20 tests, ~1 s
  (bridge forwarder unit, DTO family round trips, real-server bridge
  session end-to-end, supervisor lifecycle, CSP/capability guards).
- Desktop member compiles and links locally on AerynOS after installing the
  GTK/WebKit devel stack. Note: `[lib] crate-type` is rlib-only because V8
  (deno_core) TLS relocations are incompatible with a cdylib build.
- Live smoke: launch against the default endpoint adopted the user's already-
  running server (`Connected`, `pid: null`); exit left zero orphaned
  processes.

### Phase 4 baselines (2026-08-23)

- Frontend production build: ~1.5 s; `dist/assets/index.js` ~408 kB
  (~130 kB gzip) + CSS 6.55 kB (1.6 kB gzip). Combined gzip 128 kB / 160 kB
  budget. Weight is React Aria + React Router; editor/CM land in Phase 5.
- Vitest: 32 tests (~1.2 s) covering theme adapter, keyboard/focus, shell
  landmarks, connection store, render-count discipline.
- Theme authority: Rust `resolve_theme_token_snapshot` emits 91 core tokens;
  contrast rejection stays server-side. Frontend never re-resolves per frame.

### Phase 5 baselines (2026-08-23)

- Production build: shell `index.js` 465 kB (149 kB gzip) + CSS 1.6 kB;
  lazy `ClayEditor` chunk 231 kB (76 kB gzip). Shell gzip 147 / 160 kB;
  total gzip 222 / 400 kB.

### Phase 6 baselines (2026-08-23)

- Production build: shell `index.js` 507 kB (162 kB gzip) + CSS 1.9 kB;
  lazy `ClayEditor` 221 kB (73 kB gzip). Shell gzip 162 / 180 kB;
  total gzip 233 / 400 kB.
- Vitest: 62 tests (split tree, tab isolation, dirty close, persist fallback,
  plus Phase 4/5 suites).
- Vitest: 47 tests (position map, operations, session ack/reject/resync,
  editor lifecycle, existing shell/component suites).
- Position map: `src/editor/position_map.rs` and
  `frontend/src/editor/position-map.ts` share the same golden vectors.

### Phase 7 baselines (2026-08-23)

- Production build: shell 162.4 / 180 kB gzip; lazy editor/intelligence chunk
  106.2 kB gzip; total 269.4 / 400 kB gzip.
- Vitest: 72 tests including LSP-snippet acceptance, 1 MiB mount/local typing (<2,000/<100 ms) and
  1,000-span projection (<500 ms) advisory budgets.
- Rust authority suites: completion 27, language intelligence 31, decoration
  transport 20, syntax grammar 73, editor performance invariants 40 — PASS.
- Desktop bridge: 15 lib + bridge session + 3 security + 7 DTO — PASS;
  nested reactive request identity stamping is covered.

### Phase 8 baselines (2026-08-23)

- Production build: startup shell 164.3 / 180 kB gzip; code-split package
  renderer 27.8 kB gzip; CodeMirror 107.1 kB gzip; total 299.3 / 400 kB.
- Vitest: 79 tests including stable-ID targeted updates, stale rejection,
  retained package input/disclosure state, inert action routing, hostile text,
  SDUI editor composition, and atomic runtime-generation acknowledgement.
- Protocol version 26 carries complete package UI snapshots. Tauri parses
  component JSON and resolves themes before webview delivery.
- Trust-domain gates: package graph 18, package loading 47, cross-domain 7,
  package UI conformance 10, primitive docs 29 — PASS.
- Full all-target run retains the Phase 1 baseline failures only: bundled Chat
  extension-point scope and Command Centre session call-site count.

### Phase 9 baselines (2026-08-23)

- Production build: startup shell 156.5 / 180 kB gzip; lazy desktop-workflow
  chunks 34.9 kB gzip; CodeMirror 107.4 kB gzip; total 304.9 / 400 kB.
- Vitest: 84 tests including opaque menu lifecycle/intents, modal/listbox
  semantics, closed client-command routing, complete typography transactions,
  invalid-bound denial, and secret-free settings DOM.
- Rust focused gates: menu sessions 17, Path Browser 34, settings 10, protocol
  202, desktop 16 + bridge session + 3 security + 7 DTO - PASS.
- Tauri keeps `core:default` only and reuses Clay's native dialog backend; no
  broad filesystem/dialog/clipboard/shell/network plugin was added.
- Full all-target run retains the Phase 1 baseline failures: bundled Chat
  extension-point scope and Command Centre session call-site count. One
  concurrent sandbox fixture hit transient Linux `ETXTBSY`; its exact
  single-thread rerun passed.

### Phase 10 baselines (2026-08-24)

- Production build: startup shell 160.4 / 180 kB gzip; lazy Chat/AG-UI chunk
  37.1 kB gzip; total 342.9 / 400 kB. Review harness is DEV-only and is not
  in the production graph.
- Vitest: 96 tests including AG-UI transport through real `@ag-ui/client`,
  cancel/empty-prompt, ChatPanel landing/transcript/composer, and DEV fixture
  landmarks.
- Rust focused gates: `agent_agui` 6, agent picker 10, desktop lib 20,
  bridge session 1, config security 3, DTO 7, protocol documentation_coverage
  7 — PASS. `cargo fmt --check`, `cargo check --all-targets`, and
  `cargo clippy --all-targets -- -D warnings` PASS.
- Credentials never appear on adapted events. Tool/permission payloads stay
  inert `CUSTOM` events. ACP remains absent. Tauri capability stays
  `core:default`.

### Phase 11 baselines (2026-08-24)

- Release identity: crate, `clay-desktop`, `tauri.conf.json`, frontend, and
  `clay-agent` versions are `0.1.0`. Bundle icon is `icons/icon.png`; Linux
  targets are `deb`/`rpm`/`appimage`. No updater plugin or
  `createUpdaterArtifacts`.
- Desktop lib 27 tests (sidecar lookup, networked-endpoint reject, typed
  missing-binary, unsigned/wrong-target/wrong-version update policy, adopt,
  reap). config_security 4 tests. Frontend budget unchanged: shell 160.4 /
  180 kB gzip, total 342.9 / 400 kB.
- Scripts: `scripts/security-audit.sh`, `scripts/package-smoke.sh`. CI runs
  clay-agent tests and package-smoke after the frontend production build.
- Remote/container: `CLAY_ENDPOINT` local socket/pipe only; live endpoints
  are adopted. Install/uninstall is the host package manager. In-app updates
  have no apply path.

## Measured build shape

Measurements used one normal `target/` on Linux (`x86_64`, Cargo/rustc 1.96.1), `cargo clean`, then `cargo test --all-targets --no-run`. Timings are snapshots, not hard gates.

| Metric | Full debug, 33 integration roots | Line tables, 4 roots | Change |
|---|---:|---:|---:|
| Clean build/link | 89.070 s | 61.724 s | -30.7% |
| Warm relink snapshot | 15.903 s | 7.250 s | -54.4% |
| Cargo test/bench executable harnesses | 43 | 14 | -67.4% |
| `target/` | 21,942,578,072 B | 6,711,040,962 B | -69.4% |
| `target/debug/deps` | 19,521,134,614 B | 5,213,466,962 B | -73.3% |
| `target/debug/incremental` | 1,953,003,235 B | 1,031,228,815 B | -47.2% |

The warm baseline is Plan 060's pre-change relink snapshot; the after value touches one integration source before `--no-run`. No production crate split or test runner was added.

## Supported check wrapper

`scripts/check.sh` is the one supported entry point for local and CI verification on Linux:

```bash
scripts/check.sh quick   # non-release quick feedback: fmt + library unit tests
scripts/check.sh full    # serial release gate under one repo-local lock
scripts/check.sh report  # advisory target-size/executable report
```

`quick` is labeled non-release: it runs only `cargo fmt --check` and `cargo test --lib --quiet`, the measured smallest representative compile/unit set (roughly 13–20 s warm). `full` is the release gate: it acquires one repo-local lock at `target/.clay-full-check.lock` and then runs `cargo audit`, `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets --quiet`, and `cargo bench --no-run` serially in that order, printing the failed stage and exiting nonzero on the first failure. A second concurrent `full` invocation waits on the lock (documented policy: wait, never run two full gates on the same checkout). The script refuses to run `full` when `target/` is a symlink so the lock cannot be redirected outside the repo. CI invokes `scripts/check.sh full` directly so local and CI run the identical gate; the protocol suite pins this parity in `manual_smoke_docs`. Both modes reuse the repository `target/` and never set `CARGO_TARGET_DIR`.

### Build artifact report and cleanup policy

`scripts/check.sh report` is cheap, advisory, and safe to run after a failed or
partial build. It reports `target/`, `target/debug/deps`,
`target/debug/incremental`, and executable files in `target/debug/deps`; missing
subdirectories are reported as `missing`, not errors. CI runs it with
`if: always()` after `scripts/check.sh full`, so storage evidence is retained
without masking the gate's original status. The report never deletes files or
prints artifact contents.

Review storage when total `target/` exceeds 50 GiB, `target/debug/incremental`
exceeds 20 GiB, or a second target tree appears. These are advisory cleanup
thresholds, not release gates; keep useful incremental state below them when
possible and record exceptions in the baseline/performance docs. Before
cleanup, run `scripts/check.sh report`; then use the narrowest Cargo cleanup:

- `cargo clean --profile debugging` removes only opt-in full-DWARF debugging artifacts.
- `cargo clean` removes all repository build artifacts when disk pressure or stale historical hashes justify a cold rebuild.

Never set `CARGO_TARGET_DIR` for routine Clay verification and never add a
second `target/` tree to preserve incremental reuse.

## Cleanup

Use Cargo cleanup only when disk pressure or stale historical hashes justify losing incremental state:

```bash
cargo clean
cargo clean --profile debugging
```

Do not keep `target/pi-verify` or other routine duplicate target directories. Temporary version-exact rustdoc targets may still be used for isolated crate documentation, then removed.
