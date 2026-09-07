# Icon Pack Primitive Review and Generic Contract (Plan 112 Task 2)

Status: finalized contract for Plan 112. Source of truth for the semantic key list, bounded vector schema, budgets, lifecycle/fallback table, trust rules, and API names. The plan's Contract Targets section summarizes this document; where wording differs, this document wins.

Pinned upstream: `phosphor-icons/core` release **v2.0.8** (verified via GitHub API `releases/latest` on 2026-09-07; asset layout `assets/regular/<name>.svg` and `assets/duotone/<name>-duotone.svg`, MIT license). All downstream references in Plan 112 tasks replace "exact release TBD" with v2.0.8.

## 1. Primitive inventory (reusable today)

| Existing primitive | Where | Reuse for icon packs |
|---|---|---|
| Core tokens `dimension.icon.size` (16) and `text.icon` | `src/shell/theme.rs` `core_theme_value` | Icon glyph sizing and color authority. No new token needed. |
| Recipe slots `button.icon`, `dropdown.indicator`, `collapse.chevron`, `iconSlot` | `docs/development/ui-design-system-recipe-matrix.md` (button `icon` scaled to `dimension.icon.size`; iconSlot: inline SVG, `text.icon`/`opacity.disabled`, `aria-hidden` or `img`) | The design-system contract for icons already exists on paper; Tasks 7/8 ship the React consumers. |
| `ClayButton` (variants default/muted/primary/danger, React Aria) | `frontend/src/components/button.tsx` | Icon-only and icon+label actions compose onto ClayButton; no parallel widget. |
| Inert package component kinds (`button`, `list`, `label`, `dropdown`, `collapse`, `modal`, …) | `src/shell/components.rs`; DTO `src-tauri/src/bridge/dto.rs`; React `frontend/src/sdui/types.ts`, `registry.tsx` | Optional semantic icon fields added to existing kinds; no new kind. |
| Design-system activation pattern | `src/server/ops/theme.rs` `apply_design_system` (L260–399): non-reentrant lock discipline, first-party via `ensure_first_party_record_locked`, third-party via `service.enable`, descriptor from `record.contributions` | `setIconPack` copies this pattern exactly, including the plan-110 lock-reentrancy lesson. |
| Facade/op/doc-registry machinery | `runtime/js/theme.js` + `.d.ts`, `docs/reference/clay-js-api/api-inventory.toml`, generated registry | `setIconPack` becomes a sibling of `setTheme`/`setDesignSystem`. |
| Contribution validation + budgets | `src/packages/record/mod.rs` (`assemble_package_record`), `src/perf/budgets.rs` (`UI_DESIGN_SYSTEM_PAYLOAD_BUDGET_BYTES = 64 KiB`) | Icon-pack contribution validated at load with its own bounded schema/budgets. |
| Bundled inventory + extension-point scope checks | `src/packages/bundled.rs` L371–448, `build.rs` | Icon packs embed through the existing compiled inventory; bundled scope test extended to iconPack contributions. |
| Bounded contribution deny-list precedent (raw CSS/colors/URLs rejected) | `src/packages/record/mod.rs`, `src/shell/components.rs` style-variable validation | Same reject philosophy for raw SVG/URLs/CSS. |
| Trust domains (two runtimes, typed inert crossings) | `.agents/skills/project-patterns/references/authority-boundaries.md` | Third-party icon packs ride the adopted-package path; no new authority. |

## 2. Generic gaps (new work, all inside this plan)

1. **No icon renderer exists.** `src/shell/primitives.rs` / native `paint_icon_slot` were removed with the native client; the React client has no `ClayIcon` component and no icon CSS variables (verified: zero `icon` matches in `frontend/src/components/*`, `frontend/src/styles/tokens.css`, theme adapters). Task 7 adds `frontend/src/components/icon.tsx` + `icon.module.css` consuming `dimension.icon.size`/`text.icon` tokens and the `iconSlot` recipe.
2. **No tooltip composition exists.** Catalog marks Tooltip planned; icon-only controls require hover+focus help. Task 7 adds a minimal accessible tooltip (React Aria `Overlay`/`Tooltip`) only if existing overlays are insufficient — verified none exist, so tooltip is confirmed new.
3. **No semantic icon field on package-facing declarations.** Native SDUI list rows (`src/protocol/sdui.rs`) and the package component JSON DTO path carry text/detail/action only. Task 3 adds optional `icon` semantic-reference fields additively.
4. **No icon contribution kind.** `clay.contributions.iconPack` is new manifest data with its own validator (Task 3) and first-party packages (Task 4).

## 3. Finalized semantic key set (21 required keys)

