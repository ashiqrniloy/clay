# Prism 0.7 Cheap Wins: toolNames, Attention, Wiki Ingest, Graft Graph Commands

Depends on: `plans/120-Adopt-Prism-0.7.0-Pin-And-Event-Mapper.md` (live pin
must be exact Prism **0.7.0**).

This plan is **Cut 2**: opt-in / additive 0.7 (and folded 0.5.6) surfaces that
fit the existing coding host without spawn, work-scopes, or new UI.

Scope note: **no UI-surface tasks**. Attention events stay dropped by plan
120’s mapper. No new inspector chrome, no composer control for toolNames,
no prototype/approval gate. No Clay JS package change. Skip primitive-review,
package-runtime, default-loading, grammar, external-process-authority,
package UI/authoring, example-config, and live launch-test. Clay JS API and
configuration tasks are verification-only.

Context7 has no `@arnilo/prism`. Authoritative APIs: local Prism 0.7.0 tree.

Not in scope: spawn/supervisor/worktrees (122), work-scopes/fabric (123),
OCR / `createDocumentReader` / office, ACP, AG-UI-in-daemon, `activateKernel`
rewrite of wiki/graft load, Clay `cache.mode`.

Follow-ups delivered in this plan (2026-09-16, tasks below): auto-compaction
on the attention-compiler pair (making `run.setOptions.compactAfterTokens`
live) and the graft `deepModel` knowledge option (vault-resolved key, child
environment only).

## Objectives

- Pass optional `RunOptions.toolNames` from `session.prompt` into
  `session.stream`. Omitted = full registry (today). Empty = no tools.
  Unknown names fail closed. Resume will not widen. Rust server still omits
  the field (full registry) until a later caller needs it.
- Turn on `AgentConfig.attentionCompiler: true` for coding `createAgent`
  only (Chat / tool-free profiles stay off). Do not pair it with
  `contextBudget`. Do not treat it as compaction.
- When wiki is bound, expose `wiki_ingest` / `/wiki-ingest`. URL ingest
  only with a host `fetchUrl` hooked through existing Obscura validation;
  omit the hook when the binary is absent. No new HTTP client. No OCR.
- When graft is bound, set `initYes: true` so `/graft-init` is non-interactive,
  and document `/graft-init` + `/graft-build-deep` on the host graft skill.
  `/graft-build-deep` without `deepModel` stays fail-closed (no silent paid
  model pick). `GRAFT_API_KEY` stays child-env only.

## Expected Outcome

- `session.prompt` accepts optional `toolNames: string[]`; daemon tests
  cover omit / empty / unknown / happy path.
- Coding sessions assemble with the attention compiler at defaults. Chat
  sessions do not set `attentionCompiler`. Over-budget turns raise
  `AttentionBudgetError` rather than silent eviction.
- Bound wiki: `wikiTools()` includes `wiki_ingest`. `/wiki-ingest` is a
  registered slash command after `/wiki-init`. Path/text ingest works
  without Obscura. `url` without `fetchUrl` fails closed.
- Bound graft: `GraftExtensionOptions.initYes === true`. Skill text names
  `/graft-init` and `/graft-build-deep`. `/graft-build-deep` errors before
  spawn when `deepModel` is unset.
- Follow-up: coding sessions with a declared context window arm
  auto-compaction (compiler compactRatio OR two truncated turns OR the
  `compactAfterTokens` ceiling); Chat / limit-less models stay unarmed. A
  configured `graftDeepModel` lets `/graft-build-deep` spawn with the key in
  the child environment only.
- Existing wiki/graft/coding suites plus plan 120 gates still pass.

## Tasks

