# Adopt Prism 0.5.1 Across Clay Agent Surfaces

Source: Prism 0.5.1 (`CHANGELOG.md` `[0.5.1]`, plan 066) plus Clay
decision `2026-09-07-1325` (provider-mandatory wire requirements are
Prism-side; Clay `createSessionCachePolicy()` is a temporary stopgap).

This plan moves live `clay-agent` pins from exact Prism **0.5.0** to
exact **0.5.1** and rewires every Clay-owned Prism generate site onto the
0.5.1 kernel constructor. It does **not** add providers, tools, UI, or
packages.

Scope note: **no UI-surface tasks**. Effort picker, AG-UI transport, and
chat/coding panels already pass `thinkingLevel`; this plan only changes
how the daemon stamps provider requests. If execution starts changing
rendered UI, stop, load `clay-ui` plus the four mandatory design skills,
and record it. No editor mode, language mode, Clay JS package, package
runtime capability, or extension point is added or materially changed, so
no primitive-review, package-runtime trust-domain, `init.js`
default-loading, grammar, external-process-authority, or package
UI/authoring task is required. No new user-facing configuration surface:
skip example-config maintenance and live launch-test. Clay JS API and
configuration tasks below are verification-only. Manual test plan is
required because OpenCode Go, OM workers, cache-control models, and
fail-fast provider errors are user-visible.

Confirmed architecture:

- `decision-logs/2026-09-06-0432-prism-0.5.0-clay-agent-family-pins.md`
  (live 0.5.0 seven-family pins)
- `decision-logs/2026-09-07-1325-prism-provider-mandatory-wire-requirements.md`
  (stopgap to delete once Prism constructs requests)
- `.agents/skills/project-patterns/references/agent-host.md`
- `roadmap.md` Prism 0.5.0 lockstep section (update to 0.5.1)

Project patterns: `planning-checklist.md`, `agent-host.md`,
`authority-boundaries.md`, `extensions-and-ai.md`,
`product-surfaces-are-packages.md`, `clay-js-api-naming.md`,
`clay-js-api-boundary.md`, `documentation-as-code.md`,
`doc-registry-tests.md`, `maintenance-validation.md`,
`configuration-system.md`.

Library docs (Context7 has no `@arnilo/prism`; it resolves
`/stoplightio/prism`. Authoritative APIs are the local Prism 0.5.1 tree):

- `/home/arn/Projects/prism/docs/migrate-to-0.5.md` §8 (0.5.1 additive)
- `/home/arn/Projects/prism/docs/provider-request-policies.md`
- `/home/arn/Projects/prism/docs/provider-packages.md`
- `/home/arn/Projects/prism/docs/thinking-and-reasoning.md`
- `/home/arn/Projects/prism/CHANGELOG.md` `[0.5.1]`
- `/home/arn/Projects/prism/plans/066-Provider-Request-Construction-And-0-5-1.md`

npm: `@arnilo/prism@0.5.1` is published (confirmed `npm view`). Clay keeps
exact pins, not `^0.5.1`.

Not in scope: new adapters, Azure/Bedrock/Vertex factory wiring, ACP,
AG-UI-in-daemon, `@arnilo/prism-office`, workflows/supervisors, cache.mode
Clay config, wrapping `AIProvider.generate()`, package-policy
auto-activation, UI chrome.

Clay Prism surfaces this plan must leave working on 0.5.1 (no host
policy):

| Surface | Clay path | 0.5.1 host change |
| --- | --- | --- |
| Main `createAgent` (Chat + Coding) | `clay-agent/src/host.ts` `createSession` | Drop `providerRequestPolicies: createSessionCachePolicy()` |
| Branch-summary worker | `refineBranchSummary` | Same drop |
| Per-prompt thinking | `sessionPrompt` → `session.stream` | `RunOptions.thinkingLevel`; stop hand-merging `applyThinkingLevelForModel` into `providerOptions` |
| Model picker levels | `modelList` | Keep `thinkingLevelsForModel` (unchanged) |
| Observational memory | `attachOm` → `@arnilo/prism-memory` | None. Kernel uses `om:{session.id}` |
| LLM compaction | `compaction.ts` `createLlmCompactionStrategy` | None. Kernel reuses agent session id |
| OM compaction | `createObservationalMemoryCompactionStrategy` | None |
| Coding tools / document ops | `coding-tools.ts`, `document-ops.ts` | None |
| MCP | `mcp.ts` `connectMcpTools` | None. Still no SDK v2 imports |
| Wiki / graft | `host.ts` `enableWiki` / `enableGraft` | None |
| Obscura / browser | `obscura.ts` | None |
| Credentials / sqlite | `create` / `createSqlitePersistence` | None. Tenant + keyring 2 already 0.5.0 |
| Skills / commands / identity | `host.ts` | None |
| Custom `provider.generate` | **none in clay-agent src** | Do not add `applyDefaultProviderRequestOptions` |