Every key verified to exist in **both** regular and duotone assets at v2.0.8 (HTTP 200 checked per file). No speculative keys; keys without a concrete consumer were removed.

| Semantic key | Regular asset | Duotone asset | Consumers (Plan 112 tasks) |
|---|---|---|---|
| `action.close` | `x` | `x-duotone` | tab close, modal close, panel header close (T7/T8) |
| `action.new` | `plus` | `plus-duotone` | tab-strip `+` (T8) |
| `document.save` | `floppy-disk` | `floppy-disk-duotone` | editor toolbar (T8) |
| `document.reload` | `arrows-clockwise` | `arrows-clockwise-duotone` | editor toolbar (T8) |
| `document.open` | `folder-open` | `folder-open-duotone` | empty Files view "Open file/Open folder" (T8/T9) |
| `message.send` | `paper-plane-right` | `paper-plane-right-duotone` | chat + agent composers (T8) |
| `generation.stop` | `stop` | `stop-duotone` | existing generation-stop/abort path only (T8; absent → key ships but stays unused) |
| `navigation.back` | `arrow-left` | `arrow-left-duotone` | agent Context/Memory detail back rows (T8) |
| `navigation.up` | `arrow-up` | `arrow-up-duotone` | file browser parent row (T9) |
| `disclosure.right` | `caret-right` | `caret-right-duotone` | dropdown trigger indicator, collapsed disclosure (T7/T8) |
| `disclosure.down` | `caret-down` | `caret-down-duotone` | dropdown open indicator, expanded disclosure (T7/T8) |
| `file.folder` | `folder` | `folder-duotone` | file browser directory rows (T9) |
| `file.file` | `file` | `file-duotone` | file browser file rows (T9) |
| `file.symlink` | `link` | `link-duotone` | file browser symlink rows (T9) |
| `session.resume` | `play` | `play-duotone` | recent-session compact Resume action (T8) |
| `session.search` | `magnifying-glass` | `magnifying-glass-duotone` | empty Files "Search sessions…" (T8) |
| `git.branch` | `git-branch` | `git-branch-duotone` | git status branch label (T9) |
| `status.success` | `check-circle` | `check-circle-duotone` | git/status labels (T9) |
| `status.warning` | `warning` | `warning-duotone` | git/status labels (T9) |
| `status.error` | `x-circle` | `x-circle-duotone` | git/status stale/error labels (T9) |
| `preview.toggle` | `eye` | `eye-duotone` | markdown preview toggle (T9) |

Package-specific meanings continue to use the declaring package's prefix (`<package>.<key>`); the 21 core keys are reserved to core.

## 4. Bounded vector schema (finalized)

One icon = versioned record, normalized at build time from upstream SVG (no runtime XML parsing):

```text
IconGeometry {
  schemaVersion: u32 = 1          // reject unknown versions
  viewBox: [f32; 4]               // side length in [1, 512]; all four finite
  paths: Vec<IconPath>            // 1..=8 paths; non-empty geometry required
}
IconPath {
  d: string                       // bounded SVG path data, ≤ 2048 bytes
  opacity: Option<f32>            // finite [0,1]; duotone shade layer carries 0.2
}
Parsed commands = absolute M | L | C | Q | H | V | A | Z only
  (A = rx, ry, rotation, large-arc/sweep flags 0/1, x, y; radii ≥ 0)
```

- Contributions ship bounded `d` strings (the upstream-native encoding, so the
  Task 4 generator passes Phosphor data through verbatim); the load-time
  validator parses them once into absolute commands. The full SVG 1.1 command
  set (MmLlHhVvCcSsQqTtAaZz) is accepted — relative forms, implicit repetition,
  and S/T reflection are normalized into absolute M/L/C/Q at parse (arc
  support is required: Phosphor rounds every corner with `a` arcs). Runtime
  never re-parses path data; the record and wire snapshot carry parsed
  geometry.

- Foreground color is never in geometry; renderers use `currentColor` / `text.icon`-derived theme roles. Duotone is Regular geometry plus additional lower-opacity background paths (verified upstream structure), so a **single** color authority holds for both styles.
- Upstream `fill="currentColor"` and `viewBox="0 0 256 256"` are normalized into the record; no `style`, `class`, `fill-rule` overrides, stroke geometry, URLs, or `transform` are accepted.
- Build-time validation rejects any upstream file whose structure does not normalize into this schema (Task 4 checks per file with integrity records); the load-time parser applies the identical bounds, so drift fails deterministically (Task 4 tests).

## 5. Budgets (measured, then bounded)

