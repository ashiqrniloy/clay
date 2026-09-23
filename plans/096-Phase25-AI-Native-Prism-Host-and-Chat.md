# Phase 25: AI-Native Entry, Prism 0.3.0 Host, and Chat

Source: `roadmap.md` Phase 25 (25.1–25.6). Coding-agent CLI parity is
Phase 29 and is **out of this plan**. This plan ships the host, generic
pane-content / composer / transcript primitives, provider/model/setup,
and first-party `@clay/chat` (no tools).

Product surfaces are packages. Core is host + primitives.

- `@clay/chat` owns the default landing/entry surface and the Chat
  profile. Users load it with one `loadPackage("@clay/chat")` line.
  A third-party package may `replaces` it (user approval) and ship a
  completely different landing page.
- `@clay/coding-agent` (Phase 29) is a separate first-party package on
  the same host/primitives. Third-party packages `extends` or `replaces`
  it through declared extension points + user approval.
- `clay-agent` (Prism Node child), credentials, and Clay IPC stay
  Clay-core. Packages never spawn or speak to the daemon. They use
  public `agent.*` APIs.

Confirmed architecture:

- `roadmap.md` Phase 25 preamble and 25.1–25.6
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
  (no ACP/AG-UI; Prism native host).
- `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`
  (landings/agents are first-party packages; core owns host/primitives).

Project patterns: `planning-checklist.md`, `agent-host.md`,
`authority-boundaries.md`, `protocol-and-performance.md`,
`extensions-and-ai.md`, `clay-js-api-naming.md`, `clay-js-api-boundary.md`,
`clay-js-api-schema.md`, `configuration-system.md`,
`documentation-as-code.md`, `doc-registry-tests.md`,
`package-ui-layout.md`, `package-runtime-trust-domains.md`,
`package-manifest-single-source.md`, `mode-primitive-first.md`,
`ui-modernization.md`, `ui-visual-review.md`, `typography-role-ownership.md`,
`maintenance-validation.md`.

Decision sources for required plan tasks:

- `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
- `decision-logs/2026-07-14-2023-language-server-package-authority.md`
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
- `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
- `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
- `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`
- `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`
- `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`

Not in scope: ACP, AG-UI, coding tools, MCP, diffs, AI-safe mutation, PTY,
auto-routing, Work/PA/Research profiles, Cursor SDK, `prism --mode rpc`,
`@clay/coding-agent` implementation.

Library docs: Context7 has no `@arnilo/prism` (it resolves PHP Prism).
Authoritative APIs are local Prism 0.3.0 docs under
`/home/arn/Projects/prism/docs/`.

## UI Skill Gate (mandatory for every UI task)

Surfaces: `@clay/chat` entry SDUI (greeting, chrome buttons, composer,
transcript), Command Centre session kinds for agent/provider/model/setup/
sessions, core empty-tab fallback.

Before reviewing existing UI, planning, designing, or editing any UI-related
task, use the current project UI skill requirements. Load the complete mandatory project-local UI skill stack,
apply them to Clay's native Masonry/Parley/Vello token context. Repeat per
independently executed UI task; prior evidence does not satisfy the next
task. After routing, load `.agents/skills/clay-ui/` plus
`references/components.md` and `references/tokens.md`. Also read
`docs/reference/ui-components.md`.

Plan-creation routing (2026-08-21):

```text
load the complete mandatory project-local UI skill stack
load the complete mandatory project-local UI skill stack
load the complete mandatory project-local UI skill stack
load the complete mandatory project-local UI skill stack
load the complete mandatory project-local UI skill stack
```

Plan-alignment routing (2026-08-22): same commands/skills. Ownership
change only: product chrome is package SDUI on catalog primitives, not a
hard-coded native Chat widget.

- Category: `interaction` + `accessibility`
- Skills: `prototyperai/build-primitive`, `jakubkrehel/better-accessibility`
- Native translation (no React/CSS/ARIA-on-div):
  - Core owns Masonry widgets, catalog, tokens, pane host, Command Centre.
  - `@clay/chat` declares an inert entry-surface tree. Clay renders it
    through existing `PackageRegionWidget` hosted in the pane main slot.
  - Greeting: heading. Composer: new generic multiline `textArea` kind
    (local buffer; placeholder is not the name). Transcript: `list` +
    `scroll`, `role=log` + polite status. Chrome buttons: named.
  - Focus composer on show. Visible focus via `paint_focus_ring` /
    `border.focus`. Hit targets meet catalog button sizes.
  - Pickers reuse Command Centre. No package `dropdown` for provider/model.
  - Keep Send enabled until the request starts. Empty submit is a no-op.
  - Secrets never in a11y names, menu snapshots, or logs.
  - Tokens only. Light and dark both required at visual review.

## Objectives

- Launch and `tabs.new` show the loaded entry-surface package. Default
  first-party `@clay/chat`: greeting (“What do you want to do today?”)
  and a focused composer. Chat needs no workspace.
- Spawn one `clay-agent` (Prism 0.3.0) per Clay server. Chat profile is
  registered by `@clay/chat`, not compiled into core.
- Provider/model/setup live in Command Centre. Entry-surface buttons
  call the same `agent.*` commands.
- No ACP/AG-UI crates. Event union includes tool/permission variants so
  Phase 29 does not rewrite IPC; Chat never emits them.
- Public core `agent.*` APIs plus package `chat.*` APIs, `init.js`
  `loadPackage("@clay/chat")`, test-plan, wiki.
- Third-party replacement of `@clay/chat` is a supported graph operation,
  not a follow-up invention.

## Expected Outcome

- With `@clay/chat` loaded, user configures a Prism 0.3.0 provider, picks
  a model, types in a new tab, sees a streamed reply, cancels, resumes
  after restart.
- Without any entry-surface package, core fallback is Open File / Open
  Folder only (no Chat chrome, no fake editor buffer).
- Open File still opens an editor. Open Folder binds workspace and leaves
  the entry surface up.
- `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, protocol tests, and
  `node --check examples/init.js` pass on Linux.

## Tasks

