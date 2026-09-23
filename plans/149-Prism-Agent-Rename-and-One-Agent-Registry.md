# Plan 149 — Prism Agent Rename and One Agent Registry

Source: decision `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`
(option A′). The native coding agent becomes the **Prism agent** — registry
id `prism`, config root `~/.clay/agents/prism/` — and the agent registry
gains runtime descriptors (kind, runtime id, capability declaration
placeholder) so third-party runtimes (plan 153) list beside it with one
shape. Identity rename only: the `clay-agent` daemon binary/directory name,
spawn path, and sidecar engineering are unchanged (decision log
non-goal).

Roadmap position: Phase 2 agent-infrastructure follow-through, prerequisite
for plans 152/153. Independent of in-flight plan 136 and plan 142 (142
references `~/.clay/agents/<type>/` generically — unaffected). Execute after
136 closes or in parallel; no file overlap with 136 (capability grants /
provider lanes) and no config-authority overlap with 139. The registry
descriptors this plan introduces are consumed by plan 152 (capability
gate) and plan 151 (Apps navigator in the shell).

Binding prior decisions:

- `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`:
  this plan's source.
- `decision-logs/2026-09-09-1420-per-agent-config-layout-agents-coding-agent.md`:
  per-agent config layout under `~/.clay/agents/<id>/`; this plan renames the
  shipped agent's id and owns the one-time migration of that layout.
- `decision-logs/2026-09-14-1602-ts-rs-generated-webview-contract-from-dto-layer.md`:
  protocol DTO changes regenerate the webview contract.
- `decision-logs/2026-09-14-1705-workspace-scoped-agent-sessions.md`:
  session↔agent binding shape is unchanged by the id rename.

## Objectives

- Rename the native agent's identity from `coding-agent` to `prism`
  everywhere users and config see it: agent registry entries, agent-type
  picker, `session.setAgent` default, seeded config root, starter files,
  docs — with a one-time, non-destructive migration of an existing
  `~/.clay/agents/coding-agent/` root to `~/.clay/agents/prism/`.
- Extend the agent registry descriptors with `kind` (`native` |
  `external`), `runtimeId`, and a capability-declaration placeholder field
  consumed by plan 152 — so the picker and protocol can carry third-party
  runtimes without a second registry.
- Keep every existing boundary: no daemon protocol changes beyond the id
  and descriptor fields, no picker behavior changes beyond label/data, no
  new trust surface.

## Expected Outcome

- A fresh install seeds `~/.clay/agents/prism/` (skills, SYSTEM.md,
  starter configs renamed accordingly); the picker shows "Prism" as the
  native agent; `session.setAgent("prism")` works; `coding-agent` no longer
  appears in user-visible surfaces or shipped examples.
- An existing install with `~/.clay/agents/coding-agent/` boots with the
  root migrated to `prism/` exactly once (never overwriting an existing
  `prism/` root; a diagnostic is emitted when both exist).
- The picker/protocol registry data carries `kind`/`runtimeId`/
  capabilities for every listed agent (native = `kind: native`,
  `runtimeId: prism`), verified by protocol tests.

## Tasks

- [ ] Baseline: inventory naming sites and green gates
  - Acceptance Criteria:
    - Functional: a recorded inventory (task evidence) lists every
      `coding-agent` / "coding agent" naming site: daemon default agent id
      (`clay-agent/src/host.ts` agent config root default), Rust protocol
      and picker defaults (`src/server/agent_picker.rs`,
      `src/protocol/agent.rs`), config seeding, `examples/config/agents/`,
      docs, tests, fixtures.
    - Performance: none (inventory task).
    - Code Quality: inventory distinguishes user-visible identity sites
      from internal plumbing (`clay-agent` binary/dir/spawn names —
      explicit non-goals) so later tasks touch only identity sites.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`
      - `decision-logs/2026-09-09-1420-per-agent-config-layout-agents-coding-agent.md`
    - Options Considered:
      - Rename including binary/directory (`clay-agent` → other) — rejected:
        release sidecars, `src-tauri/src/release.rs` naming, spawn paths
        churn for zero user value (decision non-goal).
      - Identity-only rename — chosen.
    - Chosen Approach: `rg -n "coding-agent|coding agent"` sweep, classify,
      record.
    - Files to Create/Edit: none (evidence in task notes).
    - References: `src/server/agent_picker.rs`,
      `clay-agent/src/host.ts` (`agentConfigRoot` default),
      `src/server/agent.rs`.
  - Test Cases to Write: none.

- [ ] Rename the agent id to `prism` (daemon + Rust + protocol)
  - Acceptance Criteria:
    - Functional: default agent id is `prism`; `session.setAgent` accepts
      `prism`; sessions recorded under `coding-agent` id still resolve
      (session store keys are workspace-scoped, not id-scoped — verify, and
      if any store keys embed the id, map old→new on read).
    - Performance: no regression in session-bind latency (existing gates).
    - Code Quality: single constant/source of truth for the id in each
      layer; no hardcoded `"coding-agent"` strings remain outside
      migration code and tests.
    - Security: unknown agent ids still fail closed; no new path derived
      from the id.
  - Approach:
    - Documentation Reviewed: daemon `session.setAgent` handling
      (`clay-agent/src/host.ts`); `AgentPickerKind` /
      `AgentProfileInfo` (`src/protocol/agent.rs`).
    - Options Considered:
      - Accept both ids as aliases forever — rejected: permanent ambiguity.
      - Rename + one-time migration — chosen.
    - Chosen Approach: rename constants; migration in the following task;
      alias accepted only inside the migration reader for the config root.
    - API Notes and Examples:
      ```rust
      // src/server/agent_picker.rs — default entry
      const NATIVE_AGENT_ID: &str = "prism";
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: default agent id + config-root default
        `agents/prism`.
      - `src/server/agent_picker.rs`, `src/server/agent.rs`,
        `src/protocol/agent.rs`: id, label "Prism", descriptor fields.
      - Tests/fixtures referencing the old id.
    - References: `decision-logs/2026-09-09-1420-…` (layout),
      `2026-09-14-1705-…` (session scoping).
  - Test Cases to Write:
    - Picker lists `prism` with `kind: native`, no `coding-agent` entry.
    - `session.setAgent("coding-agent")` fails closed with the documented
      unknown-agent error (after migration window).

