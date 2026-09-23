# Icon Pack Runtime

**Files:** `src/shell/icons.rs`, `src/packages/record/icons.rs`, `src/packages/extension_points.rs`, `src/server/ops/theme.rs`, `src/server/mod.rs`, `src/server/configuration.rs`, `src/protocol/runtime.rs`, `src/shell/file_browser.rs`, `src-tauri/src/bridge/dto.rs`, `runtime/js/theme.js` (+ `.d.ts`), `frontend/src/icons/`, `frontend/src/state/icon-store.ts`, `frontend/src/components/icon.tsx`, `frontend/src/components/button.tsx`, `frontend/src/components/tooltip.tsx`, `frontend/src/components/icon.module.css`, `scripts/generate-icon-packs.mjs`, `packages/icons-phosphor-regular/`, `packages/icons-phosphor-duotone/`, `packages/icon-sources/`  
**Tests:** `tests/icon_packages.rs`, `tests/runtime_update_protocol.rs`, `src-tauri/tests/dto_roundtrips.rs`, `src/shell/icons.rs` (unit), `src/shell/file_browser.rs` (unit), `src/server/ops/theme.rs` (unit), `src/server/js_runtime/tests/` (`plan112_*`), `frontend/src/test/icons.test.tsx`, `frontend/src/test/design-system-consumption.test.ts`  
**Reference Docs:** `docs/reference/clay-js-api/theme/set-icon-pack.md`, `docs/reference/icon-packs.md`, `docs/reference/ui-components.md` (Plan 112 section), `docs/reference/packages/creating-packages.md`, `docs/development/icon-pack-primitive-review.md`

---

## 1. Overview and Responsibilities

The Icon Pack Runtime is the host-owned system for configurable UI icon styles.
Packages contribute **inert, bounded, host-validated vector geometry**; Clay
owns rendering, color, and interaction. Users select one active pack globally
via `setIconPack` (mirroring `setDesignSystem`), and every UI surface requests
icons by **semantic key** (`action.close`, `document.save`, …), never by
upstream library name.

Responsibility split:

- **Packages** supply bounded geometry data in `package.json`
  (`clay.contributions.iconPack`) with pinned upstream provenance. No scripts,
  no JSX, no CSS, no URLs, no rendering authority.
- **Host** validates, resolves, version-stamps, transports, and renders.
  Bundled first-party packs ship in the compiled inventory; a generated
  fallback subset renders with zero configuration.

Non-responsibilities: packs cannot change color (active theme is the sole
color authority), cannot alter layout or interaction, and cannot execute
anything at load or render time.

## 2. Bounded Geometry Schema (schemaVersion 1)

Core types (`src/shell/icons.rs`):

```rust
pub const ICON_SCHEMA_VERSION: u32 = 1;
pub const MAX_ICON_PATHS: usize = 8;
pub const MAX_PATH_COMMANDS: usize = 512;
pub const MAX_ICONS_PER_PACK: usize = 64;      // per pack
pub const MAX_VIEWBOX_SIDE: f32 = 512.0;
pub const MAX_ICON_COORDINATE: f32 = 4096.0;

pub struct IconGeometry { pub view_box: [f32; 4], pub paths: Vec<IconPath> }
pub struct IconPath { pub commands: Vec<IconPathCommand>, pub opacity: Option<f32> }
pub enum IconPathCommand { MoveTo, LineTo, CurveTo, QuadTo, ArcTo, HorizontalTo, VerticalTo, ClosePath }
```

- Validation (`validate_icon_geometry`): square viewBox side in `[1, 512]`,
  1–8 paths, each path 1–512 commands, finite coordinates in
  `[-4096, 4096]`, opacity finite in `[0, 1]`, and a **moveto-first
  invariant** enforced at the wire boundary (first command must be
  `MoveTo`).
- Wire format is per-path SVG `d` strings (≤ 2048 bytes per icon,
  64 KiB per pack). `parse_icon_path` normalizes relative commands, implicit
  repetition, and SVG smooth forms (`S`/`T` folded to `CurveTo`/`QuadTo` by
  control-point reflection) into the absolute 8-command set. Arcs are
  required — Phosphor rounds corners with `a` arcs.
