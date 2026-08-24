# Dependency Security Policy

`cargo audit` is a blocking gate (CI workflow `.github/workflows/ci.yml`, and
expected locally before release). Any vulnerability not explicitly ignored in
`.cargo/audit.toml` fails the audit. Ignores are **temporary, expiring
exceptions**: each one is recorded below with its exact dependency path,
runtime reachability, upstream remediation reference, owner, and expiry. The
`audit_exceptions_are_documented_and_unexpired` test in
`tests/primitives_docs.rs` fails once any expiry date passes — renew only with
fresh upstream evidence, or remove the exception.

Warnings (`unmaintained`, `unsound` informational advisories) do not fail the
audit but are classified below so nothing is silently allowed.

## Expiring vulnerability exceptions

None. The current lockfile has zero known vulnerabilities (`cargo audit`
reports 0). The two former quick-xml exceptions (RUSTSEC-2026-0194,
RUSTSEC-2026-0195) were retired when the native client dependency chains that
pinned `quick-xml` 0.39.4 (`winit`/`masonry_winit`/`accesskit`) were deleted
in Plan 097 Phase 12; the remaining `quick-xml` copy is the fixed 0.41 line.
If a future vulnerability requires an exception, restore an entry here AND in
`.cargo/audit.toml` with all fields shown in the warnings table below.

| Advisory | Crate | Dependency path | Runtime reachability | Owner | Upstream reference | Disposition / recheck |
| --- | --- | --- | --- | --- | --- | --- |
| RUSTSEC-2025-0141 (unmaintained) | bincode 1.3.3 | `deno_core 0.400 → bincode` | V8 snapshot (de)serialization inside the embedded runtime; inputs are host-produced snapshots, not untrusted data | deno_core | bincode 3 rewrite (`https://git.sr.ht/~stygianentity/bincode`, README announcement) pins bincode 1.x | No compatible fix in 1.x; recheck with every `deno_core` upgrade; hard recheck **2026-12-16** |
| RUSTSEC-2024-0436 (unmaintained) | paste 1.0.15 | `v8 → paste`; also `glib-macros → proc-macro-crate → proc-macro-error` chain uses it transitively | Build-time macro expansion only; no runtime reachability | v8 (deno/rusty_v8) | dtolnay/paste (`https://github.com/dtolnay/paste`) — maintained fork via `paste-impl` upstreamed into proc-macro2 | No action available; recheck with every `v8` upgrade; hard recheck **2026-10-07** |
| RUSTSEC-2024-0370 (unmaintained) | proc-macro-error 1.0.4 | `clay-desktop → gtk3-macros/glib-macros → proc-macro-crate → proc-macro-error`; also reached from `deno_core` macro chains | Build-time macro expansion only; no runtime reachability | gtk-rs / deno land | `https://github.com/rustsec/advisory-db` (successor: `proc-macro-error2`) | Upstream macro crates must move first; recheck with every Tauri/`deno_core` upgrade; hard recheck **2026-12-16** |
| RUSTSEC-2025-0075, RUSTSEC-2025-0080, RUSTSEC-2025-0081, RUSTSEC-2025-0098, RUSTSEC-2025-0100 (unmaintained) | unic-* 0.9 family (`unic-char-range`, `unic-common`, `unic-char-property`, `unic-ucd-version`, `unic-ucd-ident`) | `deno_core → urlpattern → unic-ucd-ident → unic-char-property/unic-char-range`, `unic-ucd-version → unic-common` | Unicode identifier tables used by URL pattern matching inside the embedded runtime; inputs are host/package strings already validated by Clay facades | deno_core / urlpattern | `https://github.com/rustsec/advisory-db` (unic is unmaintained upstream; urlpattern owns the replacement decision) | Recheck with every `deno_core` upgrade; replace when urlpattern drops unic; hard recheck **2026-12-16** |
| RUSTSEC-2024-0411..-0420 incl. RUSTSEC-2024-0413 atk, RUSTSEC-2024-0415 gtk, RUSTSEC-2024-0420 gtk-sys; RUSTSEC-2024-0429 (unsound) | atk(-sys), gdk(-sys), gdkwayland-sys, gdkx11(-sys), gtk(-sys), gtk3-macros, glib 0.18 (`VariantStrIter` unsoundness) | `clay-desktop → tauri → wry → webkit2gtk → GTK3 Linux shell (atk/gdk/gtk/glib)` | The Linux desktop window shell around the WebKitGTK webview: window chrome, dialog hosting, input surface. No untrusted data reaches these APIs from Clay code; the unsoundness is in `glib::VariantStrIter`, which Clay does not call | Tauri/wry (GTK3 → GTK4 migration is upstream work) | GTK4 releases of gtk-rs (`https://github.com/gtk-rs/gtk4-rs`); tracked as the accepted cost of WebKitGTK packaging | Accepted while Linux ships the GTK3 webview shell per platform policy (`docs/development/windows.md` keeps Windows a long-term target); recheck with every Tauri upgrade; hard recheck **2026-12-28** |

