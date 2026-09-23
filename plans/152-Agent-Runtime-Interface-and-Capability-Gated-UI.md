# Plan 152 — Agent Runtime Interface (ARI v1) and Capability-Gated UI

Source: decision `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`
(option A′). Introduces the core-owned **Agent Runtime Interface** beside
the native Prism daemon path: runtime kind/descriptor/capability types, an
`ExternalAgentRuntime` lifecycle trait with no implementation yet (plan 153
ships the pi adapter), runtime identity + capability state on the session
snapshot and AG-UI projection, and the frontend capability gate — inspector
tabs, composer controls, and header runtime state render only what the
active runtime declares. The native Prism agent is declared as the
capability superset, so the gate is dogfooded from day one.

Depends on plan 149 (registry descriptors: `kind`, `runtimeId`,
capabilities placeholder → typed here). The **UI tasks additionally depend
on plan 151** (apps-and-splits shell): the capability gate renders inside
the agent runtime app (activity-first, gear-tucked inspector) in the
shell. Execution order: 146 → 147 → 148 → 149 → 150; contract/wire tasks
of this plan depend only on 149 and may proceed earlier. No overlap with
plan 136/142.

Binding prior decisions:

- `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`:
  this plan's source (A′; two runtime depths behind one wire).
- `decision-logs/2026-09-02-1440-direct-external-coding-agent-adapters.md`:
  deliberately small contract — lifecycle, normalized bounded events,
  declared capability set; core owns launch/policy/redaction/UI; no Prism
  intermediary, no ACP requirement.
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`:
  Clay-owned daemon↔server boundary; the AG-UI projection stays the single
  frontend wire.
- `decision-logs/2026-09-14-1602-ts-rs-generated-webview-contract-from-dto-layer.md`:
  snapshot/protocol changes regenerate the webview contract.

## Objectives

- Define ARI v1 types in the Rust core: `AgentRuntimeKind`
  (`native`|`external`), `AgentRuntimeDescriptor` (id, kind, display), and
  `AgentRuntimeCapability` (typed enum with granular approval semantics),
  with the native runtime declared as the full set.
- Add the `ExternalAgentRuntime` trait — lifecycle (`start`, `prompt`,
  `cancel`, `resume`, `respond`) + bounded event stream returning existing
  `AgentWireEvent`s — and the adapter registry slot in `src/server/agent/`,
  with no adapter implementations in this plan.
- Carry runtime identity (id, kind, vendor session id placeholder) and
  capability state on `AgentSessionSnapshot` / `AgentServerMessage`, through
  the DTO layer to the frontend.
- Ship the frontend capability gate inside the plan-151 shell's agent
  runtime app: header runtime state, inspector-tab visibility
  (Memory/Context native-only in v1) behind the gear-tucked inspector,
  composer controls (steer, effort, model switching) rendered only for
  declared capabilities — behind the mandatory prototype → approval →
  implementation gate. Prototype guidelines incorporate the user's
  2026-09-23 UI concept decision
  (`decision-logs/2026-09-23-1908-apps-and-splits-shell-default-view.md`):
  design states inside the apps-and-splits shell — transcript/activity as
  the default agent-app surface, inspector tucked behind the gear icon
  (inline drawer or open-in-split), capability-absent states hidden not
  disabled, splits showing editor beside agent runtime.

## Expected Outcome

- A session snapshot exposes `runtime: { id, kind, capabilities }`; the
  frontend renders the Prism session identically to today (superset
  declared), and a test fixture declaring a reduced capability set hides
  exactly the gated controls with no layout regressions.
- The `ExternalAgentRuntime` trait compiles with a mock implementation in
  tests only; no production adapter exists yet (plan 153 adds pi).
- Approved prototype artifact `design-artifacts/approved/agent-runtime-capability-states/`
  exists and every UI task cites it.

## Tasks

- [ ] Baseline: inventory existing agent protocol primitives
  - Acceptance Criteria:
    - Functional: recorded inventory of the current projection surface:
      `AgentWireEvent`, `AgentSessionSnapshot`, `AgentServerMessage`,
      `AgentClientCommand`, `AgentPickerKind`/`AgentPickerItem` (with
      plan 149 descriptors), the `map_event` seam, DTO/ts-rs flow, and the
      frontend consumers (`frontend/src/agent/state.ts`,
      `frontend/src/coding-agent/surface-state.ts`).
    - Performance: none.
    - Code Quality: inventory states what ARI reuses vs adds — no new wire
      event types in this plan (identity/capability ride existing snapshot
      fields).
    - Security: inventory notes the redaction/secrets path every runtime
      event must pass through (`map_event` secrets set).
  - Approach:
    - Documentation Reviewed: `src/protocol/agent.rs`,
      `src/server/agent.rs` (`map_event`), `src/server/agent_agui.rs`,
      `frontend/src/agent/state.ts`.
    - Options Considered: none (mandatory inventory per plan requirements —
      this plan changes a reusable protocol capability).
    - Chosen Approach: read + record; the ARI tasks build only on listed
      primitives.
    - Files to Create/Edit: none.
    - References: decision 1440 (contract shape), 1758 (projection
      ownership).
  - Test Cases to Write: none.

- [ ] ARI v1 types: kind, descriptor, capability enum, native superset
  - Acceptance Criteria:
    - Functional: `AgentRuntimeCapability` enum covers: `StreamingEvents`,
      `Approvals` (with granularity `AllowDeny` | `AllowDenyModify`),
      `Steering`, `SessionTrees`, `Compaction`, `ContextInspection`,
      `FileHistory`, `ModelSwitch`, `EffortSwitch`, `SlashCommands`,
      `MemoryActivity`. Native prism runtime returns the full set from one
      constant. Plan 149's placeholder `Vec<String>` wire field is
      tightened to the typed enum (breaking protocol change is fine —
      single-repo, regenerated contract, no persisted schema embeds it).
    - Performance: capability set is a small bitmask-like `Vec<enum>`;
      serialized per snapshot once, not per event.
    - Code Quality: `serde` rename_all camelCase; unknown capability values
      deserialize fail-closed (drop session, per the 0.7 unknown-event
      precedent) — decided here: **unknown values fail the handshake, not
      silently drop**, because a stale frontend must not render undeclared
      controls.
    - Security: capabilities are declarations, never authority; a malicious
      runtime cannot grant itself host permissions by declaring more (only
      UI affordances are gated).
  - Approach:
    - Documentation Reviewed: decision 1440 scope bullets; 0.7 unknown-event
      drop precedent (`decision-logs/2026-09-16-0026-…`).
    - Options Considered:
      - Stringly-typed capability list (plan 149 placeholder kept) —
        rejected: no compile-time exhaustiveness in gating code.
      - Typed enum — chosen.
      - Free-form `HashMap<String, Json>` capability bag — rejected:
        unbounded, undeclarable.
    - Chosen Approach: enum in `src/protocol/agent.rs` beside the picker
      types; native constant in `src/server/agent.rs`.
    - API Notes and Examples:
      ```rust
      pub enum AgentRuntimeCapability {
          StreamingEvents,
          Approvals { modify_input: bool },
          Steering,
          SessionTrees,
          Compaction,
          ContextInspection,
          FileHistory,
          ModelSwitch,
          EffortSwitch,
          SlashCommands,
          MemoryActivity,
      }
      pub const NATIVE_RUNTIME_CAPABILITIES: &[AgentRuntimeCapability] = &[/* full set */];
      ```
    - Files to Create/Edit: `src/protocol/agent.rs`,
      `src/server/agent.rs`, `src/server/agent_picker.rs` (consumer),
      regenerated DTOs.
    - References: plan 149 (placeholder being tightened).
  - Test Cases to Write:
    - Round-trip serialization of every capability.
    - Unknown capability value fails deserialization closed.

- [ ] `ExternalAgentRuntime` trait + adapter registry slot
  - Acceptance Criteria:
    - Functional: trait in `src/server/agent/` (new `runtime` module)
      defining `descriptor()`, `start`, `prompt`, `cancel`, `resume`,
      `respond`, and an event stream yielding `AgentWireEvent`s with the
      same bounds/redaction discipline as the daemon path; a registry keyed
      by `runtimeId` with zero production entries; the native daemon path
      is explicitly *not* behind the trait (it keeps its rich channel —
      the trait exists for external runtimes only, per A′).
    - Performance: trait design keeps the event path allocation profile of
      the current mapper (no per-event heap explosion); documented budget.
    - Code Quality: trait object safe; mock implementation exists only in
      tests proving a foreign runtime's events reach the projection
      through the same `map_event` discipline.
    - Security: trait contract documents same-user-child authority
      disclosure, no shell, bounded I/O, cancellation, cleanup —
      implementers (plan 153+) must satisfy it; events pass the secrets
      redactor.
  - Approach:
    - Documentation Reviewed: decision 1440 (core-owned boundary,
      lifecycle set); `src/server/agent.rs` daemon actor (spawn/actor
      pattern to mirror).
    - Options Considered:
      - Route the native daemon through the trait too — rejected (A′
        decision: native keeps rich channel; revisit at second adapter).
      - Trait for externals only — chosen.
    - Chosen Approach: small async trait mirroring the daemon actor's
      message shapes, reusing `AgentClientCommand` variants where the wire
      already has them (`Prompt`, `Cancel`, `Steer`, approval respond).
    - Files to Create/Edit: `src/server/agent/runtime.rs` (trait +
      registry), `src/server/agent/mod` wiring.
    - References: plan 153 (first implementer).
  - Test Cases to Write:
    - Mock runtime: prompt → events → snapshot update through projection.
    - Registry with no entry fails closed for unknown `runtimeId`.

- [ ] Runtime identity + capability state on snapshot and projection
  - Acceptance Criteria:
    - Functional: `AgentSessionSnapshot` carries `runtime`
      (`{ id, kind, vendorSessionId? }`) and `capabilities`; frontend
      `useAgentSurfaceState` exposes them; ts-rs contract regenerated;
      native sessions report the superset and today's `prism` id.
    - Performance: snapshot size delta bounded (≤ a few hundred bytes); no
      per-event repetition.
    - Code Quality: one source of truth (server); frontend never infers
      capabilities from runtime id.
    - Security: vendor session id is opaque, bounded-length, redacted like
      other ids; no credential material in the field.
  - Approach:
    - Documentation Reviewed: `src/protocol/agent.rs`
      (`AgentSessionSnapshot`), DTO layer decision 1602.
    - Options Considered:
      - Infer capabilities client-side per runtime id — rejected: frontend
        must not hardcode vendor matrices.
      - Server-declared on the wire — chosen.
    - Chosen Approach: add fields; regenerate; surface in state hooks.
    - Files to Create/Edit: `src/protocol/agent.rs`, `src-tauri/src/bridge/`
      DTO pass-through if the snapshot crosses it (verify), regenerated
      frontend types, `frontend/src/coding-agent/surface-state.ts`.
    - References: decision 1440 ("extend … AG-UI projection with runtime
      identity, vendor session identity, capability state").
  - Test Cases to Write:
    - Snapshot round-trip with runtime + capabilities through the DTO
      layer test suite.

- [ ] Prototype: agent runtime capability states (inside the apps-and-splits shell)
  - Acceptance Criteria:
    - Functional: self-contained HTML prototype under
      `design-artifacts/prototypes/agent-runtime-capability-states/`
      designed **within the plan-151 shell concept** (guidelines from the
      user's 2026-09-23 decision, `decision-logs/2026-09-23-1908-apps-and-splits-shell-default-view.md`):
      agent runtime app as the default surface (transcript/activity only);
      inspector (files/memory/context) tucked behind a **gear icon** —
      inline drawer variant and open-in-split variant; capability-absent
      tab states (Memory/Context hidden vs present — hidden, not
      disabled); composer control gating (steer/effort/model) in the
      global bottom-lane overlay; header/runtime badge states for native
      Prism full vs external reduced; an editor-beside-agent split
      composition; blocked/disconnected states per runtime kind; narrow
      + wide layouts; opens over `file://`, renders against the four
      shipped content themes.
    - Performance: none (prototype).
    - Code Quality: no build step; component states per
      `references/components.md` and tokens per `references/tokens.md`.
    - Security: none.
  - Approach:
    - Documentation Reviewed: `DESIGN.md` (§14 especially),
      `.agents/skills/clay-execution/references/ui.md` + catalog
      references; approved `design-artifacts/approved/agent-lane-palette/`
      and the shell concept (decision 1908, plan 151's prototype may run
      first — coordinate states between the two artifacts).
    - Options Considered:
      - Claim `agent-lane-palette` already covers it — rejected: capability-
      absent states and gear-tucked inspector are new states.
      - New prototype variant inside the shell concept — chosen.
    - Chosen Approach: variant of the agent-runtime app within the shell;
      substantial new-surface design loads the four design skills per the
      plan gate.
    - Files to Create/Edit: `design-artifacts/prototypes/agent-runtime-capability-states/**`.
    - References: prototype-gate duty in
      `.agents/skills/clay-execution/references/clay.md`; decision 1908
      (shell); decision 1821 (capability model).
  - Test Cases to Write: none.