- Types containing `f32` cannot derive `Eq`; the codebase pattern is a manual
  `impl Eq for T {}` documented by the invariant that the parser/validator
  rejects NaN/infinities. rkyv uses plain derives (no `rkyv(derive(...))`;
  the attributed form leaves `ArchivedVec`/`ArchivedBTreeMap` trait gaps).

## 3. Contribution, Provenance, and Trust

- Contributions parse in `src/packages/record/icons.rs` following the
  theme-record pattern: payload budget check (`ICON_PACK_PAYLOAD_BUDGET_BYTES`
  = 64 KiB — the duotone 21-key manifest exceeds the 8 KiB behavior budget),
  forbidden-field deny list (`svg`, `url`, `style`, `fill`, `script`, …),
  strict schema validation, and package-namespace checks. Geometry is
  inlined directly in `package.json`; there is no separate icons file.
- Core semantic keys (22: `action.close`, `action.new`, `control-center.open`,
  `document.save`,
  `document.reload`, `document.open`, `message.send`, `generation.stop`,
  `navigation.back`, `navigation.up`, `disclosure.right`, `disclosure.down`,
  `file.folder`, `file.file`, `file.symlink`, `session.resume`,
  `session.search`, `git.branch`, `status.success`, `status.warning`,
  `status.error`, `preview.toggle`) may only resolve from compiled-inventory
  first-party records (`ActiveIconPack::from_record` checks bundled name +
  version). Third-party packs are limited to own-prefixed keys.
- First-party packages are generated by `scripts/generate-icon-packs.mjs`
  from vendored upstream SVGs (`packages/icon-sources/phosphor-2.0.8/`,
  sha256-recorded) into `@clay/icons-phosphor-regular` (recommended default)
  and `@clay/icons-phosphor-duotone` (same 22 keys; duotone = regular
  geometry + background paths at opacity 0.2, so one color authority covers
  both). `--check` verifies byte-deterministic regeneration; the same
  pipeline emits `frontend/src/icons/fallback.generated.ts` (the zero-config
  fallback subset).

## 4. Selection Lifecycle (load ≠ select)

Mirrors the design-system lifecycle (`src/server/ops/theme.rs`,
`src/server/mod.rs`):

1. `setIconPack(specifier)` — `clay:theme` facade over
   `op_clay_theme_set_icon_pack`, registered in the **trusted extension
   only** (97 ops vs 46 in the package extension); package callers cannot
   change user-global selection.
2. `apply_icon_pack` resolves the record: first-party via the held package
   mutex (`ensure_first_party_record_locked`); third-party only from
   already-enabled records — selection never installs, adopts, or promotes
   trust. Errors: `theme.invalid_request`, `theme.load_failed`,
   `theme.invalid_icon_pack`.
3. Prepare phase revalidates the selected/previous pack against enabled
   records (revocation → fallback to `None`), stamps the runtime generation,
   and the commit guard treats the icon pack as part of the generation
   fingerprint.
4. Transport: `RuntimeStateSnapshot.active_icon_pack`
   (`src/protocol/runtime.rs`, rkyv + serde, wire-validated) →
   `IconPackSnapshotDto` (`src-tauri/src/bridge/dto.rs`, defensive
   re-validation, f32→f64 widening) → `frontend/src/state/icon-store.ts`
   (`IconStore` singleton: generation-based stale/identical no-ops, bounded
   validation constants, `resetToFallback` on disconnect).
   `BootstrapDto.active_icon_pack` is `None` — honest value until the first
   runtime snapshot (mirrors the design-system core-fallback treatment).
5. Persistence: `iconPack` preference key (`src/server/configuration.rs`)
   with a deliberately lenient specifier validator; selection is
   re-validated at every commit and revoked packs fall back to the bundled
   Regular subset. Apply-on-reload re-applies the preference with a sanitized
   diagnostic on rejection (previous valid generation preserved).

## 5. Rendering and Accessibility

- `ClayIcon` (`frontend/src/components/icon.tsx`) renders bounded pack
  geometry as inline SVG: decorative by default (`aria-hidden`),
  `role="img"` + `aria-label` when labeled; `fill="currentColor"` only —
  zero per-path color/style/class output, so the active theme stays the sole
  color authority.