## Objectives

- Pin the seven adopted `@arnilo/prism*` families to exact `0.5.1`. Keep
  `better-sqlite3@13.0.3` and `playwright-core@1.61.0`.
- Delete the Clay-side `createSessionCachePolicy()` stopgap on both
  `createAgent` sites. Kernel fills `sessionId`/`cacheKey`; OpenCode Go
  emits `x-opencode-session` without host policy; OM workers get
  `om:{session.id}`.
- Move per-prompt effort onto `session.stream(..., { thinkingLevel })`.
  Keep `parseThinkingLevel` fail-closed at the RPC boundary and
  `thinkingLevelsForModel` on `model.list`.
- Adopt kernel default cache breakpoints (`system_prompt` +
  `last_stable_message`, `cacheRetention: "short"`) for
  `cache_control` / `explicitBreakpoints` models. Do not add a Clay
  `cache.mode` knob.
- Prove every existing clay-agent suite plus Linux cargo gates still
  pass. No persisted-schema migration. Rollback = exact 0.5.0 pins +
  `npm ci`.

## Expected Outcome

- `clay-agent/package.json` pins `@arnilo/prism@0.5.1` and the six sibling
  families at exact `0.5.1`. `initialize` reports `{ prism: "0.5.1" }`.
- `clay-agent/src/host.ts` contains neither `createSessionCachePolicy` nor
  `applyThinkingLevelForModel`. Both `createAgent` calls omit
  `providerRequestPolicies`.
- Capturing-provider tests show `options.sessionId` / `options.cacheKey`
  equal the Prism session id with no host policy; thinking wire fields
  still match family stamps; OM worker requests use `om:` prefix.
- `npm ci && npm run build && npm test` pass in `clay-agent`.
- `tests/agent_protocol.rs` `phase25_dependencies_deny_acp_agui_mcp`
  asserts README + exact `0.5.1` pins and that host sources do not
  import `createSessionCachePolicy`.
- Linux: `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo test --test agent_protocol`.
- Wiki + `agent-host.md` + `roadmap.md` say live pins are Prism 0.5.1
  and the stopgap is gone.

## Tasks

- [x] Record the Prism 0.5.1 clay-agent pin set in `decision-logs/`
  - Acceptance Criteria:
    - Functional: A new decision log states the 0.5.1 pin set (seven
      families at exact `0.5.1` + `better-sqlite3@13.0.3`; drop the
      `createSessionCachePolicy` stopgap; adopt kernel session/cache/
      thinking construction; keep Azure/Bedrock/Vertex stubs; never
      adopt office/ACP/AG-UI in the daemon) and marks decision 1325's
      follow-up done.
    - Performance: Logging adds no runtime work.
    - Code Quality: Filename `YYYY-MM-DD-HHMM-*.md`; follows
      `create-decision-log` template; existing logs stay immutable.
    - Security: Log records no secrets; restates no-ACP/AG-UI-as-bus
      and that session/cache keys are correlation ids, never secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `.agents/skills/project-patterns/references/agent-host.md`
      - `decision-logs/2026-09-06-0432-prism-0.5.0-clay-agent-family-pins.md`
      - `decision-logs/2026-09-07-1325-prism-provider-mandatory-wire-requirements.md`
      - `/home/arn/Projects/prism/docs/migrate-to-0.5.md` §8
    - Options Considered:
      - Skip the log and treat migrate-to-0.5 §8 as enough: faster, but
        0.4/0.5 pin cuts logged first and 1325 requires an explicit
        follow-up when Prism enforcement ships.
      - Log after user approval, then implement: selected.
    - Chosen Approach:
      - Stop if the user has not explicitly approved logging. After
        approval, write one log. Fold the durable 0.5.1 pin + “no host
        session-cache policy” rule into `agent-host.md`. Point roadmap
        live pins at 0.5.1.
    - API Notes and Examples:
      ```text
      decision-logs/YYYY-MM-DD-HHMM-prism-0.5.1-clay-agent-family-pins.md
      ```
    - Files to Create/Edit:
      - `decision-logs/YYYY-MM-DD-HHMM-prism-0.5.1-clay-agent-family-pins.md`: new log
      - `.agents/skills/project-patterns/references/agent-host.md`: live
        pins `0.5.0` → `0.5.1`; delete the stopgap sentence
      - `roadmap.md`: live-pin / Phase 2.1 lockstep section → 0.5.1
    - References:
      - `.agents/skills/project-patterns/SKILL.md`
      - `decision-logs/2026-09-07-1325-prism-provider-mandatory-wire-requirements.md`
  - Test Cases to Write:
    - None. Decision logs are not executed.
  - Completion (2026-09-07):
    - Log written: `decision-logs/2026-09-07-2149-prism-0.5.1-clay-agent-family-pins.md`
      (status approved; closes the 1325 stopgap follow-up; records exact
      0.5.1 seven-family pins + `better-sqlite3@13.0.3`, stopgap deletion,
      `RunOptions.thinkingLevel`, no `cache.mode` knob, no
      `applyDefaultProviderRequestOptions`, office/ACP/AG-UI excluded).
    - `.agents/skills/project-patterns/references/agent-host.md`: live pins
      → `0.5.1`; stopgap sentence replaced with the kernel-construction
      rule; decision list adds `2026-09-07-2149`, marks 0432 superseded for
      live pins.
    - `roadmap.md`: governing-decisions note, Product Shape daemon bullet,
      capability-review header/live-pins paragraph, new 0.5.1 increment
      paragraph in the lockstep section, Resolved-this-iteration bullet,
      open decision 7 resolved + new item 8 resolved (`2026-09-07-2149`).

