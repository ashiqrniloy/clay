# Plan 128 live harness — shared incremental position index (2026-09-20)

Isolated live launch used for the plan-128 manual-test-plan step (module 08
`S35`, module 11 `Q43`). This is the *live presence* half; the measured/correctness
half lives in the automated companions (`packages/lsp-shared/adapter.test.mjs`,
the four `packages/lsp-*` suites, `lsp_bridge`, and the adapter cost probe in
`code-reviews/2026-09-19-plan128-task3/`).

## Layout

| Path | What it is |
|---|---|
| `launch-live.sh start lsp\|mid\|large [bytes]` | Creates a private root at `/tmp/clay-plan128-live` (HOME, XDG dirs, socket, TMPDIR, perf report dir, `~/.rustup` + `~/.cargo` symlinks) and launches the installed `target/debug/clay` + `clay-desktop` against a generated Cargo crate. |
| `launch-live.sh stop` | Graceful TERM so the daemon flushes `report/clay-server-perf-summary.json`. |
| `init-lsp.js` | Authorizes `@clay/lsp-rust`, registers the Rust package, binds `Ctrl+J` (completion) and `Ctrl+U` (inlay hints). |
| `probe.py`, `portal-shot.py` | Copied from `artifacts/126-access-paths/`: AT-SPI probe and window-cropped portal capture (full-desktop captures are never retained). |
| `screenshots/`, `logs/` | Retained evidence: window crops, the daemon log for the failing leg, and the perf summary. |

Fixture shapes (all generated, all inside the private root):

- `lsp` — 4 KiB crate (`src/main.rs` + `src/helper_items.rs`), small enough for
  the package analyzer; the crate also carries a rust-analyzer-only probe
  (`fn broken() -> u32 { let value: u32 = "text"; value }`).
- `mid` — the same crate plus a ~250 KiB `src/probe.rs` module, opened.
- `large` — a 1,258,277-byte `review.rs` (225,209 words), opened.

## Result

| Leg | Verdict | Evidence |
|---|---|---|
| Language route on an accepted document (`lsp`, 4 KiB) | **PASS live** | rust-analyzer + `rust-analyzer-proc-macro-srv` run inside the isolated root (`pgrep` under `/tmp/clay-plan128-live/home/.rustup`); the daemon log has no language failure; the bridge delivered analyzer patches in the same size class (`bridge.patch_delivery` p50 0.089 ms, p95 0.121 ms, 2 samples, `logs/clay-server-perf-summary.json` from the `lsp`-mode run); capture `screenshots/lsp-small-semantic.png`. |
| ≈250 KiB open file (`mid`, 46,360 words) | **FAIL live — pre-existing bound, found by this run** | First semantic payload kills the analyzer: `Error: lsp.invalid_semantic_tokens: bounded five-integer records required` (`logs/server-mid-semantic-bound.log`), UI falls back to the "Document analyzer stopped; baseline language support remains active." status (`screenshots/mid-probe-semantic-bound.png`). Cause: `packages/lsp-shared/mapping.js` `MAX_SEMANTIC_TOKENS = 128` (rejects any payload above 640 integers). |
| ≥1 MiB open file (`large`) | **PASS fail-closed / the plan's ≥1 MiB language-server leg is unreachable** | Status bar: "Document exceeds the package analysis limit; baseline language support remains active." (`screenshots/large-analysis-limit.png`); no rust-analyzer process exists for that run (the analyzer is never started above `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES = 256 KiB`, `src/perf/budgets.rs:33`); a pointer hover over an identifier still issues a language request, which comes back `providerError` and surfaces as the "Language provider failed" toast (`frontend/src/editor/extensions/intelligence.ts:88`) in the same capture. |
| Typing echo / typing latency (`server.edit_ack`) on ≥1 MiB | **UNRESOLVED live — host input** | `Ctrl+End`, `Ctrl+B` (the status bar's own "hide lane" toggle) and 60-char `type_text` bursts produced no document change (file mtime unchanged, `grep` for the typed marker = 0, state stayed `clean`, no `edit_apply`/`edit_ack` counters in the perf summary). The MCP readiness probe reports `can_send_development_input: false` with the "enable XDG RemoteDesktop portal input" blocker; keystrokes are handed to a portal session without remote-interaction permission and dropped. Re-running this leg needs the portal's Remote Desktop dialog answered with **Allow Remote Interaction enabled** before Share (or a keyboard backend such as `wtype`/`ydotoold`). |
| Diagnostics leg (rust-analyzer `mismatched types`) | **Observation, not investigated** | The `lsp` fixture contains `let value: u32 = "text";`; no lint-gutter marker appears after ~30 s (pixel check of the gutter band and a 5× zoom of the line). The bridge does carry a diagnostics path (`packages/lsp-shared/bridge.js` push/pull + `diagnosticsToClay`), so this is a separate probe, not evidence about the position index. |

## Not claimed

- No live frame-timing number. The AT-SPI probe's resolution (one tree walk,
  ~0.9 s warm) is coarser than a keystroke→paint event, and this run produced no
  keystrokes at all.
- No claim that the semantic colours visible in `lsp-small-semantic.png` prove
  the LSP route specifically: the syntax tier paints function/namespace colours
  too. The analyzer-alive evidence above is process presence + delivered
  patches + absence of failure, not colour attribution.