- [ ] One-time config-root migration `coding-agent` → `prism`
  - Acceptance Criteria:
    - Functional: at agent-config resolution, if `~/.clay/agents/coding-agent/`
      exists and `~/.clay/agents/prism/` does not, rename the directory
      (atomic rename on same filesystem; fall back to copy+verify+remove if
      cross-device). If both exist, keep `prism/` untouched, emit a
      diagnostic naming the leftover `coding-agent/` root, never merge or
      overwrite. Migration runs at most once per root (idempotent).
    - Performance: migration is a rename, not a file walk, in the common
      case; runs off the session hot path.
    - Code Quality: unit-tested edge cases (both-exist, neither-exists,
      re-run after success); errors are diagnostics, never fatal to boot.
    - Security: symlink-escape discipline from skill discovery applies; the
      migrated root keeps its trust posture (no new roots, no widening).
  - Approach:
    - Documentation Reviewed: config-root trust pattern in
      `decision-logs/2026-09-09-1341-mcp-stdio-config-sources-no-approval-gate.md`
      (user-owned `~/.clay` precedent); symlink-escape handling in
      `clay-agent/src/host.ts` skill discovery.
    - Options Considered:
      - Read-old-write-new (dual read forever) — rejected: permanent alias.
      - Directory rename once — chosen.
    - Chosen Approach: Rust-side (config root is host-resolved) at
      `agent_picker`/agent-config resolution, before first session build.
    - Files to Create/Edit:
      - `src/server/agent_settings.rs` (or the config-root resolution site):
        migration function + tests.
    - References: `examples/config/agents/coding-agent/` (starter files
      renamed in a later task).
  - Test Cases to Write:
    - `migrates_once_and_is_idempotent`
    - `both_roots_exist_keeps_prism_and_diagnoses`
    - `missing_source_is_silent_noop`