- [x] Bump clay-agent family pins to exact 0.5.1
  - Acceptance Criteria:
    - Functional: All seven `@arnilo/prism*` dependencies are exact
      `"0.5.1"`. `initialize` reports `prism: "0.5.1"`. Lockfile
      resolves registry 0.5.1 tarballs. `better-sqlite3` stays
      `13.0.3`. No 0.5.0 pin remains in `clay-agent/package.json`.
    - Performance: No new process, RPC method, or event-queue change.
    - Code Quality: Lockstep only — do not mix 0.5.0 and 0.5.1.
      Subpath imports unchanged.
    - Security: No ACP/AG-UI/office/antigravity/SDK-module pins.
      `npm ls --all` still shows MCP SDK only under `@arnilo/prism-mcp`.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.5.md` Upgrade steps
        1, 7–8 (bump `@arnilo/*` to 0.5.1; no persisted migration)
      - `/home/arn/Projects/prism/CHANGELOG.md` `[0.5.1]` lockstep bump
      - `clay-agent/package.json`, `clay-agent/README.md` Pins / Upgrade
      - `tests/agent_protocol.rs` `phase25_dependencies_deny_acp_agui_mcp`
    - Options Considered:
      - Range pin `^0.5.1`: rejected. Pattern is exact reviewed pins.
      - Partial bump (`@arnilo/prism` only): rejected. 0.5.1 is lockstep.
      - Exact seven-family `0.5.1`: selected.
    - Chosen Approach:
      - Edit `package.json`, `npm install` in `clay-agent`, bump the
        `initialize` version string and README/tests that assert
        `"0.5.0"`. Leave host policy wiring for the next task so the
        pin cut is bisectable.
    - API Notes and Examples:
      ```json
      {
        "@arnilo/prism": "0.5.1",
        "@arnilo/prism-core": "0.5.1",
        "@arnilo/prism-providers": "0.5.1",
        "@arnilo/prism-coding-tools": "0.5.1",
        "@arnilo/prism-web-tools": "0.5.1",
        "@arnilo/prism-memory": "0.5.1",
        "@arnilo/prism-mcp": "0.5.1",
        "better-sqlite3": "13.0.3"
      }
      ```
    - Files to Create/Edit:
      - `clay-agent/package.json`: pins + description
      - `clay-agent/package-lock.json`: regenerate
      - `clay-agent/src/main.ts`: `prism: "0.5.1"`
      - `clay-agent/src/providers.ts`: comment 0.5.0 → 0.5.1
      - `clay-agent/src/__tests__/host.test.ts`: initialize assertion
      - `clay-agent/README.md`: version, Pins, Upgrade Prism
      - `tests/agent_protocol.rs`: README + exact-pin strings `0.5.1`
    - References:
      - `decision-logs/2026-09-06-0432-prism-0.5.0-clay-agent-family-pins.md`
      - `.agents/skills/project-patterns/references/agent-host.md`
  - Test Cases to Write:
    - `initialize reports prism 0.5.1`: stdio initialize →
      `{ ok: true, prism: "0.5.1" }`
    - `phase25_dependencies_deny_acp_agui_mcp`: exact `"0.5.1"` pins;
      no retired 0.3 names; no SDK modules in clay-agent package.json
  - Completion (2026-09-07):
    - `clay-agent/package.json`: description + seven families → exact
      `0.5.1`; `better-sqlite3@13.0.3` / `playwright-core@1.61.0`
      unchanged; lockfile regenerated via `npm install` (registry
      `@arnilo/prism@0.5.1` confirmed).
    - Version strings: `main.ts` initialize → `prism: "0.5.1"`;
      `host.test.ts` test name + assertion; `providers.ts` comment;
      `host.ts` comment version-neutral; `thinking-level.test.ts`
      comment → `0.5.x`; README header + Pins section → `0.5.1`;
      `tests/agent_protocol.rs` README assert + seven pin needles →
      `0.5.1`.
    - Gates: `npm run build` + `npm test` → 97 pass / 1 skip (same as
      0.5.0 baseline); `cargo fmt --check` clean;
      `cargo test --test protocol` → 208 pass. Note: `agent_protocol.rs`
      runs as a module of the `protocol` suite (`autotests = false`),
      not as its own target.
    - Pre-existing gate fixes (unrelated drift, blocked the suite on
      clean HEAD): parity ledger `agent.codingAgent.parity` was missing
      module-17 steps C1–C20 (plan 109 steps never recorded) and
      Phase 112 `theme.setIconPack` had no ledger row — both added to
      `docs/development/tauri-react-parity-ledger.json`.

- [x] Drop host session-cache policy; pass `RunOptions.thinkingLevel`
  - Acceptance Criteria:
    - Functional: Neither `createAgent` site sets
      `providerRequestPolicies`. `session.prompt` with a valid
      `thinkingLevel` passes `{ thinkingLevel }` on `session.stream`;
      empty/non-string still RPC `-32602`. Kernel stamps
      `options.sessionId`/`cacheKey` and thinking compat without Clay
      merging `providerOptions`.
    - Performance: No extra generate hop, policy chain, or RPC.
    - Code Quality: Remove unused `createSessionCachePolicy` and
      `applyThinkingLevelForModel` imports. Keep `parseThinkingLevel`
      and `thinkingLevelsForModel`. No custom `provider.generate`
      wrapper.
    - Security: Session/cache keys stay correlation ids. Prompt catch
      still redacts via `rpcError` + existing secret redactor.
      `ProviderRequirementError` messages (no bodies) may surface as
      `-32000` text; do not log request bodies.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.5.md` §8 steps 1–4, 8
      - `/home/arn/Projects/prism/docs/provider-request-policies.md`
        (`applyDefaultProviderRequestOptions` is kernel-owned;
        `createSessionCachePolicy` is overlay-only)
      - `/home/arn/Projects/prism/docs/thinking-and-reasoning.md`
        session entry: `AgentConfig.thinkingLevel` /
        `RunOptions.thinkingLevel`
      - `clay-agent/src/host.ts` `createSession` L797–L836,
        `sessionPrompt` L1615–L1646, `refineBranchSummary` L1892–L1906
    - Options Considered:
      - Keep `createSessionCachePolicy` as overlay “just in case”:
        rejected. 1325 + migrate §8 say drop it when it existed only
        for session correlation. Kernel already fills; overlay is
        redundant and hides coverage holes.
      - Keep `applyThinkingLevelForModel` into `providerOptions`:
        rejected. 0.5.1 session API is `thinkingLevel`; hand-merge is
        the 0.5.0 host workaround.
      - Set `createAgent({ thinkingLevel })` as a session default:
        rejected. Clay effort is per-prompt (`pendingEffort`), not a
        persisted agent default.
      - Add Clay `cache.mode: "off"` overlay: rejected. Adopting 0.5.1
        includes P2 default breakpoints. Add a knob only if cache cost
        becomes a measured problem.
      - Drop policy + `stream({ thinkingLevel })`: selected.
    - Chosen Approach:
      - Delete both `providerRequestPolicies` lines. Replace the
        `providerOptions = applyThinkingLevelForModel(...)` block with
        `thinkingLevel: resolved` on the existing `stream` options.
        Branch-summary worker keeps `createMemorySessionStore()`; its
        new session id is enough for OpenCode Go. Do not call
        `applyDefaultProviderRequestOptions` — Clay has no raw
        generate site.
    - API Notes and Examples:
      ```ts
      const agent = createAgent({
        id: profile,
        model,
        providerSource: createProviderResolver(this.kernel.registries.providers),
        store: this.persistence,
        runLedger: this.persistence,
        redactor: this.redactor,
        validator: createJsonSchemaToolArgumentValidator(),
        limits: { /* unchanged */ },
        identity: this.runIdentity(),
        // no providerRequestPolicies
      });

      const level = parseThinkingLevel(rawLevel);
      if (rawLevel !== undefined && level === undefined) {
        throw rpcError(-32602, "thinkingLevel must be a non-empty string");
      }
      const resolved = typeof level === "string" ? level : level?.opaque;
      const stream = live.session.stream(text, {
        maxQueuedEvents: MAX_QUEUED_EVENTS,
        overflow: "drop_oldest",
        identity: this.runIdentity(),
        ...(resolved ? { thinkingLevel: resolved } : {}),
      });
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: both `createAgent` sites +
        `sessionPrompt` thinking block + imports
    - References:
      - `decision-logs/2026-09-07-1325-prism-provider-mandatory-wire-requirements.md`
      - `/home/arn/Projects/prism/docs/migrate-to-0.5.md` §8
  - Test Cases to Write:
    - Existing `thinking-level.test.ts` wire-field cases still pass
      with `thinkingLevel` on the run (Anthropic `output_config.effort`,
      xAI `reasoning_effort`, Google `thinkingLevel`, snap, fail-closed,
      non-reasoning no-op).
  - Completion (2026-09-07):
    - `host.ts`: both `providerRequestPolicies: createSessionCachePolicy()`
      lines removed (main `createAgent` keeps a one-line breadcrumb
      pointing at decision 2149); `sessionPrompt` resolves the level via
      `parseThinkingLevel` (fail-closed `-32602` kept) and passes
      `thinkingLevel` on `session.stream` — the model lookup +
      `applyThinkingLevelForModel` hand-merge block is gone; imports drop
      `createSessionCachePolicy` + `applyThinkingLevelForModel`, keep
      `parseThinkingLevel` + `thinkingLevelsForModel`. No
      `applyDefaultProviderRequestOptions` (no raw generate site).
    - Verified against installed 0.5.1 types: `RunOptions.thinkingLevel?: string`
      (`contracts-protocol.d.ts:79`, overrides `AgentConfig.thinkingLevel`).
    - Gates: `npm test` 97 pass / 1 skip (thinking wire-field cases green
      through the kernel path); `cargo test --test protocol agent_protocol`
      20 pass.