- [x] Review existing editor primitives and plan generic primitive gaps before package work
  - Acceptance Criteria:
    - Functional: Written inventory of pane host, welcome overlay, Command
      Centre, process spawn, protocol, reserved API domains, credential
      surfaces, package UI/SDUI, and package graph replace/extends.
      States what Phase 25 reuses vs adds. Locks a **generic pane-content
      contribution** (empty main slot / new tab) as the seam — not a
      product-named `PaneContent::Agent`. `@clay/chat` is a bundled
      first-party JS package; `clay-agent` is not.
    - Performance: Agent I/O and package JS stay off typing/paint/layout/scroll.
    - Code Quality: Cites primitive docs and wiki. New Rust is generic
      (pane content contribution, `textArea`, agent host), reusable by
      later terminal/coding-agent/third-party landings.
    - Security: Packages cannot spawn or speak to the daemon. Core-owned
      spawn, not a package-triggered process grant. Two package-runtime
      trust domains unchanged. Third-party `replaces` of `@clay/chat`
      stays in the third-party runtime.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`
      - `docs/reference/primitives/registry.md`
      - `docs/reference/primitives/shell-layout-strategy.md`
      - `docs/wiki/modules/primitive-architecture.md`
      - `docs/wiki/modules/masonry-shell.md`
      - `docs/wiki/modules/masonry-sdui-region.md`
      - `docs/wiki/modules/third-party-runtime-authority.md`
      - `roadmap.md` Phase 22 (generic pane host; pane-content contribution
        path not yet public), Phase 24 (Command Centre)
      - Prism 0.3.0: `docs/agent-session-runtime.md`, `docs/agent-events.md`,
        `docs/agent-definitions.md`
      - Patterns: `agent-host.md`, `authority-boundaries.md`,
        `package-runtime-trust-domains.md`, `package-ui-layout.md`
    - Options Considered:
      - Chat as irreplaceable Clay-native `PaneContent::Agent`. Smaller
        first diff; blocks user-replaceable landing and makes coding agent
        a core stub. Rejected (user philosophy + Plan 061 replace/extends).
      - Generic pane-content contribution + `@clay/chat` as first consumer.
        Chosen.
      - Package-triggered process grant for Node. Wrong owner. Core spawn.
        Chosen.
      - Put Prism inside `deno_core`. Node >= 20 required. Rejected.
    - Chosen Approach:
      - Review complete; no runtime code or protocol changed in this task.
      - 2026-08-22 correction: earlier draft treated Chat as core native
        UI. Inventory now matches package-replaceable product surfaces.
      - Reuse `PaneContentHost`, `PackageRegionWidget` / package SDUI,
        Welcome as **core fallback** when no entry contribution is loaded,
        Command Centre, `CommandExecutor`, rkyv IPC, language-server child
        I/O mechanics (not grants), `dependsOn`/`extends`/`disables`/`replaces`.
      - Confirmed generic gaps:
        - Public pane-content contribution for empty/new-tab `main` (today
          “not yet public”). Host with existing package-region widgets.
          Keep `PaneContent` closed; add a generic `Package` (or equivalent)
          variant, not `Agent`, not a trait-object `Custom`.
        - Catalog `textArea` (multiline, local buffer, named, submit
          intent) is required because Phase 25.5 needs a newline chord. Do
          not add `agentChat`.
        - Bounded agent-session projection keyed by server session/run IDs.
        - `src/protocol/agent.rs` boxed variants; tool/permission reserved.
        - One core-owned `clay-agent` child per server.
        - Command Centre form fields for provider setup (secrets never in
          snapshots).
        - Core `agent.*` APIs packages call: prompt/cancel/registerProfile/
          pickers/sessions. No daemon handle.
      - Bootstrap: hidden `InitialDocument` sentinel may remain for tab
        binding; package entry surface must not mount it as editable text.
      - Not a gap: Command Centre, parse, decorations, completion, LSP.
        Pickers stay host-owned.
    - API Notes and Examples:
      ```text
      PaneContent: Placeholder | Editor | Document | Package
      Entry contribution: empty/new-tab main slot, one winner
      Core fallback: Open File / Open Folder only
      Package: loadPackage("@clay/chat")
      Replace: clay.replaces ["@clay/chat"] + user approval
      Spawn: Clay server, one core-owned child, Node >= 20
      Chat profile: AgentDefinition, omitted tools/skills, registered by package
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/phase25-agent-host-primitive-review.md`
      - `docs/wiki/index.md`
    - References:
      - `src/masonry_pane_host.rs` `PaneContent` EXTENSION SEAM
      - `src/masonry_package_region.rs`
      - `src/packages/manifest.rs` `extends`/`replaces`/`RESERVED_CORE_API_DOMAINS`
      - `docs/wiki/modules/phase25-agent-host-primitive-review.md`
  - Completion Evidence:
    - UI preflight 2026-08-21 plus alignment re-route 2026-08-22
      (`interaction`/`accessibility`; `prototyperai/build-primitive`,
      `jakubkrehel/better-accessibility`).
    - Wiki review updated for package-owned entry/chat; daemon stays core.
    - `cargo test --test protocol primitives_docs::` after wiki correction.
  - Test Cases to Write:
    - none (research-only)

- [x] Review Clay UI catalog and plan primitive/component reuse before UI work
  - Acceptance Criteria:
    - Functional: Completed catalog reuse matrix. Existing `textInput` is
      single-line and cannot satisfy Phase 25.5's newline chord, so the plan
      promotes one generic package-facing `textArea` gap (not `agentChat`).
      Entry uses the pane `main` contribution; Welcome remains the core
      fallback; transcript uses bounded `scroll` + `list`; pickers use
      Command Centre. The later package-guide task must state that replacing
      `@clay/chat` replaces the default landing (or an empty-tab contribution
      wins when no default is loaded), subject to one-winner composition.
    - Performance: No paint-path JS. Transcript paints bounded snapshots.
      Composer keystrokes stay in the native `textArea` widget.
    - Code Quality: `textArea` remains generic, token-driven, state-complete,
      and reusable by later agents/packages. Catalog + `creating-packages.md`
      update in the implementation task that promotes the primitive; drift
      still fails `cargo test`.
    - Security: Secret fields never in menu snapshots or a11y names.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `docs/reference/ui-components.md`
      - legacy selection workflow task routing: the UI guidance current at execution time; architecture category;
        `mattpocock/codebase-design` (deep reusable interfaces, deletion test)
      - Prior plan routing: `prototyperai/build-primitive`,
        `jakubkrehel/better-accessibility`
      - `package-ui-layout.md`, `ui-modernization.md`,
        `ui-visual-review.md`, `typography-role-ownership.md`
    - Options Considered:
      - Native AgentView only, packages cannot replace. Rejected.
      - Package SDUI entry + catalog primitives + Command Centre pickers.
        Chosen.
      - `ComponentKind::Dropdown` on the landing chrome. Duplicates
        Command Centre. Rejected.
    - Chosen Approach:
      - Entry tree: package `main` contribution composed from `panel`, `flex`,
        `label`, `button`, `scroll`, and `list`; no `Agent`, `agentChat`, or
        custom landing widget. Open File/Open Folder stay existing commands.
      - Composer: add generic `textArea` before Chat UI. Reuse the retained
        native `TextArea<true>` substrate and existing package text-input
        plumbing, configure `InsertNewline::OnShiftEnter`, keep the buffer
        local, and submit Enter through an inert command intent. Reuse
        `placeholderColor`, `validationState`, focus ring, typography, and
        text/caret/selection tokens; add no Chat-specific style variable.
      - Transcript: reuse `scroll` + bounded `list` rows and `statusItem` for
        non-secret status. Do not add a transcript/message kind. If explicit
        `Role::Log` is required, implement it at the generic pane/transcript
        host seam because package declarations cannot provide arbitrary
        AccessKit roles.
      - Provider/model/agent/setup/session selection stays in the existing
        centered Command Centre; do not add a landing `dropdown` or second
        overlay system. `WelcomeWidget` remains the no-package fallback.
      - Styling reuses `surface.main`/`panel`/`control`/`list`/`selected`,
        `text.primary`/`muted`/`disabled`, `border.focus`, `spacing.xs`/`sm`/
        `md`, semantic typography variants, and `opacity.disabled`.
    - API Notes and Examples:
      ```text
      Commands: agent.clientOpenProviderPicker (etc.)
      Kinds: panel, flex, label, button, scroll, list, statusItem, textArea
      (new generic multiline kind)
      TextArea<true>::with_insert_newline(InsertNewline::OnShiftEnter)
      Enter -> TextAction::Entered; Shift+Enter -> newline
      Tokens: surface.main, surface.panel, surface.control, text.primary,
      text.muted, text.disabled, spacing.xs/sm/md, typography.display/section/
      body/detail/caption, border.focus, opacity.disabled.
      ```
    - Files to Create/Edit:
      - `plans/096-Phase25-AI-Native-Prism-Host-and-Chat.md`: record this
        reuse matrix and implementation handoff.
      - `docs/wiki/modules/phase25-agent-host-primitive-review.md`: record
        catalog reuse, the `textInput` ceiling, and host-owned a11y seam.
      - `.agents/skills/clay-ui/references/components.md`: update when the
        implementation task promotes `textArea`; do not add an implemented
        row before `src/shell/components.rs` and paint/reconcile paths exist.
      - `docs/reference/packages/creating-packages.md`: update in the package
        authoring task once pane-content/`textArea` APIs are implemented.
    - References:
      - `src/shell/components.rs` (`ComponentKind`, style variables, drift gate)
      - `src/masonry_package_region.rs` (`PackageTextInput`, retained widget)
      - `/home/arn/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/masonry-0.4.0/src/widgets/text_area.rs`
      - `src/shell/primitives.rs`, `src/shell/transient_menu.rs`
      - `src/masonry_welcome.rs` (core fallback)
      - `packages/settings/` as SDUI package precedent
      - `docs/wiki/modules/phase25-agent-host-primitive-review.md`
  - Completion Evidence:
    - UI preflight completed for this task: the UI guidance current at execution time,
      the UI guidance current at execution time, and
      the UI guidance current at execution time; `clay-ui`, component
      catalog, token catalog, UI navigation, and relevant project patterns
      were read.
    - Local Cargo metadata confirms `masonry v0.4.0`; `cargo tree -i
      masonry --depth 1` shows Clay's direct dependency. Local source
      confirms `TextArea<true>` newline/accessibility behavior.
    - `cargo test --test editor package_ui_conformance --quiet` passed
      (10 tests); `cargo test --test editor ui_primitive_conformance
      --quiet` passed (12 tests); `cargo test --test protocol
      primitives_docs --quiet` passed (28 tests).
    - No runtime UI code or implemented catalog row was changed in this
      review; `textArea` and pane-content docs are implementation handoff.
  - Test Cases to Write:
    - Existing catalog drift tests pass before implementation; add the
      `textArea` enum/catalog/paint parity case when the kind is promoted.
    - Add native composer checks for Enter submit, Shift+Enter newline,
      multiline a11y role, focus ring, local edits, and bounded layout.

- [x] Implement `clay-agent` daemon (Prism 0.3.0 Chat host)
  - Acceptance Criteria:
    - Functional: Standalone Node >= 20 process. Stdio JSON-RPC: session
      new/list/load/resume/delete, prompt, cancel, steer, provider
      list/status, model list/search, credential
      put/oauth-start/oauth-poll/delete, agent-profile list/register.
      Streams redacted `AgentEvent`s. SQLite sessions under Clay data
      dir. Encrypted credential vault; OS keychain when available; no
      silent plaintext fallback. Chat profile is **not** hard-coded as
      the only possible definition; daemon hosts whatever profiles Clay
      registers (Phase 25 registers Chat via `@clay/chat`).
    - Performance: First event after prompt within the later CI budget.
      `SubscribeOptions.maxQueuedEvents` bounded. No unbounded event buffer.
    - Code Quality: Exact `@arnilo/prism@0.3.0` pin plus first-party
      provider packages listed in roadmap 25.1. Not a Clay JS package
      under `packages/`. Not in bundled Deno inventory.
    - Security: Credential resolvers only. Prism never reads `process.env`
      for secrets. Logs scrub secrets. No ACP/AG-UI/coding-agent/MCP/
      browser/web-tools deps. Core-owned spawn target only.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/agent-session-runtime.md`
      - `/home/arn/Projects/prism/docs/agent-events.md`
      - `/home/arn/Projects/prism/docs/agent-definitions.md`
      - `/home/arn/Projects/prism/docs/provider-layer.md`
      - `/home/arn/Projects/prism/docs/credential-storage.md`
      - `/home/arn/Projects/prism/docs/extension-authoring.md`
      - `/home/arn/Projects/prism/docs/provider-packages.md`
    - Options Considered:
      - `prism --mode rpc` as product transport. Rejected (roadmap).
      - Clay-owned stdio JSON-RPC wrapping `AgentEvent`. Chosen.
      - Memory session store. Rejected.
      - `createSqlitePersistence` (0.3.0 export; there is no
        `createSqliteSessionStore`). Chosen.
    - Chosen Approach:
      - New `clay-agent/` Node package at repo root (not under `packages/`).
        NDJSON JSON-RPC 2.0 on stdio. `--data-dir` + `initialize.passphrase`.
      - Extension kernel loads HTTP `prism-provider-*` packages with a stored
        credential resolver. Azure/Bedrock/Vertex are pinned and expose auth
        stubs until endpoint/region/project exist (their factories require
        those fields at construct time).
      - Profiles via `agentProfile.register`. Chat is not compiled in.
      - Events as `{ method: "event" }` notifications; `maxQueuedEvents: 256`,
        `overflow: "drop_oldest"`.
      - Encrypted vault is source of truth; keychain dual-write when the OS
        secret service answers. No plaintext fallback.
      - `@arnilo/prism-model-router` is pinned but not wired (needs an
        enterprise state store). `@arnilo/prism-tool-validator-json-schema`
        is passed on `createAgent`.
    - API Notes and Examples:
      ```ts
      import { createAgent, createExtensionKernel, createMockProvider } from "@arnilo/prism";
      import { createSqlitePersistence } from "@arnilo/prism-session-store-sqlite";
      import {
        openEncryptedCredentialStore,
        createStoredCredentialResolver,
      } from "@arnilo/prism-credentials-node";
      // spawn: node dist/main.js --data-dir DIR [--mock]
      // first RPC: initialize { passphrase }
      ```
    - Files to Create/Edit:
      - `clay-agent/package.json`: exact 0.3.0 pins
      - `clay-agent/src/{main,host,providers,rpc,redact}.ts`
      - `clay-agent/src/__tests__/{host,rpc}.test.ts`
      - `clay-agent/README.md`: spawn contract for the Rust manager
      - `docs/wiki/modules/clay-agent.md` + `docs/wiki/index.md`
    - References:
      - `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
      - `roadmap.md` Phase 25.1
      - `/home/arn/Projects/prism/docs/{agent-session-runtime,agent-events,credential-storage,provider-packages}.md`
      - Context7 has no `@arnilo/prism` (PHP Prism only); local 0.3.0 docs used.
  - Completion Evidence:
    - `cd clay-agent && npm test` — 8 passed (prompt/persist/resume, cancel,
      missing tools, secret redaction, oversize frames, unreadable vault
      create + process exit 1).
    - Exact `0.3.0` pins; no ACP/AG-UI/coding-agent/MCP/browser/web-tools deps.
    - Azure/Bedrock/Vertex host-config stubs documented in README/wiki.
  - Test Cases to Write:
    - Node tests with `createMockProvider`: prompt, cancel, persist, resume
    - Reject oversize frames; redact secret-shaped values in errors
    - Missing tools/skills fail closed (empty registry)
    - Daemon exits non-zero on unreadable vault

- [x] Implement Clay server process manager and agent IPC
  - Acceptance Criteria:
    - Functional: One daemon per Clay server. Typed IPC for sessions,
      stream snapshots, provider/model inventory, credential intents (no
      secret echo), profile registration/selection. `agent` added to
      `RESERVED_CORE_API_DOMAINS`. Event enum includes tool/permission
      variants (Chat unused). `PROTOCOL_VERSION` bumped. Package JS
      reaches this only through documented `agent.*` facades.
    - Performance: Daemon I/O never on keypress-to-local-paint. Bounded
      frames. Compatibility tests for every new message.
    - Code Quality: `pub(crate)` internals. Public behavior via Clay JS
      facades in a later task. No ACP crate.
    - Security: Package ops cannot talk to the daemon pipe. Trusted and
      third-party runtimes get no daemon handle. Node missing → clear
      diagnostic, not a hang. Child env cleared except explicit inherited
      names. No shell wrapper.
  - Approach:
    - Documentation Reviewed:
      - `protocol-and-performance.md`
      - `src/protocol/mod.rs`
      - `tests/editor_intelligence_protocol.rs`
    - Options Considered:
      - Per-tab daemon. Waste. Chosen: one daemon, multiplex sessions.
      - Rust `agent-client-protocol` crate. Rejected.
    - Chosen Approach:
      - Manager next to connection machinery. rkyv length-prefixed.
      - Transcript is server-authoritative.
    - API Notes and Examples:
      ```text
      agent.serverPrompt / agent.serverCancel
      agent.serverRegisterProfile
      agent.clientOpenProviderPicker / ModelPicker / AgentPicker /
      ProviderSetup / SessionPicker
      PROTOCOL_VERSION 23 → 24 (later 25 for empty_tab)
      ```
    - Files to Create/Edit:
      - `src/protocol/agent.rs` (new)
      - `src/protocol/mod.rs`
      - `src/server/agent.rs` (tentative)
      - `src/server/mod.rs`
      - `src/packages/manifest.rs` (`agent` in `RESERVED_CORE_API_DOMAINS`)
      - `tests/agent_protocol.rs` (new)
    - References:
      - `src/server/language_server.rs` (mechanics only)
      - `src/protocol/codec.rs`
  - Test Cases to Write:
    - Codec round trip; oversized frame reject; invalid archive reject
    - Secret fields stripped from snapshots
    - Slow daemon does not block editor typing tests
    - Package runtime cannot import a daemon handle
    - Compatibility test per new message
  - Completion Evidence:
    - `src/protocol/agent.rs` + `src/server/agent.rs`; `PROTOCOL_VERSION` 24
      then 25. `agent` reserved. `cargo test --test protocol agent_protocol`
      10 ok. Missing Node is a diagnostic.

- [x] Open generic pane-content contribution and core empty-tab fallback
  - Acceptance Criteria:
    - Functional: Empty/new-tab `main` hosts at most one validated package
      pane-content contribution. No contribution → core fallback (Open
      File / Open Folder, no fake editor buffer). Open File → `Document`
      on `DocumentOpened`. Open Folder binds workspace, entry surface
      stays. `PaneContent` gains a generic package-hosted variant, not
      `Agent`.
    - Performance: Contribution install at generation commit, not per
      keystroke. Composer (when present) is local.
    - Code Quality: Generic/reusable. Later terminal stays a distinct
      kind (PTY ≠ SDUI). No Chat-named Rust types in the pane host.
    - Security: Inert SDUI only. No client JS. Third-party contribution
      stays in third-party runtime. Replacement withdraws the target
      contribution atomically.
  - Approach:
    - Documentation Reviewed:
      - Repeat UI skill gate
      - `docs/reference/primitives/shell-layout-strategy.md` (pane-content
        contribution path not yet public)
      - `src/masonry_pane_host.rs`, `src/masonry_package_region.rs`,
        `src/masonry_welcome.rs`
    - Options Considered:
      - `PaneContent::Agent` product kind. Rejected.
      - Host `PackageRegionWidget` in main for empty tabs. Chosen.
    - Chosen Approach:
      - Promote the Phase 22 seam to a public contribution.
      - Keep WelcomeWidget as the zero-package fallback.
    - API Notes and Examples:
      ```ts
      import { serverRegisterPaneContentContribution } from "clay:ui";
      serverRegisterPaneContentContribution({
        id: "chat.entry",
        activation: "empty-tab",
        component: { kind: "panel", /* greeting, buttons, textArea, list */ },
      });
      ```
    - Files to Create/Edit:
      - `src/masonry_pane_host.rs`
      - `src/masonry_welcome.rs` (fallback only)
      - `src/server/ui.rs` / `runtime/js/ui.js` (tentative)
      - `docs/reference/primitives/shell-layout-strategy.md`
    - References:
      - Phase 22 pane host EXTENSION SEAM
      - Plan 087 welcome
  - Test Cases to Write:
    - No package: fallback Open File / Open Folder
    - One contribution: hosted in new tab; not an editable document
    - Two competing contributions: deterministic conflict, not load-order win
    - Folder bind keeps package content type
    - Open File switches to Document
  - Completion Evidence:
    - UI preflight 2026-08-22: `architecture` + `accessibility`
      (`mattpocock/codebase-design`, `jakubkrehel/better-accessibility`).
    - `ui.serverRegisterPaneContentContribution`; one empty-tab winner;
      two IDs → sorted conflict, no load-order win; replacement is reload
      omit. `PaneContent::{Package,Welcome}`. Welcome remains fallback.
    - `PackageUiSnapshot.empty_tab` (validated JSON, protocol v25).
    - `cargo test --lib pane_content_one_winner empty_tab_hosts
      host_content_transitions` ok. Welcome + editor tests still pass.
    - Full Clay JS API docs deferred to the later API task.

- [x] Ship first-party `@clay/chat`
  - Acceptance Criteria:
    - Functional: Bundled package. `loadPackage("@clay/chat")` registers
      Chat profile (no tools) and the default entry surface: greeting,
      agent/provider/model buttons, Open File, Open Folder, focused
      composer. Chat works with no workspace. Declares extension points
      so others can append/replace chrome (and whole-package `replaces`
      remains available). Coding Agent is **not** a core disabled stub;
      it appears when `@clay/coding-agent` loads in Phase 29.
    - Performance: Package JS only at load/command/SDUI update. Composer
      local until send.
    - Code Quality: Follows `@clay/settings` (inert SDUI, `apiPrefix`
      `chat`, `BUNDLED_PACKAGES` inventory). No Masonry in the package.
    - Security: Trusted bundled runtime. Load grants no filesystem,
      network, shell, daemon, or AI-mutation. Unconfigured provider is
      instructional empty state.
  - Approach:
    - Documentation Reviewed:
      - Repeat UI skill gate
      - `packages/settings/` as UI-package precedent
      - `docs/reference/packages/creating-packages.md` extension points
    - Options Considered:
      - Chat inside core `init.js` with no package. Rejected.
      - `@clay/chat` bundled + explicit load. Chosen.
    - Chosen Approach:
      - `packages/chat/` with `loadEntry` registering profile + pane
        content + commands that call `agent.*`.
      - Greeting copy lives in the package so a replacement can change it.
    - API Notes and Examples:
      ```js
      await loadPackage("@clay/chat");
      // chat.entrySurface, chat.chromeActions extension points
      ```
    - Files to Create/Edit:
      - `packages/chat/**` (package.json, loadEntry, docs, dist)
      - bundled inventory
      - `src/packages/bundled.rs` / `bundled-inventory.toml` as required
    - References:
      - Plan 061 replace/extends
      - `packages/settings/package.json`
  - Test Cases to Write:
    - Load registers Chat profile and entry contribution
    - Unload/disable restores core fallback
    - Third-party `replaces` (fixture) withdraws `@clay/chat` and does
      not enter the trusted runtime
    - Empty submit no-op; composer focused on show
    - Open File still uses existing dialog command
  - Completion Evidence:
    - UI preflight 2026-08-22: `accessibility` (`jakubkrehel/better-accessibility`)
      + `clay-ui` catalog (`panel`/`label`/`button`/`textInput` only).
    - `packages/chat/` bundled. `loadPackage("@clay/chat")` registers Chat
      profile command (no tools) + `chat.entry` empty-tab tree. Extension
      points `chat.entrySurface` / `chat.chromeActions`.
    - Open File / Open Folder use existing client dialog command IDs.
      Empty `chat.submit` is a no-op. Composer is the first textInput and
      takes focus on empty-tab sync.
    - `cargo test --lib chat_package chat_empty first_text_input` ok.
      `cargo test --test security third_party_replacement_withdraws_chat` ok.
    - Agent picker UI and `agent.*` JS facades stay the next tasks.

- [x] Command Centre agent, provider, model, setup, and session pickers
  - Acceptance Criteria:
    - Functional: Session kinds on the existing centered Command Centre,
      shared fuzzy matcher. Provider list + “Configure provider…”.
      Setup data-driven (`api_key` / `oauth` / OpenAI-compatible URL).
      Model list from configured providers only. Agent list = registered
      profiles (Chat when `@clay/chat` is loaded). Session
      list/resume/delete. Same commands from entry-surface buttons and
      Command Centre.
    - Performance: Menu open/filter within existing budgets. No catalog
      fetch per keystroke.
    - Code Quality: No second overlay system. No native dropdown widget.
    - Security: API key never in snapshots, logs, or a11y. Masked input.
      OAuth shows user code; poll; store.
  - Approach:
    - Documentation Reviewed:
      - Repeat UI skill gate
      - `src/shell/transient_menu.rs`, `src/server/control_center.rs`
      - Prism `registerAuthMethod` descriptors
    - Options Considered:
      - `ComponentKind::Dropdown` on the landing. Rejected.
      - New `TransientMenuSession` kinds. Chosen.
    - Chosen Approach:
      - Additive session kinds. Profiles come from registration, not a
        compiled enum of future agents.
    - API Notes and Examples:
      ```text
      agent.clientOpenProviderPicker
      agent.clientOpenModelPicker
      agent.clientOpenAgentPicker
      agent.clientOpenProviderSetup
      agent.clientOpenSessionPicker
      ```
    - Files to Create/Edit:
      - `src/protocol/menu.rs`
      - `src/server/menu_sessions.rs` / `src/server/control_center.rs`
      - `src/shell/transient_menu.rs`
    - References:
      - Plan 081–084 Command Centre
      - `roadmap.md` Phase 25.4
  - Test Cases to Write:
    - Unconfigured provider cannot be selected as model source
    - Secret not present in menu snapshot bytes
    - Agent picker omits Coding Agent until that package registers
    - Keyboard-only: Command Centre reaches all flows without landing
      buttons
  - Completion Evidence:
    - UI preflight 2026-08-22: `interaction`/`accessibility`,
      `jakubkrehel/better-accessibility` + `clay-ui` catalog. No new overlay.
    - Third `ServerMenuSessionKind::AgentPicker` on centered Command Centre.
      Shared fuzzy matcher. Builtins `agent.clientOpen{Agent,Provider,Model,
      ProviderSetup,Session}Picker` listed in Control Centre (keyboard-only).
      Landing buttons call the same IDs.
    - Models only from configured providers. Agent list = package `*.profile`
      + daemon profiles (no Coding Agent stub). Setup is data-driven
      (`api_key` / `oauth` / URL). Secret query projected as bullets; client
      keeps local send buffer. OAuth shows user code; poll stores.
    - `cargo test --lib agent_picker control_center_includes_built_in` ok.
      `cargo test --test protocol agent_protocol` ok. `cd clay-agent && npm test`
      8 ok. Clippy `-D warnings` ok.
    - Transcript/send still the next task.

- [x] Chat transcript and session resume
  - Acceptance Criteria:
    - Functional: Server transcript of user/assistant/thinking/error/usage.
      `@clay/chat` paints via catalog list/scroll bound to that snapshot.
      Stream deltas. Multi-turn. Cancel. Session list/resume/delete.
      New tab = new session. Restored session reopens entry surface with
      bounded redacted history. Unconfigured-provider empty state is
      instructional. No tools/approvals/diffs/MCP/slash-commands.
    - Performance: Per-delta IPC bounded. Snapshot size capped. Deltas
      never block keypress-to-local-paint.
    - Code Quality: Client does not call providers. Package does not own
      canonical transcript.
    - Security: Redacted history only. Injected tool-event variants do
      not crash Chat UI (ignore / status).
  - Approach:
    - Documentation Reviewed:
      - Repeat UI skill gate
      - Prism `docs/agent-events.md`
    - Options Considered:
      - Client-side provider SDK. Rejected.
      - Server snapshots, package SDUI projection. Chosen.
      - Concurrent send while running = second `run()`. Rejected
        (Prism fail-closed). Disable send; Escape cancels. Steer RPC
        exists; Chat UI does not expose it.
    - Chosen Approach:
      - Map `message_delta` / thinking / usage / error / finished.
        Ignore tool variants in the Chat tree.
    - API Notes and Examples:
      ```ts
      for await (const event of session.stream(input, {
        maxQueuedEvents: 256,
        overflow: "close",
      })) {
        switch (event.type) {
          case "message_delta":
          case "agent_finished":
          case "error":
            break;
          default:
            break;
        }
      }
      ```
    - Files to Create/Edit:
      - protocol snapshots, `@clay/chat` tree bindings
    - References:
      - Phase 25.5 roadmap
  - Test Cases to Write:
    - Resume after daemon restart restores bounded history
    - Cancel stops further deltas
    - Tool events if injected do not crash Chat UI
    - Empty submit no-op; unconfigured provider empty state
    - Enter sends; documented newline chord does not send
  - Completion Evidence:
    - UI preflight 2026-08-22: `interaction`/`accessibility`,
      `jakubkrehel/better-accessibility` + `clay-ui` catalog. No new kind.
    - Server owns transcript (`AgentTranscriptKind` + bounded apply). Deltas
      via existing `ServerMessage::Agent`. Client paints `chat.transcript`
      list/scroll. Tool/permission events ignored.
    - `chat.submit` creates/reuses per-tab session; new tab = new session.
      Session picker resume loads bounded history. Escape/`chat.cancel`
      marks cancelled so later deltas drop.
    - Unconfigured prompt returns empty snapshot; hint stays instructional.
      Enter on `textInput` sends. Multiline/`textArea` deferred.
    - `cargo test --lib apply_transcript agent_tool_events chat_package` ok.
      `cargo test --test protocol agent_protocol` 12 ok. Clippy `-D warnings` ok.

- [x] Define and verify the package default init.js loading experience
  - Acceptance Criteria:
    - Functional: Default path is one line
      `loadPackage("@clay/chat")` in `examples/packages/first-party.js`.
      No silent compiled Chat. Without the load, core fallback only.
      Common Chat works after that one line (no copied manifests, no raw
      ops, no manual primitive registration).
    - Performance: Load at generation, not per keystroke.
    - Code Quality: Same convention as `@clay/markdown` / `@clay/settings`.
    - Security: Load grants no daemon, filesystem, network, shell, or
      AI-mutation.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
      - `examples/packages/first-party.js`
    - Options Considered:
      - Always-on Chat with no init.js line (like `core.text`). Rejected
        for a replaceable product surface.
      - Explicit one-line load as the product default. Chosen.
    - Chosen Approach:
      - Uncommented `loadPackage("@clay/chat")` in the first-party example
        module, same as markdown/settings.
    - API Notes and Examples:
      ```js
      await loadPackage("@clay/chat");
      ```
    - Files to Create/Edit:
      - `examples/packages/first-party.js`
      - package docs under `packages/chat/docs/`
    - References:
      - `configuration-system.md`
  - Test Cases to Write:
    - One-line load enables entry surface + Chat profile
    - Missing load → fallback, no Chat profile
  - Completion Evidence:
    - `examples/packages/first-party.js` has uncommented
      `await loadPackage("@clay/chat");`. No auto-load. Absent line → core
      fallback, no `chat.profile`.
    - `packages/chat/dist/load.js` is execute-only: no `Deno.core`, no
      `clay:agent`, no `serverRegisterCommand`. Host applies package.json.
    - `packages/chat/docs/index.md` documents the one-line default.
    - `node --check examples/packages/first-party.js` ok.
    - `cargo test --lib first_party_example_loads_chat chat_load_entry
      chat_package_registers chat_package_absent
      example_configuration_loads` 5 ok. Clippy `-D warnings` ok.

- [x] Update the package UI/layout authoring contract and package guide
  - Acceptance Criteria:
    - Functional: `creating-packages.md` documents pane-content
      contribution, `@clay/chat` as the default landing, replace/extends
      rules, and that packages still cannot create Masonry widgets or
      replace Command Centre / tab bar / file dialogs.
    - Performance: Hot-path rule restated (no package JS on keypress/paint).
    - Code Quality: Matches implemented APIs, not prose.
    - Security: Trust-domain replace/extends: first-party extension point
      + user approval; replacement stays third-party; core/bootstrap and
      `clay-agent` are not package-replaceable.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md`
      - Plan 061 extension/replace sections
    - Options Considered:
      - Leave welcome “packages cannot replace”. Rejected.
      - Document entry surface as package-owned, Command Centre still
        host-owned. Chosen.
    - Chosen Approach:
      - Reverse Plan 087 “packages cannot replace welcome” for the
        **product landing**. Keep dialog/tab/Command Centre host-owned.
    - API Notes and Examples:
      ```text
      clay.replaces: ["@clay/chat"]
      clay.extensionPoints: chat.entrySurface, chat.chromeActions
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`
      - `.agents/skills/clay-ui/references/components.md` native-surface
        row (welcome = fallback; entry = package)
    - References:
      - `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
  - Test Cases to Write:
    - Docs drift tests
    - Cross-domain deny: third-party cannot import trusted chat modules
    - Replacement rollback restores `@clay/chat`
  - Completion Evidence:
    - `creating-packages.md` Phase 25 section: pane-content, `@clay/chat`
      default, `replaces`/`chat.entrySurface`/`chat.chromeActions`, host-owned
      Command Centre/tab bar/dialogs/`WelcomeWidget`/`clay-agent`.
    - Catalog: welcome = fallback; loaded landing = package pane-content.
    - `cargo test --test protocol -- phase25_package plan087_ui plan088_ui`
      ok. `third_party_cannot_import_trusted_chat` ok.
      `third_party_replacement_withdraws_chat` rollback ok.

- [x] Harden budgets, security review, and protocol compatibility
  - Acceptance Criteria:
    - Functional: Budgets recorded for daemon spawn, prompt-to-first-delta,
      per-delta IPC, transcript snapshot size, menu open/filter.
    - Performance: Deltas never block keypress-to-local-paint.
    - Code Quality: Exact 0.3.0 pin + upgrade checklist in daemon README.
    - Security: Child-process privileges, vault `0o600`, OAuth honesty,
      no secret leakage, package-code denial of daemon access, truthful
      “no sandbox / no tools” for Chat. No ACP/AG-UI in `Cargo.toml` or
      `clay-agent/package.json`.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 25.6
      - `src/perf/budgets.rs`
    - Options Considered:
      - Defer all budgets. Rejected.
      - Advisory numbers now, CI where a test can own them. Chosen.
    - Chosen Approach:
      - Compatibility tests per message. Dependency grep deny-list.
    - API Notes and Examples:
      ```text
      cargo test --test agent_protocol
      rg -n "prism-acp|prism-ag-ui|agentclientprotocol" clay-agent Cargo.toml
      ```
    - Files to Create/Edit:
      - `src/perf/budgets.rs` (if a budget is CI-owned)
      - `clay-agent/README.md`
    - References:
      - Phase 25.6
  - Test Cases to Write:
    - Protocol compatibility for every new IPC message
    - Typing test with a slow daemon still local
    - Dependency deny-list for ACP/AG-UI/coding-agent/MCP
  - Completion Evidence:
    - Advisory: `AGENT_DAEMON_SPAWN_P95_BUDGET_MS` 2000,
      `AGENT_PROMPT_TO_FIRST_DELTA_P95_BUDGET_MS` 2000,
      `AGENT_DELTA_IPC_P95_BUDGET_MS` 4. Pickers reuse Command Centre budgets.
    - Hard: delta 8 KiB, entry 32 KiB, snapshot 256 KiB / 200 entries.
    - `clay-agent/README.md` 0.3.0 pin + upgrade checklist. Chat docs:
      no tools, no sandbox.
    - OAuth: device-code vs redirect labels; poll still drops tokens.
    - `cargo test --test protocol -- phase25_agent agent_io_stays
      phase25_dependencies mock_spawn slow_daemon every_agent` ok.
    - `cargo clippy --all-targets -- -D warnings` ok.

- [x] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: Linux GUI: core fallback, `@clay/chat` empty landing,
      focused composer, unconfigured-provider, live stream, error,
      cancelled, provider/model/agent/setup/session menus, narrow/wide,
      light/dark.
    - Performance: Composer typing stays local during a stream.
    - Code Quality: Screenshots stored; findings recorded. No raw colors.
    - Security: Screen reader names contain no secrets. Keyboard-only
      configure → pick model → send → cancel.
  - Approach:
    - Documentation Reviewed:
      - Repeat UI skill gate
      - `ui-visual-review.md`, clay-ui Step 1, better-accessibility
    - Options Considered:
      - Source-only review. Forbidden.
    - Chosen Approach:
      - Launch GUI. If computer-use is available, `get_app_state` first.
      - If GUI blocked, record blocker; leave visual/a11y unresolved.
    - API Notes and Examples:
      ```text
      evidence: code-reviews/screenshots/2026-08-22-plan096-ui-review/
      ```
    - Files to Create/Edit:
      - review artifacts under `code-reviews/screenshots/2026-08-22-plan096-ui-review/`
      - `src/masonry_editor.rs`: attach visible package entry in the editor
        accessibility parent.
      - `src/masonry_pane_document.rs`: unstash package entry before initial
        composer focus request.
      - `src/masonry_package_region.rs`: expose the named package composer
        host as a text-input role.
    - References:
      - `docs/wiki/modules/ui-review-harness.md`
  - Completion Evidence:
    - UI preflight 2026-08-22: `accessibility` / `jakubkrehel/better-accessibility`;
      `clay-ui`, component catalog, and token catalog loaded.
    - `scripts/capture-ui-review.sh`: core fallback, runtime error, recovery,
      large typography, and fresh Chat dark/light/large-typography captures
      inspected under `code-reviews/screenshots/2026-08-22-plan096-ui-review/`.
    - AT-SPI via `computer-use-linux_get_app_state` verified reachable package
      region, named action buttons, focused composer, and no secret-bearing
      names. Evidence: `chat-final/accessibility.txt` and
      `chat-final/accessibility-mcp.txt`.
    - Found and fixed two host defects during review: package-entry omission
      from `EditorWidget` accessibility children (AccessKit orphan panic), and
      initial composer focus requested while the package entry was stashed.
    - `cargo test --offline --lib -- masonry_editor::tests` — 25 passed.
      `cargo test --offline --lib -- masonry_package_region::tests` — 32 passed.
      `cargo fmt --check`, `cargo build --offline`, and
      `cargo test --offline --test protocol -- primitives_docs` (29 passed)
      passed. Plan 061 inventory was updated for the new pane-content op and
      `@clay/chat` package.
    - Provider/model/agent/setup/session menus, live stream/error/cancelled
      states, keyboard-only flow, and live narrow/wide resize are explicitly
      `UNRESOLVED`: host has AT-SPI but no keyboard-capable input backend.
    - Finding: current Chat composer remains generic single-line `textInput`
      with an unnamed inner Entry; multiline `textArea` parity remains a
      follow-up, not silently marked complete.
  - Test Cases to Write:
    - Keyboard flow recorded — `UNRESOLVED` on current host; rerun when a
      keyboard-capable Wayland/X11 backend is available.
    - a11y tree names/roles for chrome + composer + transcript — Chat chrome
      and composer verified; transcript/stream states await interactive run.
    - Contrast still passes theme validation in light and dark — light/dark
      screenshots inspected; host theme validation remains structural.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Documented core `agent.*` (prompt/cancel/registerProfile/
      pickers/sessions) and package `chat.*`. `agent` in
      `RESERVED_CORE_API_DOMAINS`. Registry + `docs/index.md` links.
      `cargo test` fails on drift.
    - Performance: Facades introduce no hot-path JS.
    - Code Quality: Naming per `clay-js-api-naming.md`. Internals
      `pub(crate)`. No raw `Deno.core.ops` as the user-facing API.
    - Security: No credential-read API. No daemon handle for packages.
      Configuration does not grant AI mutation.
  - Approach:
    - Documentation Reviewed:
      - `clay-js-api-naming.md`, `clay-js-api-boundary.md`,
        `documentation-as-code.md`
    - Options Considered:
      - Only core APIs, Chat not a package. Rejected.
      - Core host APIs + package-prefixed chat APIs. Chosen.
    - Chosen Approach:
      - Facades for host ops. Package commands use `chat.` prefix.
    - API Notes and Examples:
      ```ts
      import { serverPrompt, clientOpenModelPicker } from "clay:agent";
      await serverPrompt({ sessionId, text });
      clientOpenModelPicker();
      ```
      ```text
      JS module: clay:agent
      JS export: serverPrompt
      Stable ID: agent.serverPrompt
      Package IDs: chat.* (apiPrefix chat)
      ```
    - Files to Create/Edit:
      - `runtime/js/agent.js`, `runtime/js/agent.d.ts`
      - `src/server/facades.rs`
      - `docs/reference/clay-js-api/agent/*.md`
      - `docs/index.md`
    - References:
      - `src/packages/manifest.rs`
      - `runtime/js/editor.js`
  - Test Cases to Write:
    - Existing doc-registry tests cover new pages
    - Missing Markdown / index link / registry entry fails `cargo test`

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: `init.js` options for last-used provider/model.
      Default profile is whichever package registered (Chat after
      `loadPackage("@clay/chat")`). Each option is a documented Clay JS
      API. Configuration does not grant filesystem, network, or
      AI-mutation.
    - Performance: Config read at generation, not per keystroke.
    - Code Quality: Matches server parsers, not prose.
    - Security: Vault + setup UI is the secret path. Prefer no
      key-in-config.
  - Approach:
    - Documentation Reviewed:
      - `configuration-system.md`, `examples/init.js`
    - Options Considered:
      - Silent compiled default model. Rejected.
      - Documented defaults + explicit package load. Chosen.
    - Chosen Approach:
      - Small option set. Vault remains the secret store.
    - API Notes and Examples:
      ```js
      // import { setLastUsedModel } from "clay:agent";
      // setLastUsedModel({ provider: "anthropic", model: "…" });
      ```
    - Files to Create/Edit:
      - configuration facade + docs under `docs/reference/clay-js-api/agent/`
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Undocumented option fails registry tests
    - Invalid profile value is a diagnostic, not a panic

- [ ] Update the canonical example configuration (examples/init.js)
  - Acceptance Criteria:
    - Functional: `loadPackage("@clay/chat")` in first-party module.
      Agent options documented once. `node --check examples/init.js`
      passes. No sample secrets. No claim that Chat is core-without-load.
    - Performance: n/a
    - Code Quality: Matches API docs and `api-inventory.toml`
    - Security: No sample secrets.
  - Approach:
    - Documentation Reviewed:
      - `examples/init.js`, `examples/packages/first-party.js`
    - Options Considered:
      - Required loadPackage. Chosen (this is a package).
    - Chosen Approach:
      - First-party module line + commented agent options.
    - API Notes and Examples:
      ```js
      await loadPackage("@clay/chat");
      // Third-party landing: load replacement instead; do not also load
      // @clay/chat (replaces is a graph relation + approval).
      ```
    - Files to Create/Edit:
      - `examples/packages/first-party.js`
      - `examples/init.js` if a new agent section is needed
    - References:
      - user instruction 2026-08-03
  - Test Cases to Write:
    - `node --check examples/init.js`

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Steps for core fallback, `@clay/chat` landing, provider
      setup, chat stream/cancel/resume, pickers, and (documented, even if
      fixture-only) replace-package landing. Linux run recorded.
    - Performance: Note spawn/first-delta if observed.
    - Code Quality: Numbered steps with expected results, negatives,
      ceilings.
    - Security: Steps assert secrets not visible in UI/logs.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`
    - Options Considered:
      - Automated-only. Rejected.
    - Chosen Approach:
      - New `test-plan` module for agent/chat.
    - API Notes and Examples:
      ```text
      test-plan/15-agent-chat.md
      ```
    - Files to Create/Edit:
      - `test-plan/15-agent-chat.md`
      - `test-plan/index.md`
    - References:
      - user instruction 2026-08-04
  - Test Cases to Write:
    - numbered manual steps with negatives (no package, empty submit,
      no provider, secret not echoed)

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki pages for `clay-agent`, server agent manager,
      agent IPC, pane-content contribution, `@clay/chat`. Index links.
      Implementation-level, not a copy of JS API docs.
    - Performance: Documents budgets and hot-path rules.
    - Code Quality: What/how/invariants/source/tests.
    - Security: Trust boundary, replace/extends, credential flow without
      secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass. Chosen.
    - Chosen Approach:
      - One pass after implementation and verification.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/agent-host.md
      docs/wiki/modules/clay-chat-package.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`
      - `docs/wiki/modules/agent-host.md`
      - `docs/wiki/modules/masonry-shell.md`
      - `docs/wiki/modules/protocol-codec.md`
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual: index links the new pages
    - `tests/primitives_docs.rs` still passes if primitive docs changed

## Compromises Made

- Chat has no tools (roadmap Phase 25). CLI-parity coding agent is
  `@clay/coding-agent` in Phase 29, not a core profile stub.
- No ACP/AG-UI. Revisit only for third-party ACP agents or a web UI.
- Steer RPC exists; Chat UI does not expose steer.
- Interactive PTY out until the terminal package.
- Auto-routing deferred; user picks the profile.
- `clay-agent` stays core (credentials/process). Only the product UI and
  AgentDefinition registration are packages.
- Context7 has no `@arnilo/prism`; plan uses local Prism 0.3.0 docs.
- Core empty-tab fallback is intentionally boring (file/folder only).
- Visual review completed with static and semantic AT-SPI evidence; keyboard
  and interactive agent states remain unresolved because this host lacks a
  keyboard-capable input backend.
- Chat composer currently uses existing generic single-line `textInput`; the
  planned multiline `textArea` and direct inner accessible name remain open.

## Further Actions

- Phase 29: first-party `@clay/coding-agent` on these primitives; declare
  extension points (tools, skills, approvals, MCP allow-list, prompt);
  third-party `extends`/`replaces` with user approval. Requires AI-Safe
  Mutation.
- Later Work/PA/Research/Finance: more first-party packages, same host.
- Optional: composer steer UI if mid-run injection is wanted in Chat.
- Promote generic package `textArea` / multiline composer semantics and give
  inner editor direct accessible naming; rerun Chat a11y review.
- Rerun provider/model/agent/setup/session, stream, cancel, and narrow/wide
  keyboard flows when a keyboard-capable Wayland/X11 backend is available.
