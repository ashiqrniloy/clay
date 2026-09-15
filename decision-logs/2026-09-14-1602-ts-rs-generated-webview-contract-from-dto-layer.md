---
date: 2026-09-14 16:02
status: approved
decision_about: "Single-definition webview contract: TypeScript generated from the Rust DTO layer with ts-rs; src/protocol untouched"
proposed_by: "agent (2026-09-14 architecture review §SC-1 and the codegen analysis), approved by user"
explicitly_approved_by_user: true
---

# Decision: Generate the webview TypeScript contract from the DTO layer with ts-rs

## Decision

The webview contract has exactly **one hand-written definition**: the serde-JSON DTO
layer — `src-tauri/src/bridge/dto.rs` (contract types and the `BridgeEnvelope`) plus
`src-tauri/src/bridge/errors.rs` (bridge error codes). TypeScript declarations are
**generated** from that layer with **ts-rs 12**, emitted into
`frontend/src/bridge/generated/`, checked into git, and validated by a staleness guard
in `scripts/check.sh`; hand-maintained TS mirrors of DTO fields are deleted as generated
output covers them.

Two boundaries do not move:

1. **`src/protocol` (rkyv) is untouched.** Core crates gain no TS-codegen derives; the
   Rust-to-Rust wire and the webview contract stay independent.
2. **The DTO layer stays a projection, not a copy.** Its narrowing (raw theme overrides
   never cross, package UI parsed Rust-side, identity stamped Rust-side) is a trust
   feature and is preserved.

## Context

`BridgeEnvelope` is what the webview observes. Its contract was defined by hand three
times:

1. **Core wire/JSON types** — `src/protocol` (rkyv for Rust-to-Rust) and the client
   layer's serde-JSON types (`clay::client::ClientConnectionEvent`,
   `src/client/mod.rs:882`), serialized 1:1 inside `BridgeEnvelope::Event`.
2. **The DTO layer** — `src-tauri/src/bridge/dto.rs` (910 lines, 20 `pub` structs/enums
   for bootstrap/theme/typography/design-system/icons/runtime snapshots plus
   `BridgeEnvelope` at `dto.rs:785`) and `src-tauri/src/bridge/errors.rs`
   (`BridgeErrorCode`, `BridgeError`).
3. **Hand-maintained TypeScript mirrors** — `frontend/src/bridge/types.ts` (239 lines),
   `frontend/src/theme/types.ts` (142), `frontend/src/icons/types.ts` (25),
   `frontend/src/sdui/types.ts` (227): 633 lines restating Rust fields by hand. Some are
   deliberately partial narrowings (`ShellEvent` types only the event families the shell
   consumes today and keeps `{ kind: string; data: unknown }` for the rest); others are
   straight copies, declared as such — `theme/types.ts` opens with "Typed mirrors of the
   Rust-resolved theme/typography projections (`src-tauri/src/bridge/dto.rs`)" and has
   already drifted: it makes `editorStyles` optional ("optional only for old fixtures")
   where `ThemeSnapshotDto` has it required.

Existing guards pin only the Rust side: `src-tauri/tests/dto_roundtrips.rs` has 15 serde
round-trip/validation tests with compile-time exhaustive `family` matchers, so a new
protocol family cannot be forgotten in Rust — yet the TS mirror is unchecked by
construction. That asymmetry is the drift class this decision removes.

The 2026-09-14 review (§SC-1, "triple-defined wire protocol") proposed generating the TS
from the DTO layer, and recorded the correction that this deletes the *third* copy, not
the DTO layer: the projection has a proving test
(`runtime_snapshot_parses_package_components_and_hides_raw_theme_overrides` asserts the
webview sees parsed package UI and that `activeTheme.overrides` is absent). Deleting the
DTO layer would widen the webview to internal protocol state — the opposite of the intent.

## Approval

- Proposed by: agent (review §SC-1; 2026-09-14 codegen analysis/recommendation).
- Approved by user: Yes.
- Approval evidence:
  - 2026-09-14 04:48 — "Create a plan document using @.agents/skills/create-plan/ to make
    the recommended changes at @code-reviews/2026-09-14-editor-and-agent-architecture-review.md
    considering the Recommendation: ts-rs codegen from the DTO layer, not the protocol
    layer for SC-1".
  - 2026-09-14 (same day, execution) — "Complete SC-1 decision log: ts-rs codegen from
    the DTO layer", recorded here; `plans/119-Editor-and-Agent-Architecture-Remediation.md`
    carries the same scope ("ts-rs codegen from the DTO layer, not the protocol layer" —
    `src/protocol` untouched).
  - The explicit exclusion of the protocol layer was re-stated by the user during review
    of the review, and is authoritative over any broader codegen reading.