- [x] Add kernel-construction tests and deny the stopgap
  - Acceptance Criteria:
    - Functional: A capturing mock prompt with **no** host policy
      yields `options.sessionId === session.id` and
      `options.cacheKey === session.id`. A `cache.kind: "cache_control"`
      model gets default `{ system_prompt, last_stable_message }`
      breakpoints unless the test sets mode/off. OM worker generate
      (when captured) uses `om:` + attached session id. Host sources
      contain no `createSessionCachePolicy`.
    - Performance: Tests are in-process mock providers; no network.
    - Code Quality: Extend existing capturing-provider harness;
      one new test file only if the thinking file would mix concerns.
    - Security: Assertions check correlation ids only; no secret
      fixtures. `ProviderRequirementError` is not thrown on Prism-owned
      session runs.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/provider-request-policies.md`
        helper contract + OpenCode Go fail-fast
      - `/home/arn/Projects/prism/docs/migrate-to-0.5.md` §8 OM vs
        compaction ids
      - `clay-agent/src/__tests__/thinking-level.test.ts`
      - `clay-agent/src/__tests__/om.test.ts`
      - `tests/agent_protocol.rs` pin/deny test
    - Options Considered:
      - Live OpenCode Go in CI: rejected. Needs credentials; keep as
        manual test-plan step.
      - Assert `x-opencode-session` via the real adapter in unit tests:
        unnecessary if sessionId/cacheKey are filled — adapter map is
        Prism's. Clay asserts the kernel inputs.
      - Capturing mock + source deny: selected.
    - Chosen Approach:
      - Extend the thinking capturing harness (or a sibling
        `request-construction.test.ts`) for session/cache/default
        breakpoints. Capture OM worker `options.sessionId` from the
        existing routing provider if cheap; otherwise assert via the
        thinking/session mock plus the OM suite remaining green.
      - Add `assert!(!host.contains("createSessionCachePolicy"))` next
        to the 0.5.1 pin asserts in `agent_protocol.rs`.
    - API Notes and Examples:
      ```ts
      // after session.prompt with no thinkingLevel and no host policy
      assert.equal(captured.options?.sessionId, sessionId);
      assert.equal(captured.options?.cacheKey, sessionId);
      ```
    - Files to Create/Edit:
      - `clay-agent/src/__tests__/thinking-level.test.ts` and/or
        `clay-agent/src/__tests__/request-construction.test.ts`
      - `clay-agent/src/__tests__/om.test.ts`: OM `om:` prefix if the
        routing provider can observe `request.options`
      - `tests/agent_protocol.rs`: deny `createSessionCachePolicy` in
        `clay-agent/src/host.ts`
    - References:
      - `/home/arn/Projects/prism/plans/066-Provider-Request-Construction-And-0-5-1.md`
        Expected Outcome
  - Test Cases to Write:
    - `session.prompt fills sessionId/cacheKey without host policy`
    - `cache_control model gets default short breakpoints`
    - `thinkingLevel still reaches family wire fields`
    - `host.ts does not mention createSessionCachePolicy`
    - `OM worker sessionId is om:{id}` (if capturable; else document
      as covered by Prism 066 + OM suite green)
  - Completion (2026-09-07):
    - New `clay-agent/src/__tests__/request-construction.test.ts`
      (capturing-mock harness in the thinking-level style): kernel fills
      `options.sessionId`/`cacheKey` = session id with no host policy;
      `cache.kind: "cache_control"` model gets `cacheRetention: "short"`
      + breakpoints `[{location: "system_prompt"},
      {location: "last_stable_message"}]`; uncached model gets no cache
      patch. Verified against `provider-request-policy.js`
      (`applyDefaultProviderRequestOptions` semantics).
    - `om.test.ts`: routing provider gained an optional request capture
      (on the top-level `mockProvider` — workers resolve the registry
      provider, not the OM config instances); the activity drill asserts
      a captured worker request with `om:`-prefixed `options.sessionId`.
    - `tests/agent_protocol.rs`: deny
      `createSessionCachePolicy` in `clay-agent/src/host.ts` next to the
      0.5.1 pin asserts (host.ts breadcrumb comment reworded so the deny
      matches bare identifier only).
    - Gates: clay-agent `npm test` 101 tests / 100 pass / 1 pre-existing
      skip; `cargo fmt --check` clean; `cargo test --test protocol`
      208 pass.

- [x] Verify every Clay Prism functionality on 0.5.1
  - Acceptance Criteria:
    - Functional: Full `clay-agent` `npm test` green (chat/mock,
      thinking, OM, compaction, coding tools, durable run, skills/
      commands, session search/tree/discard, MCP v2 import deny,
      MCP/Obscura fail-closed, wiki, graft, keychain, context).
      `cargo test --test agent_protocol` green. Linux fmt/check/clippy
      green.
    - Performance: No new daemon, no extra IPC, no paint-path agent
      work (`agent_io_stays_off_paint_and_keypress` still holds).
    - Code Quality: No leftover `"0.5.0"` in clay-agent package.json,
      main.ts initialize, host initialize test, or agent_protocol pin
      list. Wiki/README match 0.5.1 (wiki task may land later; this
      task owns code/tests).
    - Security: MCP still only through `@arnilo/prism-mcp`; no SDK
      imports; ACP/AG-UI/office denied; identity still set on
      `createAgent` and `stream`.
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/README.md` RPC / Pins / Upgrade
      - `clay-agent/src/__tests__/*.ts` current suites
      - `tests/agent_protocol.rs`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
    - Options Considered:
      - Spot-check only thinking + pins: rejected. User asked for all
        Clay Prism functionalities.
      - Run the existing suite as the matrix: selected. No new
        product tests for unchanged surfaces.
    - Chosen Approach:
      - `cd clay-agent && npm test`. Then Linux cargo gates focused on
        `agent_protocol` plus fmt/check/clippy. Treat any 0.5.1 type
        or runtime break as in-scope (fix in this plan, do not defer).
    - API Notes and Examples:
      ```bash
      cd clay-agent && npm ci && npm run build && npm test
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --test agent_protocol
      ```
    - Files to Create/Edit:
      - Tentative: only files that fail the suite after the pin/host
        cut. Prefer host.ts / tests over new abstractions.
    - References:
      - `clay-agent/src/compaction.ts`, `mcp.ts`, `coding-tools.ts`,
        `obscura.ts`, `document-ops.ts`, `providers.ts`
  - Test Cases to Write:
    - None new beyond task 4. This task is the existing-suite gate.
  - Completion (2026-09-07):
    - `clay-agent && npm test`: 101 tests / 100 pass / 1 pre-existing
      skip — full matrix green: chat/mock, thinking wire fields + snap +
      fail-closed, OM activity drill (incl. `om:` correlation from task
      4), llm/om compaction, coding tools + leases + suspend/resume
      durability, skills/commands, session search/tree/discard/fork/clone,
      MCP v2 import deny + stdio allow-list fail-closed, Obscura
      fail-closed, wiki, graft, keychain, context/document ops, vault
      redaction, `initialize reports prism 0.5.1`.
    - Linux gates: `cargo fmt --check` clean, `cargo check --all-targets`
      clean, `cargo clippy --all-targets -- -D warnings` clean,
      `cargo test --test protocol` 208 pass (pin asserts, SDK/MCP-SDK/
      ACP/AG-UI/office denials, `agent_io_stays_off_paint_and_keypress`,
      createSessionCachePolicy deny).
    - Hygiene: no `0.5.0` left in clay-agent src/package/README/
      agent_protocol.rs; identity still set on `createAgent` (host.ts
      L821), `stream` (L1636), and command drivers (L2589); MCP only via
      `@arnilo/prism-mcp`.
    - No 0.5.1 type or runtime break found — no extra fixes needed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: No new `clay:agent` export. Per-prompt
      `thinkingLevel` stays the existing RPC/AG-UI field, not a new
      facade. Inventory of public Rust functions touched by this plan
      is empty or `pub(crate)`.
    - Performance: No new ops or protocol messages.
    - Code Quality: Dotted-ID rules unchanged. Docs that claim Prism
      0.5.0 host-side `applyThinkingLevelForModel` as the Clay
      contract are updated only if such public docs exist (grep found
      none under `docs/reference/`).
    - Security: Still no credential/JS path to session/cache keys.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `docs/reference/clay-js-api/agent/*.md`
    - Options Considered:
      - Expose `agent.setThinkingLevel` in `clay:agent`: rejected.
        Effort is already a prompt argument; YAGNI.
      - Verify no new public API: selected.
    - Chosen Approach:
      - Grep public docs/inventory for 0.5.0 thinking/policy wording.
        Update only if a public page describes the deleted host
        merge. Do not add facades.
    - API Notes and Examples:
      ```ts
      // unchanged transport; not a new JS API
      sendPrompt(text, thinkingLevel?: string)
      ```
    - Files to Create/Edit:
      - Tentative: `docs/reference/clay-js-api/agent/*.md` only if
        they mention the 0.5.0 host merge (currently they do not).
    - References:
      - `frontend/src/agent/TauriClayAgent.ts` `thinkingLevel` arg
      - `src/server/connection/runtime.rs` `intent_thinking_level`
  - Test Cases to Write:
    - Existing registry/inventory tests still pass (`cargo test`
      doc-registry gates). No new API pages.
  - Completion (2026-09-07) — verification only, no code changed:
    - Grep: no `applyThinkingLevelForModel`, `createSessionCachePolicy`,
      or `0.5.0` anywhere in `docs/reference/clay-js-api/` or
      `frontend/src/agent/` — no doc updates needed.
    - No new `clay:agent` export; `TauriClayAgent.sendPrompt(text,
      thinkingLevel?)` (TauriClayAgent.ts L36/L80) and
      `intent_thinking_level` (runtime.rs L342) unchanged as the
      existing transport for per-prompt effort.
    - Gates: `cargo test --test protocol doc` 148 pass (doc-registry),
      `cargo test --lib doc` 107 pass.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: No new `init.js` option for cache mode, session
      policies, or thinking defaults. Existing `clay:agent` compact /
      autonomy / search / tree / knowledge APIs unchanged.
    - Performance: No config-eval work added.
    - Code Quality: Do not invent undocumented config keys.
    - Security: Configuration still cannot grant provider credentials,
      MCP, or cache-key authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `examples/config/init.js` (no change expected)
      - `docs/reference/clay-js-api/agent/compact.md`,
        `set-full-autonomy.md`
    - Options Considered:
      - Add `agent.providerCacheMode`: rejected. Kernel defaults are
        the 0.5.1 product; a Clay knob is speculative.
      - Verify no configuration change: selected.
    - Chosen Approach:
      - Confirm example config and agent JS API custom_properties
        still match. Skip example-config rewrite and live launch-test
        because no user-facing configuration surface changed.
    - API Notes and Examples:
      ```js
      // still not a config key
      // thinkingLevel is per prompt, not init.js
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Example
        Configuration Maintenance Task (does not apply)
  - Test Cases to Write:
    - None. `node --check examples/config/init.js` remains green as a
      non-regression if touched; do not touch.
  - Completion (2026-09-07) — verification only, no code changed:
    - Grep: no `cacheMode`/`providerCache`/`cache.mode`/
      `providerRequestPolic*`/`cacheKey` config keys in `examples/config/`,
      `frontend/src/agent/`, or `docs/reference/clay-js-api/agent/` —
      only the per-prompt `thinkingLevel` transport arg (correct:
      prompt-level, not init.js).
    - `examples/config/init.js` untouched; `node --check` green.
    - No config-eval surface changed in this plan (host.ts edits were
      kernel wiring, not config); compact/autonomy/search/tree/knowledge
      APIs unchanged and covered by the task 5 suite green.

- [x] Execute and update the manual test plan (`test-plan/`)
  - Acceptance Criteria:
    - Functional: Run `test-plan/16-agent-host.md` and
      `test-plan/17-coding-agent-parity.md` steps that cover prompt,
      effort (C4), OM, compaction, OpenCode Go. Add numbered steps
      for: (1) OpenCode Go first prompt succeeds with **no** host
      `createSessionCachePolicy`; (2) OM workers on OpenCode Go no
      longer 400 MissingSessionID; (3) effort still snaps/wires after
      `RunOptions.thinkingLevel`. Do not weaken existing steps.
    - Performance: Prompt/stream latency subjectively unchanged;
      record if a cache-control model now reports cache hits (info,
      not a fail).
    - Code Quality: Update `test-plan/index.md` coverage matrix row
      for this plan. Known ceilings stay in the module ceilings
      section.
    - Security: Live provider runs must not paste secrets into the
      test-plan file.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`
      - `test-plan/16-agent-host.md`
      - `test-plan/17-coding-agent-parity.md`
      - `/home/arn/Projects/prism/docs/providers/opencode-go.md`
    - Options Considered:
      - Skip live OpenCode Go (automated sessionId assert only):
        incomplete — 1325 was a live 400. Keep a live step; if
        credentials/GUI blocked, record UNRESOLVED with the blocker.
      - Add live steps + run automated-cited suites: selected.
    - Chosen Approach:
      - Add C-steps under module 17 (coding agent) for 0.5.1 kernel
        construction. Execute on a real Linux build when possible.
    - API Notes and Examples:
      ```text
      test-plan/17-coding-agent-parity.md  C21+ OpenCode Go / OM / effort
      test-plan/index.md  coverage matrix row for plan 113
      ```
    - Files to Create/Edit:
      - `test-plan/17-coding-agent-parity.md`: new steps + evidence
      - `test-plan/16-agent-host.md`: note 0.5.1 pins if it cites 0.5.0
      - `test-plan/index.md`: matrix row
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test
        Plan Task
  - Test Cases to Write:
    - Manual C21: OpenCode Go prompt without host policy → no
      MissingSessionID
    - Manual C22: OM-attached OpenCode Go worker turn → no 400
    - Manual C4 still: Shift+Tab / dropdown effort reaches the run
  - Completion (2026-09-07):
    - `test-plan/17-coding-agent-parity.md`: new "Plan 113 steps"
      section — C21 (kernel sessionId/cacheKey fills with no host
      policy; no MissingSessionID), C22 (OM worker `om:{session.id}`
      correlation, no 400), C23 (effort snaps/wires through
      `RunOptions.thinkingLevel`) with automated-leg citations;
      "Plan 113 execution record" documents the 0.5.1 gates (protocol
      208, clay-agent 101/1 skip, doc-registry) and records the live
      OpenCode Go legs UNRESOLVED under the standing no-credential
      blocker (plan 109 precedent). No existing steps weakened; plan
      109 record left as history.
    - `test-plan/index.md`: coverage-matrix row for plan 113
      (module 17 C21–C23 + module 16 pin asserts).
    - `test-plan/16-agent-host.md` cites no 0.5.0 pin claim — no edit
      needed.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/clay-agent.md` describes Prism
      **0.5.1**, kernel request construction, no host session-cache
      policy, `RunOptions.thinkingLevel`, OM `om:{id}` / compaction
      session-id reuse. Master index still links the page.
    - Performance: Wiki-only; no runtime work.
    - Code Quality: Replace stale “hosts Prism 0.4.0” / seven 0.4
      pins / `better-sqlite3@12.11.1` text (page is behind even 0.5.0).
    - Security: Document correlation ids vs secrets; OpenCode Go
      fail-fast `ERR_PRISM_PROVIDER_REQUIREMENT`; no secret examples.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `docs/wiki/modules/clay-agent.md`
      - `docs/wiki/index.md` clay-agent blurb if present
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass: selected.
    - Chosen Approach:
      - One wiki pass after implementation/verification and JS
        API/config/manual-test tasks.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/clay-agent.md
      docs/wiki/index.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/clay-agent.md`: 0.5.1 pins, drop stopgap,
        thinkingLevel run option, kernel cache/session construction
      - `docs/wiki/index.md`: blurb if it names a Prism version
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`
  - Test Cases to Write:
    - Manual wiki review: index links clay-agent page; page matches
      host.ts after the cut.
  - Completion (2026-09-07):
    - `docs/wiki/modules/clay-agent.md`: Prism 0.5.1 overview + seven
      0.5.1 exact family pins + `better-sqlite3@13.0.3`; How-It-Works
      item 5 documents kernel request construction (no host request
      policies, `createSessionCachePolicy` deleted + denied, kernel
      sessionId/cacheKey fills + default short breakpoints for
      `cache_control` models, `RunOptions.thinkingLevel` with
      fail-closed host validation, `ERR_PRISM_PROVIDER_REQUIREMENT`
      fail-fast, correlation ids ≠ secrets); item 6 documents OM
      `om:{session id}` worker correlation + LLM compaction session-id
      reuse. No stale 0.4.0/12.11.1 text remains.
    - `docs/wiki/index.md`: clay-agent blurb updated to Prism 0.5.1
      pins + kernel-owned request construction; page link intact.

## Compromises Made

- Default P2 cache breakpoints are adopted with no Clay `cache.mode`
  overlay. Add a knob only if cache-control cost becomes a measured
  problem.
- Azure/Bedrock/Vertex stay host-config stubs (unchanged from 0.5.0).
- No UI, no new Clay JS API, no example-config rewrite.
- Live OpenCode Go / OM 400 verification is a manual test-plan step,
  not CI.

## Further Actions

- To be filled after task completion with improvements, rationale, and
  priority.
