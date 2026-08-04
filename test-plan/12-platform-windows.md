# 12 — Platform: Windows

Linux is the required pass platform (see AGENTS.md platform-validation);
Windows is a long-term target. Run this module only when Windows verification
is explicitly requested. Authoritative detail: `docs/development/windows.md`.

## Toolchain

| # | Action | Expected |
|---|--------|----------|
| W1 | MSVC toolchain + prerequisites per `windows.md` | `cargo check` / `cargo build` succeed |
| W2 | Symlink builds if used | Build configured per docs (symlink caveats noted there) |

## IPC and launch

| # | Action | Expected |
|---|--------|----------|
| W3 | Launch app | Named-pipe IPC connects; GUI reaches `Connected — Editable` |
| W4 | `cargo run -- restart` | Returns unsupported-command error on Windows (Linux-only command) |

## Native dialogs

| # | Action | Expected |
|---|--------|----------|
| W5 | `Ctrl+O` (bound per module 03) | Native Windows file dialog opens |
| W6 | Select `.md`/`.markdown`/`.mdown` | File opens, decorations activate |
| W7 | Cancel dialog | No-op |

## Parity checks

Run modules 01 (startup subset), 04 (core editing), and 03 (files) on
Windows and note divergences. Divergences are defects only where the
platform matrix in `docs/development/file-open-save-reload-workflow.md`
promises parity; otherwise record as known platform gaps.

## Known ceilings

- Windows cross-compilation/smoke from a Linux host is NOT a pass condition
  for normal work (project policy).