- [ ] Registry descriptors: kind, runtimeId, capabilities placeholder
  - Acceptance Criteria:
    - Functional: every registry entry (protocol picker payload) carries
      `kind` (`native`|`external`), `runtimeId`, and `capabilities`
      (placeholder: native = full set constant; external default = empty,
      populated by plan 152's ARI types). Entries with no backing runtime
      (external without an installed adapter) are absent/hidden, not
      listed-disabled.
    - Performance: picker payload size unchanged to first order (three
      small fields per entry).
    - Code Quality: enum-typed in Rust (`serde` rename_all), regenerated
      ts-rs contract; no stringly-typed kind.
    - Security: descriptor data is inert metadata; nothing derives
      authority from `kind`.
  - Approach:
    - Documentation Reviewed: `src/protocol/agent.rs` (`AgentPickerKind`,
      `AgentProfileInfo`, `AgentPickerItem`); ts-rs DTO flow
      (`decision-logs/2026-09-14-1602-…`).
    - Options Considered:
      - Separate registries per kind — rejected: second registry is what
        this decision kills.
      - One registry + descriptors — chosen.
    - Chosen Approach: extend `AgentPickerItem`/`AgentProfileInfo`; native
      capability constant lives beside the enum (plan 152 formalizes the
      capability enum; here a `Vec<String>` placeholder keeps the wire
      stable — 149 tightens the type).
    - Files to Create/Edit: `src/protocol/agent.rs`,
      `src/server/agent_picker.rs`, regenerated webview DTOs,
      `frontend/src/coding-agent/` typing consumers (if any read the field).
    - References: plan 152 (capability enum consumer), plan 151 (Apps
      navigator consumer of `kind`/`runtimeId`/labels).
  - Test Cases to Write:
    - Picker payload round-trip with all three fields.
    - External entry without adapter never listed.

- [ ] Picker and agent-lane UI data update (label "Prism", descriptor-aware)
  - Acceptance Criteria:
    - Functional: agent-type picker shows "Prism" for the native entry;
      picker rows render from registry descriptors (kind badge only if the
      approved agent-lane artifact already defines one — otherwise no new
      visual element in this plan).
    - Performance: no picker open latency regression (existing gates).
    - Code Quality: label/identity from server data, not frontend constants.
    - Security: none.
  - Approach:
    - Documentation Reviewed: `DESIGN.md` §14; approved artifact
      `design-artifacts/approved/agent-lane-palette/` (covers agent-lane
      surfaces; this task is data/identity-only — no new component states,
      so no new prototype per the prototype gate's exact-surface rule).
    - Options Considered:
      - Add a kind badge now — rejected: new visual surface needs the
        prototype loop; deferred to plan 152's capability-states artifact.
      - Data-only change — chosen.
    - Chosen Approach: rename labels + wire descriptors; visual check
      against the approved artifact.
    - Files to Create/Edit: `frontend/src/coding-agent/` (picker data
      consumers), `frontend/src/agent/` labels if hardcoded.
    - References: `design-artifacts/approved/agent-lane-palette/`.
  - Test Cases to Write:
    - Picker renders "Prism" from server payload (component test with
      fixture snapshot).

- [ ] Rename shipped starter files and examples
  - Acceptance Criteria:
    - Functional: `examples/config/agents/coding-agent/` → `examples/config/agents/prism/`
      (tool-caps.json, skills.json, mcp.json, SYSTEM.md seed pattern as
      shipped); all doc references updated (`docs/`, README,
      `roadmap.md` mentions stay historical where they describe past work —
      add one pointer note in roadmap's direction section, done by the
      roadmap task of this plan's decision, not here).
    - Performance: none.
    - Code Quality: no dangling references (`rg "agents/coding-agent"`
      clean outside decision logs and historical roadmap text).
    - Security: starter file contents unchanged (trust posture preserved).
  - Approach:
    - Documentation Reviewed: `examples/config/agents/` current contents.
    - Options Considered: none meaningful.
    - Chosen Approach: `git mv` + reference sweep.
    - Files to Create/Edit: `examples/config/agents/prism/**`, docs
      referencing the old path.
    - References: `decision-logs/2026-09-09-1420-…`.
  - Test Cases to Write:
    - Doc-registry check (existing documentation truth tests) passes with
      renamed paths.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: any init.js-facing surface this plan changed (agent id
      strings in examples/comments) is updated exactly once, in section,
      matching validated server-side ids.
    - Performance: none.
    - Code Quality: `node --check examples/config/init.js` passes; active
      part stays copy-safe.
    - Security: no new grants implied by the rename.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Example Configuration
      Maintenance Task; current `examples/config/init.js` agent mentions.
    - Chosen Approach: targeted sweep; if init.js never names the agent id,
      record that as evidence and make no edit.
    - Files to Create/Edit: `examples/config/init.js` (only if it names the
      id).
  - Test Cases to Write: `node --check` as above.

- [ ] Example configuration live launch-test
  - Acceptance Criteria:
    - Functional: real Linux GUI build launched against a scratch config
      root seeded from `examples/config/`; client reaches Connected; the
      agent picker lists "Prism"; a session starts and prompts round-trip
      (mock provider acceptable); scratch-config evidence recorded.
    - Performance: startup within existing gates.
    - Code Quality: launch command + observed results recorded in task
      evidence.
    - Security: never launched against the developer profile; if an
      existing `coding-agent` root is pre-seeded in the scratch home, the
      migration visibly produces `prism/`.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Live Launch-Test Task.
    - Chosen Approach: temp `HOME`, seeded roots (one run fresh, one run
      with legacy `coding-agent/` root).
    - Files to Create/Edit: none (evidence).
  - Test Cases to Write: none (manual drill, recorded).

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: review finds no new public Rust fn requiring a facade
      (rename is internal + protocol ids); any newly-public helper from the
      migration code is made `pub(crate)` or gets an op + facade + docs per
      the dotted-ID convention.
    - Performance: none.
    - Code Quality: `cargo test` doc-registry gates green.
    - Security: no new JS-reachable surface without docs/permissions.
  - Approach:
    - Documentation Reviewed: `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach: verification-only unless the inventory task exposed a
      new public fn.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: no undocumented behavior-changing setting introduced;
      agent id is protocol data, not a config key; `skills.json` /
      `tool-caps.json` schemas unchanged (only the root moved).
    - Code Quality: coverage gates for undocumented config APIs stay green.
    - Security: rename does not widen config authority.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Clay Configuration Task.
    - Chosen Approach: verification task.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Code-wiki and documentation truth update
  - Acceptance Criteria:
    - Functional: wiki module pages touching the agent registry/picker
      reflect the `prism` id and descriptor fields; documentation truth
      tests pass; registry/index navigation updated.
    - Performance: none.
    - Code Quality: follows `docs-as-code` workflow.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Chosen Approach: one final wiki task per the create-plan skill
      template.
    - Files to Create/Edit: `docs/wiki/` affected pages,
      `docs/reference/` as needed.
  - Test Cases to Write: existing wiki truth checks.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and
  priority. Known at planning time: plan 152 tightens the `capabilities`
  placeholder into a typed enum; a kind badge in the picker is deferred to
  plan 152's approved artifact.
