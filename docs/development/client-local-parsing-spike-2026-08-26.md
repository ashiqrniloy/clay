# Client-local Lezer fail-fast gate

**Date:** 2026-08-26

**Candidate:** stock CodeMirror/Lezer, selected only by the branch-only
`client-local-parsing-spike` build mode.

**Host:** Linux 7.1.8-50.stable, `x86_64`; WebKitGTK 2.52.5;
Node 24.19.0; npm 11.17.0; Rust/Cargo 1.96.1.

## Reproduction

The following command was run before cleanup:

```bash
node tools/spikes/client-local-parsing/summarize.mjs --run
```

It read the exact lockfile versions, parsed the current five-language fixtures
and frozen modern Rust/TypeScript corpus, and wrote the machine-readable traces
to `target/perf/client-local-parsing/grammar.json` and
`target/perf/client-local-parsing/summary.json`. The disposable spike runner,
fixtures, and parser dependencies were removed after this evidence was
captured; this retained report is the historical result.

## Ordered result

| Gate                                                  | Result         | Evidence                                                          |
| ----------------------------------------------------- | -------------- | ----------------------------------------------------------------- |
| Grammar freshness                                     | **FAIL**       | `rust-modern` produced 4 recovery nodes with `@lezer/rust` 1.0.2. |
| Real WebKitGTK edit latency (p95 ≤ 8 ms; max ≤ 16 ms) | Not applicable | Stopped by grammar failure.                                       |
| Main-thread long tasks (zero ≥ 50 ms)                 | Not applicable | Stopped by grammar failure.                                       |
| Distant 10–50 MiB viewport freshness (≤ 100/200 ms)   | Not applicable | Stopped by grammar failure.                                       |
| 256 MiB resident-memory envelope                      | Not applicable | Stopped by grammar failure.                                       |
| Same-document four-pane scaling                       | Not applicable | Stopped by grammar failure.                                       |
| Three-run reproducibility                             | Not applicable | Stopped by grammar failure.                                       |

The runner intentionally does not launch the expensive real-app matrix after a
hard grammar failure. No latency, viewport, memory, or pane-pass claim is made.

The summary contains 90 planned size/shape/pane scenarios. Every one is
explicitly marked `not-applicable` after the grammar failure; no later scenario
was silently treated as a pass.

## Grammar evidence

All existing current fixtures passed with zero recovery nodes and all required
structure markers:

- `tests/fixtures/syntax/rust.rs` — `@lezer/rust` 1.0.2
- `tests/fixtures/syntax/typescript.ts` — `@codemirror/lang-javascript` 6.2.5 TypeScript language
- `tests/fixtures/syntax/typescript.tsx` — `@codemirror/lang-javascript` 6.2.5 TSX language
- `tests/fixtures/syntax/javascript.js` — `@codemirror/lang-javascript` 6.2.5
- `tests/fixtures/syntax/markdown.md` — `@lezer/markdown` 1.7.2

Frozen modern probes then failed the zero-recovery-node contract:

- Former fixture `rust-modern.rs` — 4 recovery nodes at UTF-16 parser offsets `257`, `326–329`, and `467` (twice). The probe covered an async closure, a let-chain, and a gen block.
- Former fixture `typescript-modern.ts` — 2 recovery nodes at offsets `184` and `189`. The probe covered import attributes alongside decorators and `satisfies` syntax.

The trace records only relative fixture paths, package versions, numeric byte/
offset/count data, node names, and pass/fail values. It does not record source
text, captures, credentials, absolute paths, or home-directory data.

## Decision impact

This candidate stops at the first hard gate as required. Lezer is not approved
for parser placement, and no worker, hybrid, parity, or package-execution work
is justified by this result. The next parser-placement action is the approved
server-side Tree-sitter session direction and Plan 099 replanning; later Lezer
parity and visual tasks are not applicable to this failed candidate.
