# Plan 134 task 6 — manual regression pass (2026-09-22)

Regression-only pass for the R2/R4/D4/D5/P3/R3 changes: no user-visible behavior
was supposed to change, so modules **15 (UI design systems / theme/appearance)**
and **03 (files and workspace)** were re-run on a real Linux build. **No manual
step was added, deleted, or weakened** — the modules' step tables are unchanged;
`test-plan/index.md` gained only this dated execution record.

- Tree: HEAD `40022f6` + plan 133's uncommitted split + plan 134 tasks 2–5.
- Build: `cargo build --bin clay` and
  `cargo build -p clay-desktop --bin clay-desktop` (frontend/dist present).
- Host: mango 1920×1200 Wayland session, WebKitGTK 4.1, xdg-desktop-portal
  `wlr.portal`, python3-gi AT-SPI, `wtype`.

## Harness note (not a Clay defect)

The first `capture-ui-review.sh` run on the currently focused compositor tag
tiled the Clay window beside the user's terminals and measured a 949×521
viewport, below the 900×600 review floor → `UNRESOLVED` (honest floor, no
capture recorded). Re-run on an **empty tag** (`mmsg dispatch view,8`, restored
to tag 5 afterwards) gave the full viewport 1906×1099 for every capture.

## Module 15 — theme/appearance (UI-DS-01/UI-DS-15 class)

| Run | Result | Evidence |
| --- | --- | --- |
| `--fixture ui-review-design-system` (shipped `@clay/design-instrument`, dark) | PASS | `review.status` PASS, viewport 1906×1099; AT-SPI: landmark `Design system review`, button `Primary action`, entry `Document editor`, list rows; `configuration_failed_lines=0` |
| `--fixture ui-review-design-system-light` (UI-DS-15 cross-theme) | PASS | same landmarks under the light theme; distinct screenshot (sha `74fa9110…` vs dark `9cfc0894…`) |
| `--fixture ui-review-default --theme @clay/theme-gruvbox-material-light --appearance light` | PASS | `metadata.txt` records the seeded theme/appearance; shell + status bar + Document editor render, `configuration_failed_lines=0`, distinct screenshot `7959da00…`. This is the live path through the reactor-side preference/appearance read made async in task 4 (`effective_configuration_root` → spawn_blocking) |

## Module 03 — files/workspace

| Step class | Result | Evidence |
| --- | --- | --- |
| F1-class open (workspace file through the async canonicalize/metadata path) | PASS live | `--fixture ui-review-workspace` PASS (1906×1099): AT-SPI list item `review.md`, landmark `Editor review.md`, entry `Document editor` (111-node tree), `configuration_failed_lines=0` |
| Folder open (workspace root restored + registered) | PASS live | The same run restored `layout.json`'s workspace root; the file browser lists the root's entries; the isolated `launcher.json` was written (async `record_recent_workspace`) as `{"version":1,"workspaces":["/tmp/clay-plan132-live/workspace"]}` |
| F4/F5-class type + save | PASS live | Isolated live session (plan-132 harness, `run-live.sh start editing`, `target/debug/clay` + `clay-desktop`): AT-SPI focus + `wtype` typed `// plan134 p3 probe` (editor chars 64→85), `Ctrl+S` wrote `commands.rs` to disk — content verified with `cat`, server log 0 errors/diagnostics |
| F7-class external-edit reload | **UNRESOLVED (host surface ceiling)** | The reload was not drivable live: this build exposes `documents.reload` only as the Clay JS API (no built-in command/chord — the Control Center enumeration shows only `Reload Configuration and Packages`). An external append (`echo '// reloaded by plan134' >> commands.rs`) + palette check changed nothing else. Reload semantics stay covered by automated suites: `reload_streams_new_text_and_replaces_resident_bytes`, `reload_document_unlocked` paths, `cargo test --test runtime` 75; deeper live reload needs a command surface, not a plan-134 change |
| Launch gate (module 01 server half) | PASS | Every isolated harness run bound a private mode-700 socket with 0 configuration failures; live session server log 0 errors |

## Automated companions (same tree)

- Targeted: `shell::theme` 29, `shell::layout` 84, `workspace::` 84, `launcher`
  15, `connection::` 96, `runtime_reload` 2, `configuration` 71, `ops::` 88,
  `command_execution` 27, `server::js_runtime` 241 — all green.
- Full gate (`scripts/check.sh full`, task-5 record): exit 0, 2006 passed /
  0 failed / 1 ignored, desktop 55, bindings 1.

## Artifacts

- `live/design-system-dark/`, `live/design-system-light/`,
  `live/theme-seeded-light/`, `live/workspace-open/` — each with
  `screenshot.png`, `accessibility.txt`, `a11y-tree.txt`, `metadata.txt`,
  `server.diagnostics.txt`, `review.status=PASS`.
- Live type/save session root `/tmp/clay-plan132-live` (removed by
  `run-live.sh stop`; file content and launcher store recorded above).
