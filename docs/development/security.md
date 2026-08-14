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

### RUSTSEC-2026-0194 — quick-xml `escape::PartiallyEscaped` unsoundness

- **Dependency paths:**
  1. `clay → winit 0.30.13 → sctk-adwaita → smithay-client-toolkit →
     wayland-scanner 0.31.10 (proc-macro) → quick-xml 0.39.4`
     (also via `copypasta/smithay-clipboard` and `wayland-client`).
  2. `clay → masonry_winit 0.4.0 → accesskit_winit 0.29 → accesskit_unix 0.17
     → atspi 0.25 → zbus-lockstep 0.5 → zbus_xml 5.1.1 → quick-xml 0.39.4`.
- **Runtime reachability:** path 1 is build-time only (the proc-macro parses
  the checked-in Wayland protocol XML shipped with `wayland-protocols`; no
  untrusted input ever reaches it, and `quick-xml` is not linked from that
  path). Path 2 is linked into the binary and parses D-Bus introspection XML
  from the AT-SPI accessibility bus at runtime; inputs come from same-UID
  session-bus peers. The unsoundness is in the `escape::PartiallyEscaped`
  lifetime API and requires the calling crate to misuse that API; neither
  `wayland-scanner` nor `zbus_xml 5.1.1` is known to exercise it.
- **Upstream remediation:** `wayland-rs` merged the `quick-xml 0.41` bump
  (Smithay/wayland-rs PR #938, 2026-07-08) but has not published a release
  (`wayland-scanner` 0.31.10 is the newest on crates.io). `zbus_xml 5.2.1`
  removed `quick-xml` in favour of `winnow`, but the `accesskit/atspi` chain
  carrying it requires `accesskit_winit 0.33`, which `masonry_winit 0.4.0`
  (newest release) does not yet permit.
- **Owner:** dependency maintenance (recheck at each upstream release).
- **Expiry:** 2026-10-22 — by then either a `wayland-rs` release with PR #938
  or a masonry/accesskit release line should exist; renew only with a
  rechecked `cargo tree` and upstream release status.

### RUSTSEC-2026-0195 — quick-xml reader `get_text_position` panic

- **Dependency paths:** identical to RUSTSEC-2026-0194 above.
- **Runtime reachability:** the panic triggers on malformed XML reader input.
  On the build-time path the input is the checked-in Wayland protocol XML
  (trusted). On the D-Bus path, a hostile same-UID process on the AT-SPI
  session bus could in theory send crafted introspection XML that panics the
  editor — a local denial of service only, with no memory-safety impact.
  Untrusted-input exposure is therefore limited to the local session bus and
  is accepted until the upstream chain updates. There is no D-Bus exposure
  beyond AT-SPI accessibility: Clay itself speaks no D-Bus.
- **Upstream remediation:** same as RUSTSEC-2026-0194 (no fixed 0.39.x
  release exists; 0.39.4 is the newest in the 0.39 line).
- **Owner:** dependency maintenance.
- **Expiry:** 2026-10-22.

| Advisory | Crate | Dependency path | Runtime reachability | Owner | Upstream reference | Disposition / recheck |
| --- | --- | --- | --- | --- | --- | --- |
| RUSTSEC-2025-0141 (unmaintained) | bincode 1.3.3 | `deno_core 0.400 → bincode` | V8 snapshot (de)serialization inside the embedded runtime; inputs are host-produced snapshots, not untrusted data | deno_core | bincode 3 rewrite (`https://git.sr.ht/~stygianentity/bincode`, README announcement) pins bincode 1.x | No compatible fix in 1.x; recheck with every `deno_core` upgrade; hard recheck **2026-12-16** |
| RUSTSEC-2024-0436 (unmaintained) | paste 1.0.15 | `v8 → paste` (proc-macro) | Build-time macro expansion only; no runtime reachability | v8 (deno/rusty_v8) | dtolnay/paste (`https://github.com/dtolnay/paste`) — maintained fork via `paste-impl` upstreamed into proc-macro2 | No action available; recheck with every `v8` upgrade; hard recheck **2026-10-07** |
| RUSTSEC-2026-0192 (unmaintained) | ttf-parser 0.25.1 | `winit → sctk-adwaita → ab_glyph → owned_ttf_parser → ttf-parser` | Parses bundled window-decoration fonts at runtime; fonts ship with the dependency, not from untrusted sources | sctk-adwaita / ab_glyph | harfbuzz/ttf-parser (`https://github.com/harfbuzz/ttf-parser/issues/217`) — successor `read-fonts` | Migration is an `ab_glyph`/`sctk-adwaita` decision; recheck with every winit-chain upgrade; hard recheck **2026-12-28** |

## Remediated vulnerabilities (no longer in the lockfile)

| Advisory | Crate | Fix | Evidence |
| --- | --- | --- | --- |
| RUSTSEC-2026-0221 (unsound: `!Send` tags across threads via `StackSlot`) | event-listener 5.4.1 | Upgrade to `event-listener 5.4.2` (plan 086 task 6, 2026-08-14; smol-rs PR 163); single lockfile copy, same zbus/AT-SPI path | `cargo audit` no longer reports it; `cargo tree -i event-listener` shows 5.4.2 only |
| RUSTSEC-2026-0233 (use-after-free during deserialization), RUSTSEC-2026-0234 (insufficient archive validation in hash tables), RUSTSEC-2026-0235 (out-of-bounds reads in Rc/Arc) | rkyv 0.8.16 | Upgrade to `rkyv 0.8.17` (plan 086 task 5, 2026-08-14); patch release reworks shared-pointer metadata validation (`validation/shared`) and swiss-table element-count validation (`collections/swiss_table`) | `cargo audit` reports 0 vulnerabilities; malformed/truncated/misaligned corpus sweeps in `src/protocol/codec.rs` assert rejection dominance plus panic-free bytechecked decode; single decode boundary remains `Codec::decode_frame` with generic `CheckBytes` bounds |

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