- [ ] Freeze approved capability-states artifact (explicit user approval)
  - Acceptance Criteria:
    - Functional: chosen prototype copied to
      `design-artifacts/approved/agent-runtime-capability-states/`; task
      evidence records date, approving user statement, chosen variant,
      requested changes. No implementation task starts before this exists.
    - Performance: none.
    - Code Quality: append-only artifact.
    - Security: none.
  - Approach:
    - Documentation Reviewed: prototype gate duty (freeze step).
    - Chosen Approach: present variants to user; record approval verbatim.
    - Files to Create/Edit: `design-artifacts/approved/agent-runtime-capability-states/**`.
  - Test Cases to Write: none.

- [ ] Implement the frontend capability gate (citing the approved artifact)
  - Acceptance Criteria:
    - Functional: inspector tabs, composer controls, and header runtime
      state render strictly from declared capabilities; a fixture declaring
      no `MemoryActivity` hides the Memory tab; no `EffortSwitch` hides the
      effort control; undeclared controls never render for any runtime;
      Prism session renders exactly as today (superset).
    - Performance: gating is a memoized capability lookup, not a re-render
      trigger; no transcript paint regression (existing perf gates).
    - Code Quality: one `useCapabilities()`-style accessor; zero scattered
      `runtime === "prism"` checks (lint by review).
    - Security: none (display layer only).
  - Approach:
    - Documentation Reviewed: approved artifact (cited by path in every
      implementation sub-task),
      `design-artifacts/approved/agent-runtime-capability-states/`;
      plan 151's agent runtime app (the host surface); `DESIGN.md` §14.4
      (one ring per surface).
    - Options Considered:
      - Per-component ad-hoc checks — rejected: drift.
      - Central accessor — chosen.
    - Chosen Approach: extend `useAgentSurfaceState`; gate at the tab-set
      and control-set level inside the plan-151 agent app and overlay.
    - Files to Create/Edit: `frontend/src/coding-agent/surface-state.ts`,
      inspector + composer components per artifact.
    - References: plan 149 picker descriptors (kind badge if the artifact
      includes one).
  - Test Cases to Write:
    - Reduced-capability fixture hides exactly the gated controls.
    - Superset fixture renders the full surface (snapshot test).

