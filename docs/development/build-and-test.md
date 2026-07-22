# Build and Test

Clay's required development and CI host is Linux. Routine builds use Cargo's normal repository-local `target/` directory; do not set `CARGO_TARGET_DIR` for ordinary verification. A second target tree duplicates large V8/Masonry artifacts and defeats incremental reuse.

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

Security/adversarial coverage remains in the normal gate: package authority and sandbox tests are under `security`; multi-client, filesystem, and queue tests remain library tests; audit exceptions are checked under `protocol` and by `cargo audit`.

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

## Cleanup

Use Cargo cleanup only when disk pressure or stale historical hashes justify losing incremental state:

```bash
cargo clean
cargo clean --profile debugging
```

Do not keep `target/pi-verify` or other routine duplicate target directories. Temporary version-exact rustdoc targets may still be used for isolated crate documentation, then removed.
