# Plan 142 — Agent Configuration Authority and the Config Feedback Loop

Source: the 2026-09-20 configurability review, deviation D5 and Part 3. The
stated product goal is "download Clay, ask the agent to modify it" — but the
agent's file I/O round-trips to the Rust server
(`clay-agent/src/document-ops.ts` → `src/server/agent_documents.rs`), which
hard-rejects anything outside the session's workspace root
(`WorkspaceError::OutsideRoot`) even when the user approves: writing
`~/.clay/init.js` is impossible by design, not merely gated. The daemon also
has no config knowledge (seeded skills are `wiki-searcher`, `wiki-maintainer`,
`graft` — `clay-agent/src/host.ts:256`), no reload/diagnostic feedback
methods, and no visibility into the restart matrix. The one designed write
path to the configuration root is missing.

Binding prior decisions:

- `decision-logs/2026-08-30-2157-default-acceptance-workspace-free-host-write-gated.md`:
  workspace = full agent freedom; host writes outside workspace gated on
  explicit permission; the approval UX exists and is the gate this plan
  reuses for one more scope.
- `decision-logs/2026-09-14-1705-workspace-scoped-agent-sessions.md`:
  session ownership and tool-authority roots; new config-root authority stays
  session-attributed and never widens workspace roots.
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`:
  daemon↔server is Clay-owned; reverse RPC is the sanctioned boundary.
- `decision-logs/2026-08-11-0352-configuration-watch-auto-reload-and-modular-structure.md`:
  config-root changes auto-reload through serialized
  `runtime.reloadConfiguration`; no new reload IPC is needed for the *user*
  side — the agent rides the same path.
- `decision-logs/2026-09-09-1341-…-mcp-config-sourced-stdio.md` (pattern):
  user-owned `~/.clay` config is the precedent for config-root trust; any
  *new* process/authority surface needs its own decision log — which task 2
  is.

Roadmap position: the "AI-native" pillar. Independent of plans 140/141, but
compounds with them: plan 140 makes init.js worth editing; this plan makes
the agent the one who edits it. Tier model (from the review): tier 1 =
config files (this plan, hot-reload loop), tier 2 = local packages
(plan 141), tier 3 = Clay source (already works when the workspace is the
clay repo).

## Objectives

- Add daemon→server reverse-RPC methods `config.read` / `config.write`,
  confined to the configuration root (symlink-aware containment), routed
  through document lease/CAS when the target is open in an editor so agent
  edits appear as ordinary dirty buffers — plus `config.state`,
  `config.reload`, `config.diagnostics` for the verify half of the loop.
- Gate config-root mutation behind a new `ApprovalRequestKind::ConfigEdit`
  rendered by the existing agent-lane approval strip; the daemon's acceptance
  policy routes config-root paths to that approval instead of failing
  `OutsideRoot`.
- Seed a fourth agent-delivered skill, `clay-config`, teaching: the config
  file map (`~/.clay/init.js`, `~/.clay/agents/<type>/*`, preferences), the
  init.js API cheat sheet, the approval semantics, the reload/restart matrix,
  and the three change tiers.
- Keep every existing boundary: no new authority over the vault/credentials,
  no widening of workspace roots, no package-reachable path, reads stay free
  (already true today), transcripts never echo secret-looking env values
  (`clay-agent/src/redact.ts` duty).

## Expected Outcome

- In a real session, "make my editor use the gruvbox theme and bind
  Ctrl+Alt+G to palette" results in: agent proposes `init.js` edit → one
  ConfigEdit approval → write lands → watcher reloads → agent reads
  `config.diagnostics` + `config.state` → reports success/failure with the
  diagnostic text. No restart required for tier-1 changes.
- An unapproved or denied config write mutates nothing; paths outside the
  configuration root still fail closed exactly as today.
- All gates green; e2e test covers the whole loop headlessly; the skill
  seeds idempotently with `.seed-manifest.json` stamps.

## Tasks

- [ ] Baseline gates and gap reproduction on the unmodified tree
  - Acceptance Criteria:
    - Functional: seven-stage gate results recorded; the gap reproduced
      headlessly — a session write tool call targeting
      `<tmp-config-root>/init.js` fails with `path outside workspace roots`
      (`src/server/agent_documents.rs:255`, test
      `document_write_rejects_paths_n_roots` region).
    - Performance: n/a.
    - Code Quality: baseline before edits.
    - Security: reproduction uses a scratch config root, never the developer
      profile.
  - Approach:
    - Documentation Reviewed: AGENTS.md platform validation;
      `plans/126-Document-Access-Path-Hardening.md` (the containment being
      preserved).
    - Options Considered: cite plan-126 evidence only — rejected: this plan
      modifies that exact path and needs its own before/after.
    - Chosen Approach: gate run + failing-write reproduction log.
    - API Notes and Examples:
      ```bash
      cargo test --lib agent_documents
      ```
    - Files to Create/Edit: `code-reviews/<date>-plan139-baseline/README.md`.
    - References: `src/server/agent_documents.rs:223-255`.
  - Test Cases to Write: none (evidence capture).

- [ ] Decision log: agent configuration-root authority (user approval required)
  - Acceptance Criteria:
    - Functional: `decision-logs/<date>-agent-config-root-authority.md`
      records: scope = the default configuration root only (absolute,
      symlink-resolved); authority = same-user trusted mutation behind one
      explicit approval kind (`ConfigEdit`), session-attributed; lease/CAS
      integration for open documents; reload feedback via
      `config.reload`/`config.diagnostics`; what is explicitly **not**
      granted (vault, workspace roots, arbitrary host paths, package JS
      reachability); the restart matrix as the documented truth table.
    - Performance: statement that config writes are rare, bounded
      (existing document caps), and never on editor hot paths.
    - Code Quality: create-decision-log skill followed; user approval
      statement quoted; alternatives (per-write approval, config-root as a
      workspace root, CLI-only) recorded.
    - Security: truthful containment language per the established pattern —
      this constrains Clay's API and audit record, not the OS; it is
      trusted same-user authority, never "sandboxed".
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/create-decision-log/SKILL.md`;
      `decision-logs/2026-08-30-2157-…md`;
      `decision-logs/2026-09-09-1341-…md` (new-surface-needs-decision rule).
    - Options Considered:
      - Register the config root as a workspace root for the session —
        rejected: blurs workspace scoping (decision 1705) and would silently
        widen every session's tree/search surface.
      - Per-write `ApprovalRequestKind::Mutation` reuse — rejected: config
        edits deserve their own labeled decision surface ("allow config
        changes"), not a generic file-mutation prompt.
    - Chosen Approach: dedicated kind + dedicated reverse methods; task
      stays unchecked without explicit user approval of the decision text.
    - API Notes and Examples:
      ```rust
      pub enum ApprovalRequestKind { Mutation, AskDecision, ConfigEdit }
      ```
    - Files to Create/Edit: decision log (new);
      `.agents/skills/clay-execution/references/packages.md` (Agent Host
      section gains the config-authority rule).
    - References: clay-execution SKILL.md decision-log integration.
  - Test Cases to Write: none (document).

- [ ] Implement server reverse-RPC `config.read` / `config.write` with config-root containment and lease integration
  - Acceptance Criteria:
    - Functional: new arms in the reverse-method dispatch
      (`src/server/agent_documents.rs`, beside `document.*`); both take
      `sessionId` + absolute path; containment = canonicalized path under
      the *default* configuration root (reuse the symlink-aware containment
      helper the workspace check uses); `config.write` routes through the
      document registry (lease, version bump, dirty buffer) when the target
      is open, else performs a bounded atomic write; the config watcher
      triggers reload as it already does for user edits; failures return
      typed errors surfaced as diagnostics.
    - Performance: no new hot-path work; writes bounded by existing
      document caps; `config.read` capped like `document.read`.
    - Code Quality: containment helper shared, not duplicated; `pub(crate)`
      internals; error strings follow the `agent.*` diagnostic convention.
    - Security: writes outside the config root fail closed with the
      existing `OutsideRoot`-style message; the methods never accept a
      caller-chosen root; vault/credential paths are inside the config root
      tree by default — `config.write` must reject the vault/credentials
      subpaths with a dedicated diagnostic (secrets are not agent-writable).
  - Approach:
    - Documentation Reviewed: decision log (task 2);
      `docs/wiki/modules/agent-document-operations.md` (or nearest wiki
      page); `plans/119-…md` (session-resolved roots, SC-6).
    - Options Considered:
      - A general second-root parameter — rejected: caller-chosen roots are
        the escape hatch plan 126 removed.
      - Direct FS writes bypassing the registry — rejected: breaks dirty
      buffer/lease invariants for open config files.
    - Chosen Approach: registry-integrated writes with a fallback direct
      write for closed files; the watcher remains the reload trigger.
    - API Notes and Examples:
      ```rust
      // dispatch arm sketch
      "config.write" => config_write(workspaces, agent, params).await,
      ```
    - Files to Create/Edit: `src/server/agent_documents.rs` (dispatch +
      handlers), `src/server/mod.rs` (config-root accessor if not already
      shared), `tests/agent_config_authority.rs` (new suite).
    - References: `src/server/agent_documents.rs:203`,
      `clay-agent/src/document-ops.ts`.
  - Test Cases to Write:
    - `config_write_inside_root_updates_open_document_as_dirty_buffer`.
    - `config_write_outside_root_fails_closed`.
    - `config_write_vault_path_rejected`.
    - `config_write_triggers_generation_reload_via_watcher`.

- [ ] Wire `ApprovalRequestKind::ConfigEdit` through protocol, daemon policy, and the approval strip
  - Acceptance Criteria:
    - Functional: protocol enum extended (`src/protocol/agent.rs:1044`) with
      rkyv/serde plumbing per existing variants; daemon acceptance policy
      (`clay-agent/src/coding-tools.ts::createClayAcceptancePolicy`) routes
      config-root mutation kinds to `approve` with the new kind (the host
      callback at `clay-agent/src/host.ts:2007` sends
      `approval.request`); the React agent lane approval strip renders the
      ConfigEdit prompt with config-edit labeling; deny/timeout mutates
      nothing.
    - Performance: approval round-trip only on config mutation; no caching
      (per-decision ask, matching the no-cache stance of 2157).
    - Code Quality: policy branch mirrors the existing out-of-root branch;
      no shell-kind special cases.
    - Security: full-autonomy toggle covers config edits only when the
      user turned it on (documented); the approval payload names the exact
      path + session.
  - Approach:
    - Documentation Reviewed: decision log; approved artifact
      `design-artifacts/approved/agent-lane-palette/` (approval strip
      surface — text-only extension of an approved surface/state set; no
      new component kinds, tokens, or layout, so no new prototype per the
      gate's coverage clause; deviations would re-enter the loop).
    - Options Considered: reuse `Mutation` kind with a path-derived message —
      rejected: indistinct audit trail and UI copy.
    - Chosen Approach: dedicated kind end to end.
    - API Notes and Examples:
      ```ts
      // coding-tools.ts routing sketch
      if (isConfigRootPath(action.paths)) return configEditGate(action, options);
      ```
    - Files to Create/Edit: `src/protocol/agent.rs`,
      `clay-agent/src/coding-tools.ts`, `clay-agent/src/host.ts`,
      agent-lane approval component (`frontend/src/agent/*` as located).
    - References: decision `2026-08-30-2157`;
      `design-artifacts/approved/agent-lane-palette/`.
  - Test Cases to Write:
    - `config_mutation_denied_writes_nothing` (daemon policy unit test).
    - `config_edit_approval_roundtrip_reaches_strip` (protocol/frontend
      test per existing approval tests).

- [ ] Route agent document operations for config-root paths and add `config.state` / `config.reload` / `config.diagnostics`
  - Acceptance Criteria:
    - Functional: `clay-agent/src/document-ops.ts` read/write/stat/edit
      paths detect config-root paths and call `config.*` reverse methods
      (after approval) instead of `document.*`, so the agent's normal
      `read`/`write`/`edit` tools just work on `~/.clay/**`; three new
      reverse methods return: effective configuration state (generation,
      active theme/design-system/typography selections — bounded snapshot),
      a reload trigger (serialized, reuses `reload_runtime_generation`),
      and a bounded drain of current runtime diagnostics.
    - Performance: `config.state` snapshot bounded (existing state budget);
      reload serialized with user-triggered reloads (no parallel
      generations); diagnostics drain bounded and non-blocking.
    - Code Quality: routing isolated in one predicate + thin adapters;
      Prism tool contracts preserved (same result shapes).
    - Security: reload authority is daemon-only (reverse direction), never
      package-JS-reachable; diagnostics are sanitized (existing
      RuntimeDiagnostic redaction) before crossing to the daemon.
  - Approach:
    - Documentation Reviewed: `src/server/mod.rs:970,1124` (reload
      serialization); `runtime/js/configuration.js:43`
      (`getConfigurationState` — reuse its snapshot shape server-side).
    - Options Considered: expose reload as a Clay JS API instead — rejected:
      JS-triggered reload already exists (`runtime.reloadConfiguration`
      command); the daemon needs the reverse-RPC form, not a second user
      API.
    - Chosen Approach: three small reverse methods + one routing predicate.
    - API Notes and Examples:
      ```ts
      // document-ops.ts sketch
      const method = isConfigRootPath(absolutePath) ? "config.read" : "document.read";
      ```
    - Files to Create/Edit: `clay-agent/src/document-ops.ts`,
      `src/server/agent_documents.rs`, protocol types if the daemon-facing
      payloads need shared shapes.
    - References: `clay-agent/src/document-ops.ts:155-190`.
  - Test Cases to Write:
    - `agent_edit_tool_modifies_init_js_after_approval` (e2e headless:
      scratch config root, approve via stub, edit, reload, assert state).
    - `config_state_and_diagnostics_visible_to_daemon`.
    - `reload_serialized_with_watcher_reload`.

- [ ] Seed the `clay-config` agent skill with the restart matrix and cheat sheet
  - Acceptance Criteria:
    - Functional: fourth entry in `AGENT_DELIVERED_SKILLS`
      (`clay-agent/src/host.ts:256`); SKILL.md content covers: the config
      file map (init.js, `~/.clay/agents/<type>/{SYSTEM.md,skills.json,
      mcp.json,tool-caps.json,data/}`, preferences.json closed-store note),
      the init.js API cheat sheet (theme/keybindings/editor/shell/packages
      facades with one-line examples), the approval semantics (ConfigEdit,
      when to ask), the reload/restart matrix (init.js: hot-reload;
      SYSTEM.md: next session; skills: daemon start; mcp.json: next Clay
      launch; preferences: ui-session layer), and the three change tiers.
    - Performance: seeding is daemon-start file I/O as today (bounded,
      idempotent).
    - Code Quality: skill body is static, hand-reviewable TS content
      following the existing three skills' structure; `.seed-manifest.json`
      stamping reused unchanged.
    - Security: the skill instructs redaction — never echo env values from
      `mcp.json` into transcripts; never write credential/vault paths.
  - Approach:
    - Documentation Reviewed: `clay-agent/src/host.ts:256-330` (seeding
      mechanics); existing skill bodies for tone/structure;
      `docs/wiki/modules/agent-configuration.md` (or nearest).
    - Options Considered: generate the cheat sheet from
      `docs/generated/clay-js-api-registry.json` at seed time — deferred:
      the registry is a repo artifact not shipped beside the daemon;
      plan 144's `help.lookup` is the non-drifting answer, cited from the
      skill as "when available".
    - Chosen Approach: static skill + explicit pointer to live lookup when
      the workspace is the clay repo (graft + docs there).
    - API Notes and Examples:
      ```ts
      { dir: "clay-config", skill: clayConfigSkill },
      ```
    - Files to Create/Edit: `clay-agent/src/host.ts` (+ skill content
      module), `clay-agent/src/__tests__/` seeding tests.
    - References: plan 117 (seed provenance stamps).
  - Test Cases to Write:
    - `clay_config_skill_seeds_once_and_stamps_manifest`.
    - `edited_clay_config_skill_not_overwritten`.

- [ ] Create or verify Clay JS APIs and configuration surface for the agent config loop
  - Acceptance Criteria:
    - Functional: verify no new user-facing Clay JS API is required (the
      `config.*` methods are daemon↔server reverse RPC, not Clay JS APIs);
      every new server-side Rust public function is either exposed through
      an explicit op + facade or made `pub(crate)` — inventory recorded in
      task evidence. Configuration duty: no new configuration keys are
      introduced and `~/.clay/init.js` remains the sole user entry point;
      agent-facing "configuration APIs" are the reverse methods plus the
      seeded skill, documented as such.
    - Performance: n/a.
    - Code Quality: docs-as-code bar; cross-linked from the skill;
      Rust-function inventory with disposition recorded.
    - Security: reverse-RPC methods are daemon-only and never package-JS
      reachable; containment wording truthful (trusted same-user authority,
      not OS confinement); secrets stance stated.
  - Approach:
    - Documentation Reviewed: `references/docs-as-code.md`;
      `references/js-api.md` (boundary: reverse-RPC methods are daemon
      protocol, not Clay JS APIs — the API-verification duty is satisfied
      by recording that stance and keeping new Rust fns `pub(crate)`);
      `references/config.md` (no new configuration keys).
    - Options Considered: expose `config.reload`/`config.state` as user
      Clay JS APIs — rejected: `runtime.reloadConfiguration` already exists
      as the user-facing form; duplicating it widens surface for nothing.
    - Options Considered (docs): skip agent docs, skill-only — rejected: the
      skill is user-editable and can drift; docs are the durable contract.
    - Chosen Approach: one focused docs page + wiki task coverage.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `docs/reference/agents/configuration-changes.md`
      (path per existing tree), `docs/index.md` link.
    - References: decision log (task 2).
  - Test Cases to Write: doc-link gate if one exists for the chosen path
    (else manual).

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: the changed UI is the approval strip's ConfigEdit prompt;
      launch a real Linux GUI build, trigger a config-edit approval from a
      real session, capture screenshots (prompt shown, approve path, deny
      path), compare against
      `design-artifacts/approved/agent-lane-palette/` surface by surface;
      record every deviation with disposition.
    - Performance: n/a.
    - Code Quality: evidence paths recorded in the task.
    - Security: no real secrets in screenshots (scratch profile).
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Mandatory
      UI Visual and Accessibility Review Task; `references/ui.md` +
      component/token catalogs (approval strip is cataloged surface).
    - Options Considered: skip (text-only change) — rejected: the duty is
      mandatory for UI-touching plans; the check is cheap.
    - Chosen Approach: `computer-use-linux` `get_app_state` + screenshots
      when available; else record blocker and leave unresolved.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: evidence under
      `code-reviews/<date>-plan139-visual/`.
    - References: decision `2026-08-14-0200`.
  - Test Cases to Write: manual evidence.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: agent module steps added: tier-1 loop (ask → approve →
      reload → verified), deny path, vault rejection, restart-matrix spot
      checks (SYSTEM.md next-session, mcp.json next-launch); results
      recorded on Linux.
    - Performance: n/a.
    - Code Quality: no weakened steps.
    - Security: negative steps: unapproved write mutates nothing; edited
      skill not clobbered.
  - Approach:
    - Documentation Reviewed: `test-plan/index.md`.
    - Options Considered: automated-only — rejected: approval UX is
      user-visible.
    - Chosen Approach: extend the agent-lane/approval module.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `test-plan/agent-*.md`, `test-plan/index.md`.
    - References: user instruction 2026-08-04.
  - Test Cases to Write: the added steps.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: wiki documents the reverse-method set, containment,
      approval kind, lease integration, reload feedback, and skill; index
      updated.
    - Performance: none added.
    - Code Quality: docs-as-code bar.
    - Security: boundaries documented without secrets; containment language
      truthful.
  - Approach:
    - Documentation Reviewed: `references/docs-as-code.md`.
    - Options Considered: per-task updates — rejected (churn).
    - Chosen Approach: one post-pass.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `docs/wiki/modules/agent-*.md` as located,
      `docs/wiki/index.md`.
    - References: docs-as-code reference.
  - Test Cases to Write: manual wiki review.

## Compromises Made
- To be filled after execution.

## Further Actions
- To be filled after execution. Known candidates: a session-scoped "allow
  config changes" lane toggle (narrower than full autonomy) — needs its own
  approval-UX pass; bounded agent-facing screenshot/diff tool for visual
  verification of theme changes (structural + diagnostic feedback first);
  live `help.lookup` wiring for the skill (plan 144).