## Remediated vulnerabilities (no longer in the lockfile)

| Advisory | Crate | Fix | Evidence |
| --- | --- | --- | --- |
| RUSTSEC-2026-0221 (unsound: `!Send` tags across threads via `StackSlot`) | event-listener 5.4.1 | Upgrade to `event-listener 5.4.2` (plan 086 task 6, 2026-08-14; smol-rs PR 163); single lockfile copy, same zbus/AT-SPI path | `cargo audit` no longer reports it; `cargo tree -i event-listener` shows 5.4.2 only |
| RUSTSEC-2026-0233 (use-after-free during deserialization), RUSTSEC-2026-0234 (insufficient archive validation in hash tables), RUSTSEC-2026-0235 (out-of-bounds reads in Rc/Arc) | rkyv 0.8.16 | Upgrade to `rkyv 0.8.17` (plan 086 task 5, 2026-08-14); patch release reworks shared-pointer metadata validation (`validation/shared`) and swiss-table element-count validation (`collections/swiss_table`) | `cargo audit` reports 0 vulnerabilities; malformed/truncated/misaligned corpus sweeps in `src/protocol/codec.rs` assert rejection dominance plus panic-free bytechecked decode; single decode boundary remains `Codec::decode_frame` with generic `CheckBytes` bounds |
| RUSTSEC-2026-0194, RUSTSEC-2026-0195 (quick-xml 0.39.4 unsoundness/panic) | quick-xml 0.39.4 | Removed with the native client dependency chains in Plan 097 Phase 12 (`winit`, `masonry_winit`, `accesskit_winit` deleted); remaining `quick-xml` is fixed 0.41.0 via `tauri`/Linux shell tooling | Lockfile contains `quick-xml 0.41.0` only; `.cargo/audit.toml` ignores were removed |

## Process

1. `cargo audit` runs in CI on every push/PR and must pass.
2. New vulnerabilities are remediated by lockfile/direct-dependency upgrade
   first. If an upstream constraint proves blocking, add a narrow exception to
   `.cargo/audit.toml` plus a record here with all fields shown above.
3. Renewing an exception requires re-running `cargo tree -i <crate>` and
   re-checking the upstream reference; the renewal evidence goes in the
   record.
4. An expired exception fails `cargo test -p clay --test primitives_docs`
   (and therefore the full test gate) until renewed or removed.

## Tauri desktop shell (Phase 11)

The webview is not a general-purpose browser:

- CSP is `default-src 'none'` with `script-src 'self'` and the single
  sanctioned `connect-src ipc: http://ipc.localhost` shim. No remote
  `http(s)`/`ws` origins.
- Capability `main` grants `core:default` only. Filesystem, shell, process,
  HTTP, dialog, and updater plugins are not compiled and must not appear in
  `tauri.conf.json`.
- `CLAY_ENDPOINT` may name a local Unix socket or Windows named pipe so a
  container or already-running `clay server` can be adopted. Network URLs are
  rejected. The shell never opens a TCP listener.
- There is no in-app updater. `accept_update` rejects unsigned, wrong-target,
  and non-newer manifests; signing keys stay outside the repository.
- Secrets stay in the Clay server / `clay-agent` vault. They do not cross the
  Tauri event channel.

Release checks: `scripts/security-audit.sh` and `src-tauri/tests/config_security.rs`.