- [ ] Post-implementation visual and accessibility review
  - Acceptance Criteria:
    - Functional: running UI compared against the approved artifact; each
      deviation recorded as defect fix or re-approval; keyboard traversal
      of the gated tab set works when tabs are hidden (focus order stays
      sane); screen-reader labels for runtime state.
    - Performance: part of review.
    - Code Quality: recorded in task evidence.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/planning-checklist.md`
      (review duty).
    - Chosen Approach: manual review drill per the checklist.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: inventory new/changed public Rust fns (trait, registry,
      types); anything JS-reachable gets an op + facade + docs per the
      dotted-ID convention; the trait itself is not JS-exposed (internal
      runtime surface — `pub(crate)` where possible).
    - Code Quality: doc-registry gates green.
    - Security: no JS path spawns a runtime or mutates capabilities.
  - Approach:
    - Documentation Reviewed: `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach: verification-first.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: no undocumented behavior-changing setting; capability
      declarations are runtime-provided data, not user config; any
      debug/test toggle (e.g. force-reduced capabilities for drills) ships
      documented or stays test-only.
    - Code Quality: coverage gates green.
    - Security: config cannot grant capabilities.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Clay Configuration Task.
    - Chosen Approach: verification.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Code-wiki and documentation truth update
  - Acceptance Criteria:
    - Functional: wiki pages for the agent protocol/projection document
      ARI v1 (types, trait, capability gating, native-superset rule);
      truth checks pass; ARI reference page added under `docs/reference/`
      if the phase adds user-visible surface.
    - Code Quality: docs-as-code workflow.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Chosen Approach: final wiki task.
    - Files to Create/Edit: `docs/wiki/`, `docs/reference/` as needed.
  - Test Cases to Write: existing truth checks.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion. Known at planning time: PTY fallback
  lane for runtimes without structured approvals (Phase 10 scope); a
  daemon ARI facade when a second external adapter lands. Agent state
  rollup (working/blocked/idle) moved from deferral into plan 150
  (detached-server inventory, required by plan 151's Agents navigator).