Measured at v2.0.8 (15 representative glyphs per weight, this review's samples):

| Metric | Regular raw SVG | Duotone raw SVG |
|---|---|---|
| min | 214 B (`caret-down`) | 267 B (`caret-right`) |
| max | 701 B (`eye`) | 841 B (`eye`) |
| mean | ≈ 378 B | ≈ 489 B |

Normalized JSON-with-numbers inflates roughly 1–2× the `d` string. Budgets (new constants in `src/perf/budgets.rs`, Task 3):

- `ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES = 2048` per icon (normalized), checked before allocation.
- `ICON_PACK_PAYLOAD_BUDGET_BYTES = 64 * 1024` per pack contribution — mirrors the existing `UI_DESIGN_SYSTEM_PAYLOAD_BUDGET_BYTES` precedent; the full 21-key duotone set normalizes well under 32 KiB.
- Pack icon count ≤ 64 (required set is 21; headroom for third-party extras, still bounded).
- Active-icon wire snapshot: same 64 KiB bound, generation-stamped, sent only when the active pack changes (Task 6) — never per row, node, keystroke, or frame. Semantic references in UI nodes are short strings and ride existing node budgets (a node's `icon` field counts against the existing SDUI/tree payload ceilings; the plan raises no unrelated limits).
- No lookup IPC, no hot-path parsing, no dependency-heavy renderer: the client holds one cached active map; render reads geometry from memory.

## 6. Loading, selection, and lifecycle (finalized state table)

Selection is **global user appearance choice over host-owned icon rendering** — the same authority class as `setDesignSystem`. It is not a package capability, grants no `package-control`-like permission, and never mutates another package.

| # | Event | Result |
|---|---|---|
| 1 | Startup, no selection in config | Bundled Regular safety subset active (generated from the Regular package's source, not a second map). No package load entry executes. Zero `init.js` lines give working icons. |
| 2 | `await loadPackage("@clay/icons-phosphor-regular")` | Registers the pack's inert `iconPack` contribution. Does **not** change active icons (load ≠ select; load order never wins). Idempotent. |
| 3 | `setIconPack("@clay/icons-phosphor-regular")` | Resolves the pack from already-enabled records; atomically swaps the active generation; returns `{ pack, iconCount, schemaVersion }`. |
| 4 | `setIconPack` with unknown / not-enabled / non-icon-pack specifier | Rejection (`theme.invalid_request` / `theme.load_failed` / `theme.invalid_icon_pack`); previous active snapshot preserved untouched. |
| 5 | `setIconPack` for an adopted third-party pack | Same as 3; adoption itself happens only through the existing enable/trust path. Selection never adopts or promotes trust. |
| 6 | Required configuration reload re-selecting a pack | New generation committed atomically; failure preserves the previous valid generation with a sanitized diagnostic. |
| 7 | Pack disable / remove / revocation while active | Revoked geometry withdrawn immediately; active state falls back to the bundled Regular subset. Late/stale generations cannot restore revoked data. |
| 8 | Active pack missing a required core key | That glyph falls back to the bundled Regular asset; control never goes blank (label preserved per Placement and Label Policy). |
| 9 | Unknown package-only decorative key | Omitted; consumers show text/label instead. No arbitrary fallback chain. |
| 10 | Duplicate `loadPackage` of same pack | Idempotent, no new generation. |
| 11 | Multiple packs loaded, none selected | All registered; icons remain on the previous selection (or subset fallback). |
| 12 | Multiple packs loaded, one selected | Only the selected pack's geometry is active/transported; others stay inert registration data. |
| 13 | Generation ordering | Monotonic generation counter; any stale snapshot/DTO arriving after a newer one is rejected (Tasks 5/6). |

## 7. Security (finalized deny cases and trust rules)

- **Deny at validation (Task 3), before allocation-heavy parsing:** raw SVG/XML strings, CSS, `url(...)`/external references, `<script>`/event handlers, `foreignObject`, `use`/`image`/`filter`/animation elements, unsupported path commands (relative forms, arcs `A` beyond schema, binary shortcuts), non-finite or out-of-range numbers, malformed/truncated commands, empty or invisible required glyphs, invalid viewBoxes, duplicate/prototype-sensitive keys (`__proto__`, `constructor`), keys impersonating another package's namespace, unknown `schemaVersion`, oversized geometry/packs.
- **Provenance:** first-party status comes from the compiled inventory plus exact integrity, never the `@clay/` prefix. Third-party packs carry exact adopted-package provenance through the ordinary enable path.
- **No action spoofing:** packages supply geometry and semantic references only. Hosts own accessible names, roles, states, focus, and action routing. An icon reference never changes what a control does; icon-only controls always have host-controlled accessible names and (new) tooltip text.
- **No new authority:** no filesystem, network, shell, WASM, raw ops, renderer callbacks, client JavaScript, package-manager, AI, or native-widget authority is introduced anywhere in the pipeline (converter runs at build time on pinned inputs; runtime sees only validated inert data).
- **Revocation:** state 7 above; withdrawal is immediate and stale-data-proof.

## 8. API naming (finalized per clay-js-api-naming.md)

| Layer | Value |
|---|---|
| JS module | `clay:theme` (existing domain module; no new module) |
| JS callable export | `setIconPack` |
| Stable registry ID | `theme.setIconPack` (reserved core domain `theme`; no `clay.` prefix) |
| `user_facing_name` | `Set Icon Pack` |
| Manifest contribution | `clay.contributions.iconPack` |
| Return | `{ pack: string, iconCount: number, schemaVersion: number }`, synchronous after validation |

`setIconPack` follows the `setDesignSystem` precedent: pass a specifier string or `{ specifier }`; errors use the `theme.*` error namespace (`theme.invalid_request`, `theme.load_failed`, `theme.invalid_icon_pack`). Package-owned icon keys must begin with the package's `apiPrefix`.

## 9. Transport (finalized)

- One bounded, validated, generation-stamped **active icon snapshot** (pack identity, provenance, generation, geometry map) published through the existing runtime state snapshot fan-out on connect/reload/reconnect; UI nodes carry semantic references only.
- Server validates before wire publication; DTO/render boundaries validate defensively and reject unknown fields/stale generations. Wire-format changes require rebuilding matched server/client binaries (existing protocol version discipline).
- Client: one narrow icon store holding the last authorized snapshot; transient disconnection retains it; revocation follows server state, not client guesses.

## 10. Integration files (corrected to current tree)

Plan tasks previously carried tentative/native-era paths. Corrections confirmed by this review:

- `src/shell/primitives.rs` **no longer exists** (native client removed). Icon rendering is React-only: `frontend/src/components/icon.tsx` / `icon.module.css` (new, Task 7), consuming token CSS variables projected by the existing theme/design-system adapters.
- Rust geometry model/validator: `src/shell/icons.rs` (new, Task 3) is confirmed as the right home next to `src/shell/theme.rs`/`components.rs`; `src/shell/mod.rs` gains the module declaration.
- Tooltip: confirmed **new** component `frontend/src/components/tooltip.tsx` / `tooltip.module.css` (Task 7) — no existing reusable tooltip was found.
- Everything else in the plan's file lists (protocol, record, bundled inventory, theme ops, DTO, stores, controls) matched the current tree.

## 11. Options considered (summary)

1. **Phosphor React components in the UI** — rejected: prevents inert third-party replacement, duplicates library coupling in the webview.
2. **Arbitrary SVG/URL assets from packages** — rejected: executable/resource authority, parser complexity, raw-CSS-class risks.
3. **Icon fonts** — rejected per plan scope; binding geometry to font stacks breaks theme authority and offline guarantees.
4. **Per-feature bespoke icons** — rejected: duplicates a11y/focus/disabled bugs per surface.
5. **Bounded normalized geometry + semantic references + shared host rendering** — chosen; measured costs above fit existing budget precedents.
6. **Lucide as the second family** — rejected earlier (plan scope): two outline families give less differentiation than outline vs duotone.

## 12. Approvals and decision-log status

- The icon-pack direction (Phosphor Regular default, Duotone alternate, user-owned selection, placement policy) was requested and approved by the user through Plan 112's creation; this review finalizes internals within that approved scope. **No new stable policy outside the approved proposal is introduced**, so no additional approval is required to proceed.
- Follow-up (not a blocker): after implementation passes verification, the durable icon-pack architecture decision should be recorded via `create-decision-log` with explicit user approval — do not log it preemptively from this review.

## 13. Test-contract checklist for implementation tasks

- Both consumers covered by every contract row: native SDUI tree (`src/protocol/sdui.rs` → registry) and package component JSON (`dto.rs` → `frontend/src/sdui/types.ts` → `registry.tsx`); a fixture asserting the same semantic reference renders identically through both paths (Tasks 3/9).
- Every required key maps to real assets in both pinned weights (verified here; Task 4 re-verifies per file at conversion with checksums).
- Lifecycle table §6 rows 1–13 each have a named test in Tasks 5/6/10.
- Budget boundary tests: zero/min/max/max+1 for every §5 constant (Tasks 3/10).
- Hostile corpus from §7 deny list fails closed with deterministic diagnostics (Tasks 3/10).