## Alternatives Considered

1. **Status quo: keep hand-maintained TS mirrors** — rejected. It is a third definition
   that no test pins; `theme/types.ts` already relaxed a required Rust field, and every
   contract change costs three coordinated edits (Rust wire, DTO, TS).
2. **specta / tauri-specta** — rejected. Both need a second annotation system
   (`#[derive(Type)]`) plus a runtime type registry (`Types::default().register::<T>()`,
   `collect_types!`), and tauri-specta targets Tauri's `#[tauri::command]` invoke/event
   surface. Clay's webview contract is not command-shaped: it is a JSON frame stream
   (`BridgeEnvelope`) built in the Rust bridge and delivered by the agent relay, so
   adopting specta would buy a registry to maintain for the wrong shape.
3. **JSON Schema + quicktype** — rejected. Two-step (Rust → schema → TS), weaker
   serde-attribute fidelity than a derive that reads the same attributes that produce the
   wire JSON, and one more pipeline to keep green.
4. **Conformance tests without codegen** — rejected. Same derive-level annotation work as
   ts-rs, deletes nothing, keeps three definitions. The repository already has those tests
   (`dto_roundtrips.rs`), which is direct evidence that conformance tests do not prevent
   the TS mirror from drifting.
5. **Generate at frontend build time (Vite plugin / build script calling Rust)** —
   rejected. Frontend builds would require a Rust toolchain, and a build-time artifact
   hides contract changes from review; checked-in output plus a staleness check shows the
   diff and still fails CI when bindings are stale.
6. **Generate from the protocol layer (ts-rs derives in `src/protocol`)** — rejected by
   the user. The rkyv types are the internal Rust-to-Rust wire; generating the webview
   contract from them would leak internal protocol shape into the webview and delete the
   projection's narrowing. `src/protocol` stays untouched, and core crates keep no
   codegen dependency.

## Rationale and Evidence

**ts-rs facts verified (2026-09-14, ctx7 `/aleph-alpha/ts-rs`, crates.io 12.0.1):**

- `serde-compat` is on by default and parses the existing serde attributes —
  `rename_all`, `rename_all_fields`, `tag`, `content`, `untagged`, `skip`, `flatten` — so
  generated TS matches the wire JSON with no attribute duplication.
- serde `tag` + `content` maps to TS discriminated unions, which is exactly
  `BridgeEnvelope`'s existing
  `#[serde(tag = "kind", content = "data", rename_all = "camelCase", rename_all_fields = "camelCase")]`
  shape (`{ kind: "event"; data: ... }`).
- `#[derive(TS)] #[ts(export)]` emits hidden tests that regenerate on `cargo test`
  (`Type::export_all(&Config::from_env())`); `#[ts(export_to = "...")]` or `Config` choose
  the output location (path relative to `Cargo.toml`, trailing `/` = directory).
- ts-rs is not present anywhere in the tree today (`Cargo.lock` grep empty), and
  `clay-desktop`'s dev-dependencies are only `tempfile` — this is a greenfield, dev-only
  addition. Shipped binaries and the frontend toolchain are unaffected.

**Why the DTO layer is the right source of truth.** Generation reads the same serde
attributes that produce the browser-visible JSON, so the TS shape cannot diverge from the
wire without the wire itself changing — the property a hand mirror can never have. The
DTO layer is also the boundary the webview is *supposed* to see, so making it the single
definition strengthens, rather than weakens, the projection: nothing about the narrowing
is relaxed, because the DTO types themselves are unchanged.

**Cost is bounded and mechanical.** Add the derive to ~20 `dto.rs` types plus
`errors.rs`; host the export function in the existing `runtime_projection_tests` module
(no new harness). The plan's migration order is leaves first (`EditorStyleDto`,
`TypographySnapshotDto`, …), `BridgeEnvelope` last, with the frontend typecheck as the
per-step proof — a shape mismatch surfaced during migration is a real drift bug to fix,
not to paper over.