- Sizing: `max(var(--clay-dimension-icon-size, 16px), 1em)` inline-block —
  the core token is the floor and glyphs scale with large typography.
  `display: inline-block` (not `block`) keeps icons inline with label text.
- `ClayIconButton` (`button.tsx`) requires `label` at the type level
  (unlabeled icon-only usage is impossible), wraps RAC Button in
  `ClayTooltip` (label + optional shortcut), keeps a ≥ 24px hit target, and
  falls back to a visible text label when the key resolves to no geometry —
  a failed or sparse pack can never leave a blank control.
- Recipe slots (`iconSlot`, `tooltip`) consume core tokens only for iconSlot
  (`--clay-text-icon`, `--clay-dimension-icon-size`, `--clay-opacity-disabled`)
  and `--clay-ds-*` recipes for tooltip; enforced by
  `frontend/src/test/design-system-consumption.test.ts`.

## 6. Semantic References on Host Surfaces

- Native SDUI (`src/shell/file_browser.rs`, `src/protocol/sdui.rs`,
  `src/server/ops/sdui.rs`): optional `icon` field on labels, buttons, and
  list items; runtime conversion validates core keys (runtime trees carry no
  declaring-package identity). File-browser rows map kind →
  `file.folder`/`file.file`/`file.symlink`, parent row → `navigation.up`.
- Package components (`shell/components.rs` + record/ui.rs + server/ui.rs):
  the optional `icon` field is validated at record time with package
  identity, so package-owned keys are possible there; component JSON flows to
  the client verbatim (no DTO change).
- First-party package scripts use semantic keys with text labels carrying
  full meaning independently (`packages/git/dist/status.js`: `git.branch`,
  `status.*`; `packages/markdown/dist/sdui.js`: `preview.toggle`).

## 7. Invariants and Budgets

- Icons are inert data: no scripts, events, URLs, CSS, filters, animations,
  `foreignObject`, raw SVG/XML, or literal colors (denied at record
  validation and at the wire boundary).
- Budgets: ≤ 2048 B per icon `d` data, ≤ 64 KiB per pack, ≤ 64 icons/pack,
  schemaVersion pinned to 1 (unsupported versions rejected).
- One generation-stamped wire snapshot is sent on pack change only; unchanged
  config reloads re-send no icon state (persistent worker module cache).
- Adding a pack: vendor upstream SVGs, run
  `node scripts/generate-icon-packs.mjs` (or author a manifest with
  own-prefixed keys for third-party), load with `loadPackage`, select with
  `setIconPack`. Adding a core key: extend `CORE_ICON_KEYS`, regenerate both
  packs + fallback, update the record validator, renderer fallbacks, and the
  icon suites.

## 8. Tests

- `tests/icon_packages.rs`: key equality across packs, license presence,
  fallback determinism, regeneration drift (`--check`).
- `tests/runtime_update_protocol.rs`: snapshot round-trip identity + invalid
  pack rejection; `src-tauri/tests/dto_roundtrips.rs`: DTO projection +
  malformed-geometry fail-closed.
- `src/server/js_runtime/tests/` `plan112_*`: load-then-select, either-order
  selection, modular reload no-resend, preference persistence, zero-config
  fallback, unloaded third-party fail-closed.
- `frontend/src/test/icons.test.tsx`: geometry rendering, a11y contract,
  pack-matrix semantics, DOM continuity, no-color-authority leakage.
- Commands: `cargo test --lib shell::icons`, `cargo test --test icon_packages`,
  `pnpm --dir frontend test icons` (or `npm --prefix frontend run test`).

## Related

- [UI Design System Runtime](ui-design-system-runtime.md) (selection lifecycle mirror)
- [Package Loading](package-loading.md) (record validation, trust domains)
- [React SDUI and Package UI Projection](react-sdui-package-ui.md) (icon field projection)
- [Frontend Theme Runtime](frontend-theme-runtime.md) (core token install)
- `docs/reference/icon-packs.md` (authoring guide), `docs/reference/clay-js-api/theme/set-icon-pack.md` (API)