- [x] Wire `RunOptions.toolNames` on `session.prompt`
  - Acceptance Criteria:
    - Functional: Omitted `toolNames` leaves `session.stream` options
      without the field (full registry). `[]` passes empty (no tools).
      A list of registered names is forwarded. Unknown names throw a
      JSON-RPC error before `stream()`. Non-array values fail closed
      (`-32602`). Resume does not add names the original grant lacked
      (Prism contract; assert via a durable coding run if cheap, else
      document reliance on Prism’s own tests).
    - Performance: One extra optional array copy per prompt. No extra
      provider turn.
    - Code Quality: Parse next to `thinkingLevel`; do not invent
      `RunOptions.tools` / `toolFilter` (removed/never added in 0.7).
    - Security: Names are strings from the host RPC, not model JSON.
      MCP execute still fails closed on schema/effect change (Prism).
      Empty list is a valid “no tools” grant, not “all tools”.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` §14
      - `/home/arn/Projects/prism/docs/agent-session-runtime.md`
        (`session.run` / `RunOptions.toolNames`)
      - `clay-agent/src/host.ts` `sessionPrompt` (`session.stream` options)
      - `.agents/skills/clay-execution/references/packages.md` (Agent Host)
    - Options Considered:
      - Skip the RPC and only use `toolNames` inside spawn children (122):
        smaller, but then mention/agent-type narrowing still requires a
        session rebuild.
      - Also plumb from `agent.submit` / composer: UI, rejected this plan.
      - Daemon RPC seam only, server omits: selected.
    - Chosen Approach:
      - Optional `params.toolNames` on `session.prompt`. Forward to
        `live.session.stream(..., { toolNames })` when present.
    - API Notes and Examples:
      ```ts
      const stream = live.session.stream(promptInput, {
        // existing options…
        ...(toolNames ? { toolNames } : {}),
      });
      // omit  → full registry
      // []    → no tools
      // ["read","shell"] → those names only; unknown throws
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: parse + forward
      - `clay-agent/src/__tests__/host.test.ts` or a small adjacent test file
      - `clay-agent/README.md`: one RPC row/note for `session.prompt`
        `toolNames`
    - References:
      - `/home/arn/Projects/prism/docs/tools.md` (per-run tool scoping)
  - Test Cases to Write:
    - omit → stream called without `toolNames`.
    - `[]` → no tool dispatch on a coding session (or Prism error if the
      profile requires tools — assert fail-closed, not hang).
    - unknown name → RPC error, no provider turn.
    - known subset → only those tools resolve.
  - Completion (2026-09-16):
    - `clay-agent/src/host.ts` `sessionPrompt`: parses optional
      `params.toolNames` next to `thinkingLevel`. `undefined` omits the
      field (full registry), `[]` forwards an empty grant (no tools), and
      a string array passes through verbatim as
      `...(toolNames !== undefined ? { toolNames } : {})`. Non-array
      values and non-string/empty entries fail closed with
      `rpcError(-32602, "toolNames must be an array of non-empty strings")`
      before `live.session.stream`. No `RunOptions.tools` /
      `toolFilter` invented.
    - Unknown names stay Prism-owned: `selectRunTools` rejects
      `Unknown run tool: <name>` during run assembly — inside `stream()`,
      but before the first provider turn and before any tool dispatch —
      and the existing catch maps it to JSON-RPC `-32000`. The host does
      not duplicate the registry check, because the run-visible set also
      includes Prism-owned `load_skill` and MCP-refreshed tools that
      `live.tools` does not list (duplicating it would reject valid
      grants).
    - Resume cannot widen by construction: `run.resume` accepts no
      `toolNames` field, and Prism intersects a resumed run with its
      recorded grant. No host code added; reliance on Prism's
      `selectRunTools` / `agent-run-state` tests documented here (the
      plan's “if cheap” allowance) instead of a durable-run fixture in
      this task.
    - New `clay-agent/src/__tests__/tool-names.test.ts` (5 cases, all
      passing): omit → provider request keeps read/write/edit/shell;
      `[]` → provider request has no tools; `["read"]` → exactly
      `["read"]`; `["read","nope"]` → rejects and fires zero provider
      requests; `"read"`, `42`, `{names}`, `["read",42]`, `["read",""]`
      → `-32602` and zero provider requests. The fixture is hermetic
      (`agentConfigRoot` + workspace temp dirs) so ambient MCP/skills
      cannot alter the registry these assertions read.
    - `clay-agent/README.md` RPC-methods note: optional `toolNames` on
      `session.prompt` (omit / empty / subset semantics, `-32602` shape
      failures, fail-closed unknown names, non-widening resume) and the
      statement that the Rust server still omits the field.
    - Evidence: `cd clay-agent && npm test` → 155 tests, 154 pass, 0 fail,
      1 skipped (pre-existing env-gated `CLAY_WEB_E2E` case). No Rust
      server, Clay JS API, or configuration change.

- [x] Enable attention compiler on coding `createAgent` only
  - Acceptance Criteria:
    - Functional: Coding `createAgent` sets `attentionCompiler: true`
      (Prism defaults). Tool-free / Chat profiles omit it. `RunOptions`
      may pass `attentionCompiler: false` later; this plan does not add
      that RPC. Compiler-on + `contextBudget` is not set (mutually
      exclusive). Session store, OM ledger, and history arrays stay
      unmutated (Prism contract).
    - Performance: Under the trigger ratio, request bytes match 0.6
      behavior. Over ratio, a history *clone* is rewritten; one extra
      measure pass per turn, not a second provider call.
    - Code Quality: Use `true` (defaults). Do not copy ratio tables into
      Clay config. Do not wire `compactAfterTokens` auto-compact here.
    - Security: Clone-only mutation; no new network; `excludeTools` unused
      until a payment-class tool exists (none in clay-agent).
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/attention-compiler.md`
        (`AgentConfig.attentionCompiler`, defaults, mutual exclusion)
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` §18
      - `clay-agent/src/host.ts` `createAgent` (~L1562) and branch-summary
        worker `createAgent` (~L3093)
    - Options Considered:
      - Leave compiler off until compactAfterTokens auto-compact exists:
        delays the window gate Clay already stored unused.
      - Tune `triggerRatio` / `compactRatio` in Clay: extra config, no
        measurement yet.
      - `attentionCompiler: true` on coding agents only: selected.
    - Chosen Approach:
      - Same coding predicate as `durableRunState` (coding tool names
        present). Branch-summary worker stays off (short prompt).
    - API Notes and Examples:
      ```ts
      import { createAgent } from "@arnilo/prism";
      const agent = createAgent({
        // existing fields…
        ...(coding ? { attentionCompiler: true } : {}),
      });
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: coding `createAgent` only
      - `clay-agent/src/__tests__/host.test.ts` (or coding-tools test):
        coding config has `attentionCompiler`, chat/tool-free does not
    - References:
      - `clay-agent/src/host.ts` `durableRunState`
  - Test Cases to Write:
    - Coding session agent config includes `attentionCompiler`.
    - Tool-free profile omits it.
    - Mock long history over ratio: run fails with `AttentionBudgetError`
      (or Prism’s named code), not a silent truncated prompt. If mock
      cannot force the ratio cheaply, unit-test the flag and rely on
      Prism’s compiler tests for the math.
  - Completion (2026-09-16):
    - `clay-agent/src/host.ts`: new module helper `hasCodingTools(tools)`
      (coding tool names present) now backs both `durableRunState` and the
      new compiler gate. `createSession` computes
      `attentionCompiler = hasCodingTools(tools) && model.limits.contextWindow
      is a positive safe integer` and forwards
      `...(attentionCompiler ? { attentionCompiler: true } : {})` — Prism
      defaults, no Clay ratio/knob. `contextBudget` is set nowhere, so the
      mutual exclusion holds. The branch-summary worker
      (`createAgent` ~L3093) is untouched; Chat / tool-free profiles omit
      the field.
    - Deviation from the named approach (unconditional `true` on coding),
      kept because it prevents a live regression: Prism resolves the gate
      cap at run start and fails closed with
      `attentionCompiler requires maxInputTokens or model.limits.contextWindow`
      when the resolved model declares no window. Ollama discovery models
      (`listOllamaModels` → `defineOllamaModel` ships no `limits`) and
      pass-through model ids would then fail **every** coding run instead
      of merely skipping the optimization. Limit-less models therefore
      leave the compiler off (0.6 request bytes) and keep working; models
      that declare a window get the full defaults (`triggerRatio` 0.75,
      `compactRatio` 0.9, `thinkingKeepTurns` 1, `keepLast` 3,
      `reserveTokens` 1024 — all Prism-owned). `RunOptions.attentionCompiler`
      is not wired in this plan.
    - Clone-only mutation, sticky frontier, and the “compiler-on +
      `contextBudget` is a `TypeError`” invariant are Prism contracts owned
      by its own tests; this task asserts only the host-owned enablement
      and the fail-closed over-budget surface (as the plan allows).
    - New `clay-agent/src/__tests__/attention-compiler.test.ts` (5 cases,
      all passing): coding + `windowed` model
      (`limits.contextWindow` 4096) → config `attentionCompiler === true`;
      Chat / tool-free → `undefined`; coding + limit-less `demo` →
      `undefined` and the session still prompts; under-ratio turn → one
      provider request, no `attention_compiled` event; over-ratio turn
      (~10k estimated tokens vs the 2048-token cap, no stubbable rows) →
      rejects with `attention budget exceeded` and zero provider requests.
    - Evidence: `cd clay-agent && npm test` → 160 tests, 159 pass, 0 fail,
      1 skipped (pre-existing env-gated `CLAY_WEB_E2E` case). Existing
      coding suites run the limit-less `demo` mock model, so they stay
      compiler-off and byte-identical to 0.6 behavior.

- [x] Expose wiki ingest on the existing wiki binding
  - Acceptance Criteria:
    - Functional: After `enableWiki`, `wikiTools()` resolves
      `wiki_ingest` plus the three existing tools. `/wiki-ingest` is
      dispatchable (extension-registered; no new `/wiki-init`-style
      initiator). `text`/`path` ingest stages under `.wiki/raw/ingest/`
      with `metadata.trust: "untrusted_external"`. `path` outside the
      workspace fails closed. `url` without Obscura/`fetchUrl` fails
      closed. `url` with Obscura uses `validateObscuraWebUrl` then fetch;
      private/link-local hosts rejected before fetch. Caps stay Prism’s
      (32 MiB in, 2 MiB extract). No OCR, no `extractDocument`.
    - Performance: Ingest is user-triggered, not on the editor hot path.
    - Code Quality: Import `WIKI_INGEST_TOOL_NAME` from
      `@arnilo/prism-memory/wiki` (already the pattern for the other
      three names). Do not reimplement staging.
    - Security: Wiki package never fetches; Clay must not add a raw
      `fetch()`. SSRF = Prism `assertSsrfAllowedUrl` + Obscura URL
      validator. Staged content is untrusted. `log.md` **Ingested** only
      when the wiki root exists.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/wiki.md` tools, `/wiki-ingest`,
        `fetchUrl` Obscura example, caps
      - `clay-agent/src/host.ts` `enableWiki` / `wikiTools`
      - `clay-agent/src/obscura.ts`, `clay-agent/src/resolve-obscura.ts`
      - `clay-agent/src/__tests__/wiki-knowledge.test.ts`
      - `.agents/skills/clay-execution/references/packages.md`
        (Obscura hidden-when-absent)
    - Options Considered:
      - Host `fetch()` for URL ingest: extra HTTP client, worse SSRF.
      - Always require Obscura: blocks path/text ingest.
      - Path/text always; `fetchUrl` only when Obscura resolves: selected.
    - Chosen Approach:
      ```ts
      import {
        WIKI_INGEST_TOOL_NAME,
        WIKI_READ_PAGE_TOOL_NAME,
        WIKI_RECORD_INSIGHT_TOOL_NAME,
        WIKI_SEARCH_TOOL_NAME,
        createWikiExtension,
      } from "@arnilo/prism-memory/wiki";
      import { runObscuraCli, validateObscuraWebUrl } from "@arnilo/prism-web-tools/obscura";

      const options: WikiExtensionOptions = {
        workspaceRoot, wikiRoot: ".wiki", autoDeploySkills: false,
        ...(qmdPath ? { qmdPath } : {}),
        ...(obscuraPath ? {
          fetchUrl: async ({ url }) => {
            validateObscuraWebUrl(url);
            const run = await runObscuraCli({
              command: obscuraPath,
              args: ["fetch", url, "--dump", "markdown"],
            });
            return { text: run.stdout, filename: "source.md" };
          },
        } : {}),
      };
      ```
      Add `WIKI_INGEST_TOOL_NAME` to `wikiTools()`. Update delivered
      wiki-maintainer `toolNames` allowlist tests.
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: `enableWiki` options, `wikiTools()`, comments
      - `clay-agent/src/__tests__/wiki-knowledge.test.ts`: `WIKI_TOOLS`
      - `clay-agent/src/__tests__/agent-skill-files.test.ts`: wiki tool names
      - delivered wiki skill files if the host copies toolNames lists
        (seeded `wiki-maintainer` / `wiki-searcher` under agent config)
    - References:
      - plan 117 wiki initiator (do not clone it for ingest)
  - Test Cases to Write:
    - Bound wiki resolves `wiki_ingest`.
    - Unbound wiki: tool missing.
    - `text` ingest writes `raw/ingest/.../extract.md`.
    - `path` traversal / outside root fails closed.
    - `url` with no `fetchUrl` fails closed.
    - `url` to a blocked host fails before process spawn (mock validator).
  - Completion (2026-09-16):
    - `clay-agent/src/host.ts`: imports `WIKI_INGEST_TOOL_NAME`
      (pattern-consistent) and `runObscuraCli`/`validateObscuraWebUrl` from
      `@arnilo/prism-web-tools/obscura`. `wikiTools()` now resolves
      `wiki_search`, `wiki_read_page`, `wiki_record_insight`, and
      `wiki_ingest`. `enableWiki` computes
      `fetchUrl: WikiExtensionOptions["fetchUrl"]` **only when
      `this.resolveObscura()` returns a binary** (the same host-owned path
      the capability harness uses); the hook re-validates the URL
      (`validateObscuraWebUrl`: public http(s), no credentials — Prism's
      `assertSsrfAllowedUrl` already ran before the hook), runs
      `obscura fetch <url> --dump markdown` per ingest (never on the editor
      hot path, never at session start), and returns
      `{ text: stdout, filename: "source.md" }` (empty stdout ⇒ `null`, the
      wiki's fail-closed path). No raw `fetch()`, no `extractDocument`, no
      cap overrides — input/extract caps stay Prism's 32 MiB / 2 MiB, and
      images keep the no-OCR stub.
    - `/wiki-ingest` needed no new initiator and no new wiring: the
      extension registers the command with the binding, and
      `commandDispatch` already injects drivers for a live session, so the
      command stages and calls `drivers.startRun(brief,
      { activeSkills: ["wiki-maintainer"] })`; without a live session it is
      stage-only (`runStarted: false`).
    - Plan-doc correction: the raw layer is
      `<workspaceRoot>/raw/ingest/<utc>-<slug>/` (workspace-relative posix
      paths in results), **not** `.wiki/raw/ingest/`. Prism 0.7
      `ingestWikiSource` defaults `ingestRoot` to `raw/ingest` beside
      `.wiki/`; the plan's own Test Cases line already said
      `raw/ingest/.../extract.md`. Tests assert the real layout.
    - Delivered skill `toolNames`: no change. `wiki-searcher`'s daemon-owned
      allowlist is Prism's `wikiSearcherSkill.toolNames` (three names) and in
      0.7 it is a *presence* requirement for `load_skill`, not a grant;
      `wiki-maintainer` ships no allowlist (full registry). Adding ingest to
      the searcher list would needlessly block the searcher under a narrowed
      `RunOptions.toolNames` grant, so `agent-skill-files.test.ts` stays as
      is (still green).
    - Tests (`clay-agent/src/__tests__/wiki-knowledge.test.ts`): `WIKI_TOOLS`
      gained `wiki_ingest` (so the existing bound/unbound/gate assertions
      cover it), plus three new cases — staging `text` before scaffolding
      creates no `.wiki/`, then with the wiki root present `log.md` gains
      **Ingested**, `/wiki-ingest` dispatch through a live session starts the
      maintainer run (`runStarted: true`) and stays stage-only without
      drivers; `path` outside the workspace is denied by realpath
      containment; a private-host URL is rejected before the fake Obscura
      CLI spawns (marker file absent); an allowed URL spawns it exactly once
      and stages `source.md` with the markdown dump; with no Obscura, `url`
      rejects with the missing-hook error while `text` still stages.
    - Evidence: `cd clay-agent && npm test` → 163 tests, 162 pass, 0 fail,
      1 skipped (pre-existing env-gated `CLAY_WEB_E2E` case);
      `npx tsc -p tsconfig.json --noEmit` clean. No Rust changes.
    - Compromise: Obscura presence is resolved at bind time (same contract as
      the capability harness), so installing Obscura after a binding was
      created needs a rebind (disable/re-enable or workspace switch) to gain
      URL ingest; text/path ingest never depends on it. The CLI's
      `truncated` flag is not surfaced, and a non-markdown URL extension is
      intentionally staged as `source.md` (it is a markdown dump).

- [x] Graft `/graft-init` and `/graft-build-deep` on the existing binding
  - Acceptance Criteria:
    - Functional: `enableGraft` passes `initYes: true` (and does not set
      `initWireMcp`). `/graft-init` is non-interactive (`--yes`,
      `--no-global`). `/graft-build-deep` without `deepModel` errors
      before spawn. Host graft skill instructions mention both commands.
      `GRAFT_API_KEY` is never placed on argv (Prism child-env only).
    - Performance: Commands stay user-triggered. No graft build at session
      start.
    - Code Quality: Do not add `deepModel` to `init.js` this plan.
    - Security: `graft init --no-global` never writes user-level agent
      state. MCP/hook/statusline wiring stays off. Fail-closed if CLI
      missing (existing).
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/graft.md` (`initYes`,
        `/graft-init`, `/graft-build-deep`, `GRAFT_API_KEY`)
      - `clay-agent/src/host.ts` `enableGraft`, `graftSkill`
      - `clay-agent/src/__tests__/graft-knowledge.test.ts`
    - Options Considered:
      - Silently set `deepModel` from the session model: hidden paid call.
      - Leave `deepModel` unset, fail closed, document: selected.
    - Chosen Approach:
      ```ts
      const extensionOptions: GraftExtensionOptions = {
        projectDir: workspaceRoot,
        mode: options.mode ?? "pull",
        initYes: true,
        // initWireMcp omitted (default off)
        ...(options.cliPath ? { cliPath: options.cliPath } : {}),
        appendEntry, getEntries,
      };
      ```
      Update `graftSkill.instructions` with `/graft-init` and
      `/graft-build-deep` (needs host `deepModel` to run).
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: `enableGraft`, `graftSkill`
      - `clay-agent/src/__tests__/graft-knowledge.test.ts`
      - seeded graft `SKILL.md` if file-backed copy exists
    - References:
      - plan 117 graft default-on
  - Test Cases to Write:
    - Loaded graft extension options include `initYes: true`.
    - `/graft-build-deep` with no `deepModel` returns an error, no child.
    - Skill catalog/body names the two commands when graft is bound.
  - Completion (2026-09-16):
    - `clay-agent/src/host.ts` `enableGraft`: `GraftExtensionOptions` now
      carries `initYes: true` with a comment naming the fixed argv Prism
      derives from it (`--yes --no-global --no-mcp --no-hooks
      --no-statusline`). `initWireMcp`, `initAgents`, `deepModel`,
      `providerEnv`, and `allowUpstreamTelemetry` stay unset: no MCP/hook/
      statusline wiring, no user-level state, no hidden paid LLM call.
    - `graftSkill.instructions` (host-side copy; the seeded `SKILL.md` is
      rendered from it) gained two lines: `/graft-init` (non-interactive
      `--yes`, never user-level state, wiring off) and `/graft-build-deep`
      (graft's own LLM pass; refuses to spawn unless the host configured
      `deepModel` / `GRAFT_PROVIDER`+`GRAFT_MODEL`+`GRAFT_API_KEY` — do not
      invent one).
    - `/graft-build-deep` needed no host code: Prism's handler already
      returns a pre-spawn `error` result when `GRAFT_PROVIDER` /
      `GRAFT_MODEL` / `GRAFT_API_KEY` are absent, and `commandDispatch`
      returns the command result (message + `error`) verbatim, so the
      composer shows the refusal. `/graft-build` (non-deep) is unaffected.
    - No `init.js`/config surface was added for `deepModel` (plan AC) and no
      `GRAFT_API_KEY` reaches argv: the host configures no deep model at
      all, so there is no key in play; Prism's child-env-only contract is
      covered by Prism's own tests.
    - Tests (`clay-agent/src/__tests__/graft-knowledge.test.ts`, one new
      case): a recording stub CLI logs every spawn's argv, then proves —
      `/graft-build-deep` returns the `requires the host to configure
      deepModel` error with **zero** spawns; `/graft-init` spawns
      `init --no-global --no-mcp --no-hooks --no-statusline --yes` and
      reports `value.yes: true`; no `--api-key`/`GRAFT_API_KEY` token on
      argv; `/graft-build` still spawns `build`; the registered `graft`
      skill body names both commands.
    - Evidence: `cd clay-agent && npm test` → 164 tests, 163 pass, 0 fail,
      1 skipped (pre-existing env-gated `CLAY_WEB_E2E` case);
      `npx tsc -p tsconfig.json --noEmit` clean. No Rust changes.
    - Compromise: the agent skill file is the source of truth (plan 117),
      so an already-seeded `~/.clay/agents/coding-agent/skills/graft/
      SKILL.md` keeps its old body — the new lines reach fresh agent
      configs, and deleting the file regenerates it. No version-stamp
      migration was added (user edits must never be clobbered).

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: No new Clay JS API. `session.prompt` `toolNames` is
      daemon JSON-RPC, not a `clay:` export. Packages still cannot speak
      to the daemon.
    - Performance: Verification adds no runtime work.
    - Code Quality: `js-api.md` naming unchanged.
    - Security: No new public mutation API.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`
      - `.agents/skills/create-plan/references/clay.md`
    - Options Considered:
      - `agent.setToolNames()` Clay JS API: rejected (no package daemon
        access; YAGNI until UI needs it).
      - Verification-only: selected.
    - Chosen Approach:
      - Diff review; expected no new APIs.
    - API Notes and Examples:
      ```ts
      // none
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `docs/index.md`
  - Test Cases to Write:
    - Existing doc-registry tests pass.
  - Completion (2026-09-16):
    - Verification-only, as planned: **no file was created or edited by this
      task**. Diff review of the live working tree (plans 120 + 121)
      confirms no new Clay JS API and no new public mutation surface.
    - Evidence:
      - `git diff --name-only` has **no** path under `packages/` (including
        `packages/javascript`/`packages/coding-agent`) and none under
        `src/clay_js*`/`src/server/js_runtime` — nothing was added to the
        `clay:` export set, so packages still cannot reach the daemon.
      - `.agents/skills/clay-execution/references/js-api.md` is unchanged:
        the naming contract stands.
      - `docs/reference/clay-js-api/agent/set-run-options.md` changed only by
        plan 120's version mention (`Prism 0.5.5` → `0.7.0`); no field,
        binding, or parameter was added, and `setRunOptions` remains the
        only run-policy facade.
      - `RunOptions.toolNames` exists only as daemon JSON-RPC in
        `clay-agent/src/host.ts`. The Rust side's only `toolNames` handling is
        pre-existing: the `skill.register` field passthrough
        (`src/server/agent/run.rs:914`) and the `agentProfile.register`
        array-of-strings validation list (`src/server/ops/agent.rs:110`) —
        neither is `session.prompt`, and the server omits the run field, so
        no Clay caller can widen a tool grant yet.
      - Doc-registry gates: `cargo test -p clay --test protocol
        clay_js_doc_registry` → 51 passed; `... clay_js_api_inventory` →
        14 passed (includes `clay_js_api_inventory_unchanged_or_documented`,
        `every_public_api_contract_matches_generated_markdown_metadata`,
        `plan118_configuration_documents_choice_set_fallback_and_every_option`);
        `... documentation_coverage` → 11 passed.
    - Performance/security: nothing shipped, so no runtime work and no new
      mutation API — the ACs are satisfied by absence, verified above.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: No new `init.js` option for attention ratios, wiki
      fetch, or graft `deepModel`.
    - Performance: No config reload change.
    - Code Quality: Attention defaults live in Prism, not Clay settings.
    - Security: No config grant for URL fetch; Obscura presence is still
      fail-closed binary resolution.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (Configuration Task)
    - Options Considered:
      - `run.setOptions({ attentionCompiler })`: extra knob, no need.
      - Verification-only: selected.
    - Chosen Approach:
      - Confirm example config unchanged.
    - API Notes and Examples:
      ```js
      // no new init.js surface
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `examples/config/init.js`
  - Test Cases to Write:
    - None.
  - Completion (2026-09-16):
    - Verification-only completed; no `init.js` option, Clay
      configuration facade, config parser, or config reload behavior was
      added or changed by this task.
    - `examples/config/init.js` passes `node --check`. Its only working-tree
      diff is two comment-only Prism-family metadata updates from plan 120
      (`Node >= 20` → `Node >= 22`, `Prism 0.5.5` → `Prism 0.7.0`): no
      executable configuration changed, and the file contains no
      `attentionCompiler`/ratio, `fetchUrl`, or `deepModel` option.
    - Exhaustive indexed searches confirm the new surfaces remain host
      internals, not configuration: `attentionCompiler` appears only in
      `clay-agent/src/host.ts` and its attention tests; `deepModel` only in
      the graft host comment/tests; `fetchUrl` only in the wiki host hook and
      its tests. `setRunOptions` remains the documented policy facade; no
      `agent.setToolNames()` or compiler-ratio facade was introduced.
    - Attention defaults stay in Prism: Clay passes only the coding-agent
      `attentionCompiler: true` switch and does not copy Prism's ratio table
      into `init.js`/Clay settings. Wiki URL fetch stays an internal
      `WikiExtensionOptions.fetchUrl` callback, created only when the
      host-owned Obscura binary resolves; it is not a config grant or raw
      HTTP client. Existing wiki tests retain fail-closed coverage for
      missing Obscura, blocked URLs, and missing hooks.
    - Configuration/API gates passed: `cargo test -p clay --test protocol
      configuration` → 23 passed; `... canonical_example` → 5 passed;
      `... package_loading_docs::language_package_docs_have_no_hidden_configuration_surface`
      → 1 passed. These cover the closed configuration surface, documented
      custom properties/defaults, canonical example parity, and absence of
      package-specific hidden configuration.
    - Security/performance: no new authority or reload work shipped;
      Obscura remains presence-gated and URL validation remains in the
      existing host/Prism path. Plan reference corrected to the actual
      canonical path `examples/config/init.js`.

- [x] Execute and update the manual test plan (`test-plan/`)
  - Acceptance Criteria:
    - Functional: Add numbered steps on a real Linux build for (1)
      `/wiki-ingest` path/text after `/wiki-init`, (2) `/wiki-ingest`
      url without Obscura fails closed, (3) graft skill/help shows
      `/graft-init` and `/graft-build-deep` when graft is bound. Attention
      is automated-only (no chrome). Do not weaken existing 16/17 steps.
    - Performance: Ingest is not on the typing path; no new latency
      budget.
    - Code Quality: Update `test-plan/index.md` coverage matrix.
    - Security: URL ingest negative check stays in the module.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`, `test-plan/16-agent-host.md`,
        `test-plan/17-coding-agent-parity.md`
      - `.agents/skills/create-plan/references/clay.md` (Manual Test Plan)
    - Options Considered:
      - Skip manual because tools are slash-level: rejected; 117 added
        `/wiki-init` steps for the same class.
      - Add C-steps to module 17 plus 16 host note: selected.
    - Chosen Approach:
      - New 17 steps C-series continuation; 16 notes the daemon RPC
        `toolNames` (not user-visible).
    - API Notes and Examples:
      ```text
      test-plan/17-coding-agent-parity.md — Cxx /wiki-ingest, graft commands
      ```
    - Files to Create/Edit:
      - `test-plan/17-coding-agent-parity.md`
      - `test-plan/16-agent-host.md`
      - `test-plan/index.md`
    - References:
      - plan 117 wiki/graft manual steps
  - Test Cases to Write:
    - Numbered manual steps as above; record pass/fail on a real build
      or the exact GUI blocker.
  - Completion (2026-09-16):
    - Added module 17 steps C45–C49 for `/wiki-init` → `/wiki-ingest`
      text/path staging, missing-Obscura URL rejection, graft skill/help,
      non-interactive `/graft-init`, and fail-closed `/graft-build-deep`.
      Added module 16 A20 for daemon-only `session.prompt.toolNames`.
    - Updated `test-plan/index.md` coverage matrix and added the Plan 121
      execution record. Existing module 16/17 steps were not weakened.
    - Real Linux build passed with `cargo build --bin clay` and
      `cargo build -p clay-desktop --bins`; isolated `CLAY_AGENT_MOCK=1`
      server/desktop launch passed under `/tmp/clay-manual-121`, with the
      coding profile registered and scratch roots only. `cd clay-agent &&
      npm test` passed: 163 pass, 0 fail, 1 pre-existing skip.
    - Interactive C45–C49 steps remain **UNRESOLVED** on this host, not
      falsely passed: AT-SPI/window discovery works, but
      `computer-use-linux doctor` reports no development input backend
      (`/dev/uinput` is root-only, no connectable `ydotoold` socket,
      incompatible `wtype`, and human-required portal input consent).
    - Evidence is recorded in `test-plan/16-agent-host.md`,
      `test-plan/17-coding-agent-parity.md`, and `test-plan/index.md`.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation
      tasks are complete, or explicitly verified as unchanged for non-code
      work.
    - Performance: Wiki updates add no runtime work and document
      performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it
      works, invariants/tradeoffs, source/test paths, examples where
      useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries,
      permissions, validation, secrets handling, or external authority
      without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass: selected.
    - Chosen Approach:
      - `docs/wiki/modules/clay-agent.md`: attention on coding, wiki
        ingest/SSRF, graft `initYes`, `session.prompt` `toolNames`.
    - Files to Create/Edit:
      - `docs/wiki/index.md`
      - `docs/wiki/modules/clay-agent.md`
      - `docs/development/tauri-react-parity-ledger.json`: keep new manual IDs
        A20 and C45–C49 covered by the documentation gate.
  - Test Cases to Write:
    - Manual wiki review: index links; pages match shipped behavior.
  - Completion (2026-09-16):
    - Updated `docs/wiki/modules/clay-agent.md` with Plan 121's
      context-window-gated attention compiler, per-run `toolNames` grant
      semantics, `wiki_ingest` staging/fetch flow, graft init/deep-build
      behavior, security invariants, source paths, and test coverage.
    - Updated the `clay-agent` entry in `docs/wiki/index.md` so the master
      index exposes the new implementation topics.
    - Added A20 and C45–C49 to
      `docs/development/tauri-react-parity-ledger.json`; the existing
      documentation gate requires every manual step to be ledger-backed.
    - Manual wiki review passed: `cargo test --test protocol
      documentation_coverage -- --nocapture` — 11 passed, 0 failed.
    - No new public Clay JS API or `init.js` option was added; the wiki records
      the daemon/extension boundary instead of creating duplicate reference
      documentation.

- [x] Wire auto-compaction on the attention-compiler pair (`compactAfterTokens` becomes live)
  - Acceptance Criteria:
    - Functional: Coding sessions whose resolved model declares a context
      window arm `AgentConfig.compaction` with a trigger; Chat, tool-free,
      and limit-less models arm nothing (no implicit branch rewrite). The
      gate fires when the assembled input reaches the attention compiler's
      `compactRatio` (0.9 of the resolved input cap), when
      `createAttentionTruncationTrigger` reaches two consecutive
      `truncated` reports, or when the absolute
      `run.setOptions.compactAfterTokens` ceiling is reached. Prism runs
      `autoCompact` once per prompt, before provider turns.
    - Performance: The automatic pass uses Prism's local deterministic
      strategy — no provider call in the unattended path; the trigger
      re-measures once per prompt (Prism memoizes the estimate and cap).
    - Code Quality: One composed `CompactionTrigger` with a short comment
      naming each gate; no duplicated ratio math (import
      `DEFAULT_ATTENTION_COMPACT_RATIO` /
      `DEFAULT_ATTENTION_TRUNCATION_THRESHOLD`); the trigger instance rides
      `LiveSession` and is rebuilt on model switch; `run.setOptions`
      applies at `createSession` like every other run option.
    - Security: Compaction runs with the host secret list so summaries are
      redacted; a throwing trigger decides `false` (Prism), so a missing
      cap can never compact on a guess. Explicit `session.compact` /
      `/compact` keep honoring `run.setOptions.compaction`.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/compaction-and-retry.md`
      - `/home/arn/Projects/prism/src/contracts-core/compaction.ts`
        (`CompactionOptions`, `CompactionTrigger`, `resolveShouldCompact`)
      - `/home/arn/Projects/prism/src/attention-compiler.ts`
        (`compactRatio`, `createAttentionTruncationTrigger`, throw-on-
        exhaustion, `truncated` semantics)
      - `/home/arn/Projects/clay/clay-agent/src/host.ts`
        (`createSession`, `attachOm`, `runSetOptions`)
    - Options Considered:
      - Trigger-less `AgentConfig.compaction`: dead — Prism's `autoCompact`
        returns early without `trigger` or `thresholdEntries`, and
        `CompactionOptions` carries no `compactAfterTokens` field.
      - Reimplement Prism's ratio gate as a plain `>= compactAfterTokens`
        custom trigger: honors the knob but ignores the compiler pair and
        fires far too late on normal windows.
      - Truncation trigger only: targeted, but a long uncapturable text
        branch would still fail closed with `AttentionBudgetError` at
        assembly instead of compacting first.
      - Composed trigger (ratio OR truncation OR ceiling): selected —
        covers Prism's two recommended mechanisms and makes the previously
        dead knob live.
      - Honor `run.setOptions.compaction` (default `llm`) as the automatic
        strategy: rejected — an unattended gate would turn a provider
        outage or empty summary into a failed run at assembly, and Prism's
        own CLI host leaves `strategy` unset for auto-compaction.
    - Chosen Approach:
      - `autoCompaction()` builds the trigger + the local default strategy
        with the host secrets; `createSession` installs it beside
        `attentionCompiler: true` and returns the truncation handle;
        `sessionPrompt` feeds every `attention_compiled` event to it.
    - API Notes and Examples:
      ```ts
      const truncation = createAttentionTruncationTrigger();
      const trigger: CompactionTrigger = {
        type: "custom",
        shouldCompact: (context) => {
          const armed = truncation.streak() >= DEFAULT_ATTENTION_TRUNCATION_THRESHOLD;
          const ratio = context.estimatedInputTokens >= context.inputCapTokens * DEFAULT_ATTENTION_COMPACT_RATIO;
          const ceiling = context.estimatedInputTokens >= compactAfterTokens;
          if (!armed && !ratio && !ceiling) return false;
          truncation.reset();
          return true;
        },
      };
      createAgent({ /* … */ compaction: { secrets, trigger } });
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: `autoCompaction`, `LiveSession`
        `attentionTruncation`, event-loop observe, `runSetOptions` comment
      - `clay-agent/src/__tests__/auto-compaction.test.ts`
      - `docs/reference/clay-js-api/agent/set-run-options.md` +
        `agent/compact.md` + generated registry
      - `clay-agent/README.md`, `packages/coding-agent/docs/index.md`,
        `runtime/js/agent.d.ts`
    - References:
      - Prism plan 074 P4 (truncation follow-up policy), C11 (`resolveShouldCompact`)
  - Test Cases to Write:
    - Armed exactly on coding + windowed; absent for chat and limit-less.
    - Compact-ratio gate compacts locally with zero provider requests.
    - `compactAfterTokens` ceiling fires under the ratio gate.
    - Real `attention_compiled` events reach `observe`; two truncated turns
      fire at the next prompt; a non-truncated report clears the streak.
    - Over-budget single prompt still fails closed with
      `AttentionBudgetError`.
  - Completion (2026-09-16):
    - `clay-agent/src/host.ts`: new `autoCompaction()` returns
      `{ options: { secrets, trigger }, truncation }`; `createSession`
      installs it only when `attentionCompiler` is on (same
      `hasCodingTools` + positive-safe `limits.contextWindow` predicate)
      and returns the truncation handle through the `LiveSession`
      `attentionTruncation` field (kept across `recreateSessionModel`).
      `sessionPrompt`'s stream loop calls
      `live.attentionTruncation?.observe(event)` for every
      `attention_compiled` event. The trigger ORs the truncation streak
      (Prism default threshold 2), the compiler compactRatio
      (`DEFAULT_ATTENTION_COMPACT_RATIO`), and the session's captured
      `runConfig.compactAfterTokens`; it calls `truncation.reset()` when it
      fires so one armed streak compacts once.
    - Strategy: omitted, so Prism uses `createDefaultCompactionStrategy`
      seeded from `options.secrets` — the same shape Prism's own CLI host
      uses. Rationale recorded in the code comment: the automatic gate
      must not turn a provider outage or an empty summary into a failed
      run at prompt assembly. `run.setOptions.compaction` still governs
      `session.compact`, `/compact`, and per-run `compaction`.
    - New `clay-agent/src/__tests__/auto-compaction.test.ts` (4 cases, all
      passing): arming predicate (coding+windowed armed with a custom
      trigger and no strategy; chat and `demo` limit-less unarmed); the
      compactRatio gate compacts at the prompt boundary
      (`compaction_started` → `compaction_finished`, a local summary, zero
      provider requests) while the run still fails closed with
      `AttentionBudgetError`; a `run.setOptions({ compactAfterTokens: 200 })`
      ceiling fires far below the ratio gate (proving the knob is live);
      and the event-loop hook (spy on the live trigger) receives real
      reports, a non-truncated report keeps the streak at 0, two truncated
      turns fire at the next prompt, and the trigger resets after firing.
    - Docs: `set-run-options.md` (frontmatter description, description,
      Options, key-binding list) now states the ceiling + companion gates;
      `agent/compact.md` cross-references the two distinct
      `compactAfterTokens` meanings; `runtime/js/agent.d.ts`,
      `clay-agent/README.md`, `packages/coding-agent/docs/index.md`, the
      wiki, and `docs/generated/clay-js-api-registry.json`
      (`cargo run --bin update-doc-registry`) follow.
    - Evidence: `cd clay-agent && npm test` → 170 tests, 169 pass, 1
      skipped, 0 fail; `cargo test --test protocol` → 216/216;
      `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
      clean.

- [x] Expose the graft `deepModel` as an explicit knowledge option
  - Acceptance Criteria:
    - Functional: `knowledge.setOptions { graft: true, graftDeepModel }`
      accepts `{ provider, model, apiKey?, baseUrl? }` with graft's own
      provider ids; an omitted `apiKey` resolves the stored credential for
      that provider, an inline key is accepted for keyless providers, and
      `/graft-build-deep` then spawns. A changed deep model rebinds the
      extension in place; an identical one is a no-op.
    - Performance: One vault lookup per `knowledge.setOptions` call; one
      extension rebind only when the deep model identity changes; no work
      on any prompt path.
    - Code Quality: Shape validation in the daemon next to the other
      knowledge options with the existing `-32602` discipline; a small
      Rust pre-queue shape check mirroring the validator style; the
      binding record carries an in-memory identity string only.
    - Security: The key never appears on argv, in logs, or in the option
      echo; inline keys join the redactor set; `graftDeepModel` without
      `graft: true`, a malformed shape, or an unresolvable key fails
      closed before any extension load.
  - Approach:
    - Documentation Reviewed:
      - `@arnilo/prism-memory/dist/graft/types.d.ts`
        (`GraftDeepProvider`, `GraftDeepModel`) and `extension.js`
        (`deepProviderEnv`: `GRAFT_PROVIDER`/`GRAFT_MODEL`/`GRAFT_API_KEY`,
        `GRAFT_BASE_URL`, `GRAFT_*`-only filter)
      - `@arnilo/prism-memory/dist/graft/commands.js` (deep build refuses
        before spawn without the env; the key never touches argv)
      - `clay-agent/src/host.ts` (`knowledgeSetOptions`, `enableGraft`,
        `credentialPut`, `defaultCredentialName`, `vault.get`)
      - `docs/reference/clay-js-api/agent/knowledge-set-options.md`
    - Options Considered:
      - Only an inline `apiKey`: simple, but forces the secret into
        `init.js` — rejected.
      - Only the vault: safest, but litellm/orcarouter have no stored
        credential path — accepted inline key as the fallback.
      - Reuse `providerEnv` on the extension: bypasses daemon validation
        and the vault, and the host has no `providerEnv` surface — rejected.
    - Chosen Approach:
      - Vault-first with an optional inline key; resolve in the daemon
        before activation so a bad shape never half-binds graft.
    - API Notes and Examples:
      ```ts
      await knowledgeSetOptions({
        workspaceRoot, graft: true,
        graftDeepModel: { provider: "anthropic", model: "claude-sonnet-4-5" },
      }); // key comes from the stored `anthropic` credential
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: `GRAFT_DEEP_PROVIDERS`,
        `deepModelIdentity`, `resolveGraftDeepModel`, `enableGraft`
        rebind, `this.graft` record
      - `src/server/ops/agent.rs`: pre-queue shape validation + unit test
      - `runtime/js/agent.d.ts`, `docs/reference/clay-js-api/agent/
        knowledge-set-options.md`, `api-inventory.toml` security notes
      - `clay-agent/src/__tests__/graft-knowledge.test.ts`
      - `clay-agent/README.md`, `examples/config/init.js` (commented),
        wiki page
    - References:
      - Plan 121 graft task (deep build currently dark),
        plan 108 task 13 (graft binding discipline)
  - Test Cases to Write:
    - Vault-resolved key reaches the child env only; deep build now spawns.
    - Identical deep model does not rebind; a changed one does.
    - Inline key path for litellm; `orcarouter` without a key fails closed.
    - `graft: false` + `graftDeepModel`, unknown provider, empty model,
      non-string key/base URL fail closed pre-queue.
  - Completion (2026-09-16):
    - `clay-agent/src/host.ts`: `knowledgeSetOptions` parses
      `graftDeepModel` (requires `graft: true`, `-32602` otherwise),
      resolves it through `resolveGraftDeepModel` before any activation,
      and echoes only `{ provider, model }`. `resolveGraftDeepModel`
      validates graft's provider union
      (`openai|anthropic|litellm|orcarouter`), non-empty `model`, an
      optional non-empty `apiKey`, an optional string `baseUrl`, and —
      when no inline key is passed — reads
      `vault.get({ name: defaultCredentialName(provider), provider })`,
      remembering an inline key with `rememberSecret` and failing closed
      when neither exists. `enableGraft` gains `deepModel`, passes it as
      the extension's `deepModel` (Prism merges it into the child env as
      `GRAFT_*`, never argv) and stores `deepModelIdentity` on the binding
      so an equal identity is a no-op and a changed one rebinds.
    - `src/server/ops/agent.rs` `validate_registration_shape`: shape-only
      pre-queue validation for `graftDeepModel` (object; non-empty
      `provider`/`model`; string `apiKey` non-empty; string `baseUrl`)
      with a unit test (`graft_deep_model_shape_fails_closed_before_queueing`)
      covering the rejections and the well-formed with/without-key shapes.
    - `graft-knowledge.test.ts` (8 cases, all passing): a recording CLI
      stub proves `/graft-build-deep` refuses before a model exists, then
      spawns after the vault-backed deep model is set — argv carries
      provider/model/base-url but never the key, and the child env line is
      `ENV anthropic|claude-deep|vault-key-121|https://api.example/v1`. It
      also proves the same deep model keeps the binding, a changed one
      rebinds (`claude-deep-2` in the next child env), the inline litellm
      key reaches the child, and missing key / `graft: false` / bad
      provider / empty model all fail closed with `-32602`.
    - Docs: `knowledge-set-options.md` (options, description, return,
      security), `api-inventory.toml` security notes,
      `runtime/js/agent.d.ts`, `clay-agent/README.md` (new knowledge-bases
      section), `examples/config/init.js` (commented example that keeps
      keys out of config), the wiki page, and the regenerated
      `docs/generated/clay-js-api-registry.json`.
    - Evidence: `cd clay-agent && npm test` → 170 tests, 169 pass, 1 skip;
      `cargo test --lib` includes the new shape test; `cargo fmt --check`
      clean; `node --check examples/config/init.js` clean.

## Compromises Made

- `toolNames` is a daemon RPC seam only; the Clay server/composer does not
  send it (full registry). Add UI later if mention/agent-type narrowing
  needs it without a session rebuild.
- Attention uses Prism defaults (`true`), no Clay ratio knob — and it is
  enabled only when the resolved model declares `limits.contextWindow`.
  Prism resolves the gate cap at run start and fails closed without one, so
  limit-less discovery models (Ollama) and pass-through model ids skip the
  optimization instead of failing every run.
- Graft `/graft-init` is non-interactive (`initYes: true`, `--no-global`,
  wiring off) and `/graft-build-deep` stays dark until a host `deepModel` is
  configured — now via the explicit `graftDeepModel` option (task above);
  no silent session-model reuse. The updated graft skill instructions only
  reach fresh agent configs — an existing seeded `skills/graft/SKILL.md` is
  user-owned and never rewritten.
- Auto-compaction stays on Prism's local default strategy: the automatic
  gate never spends a provider call, and `run.setOptions.compaction` keeps
  governing explicit `session.compact` / `/compact` and per-run
  `compaction`. A run whose own turns cross the cap mid-run still fails
  closed with `AttentionBudgetError` (auto-compaction evaluates once per
  prompt, not per turn). The truncation streak is per-agent state: a mid-run
  model switch rebuilds the agent and starts it empty (the ratio and
  ceiling gates are re-derived from the new window).
- `compactAfterTokens` is a ceiling, not a floor: on normal windows the
  compactRatio gate fires first by design, so the knob only decides on very
  large windows. It stays inert for Chat and limit-less models.
- No OCR / document-reader for compressed PDF/DOCX; those ingest paths
  fail closed.

## Further Actions

- Plan 122 spawn. Plan 123 work-scopes.
- Auto-compaction is prompt-boundary only; a single run that grows past the
  cap mid-run still fails closed. Wire a turn-boundary check (or a
  compaction-aware retry) if long autonomous runs need to survive it.
- `graftDeepModel` has no per-agent config-file surface: it is an
  `init.js` / trusted-config call. Add one if operator profiles need
  different deep models per agent type.