**Coverage boundary found while writing this log** (recorded so the implementation does
not silently widen scope): the contract's Rust surface is wider than `dto.rs`.
`BridgeEnvelope::Event` carries `clay::client::ClientConnectionEvent` 1:1, and some DTO
fields reference core-crate projection types (e.g.
`clay::shell::theme::ThemeTokenValueDto`). Under this decision the core crates
(`src/protocol`, `src/client`, `src/shell`) gain no ts-rs derives, so a contract type the
webview needs is either **projected in `src-tauri/src/bridge/dto.rs`** (the boundary
already does this for themes via `resolve_theme_token_snapshot`) or stays a
**hand-narrowed narrowing** in TS (`ShellEvent`'s partial union with an opaque catch-all,
which is a client-side narrowing, not a copy of Rust fields). A hand copy of Rust fields
is what this decision removes.

**Security.** The generated set mirrors exactly what `dto.rs` serializes today: no
contract widening, no new fields, no authority change. Identity stamping, theme-override
elision, and package-UI parsing stay in Rust, and the existing projection test keeps
proving it.

**Follow-through already specified in plan 119 (SC-1).** `scripts/check.sh` regenerates
the bindings and fails on `git diff --exit-code -- frontend/src/bridge/generated`;
frontend typecheck is the semantic guard; the remaining hand file keeps branded types
(`DocumentId`, `MenuSessionId` + their constructors) and re-exports. Frontend mirror
modules (`theme/types.ts`, `icons/types.ts`, `sdui/types.ts`) are covered by this decision
"as covered" — each is deleted or reduced when generated output covers its fields.

## References

- `/aleph-alpha/ts-rs` via ctx7 (`npx ctx7@latest docs /aleph-alpha/ts-rs ...`) —
  configuration (`#[ts(export)]` → export tests, `Config::from_env`), derive attributes
  (`export_to`), usage examples (serde `tag`/`content` → discriminated unions).
- [ts-rs repository](https://github.com/aleph-alpha/ts-rs) — generator behavior and
  serde-compat feature.
- [specta README](https://github.com/specta-rs/specta/blob/main/README.md) — runtime
  `Types` registry and multi-language export, cited for the rejection above.
- `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §SC-1/§SC-5 —
  triple-defined protocol finding and the DTO-projection correction.
- `plans/119-Editor-and-Agent-Architecture-Remediation.md` — "SC-1 decision log" and
  "SC-1: generate the webview TS contract from the DTO layer" tasks (acceptance criteria,
  migration order, drift guard).
- `src-tauri/src/bridge/dto.rs` (`BridgeEnvelope` at :785,
  `runtime_projection_tests`), `src-tauri/src/bridge/errors.rs`,
  `src/client/mod.rs:882` (`ClientConnectionEvent`), `src/shell/theme.rs`
  (`ThemeTokenValueDto`).
- `frontend/src/bridge/types.ts`, `frontend/src/theme/types.ts`,
  `frontend/src/icons/types.ts`, `frontend/src/sdui/types.ts` — the hand mirrors.
- `src-tauri/tests/dto_roundtrips.rs` — 15 tests pinning Rust JSON shape; evidence that
  conformance tests alone do not keep the TS mirror honest.
- Commands: `grep -rn 'ts-rs' Cargo.toml src-tauri/Cargo.toml Cargo.lock` (empty,
  2026-09-14); `wc -l` inventories as quoted above.

## Consequences

- **Positive:** one hand-written contract definition; a Rust contract change that forgets
  TypeScript now fails `cargo test` + `git diff --exit-code` in CI instead of surfacing as
  a runtime `undefined` in the webview; the 633-line mirror surface shrinks to branded
  types plus re-exports; future contract edits touch two files (DTO + regenerated output)
  instead of three.
- **Dev-only dependency:** ts-rs is a `clay-desktop` dev-dependency; no runtime, bundle,
  or frontend-build impact.
- **Generated-output churn:** checked-in bindings create formatting/text diffs on contract
  changes. Accepted: the staleness guard makes such diffs intentional, and they are the
  review surface for contract changes.
- **Branded types stay hand-written:** ts-rs emits structural types only, so `DocumentId`,
  `MenuSessionId`, and their constructors remain hand-maintained in the small remaining
  file and are re-exported next to generated types.
- **Event-union boundary:** `ClientConnectionEvent` (and core-crate `…Dto` types) are not
  generated by this decision. If hand-narrowing that union turns into a drift source, the
  fix inside this decision is a DTO-layer projection in `dto.rs`, not a derive in the core
  crates.
- **Revisit when:** ts-rs stops supporting a serde feature the contract uses (generation
  fails loudly rather than silently), a second non-TS webview consumer appears and makes
  the JSON DTO layer the wrong contract home, or the DTO layer's projection work grows
  beyond the narrowing the webview actually needs.
