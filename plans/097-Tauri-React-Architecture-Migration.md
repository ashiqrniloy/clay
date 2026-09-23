# Tauri v2 and React Architecture Migration to Current Feature Parity

Source: `roadmap.md`, replaced on 2026-08-23. Approved decisions:
`decision-logs/2026-08-23-0052-tauri-react-client-architecture.md` and
`decision-logs/2026-08-23-0115-mandatory-project-local-ui-skill-stack.md`.

This is the executable master plan for Roadmap Phases 1–12. Each task must be
split into a smaller numbered implementation plan before execution when its
file list or review surface cannot be completed safely in one change. The
master checkbox closes only when all child plans and listed checks pass.

## Objectives

- Replace the Masonry/Vello/Parley client with a Tauri v2, React, TypeScript,
  Vite, React Router, and CodeMirror 6 client.
- Preserve the separate Rust server, `rkyv` transport, canonical document and
  workspace authority, package model, two `deno_core` runtime trust domains,
  language services, and Prism `clay-agent` daemon.
- Adopt AG-UI as the React-facing agent stream over typed Tauri channels while
  keeping ACP out of the first-party path.
- Reach verified parity with every currently implemented user workflow,
  security boundary, accessibility contract, performance invariant, package
  surface, public API, and operational behavior before deleting the native
  client.
- Rewrite current-state documentation and maintenance checks so the repository
  describes one internally consistent Tauri/React architecture.

## Expected Outcome

- `clay` launches one production Tauri desktop client; `clay server` remains
  independently runnable for local, remote, container, and headless use.
- React renders Clay shell and package UI; CodeMirror owns immediate editor
  state; Tauri Rust owns only narrow OS/process/transport integration; Clay
  server remains canonical.
- Current files/workspaces, editing, language intelligence, tabs/splits,
  command/path surfaces, configuration, packages, themes, settings, Git, and
  Chat workflows pass the parity ledger on Linux.
- Masonry, Vello, Parley, winit, local native accessibility patches, native UI
  source, and native-only tests/docs are removed after parity certification.
- Rust, TypeScript, frontend, Node daemon, security, documentation, visual,
  accessibility, performance, and packaged-install gates pass.

## Plan Evidence and Project Patterns

Mandatory project-local UI skill stack reviewed for this plan:

- `.agents/skills/clay-ui/SKILL.md`
- `.agents/skills/clay-ui/references/components.md`
- `.agents/skills/clay-ui/references/tokens.md`
- `.agents/skills/impeccable/SKILL.md`
- `.agents/skills/full-output-enforcement/SKILL.md`
- `.agents/skills/high-end-visual-design/SKILL.md`
- `.agents/skills/design-taste-frontend/SKILL.md`

Every UI task below must load and list this complete stack again before source
review or edits. Apply the skills as complementary quality lenses; the user
brief, Clay's Operate-mode product identity, accessibility, security, authority
boundaries, component compatibility, and typed theme tokens resolve conflicts.

Relevant project patterns:

- `.agents/skills/project-patterns/references/tauri-react-client.md`
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
- `.agents/skills/project-patterns/references/package-ui-layout.md`
- `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
- `.agents/skills/project-patterns/references/mode-primitive-first.md`
- `.agents/skills/project-patterns/references/product-surfaces-are-packages.md`
- `.agents/skills/project-patterns/references/agent-host.md`
- `.agents/skills/project-patterns/references/documentation-as-code.md`
- `.agents/skills/project-patterns/references/maintenance-validation.md`
- `.agents/skills/project-patterns/references/ui-modernization.md`
- `.agents/skills/project-patterns/references/ui-visual-review.md`
- `.agents/skills/project-patterns/references/ui-skill-stack.md`

## Tasks

- [x] Phase 1 — Freeze the native client and create the feature-parity ledger

  Completed 2026-08-23 without a child plan (single-surface docs/tests change).
  Evidence:

  - `docs/development/tauri-react-parity-ledger.json` — 18 capability rows;
    exact partition of 507 manual steps (`test-plan/01`–`14`), 130 public Clay
    JS API IDs, and all 26 client / 40 server / agent-protocol message
    families; statuses start `pending`.
  - `docs/development/tauri-react-parity-ledger.md` — ownership map,
    completion rules (`ported` → `verified` requires named automated +
    manual evidence), native-freeze scope, baseline validation record.
  - `tests/documentation_coverage.rs` (wired into the `protocol` suite) —
    coverage test (each step/API referenced exactly once, no stale refs,
    protocol families covered), status test (`verified` rows require named
    evidence, rows well-formed), native freeze guard (frozen module set;
    additions fail until Plan 097 records the exception).
  - Baseline recorded: fmt/check/clippy PASS; full test run has exactly two
    pre-existing failures (bundled chat extension-point scope check;
    command-centre session call-site count) recorded as blockers; cargo audit
    shows no vulnerabilities (rkyv advisories resolved at 0.8.17; three
    allowed unmaintained warnings); clay-agent npm tests 8/8; review harness
    not re-run at freeze (desktop-session requirement), latest retained
    record 2026-08-21.
  - Acceptance Criteria:
    - Functional: Every implemented manual-test step, public Clay JS API,
      protocol message family, package contribution, command, keybinding,
      configuration option, UI state, accessibility behavior, and current
      platform workflow maps to one target migration task and automated/manual
      verifier. The ledger distinguishes implemented behavior from unfinished
      historical roadmap work.
    - Performance: Existing budgets and representative measurements are
      recorded before migration; no budget is silently removed or raised.
    - Code Quality: Source is classified as keep, adapt, port, or delete, with
      current and target owners. Native feature expansion is frozen except for
      baseline security/release fixes.
    - Security: Current vulnerabilities, capability boundaries, process
      authority, package trust-domain tests, and known failing checks are
      recorded as blockers rather than waived.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `roadmap.md`: Target phases and parity definition.
      - `test-plan/index.md`: Current manual behavior inventory and coverage
        matrix.
      - `docs/development/architecture-ownership.md`: Current ownership map.
      - `docs/wiki/index.md`: Implemented module inventory.
      - `Cargo.toml`, `frontend` when created, and `clay-agent/package.json`:
        dependency/test surfaces.
    - Options Considered:
      - Port from memory and discover gaps late: fastest initial coding, but
        cannot prove parity. Rejected.
      - Keep old roadmap as the parity source: mixes completed, unfinished,
        deferred, and superseded work. Rejected.
      - Build a ledger from implemented tests/docs/source and freeze it before
        UI work: selected.
    - Chosen Approach:
      - Add a checked-in parity ledger keyed by stable capability IDs. Every row
        contains current source/tests, target owner, target tests, migration
        phase, and state (`pending`, `ported`, `verified`, `approved-removed`).
      - Run `cargo fmt --check`, `cargo check --all-targets`,
        `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`,
        `cargo audit`, current Node tests, generated-registry checks, and the
        current review harness. Record exact failures.
    - API Notes and Examples:
      ```text
      capability_id = editor.optimistic-edit
      current_owner = src/masonry_pane_document.rs
      target_owner = frontend/src/editor/ClayEditor.tsx
      automated = tests/editor + frontend editor integration
      manual = test-plan/04-core-editing.md
      status = pending
      ```
    - Files to Create/Edit:
      - `docs/development/tauri-react-parity-ledger.md`: Human-readable ledger,
        ownership map, and completion rules.
      - `docs/development/tauri-react-parity-ledger.json`: Deterministic ledger
        consumed by validation tests.
      - `tests/documentation_coverage.rs` or successor: Fail when required
        implemented surfaces lack ledger rows.
      - `roadmap.md`: Update phase status only if baseline work changes scope.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - Ledger coverage test: Every current manual-test step and public API ID is
      referenced exactly once.
    - Ledger status test: `verified` rows require named automated and manual
      evidence.
    - Native-freeze guard: New native client feature modules require an
      explicit migration-plan reference.

- [x] Phase 1 — Review existing editor, package, protocol, and runtime primitives before port work

  Completed 2026-08-23 without a child plan (single-surface docs/ledger/test
  change; no runtime code). Evidence:

  - `docs/development/tauri-react-primitive-migration.md` — primitive reuse and
    gap matrix covering documents, behavior manifests, edits, decorations,
    completion, intelligence, SDUI, package UI, configuration, themes, menus,
    tabs/splits, agents, and persistence with a closed disposition vocabulary
    (reuse / adapter / projection / delete), hot-path contract, DTO deny list,
    parity-ledger coverage map for all 18 capability rows, and the generic gaps
    carried from the Phase 25 review: UTF-16 position-map conversion module
    (pure conversion), generic AG-UI event adapter (`src/server/agent_agui.rs`,
    child plan before `@clay/chat` consumes it), generic pane-content package
    contribution, and multiline `textArea` catalog kind.
  - `docs/development/tauri-react-parity-ledger.json` — every capability row now
    links a sorted `primitives` array naming its backing
    `docs/reference/primitives/registry.md` categories.
  - `tests/documentation_coverage.rs` (protocol suite) — two new guards:
    `primitive_migration_matrix_covers_ledger_and_registry_primitives` (every
    ledger row appears in the matrix; every cited primitive exists in the
    registry — no invented primitives) and
    `frontend_bridge_sources_stay_free_of_forbidden_authority_markers`
    (DTO deny-list tripwire over future `src-tauri/src` / `frontend/src`
    sources; pins the deny list in the matrix until those directories exist).
  - Verification: `cargo fmt --check` PASS; `cargo clippy --test protocol --
    -D warnings` PASS; `cargo test --test protocol` 200 passed / 0 failed
    (includes all five `documentation_coverage` tests); `primitives_docs`
    29 passed; `git diff --check` clean. No new Rust primitives were added, so
    no registry/backlog/wiki-nav changes were required in this phase.
  - Acceptance Criteria:
    - Functional: Inventory states what target frontend can reuse unchanged,
      what needs a Tauri adapter, what needs a React/CodeMirror projection, and
      what is native-only deletion. Package/mode implementation starts only
      after this inventory identifies generic gaps.
    - Performance: Hot-path ownership is explicit: CodeMirror local
      transactions, bounded edit queues, viewport-bounded rendering, and no
      synchronous package/server work.
    - Code Quality: New Rust primitives are generic across modes/packages; no
      Markdown-, Rust-, Chat-, or CodeMirror-specific server authority is added.
    - Security: Existing two runtime domains, package provenance, server
      permissions, external-process rules, and document/workspace authority are
      preserved.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/primitives/index.md`
      - `docs/reference/primitives/registry.md`
      - `docs/wiki/modules/primitive-architecture.md`
      - `docs/wiki/modules/server-driven-ui.md`
      - `docs/wiki/modules/decoration-transport.md`
      - `docs/wiki/modules/parse-coordinator.md`
      - `docs/wiki/modules/embedded-js-runtime.md`
      - `docs/wiki/modules/phase25-agent-host-primitive-review.md`
    - Options Considered:
      - Recreate primitives in TypeScript: duplicates canonical Rust behavior.
        Rejected.
      - Expose low-level server internals directly to React: broad and unsafe.
        Rejected.
      - Keep Rust primitives and add bounded projection adapters: selected.
    - Chosen Approach:
      - Produce one migration matrix covering documents, behavior manifests,
        edits, decorations, completion, intelligence, SDUI, package UI,
        configuration, themes, menus, tabs, agents, and persistence.
      - New frontend capabilities consume generic server primitives; missing
        generic primitives receive their own child plan and docs/tests before a
        package or mode uses them.
    - API Notes and Examples:
      ```text
      Rust authority -> validated protocol state -> Tauri DTO/channel
      -> frontend adapter -> CodeMirror extension or React component
      ```
    - Files to Create/Edit:
      - `docs/development/tauri-react-primitive-migration.md`: Primitive reuse
        and gap matrix.
      - `docs/reference/primitives/registry.md`: Add target adapters only after
        implementation, not speculative API entries.
      - `docs/development/tauri-react-parity-ledger.json`: Link primitive owner
        for each affected row.
    - References:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - Primitive documentation coverage: Every newly added generic primitive is
      listed in reference docs and wiki navigation.
    - Architecture deny test: Frontend DTOs contain no filesystem handles,
      process handles, V8 values, raw ops, or archived-byte access.

- [x] Phase 1/4 — Review Clay UI catalog and lock React component/token reuse before UI implementation

  Completed 2026-08-23. Evidence:

  - `docs/development/react-ui-catalog-mapping.md` — locked before UI work:
    six binding decisions (native HTML first; React Aria Components as headless
    behavior layer beneath Clay-owned wrappers; CodeMirror owns editor text
    state only; react-resizable-panels for splits; TanStack Virtual for
    viewport-scale lists; token names/kind names preserved 1:1 during parity),
    a target-renderer + accessibility-contract row for every package-facing
    `ComponentKind`, a surface/chrome-primitive mapping (shell root, split
    tree, slots, status bar, welcome, transient menus, completion pop-up,
    Command Centre, path browser, file browser, tab bar, and all nine
    `paint_*` primitives plus editor chrome → CM extensions), the complete
    core-token → CSS-custom-property projection table (91 tokens,
    `token.name` → `--clay-token-name`, resolved once per theme snapshot
    install), derived font/hierarchy variables, explicit "internal:
    CodeMirror" block for `StyleRegistry` keys, five-state DOM mechanism
    table with native precedence, performance locks (no React rerender per
    keystroke, conditional heavy imports), security locks (validated declarative
    data only, single intent dispatcher, no Tauri API/global CSS/secrets in
    package trees), and justified gaps (`table` reserved, multiline
    `textArea`, generic pane-content contribution, toast).
  - Tests (protocol suite): `react_catalog_maps_every_component_kind`
    (catalog drift guard — every kind exactly one mapping row with filled
    renderer/accessibility cells) and
    `core_tokens_project_to_css_variables_or_internal_codemirror_values`
    (token drift guard — every `tokens.md` core token has its `--clay-*`
    projection; StyleRegistry keys pinned internal).
  - Verification: `cargo fmt --check` PASS; clippy `-D warnings` PASS;
    `cargo test --test protocol` 202 passed / 0 failed; `git diff --check`
    clean.
  - Acceptance Criteria:
    - Functional: Every current package component, shell primitive, overlay,
      menu, pane/tab control, theme token, typography role, and accessibility
      state maps to React Aria/native HTML, a small Clay component, CodeMirror,
      or an explicitly justified target gap.
    - Performance: Hot editor and long-list surfaces avoid broad React
      subscriptions; heavy renderers are conditionally loaded.
    - Code Quality: Component API favors composition, stable IDs, semantic
      tokens, and colocated tests. New host components are generic and added to
      the target catalog.
    - Security: Package UI cannot inject global CSS, arbitrary DOM callbacks,
      Tauri APIs, secrets, native handles, or direct command execution.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - [React Aria Components](https://react-spectrum.adobe.com/react-aria/components.html)
      - [WAI-ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/)
    - Options Considered:
      - Hand-roll every DOM interaction: repeats native-client mistake.
        Rejected.
      - Adopt a styled design-system framework as Clay's public contract:
        constrains themes/packages. Rejected.
      - Use accessible headless primitives beneath Clay-owned components and
        tokens: selected.
    - Chosen Approach:
      - Create a mapping table before components. Prefer native `button`,
        `input`, `textarea`, `dialog`, lists, headings, and landmarks; use React
        Aria for complex collection/menu/combobox/tab behavior.
      - Preserve semantic token names and component kinds during parity. Schema
        changes require migration tests rather than silent reinterpretation.
    - API Notes and Examples:
      ```tsx
      <ClayButton variant="primary" onPress={sendIntent}>
        Open File
      </ClayButton>
      ```
    - Files to Create/Edit:
      - `docs/development/react-ui-catalog-mapping.md`: Current-to-target
        component, token, state, and accessibility mapping.
      - `.agents/skills/clay-ui/references/components.md`: Update incrementally
        as target components become implemented; retain current-state labels.
      - `.agents/skills/clay-ui/references/tokens.md`: Document CSS-variable and
        CodeMirror projections after implementation.
      - `docs/reference/ui-components.md`: Target navigation after cutover.
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
      - `.agents/skills/project-patterns/references/ui-modernization.md`
  - Test Cases to Write:
    - Catalog drift test: Every package component kind has one target renderer
      and documented accessibility contract.
    - Token drift test: Every active semantic token resolves to a CSS custom
      property or an explicitly internal CodeMirror value.

- [x] Phase 2 — Establish the Tauri v2 workspace and secure desktop skeleton

  Completed 2026-08-23. Evidence:

  - **Workspace**: root `Cargo.toml` gained `[workspace] members =
    ["src-tauri"]` while remaining package `clay`; `src-tauri` (`clay-desktop`,
    tauri 2.11 / tauri-build 2.6) resolves through the shared lockfile
    (`Cargo.lock`). `clay server` is untouched and independently runnable.
  - **Desktop shell**: `src-tauri/src/{lib,main,commands,server}.rs` —
    `Supervisor` launches the real `clay-server` binary (resolution:
    `$CLAY_SERVER_BIN` → sibling of the executable → `PATH`) against
    `clay::ipc::default_endpoint()`, probes transport readiness (unix socket
    connect / named-pipe open; no protocol bytes), reports typed
    `ServerStatus` (serde internally tagged `connecting|connected|
    disconnected`, camelCase), and kills+reaps the child on shutdown, restart,
    or drop so the app can never orphan a server. A generation counter makes
    stale probe threads from a previous attempt inert. Commands:
    `server_status`, `server_restart`. Window close (`RunEvent::Exit`) shuts
    down cleanly.
  - **Frontend skeleton**: `frontend/` Vite 7 + React 19 + TypeScript strict
    (`noUncheckedIndexedAccess`, `verbatimModuleSyntax`); relative base,
    fixed dev port 1420; typed invoke wrapper (`src/lib/server.ts`) mirrors
    the Rust serde contract; `App.tsx` shows loading/error/reconnect states
    driven by a pure unit-tested view-model (`connection.ts`). ESLint
    (typescript-eslint + react-hooks), Prettier, Vitest wired as npm scripts.
  - **Security**: production CSP `default-src 'none'; script-src 'self';
    style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src ipc:
    http://ipc.localhost`; single `main` window capability granting only
    `core:default`; no fs/shell/process/http/dialog/opener plugins compiled or
    configured. Server spawn is desktop-Rust authority only — no webview
    command can spawn processes.
  - **Tests**: `src-tauri/src/server.rs` unit/integration tests — serde state
    contract; fake-child lifecycle (spawn → probe timeout → kill/reap → no
    `/proc` residue); restart pid replacement; drop-reaps-child; and a real
    end-to-end `clay-server` smoke (Connected transition + orphan-free
    teardown; self-skipping when the binary is unbuilt). `tests/
    config_security.rs` — CSP deny-by-default/no remote origins guard,
    core-only capability permission scan, privileged-plugin dependency ban.
    Frontend: 4 Vitest tests over the connection state machine.
  - **CI** (`.github/workflows/ci.yml`): Linux job now installs Tauri v2
    prerequisites (webkit2gtk-4.1, gtk3, appindicator, librsvg, xdo, ssl),
    sets up Node 24 with npm cache, runs frontend gates (npm ci, lint,
    format:check, test, build), then `scripts/check.sh full` which now spans
    both workspace members.
  - **Baselines** (recorded in `docs/development/build-and-test.md`
    "Phase 2 baselines"): production bundle JS 195 kB (61 kB gzip), CSS
    0.94 kB; `npm run build` <2 s; Vitest 4 tests + desktop crate 6 tests
    (incl. real `clay-server` end-to-end) all green; renderer performs no
    package/runtime initialization (only status polling until connected).
  - **Verification on this host (AerynOS)**: full local compile+link+run now
    VERIFIED after installing the GTK/WebKit devel stack — `cargo fmt --check`,
    `cargo clippy -p clay-desktop --all-targets -- -D warnings`, `cargo test
    -p clay-desktop`, and a live window run all PASS. Findings fixed during
    verification: probe-generation off-by-one (fetch_add old-value) that
    silenced readiness transitions; Tauri-template cdylib crate-type cannot
    link V8 (TLS relocations) → rlib-only; missing bundle icon; async command
    Result-return bound; supervisor now ADOPTS an already-running server
    (`Connected`, `pid: null`) instead of double-spawning. Root package
    behavior unchanged vs Phase 1 baseline.
  - Acceptance Criteria:
    - Functional: A Linux Tauri window loads the React application, discovers
      or launches a real Clay server, displays loading/error/reconnect state,
      and shuts down cleanly. Existing `clay server` remains independently
      runnable.
    - Performance: Development startup and production bundle baselines are
      recorded; launch performs no unnecessary package/runtime initialization
      in the renderer.
    - Code Quality: Strict TypeScript, deterministic dependency lock, lint,
      format, unit test, production build, and Rust workspace checks are wired.
    - Security: Strict CSP and minimal window/webview capabilities; no broad
      filesystem, shell, process, or network plugin permission in the main
      webview. Server spawn is Clay-core authority, never package authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - Context7 `/tauri-apps/tauri-docs`: Tauri v2 commands, capabilities,
        Vite host configuration, Linux prerequisites.
      - [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
      - [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
      - [Tauri CSP](https://v2.tauri.app/security/csp/)
      - [Tauri sidecars](https://v2.tauri.app/develop/sidecar/)
    - Options Considered:
      - Fold server into Tauri: breaks remote/headless isolation. Rejected.
      - Start a localhost HTTP server for UI: extra listener and auth surface.
        Rejected.
      - Tauri manages existing server process/endpoint through narrow Rust code:
        selected.
    - Chosen Approach:
      - Add root workspace membership while retaining the root `clay` package.
        Put desktop crate under `src-tauri/` and frontend under `frontend/`.
      - Keep one main webview initially. Separate isolated webviews are added
        only by a later approved package-UI requirement.
      - Use the existing server endpoint/discovery logic through a small shared
        Rust library seam; avoid shelling out through a command string.
    - API Notes and Examples:
      ```rust
      mod commands;

      pub fn run() {
          tauri::Builder::default()
              .invoke_handler(tauri::generate_handler![commands::server_status])
              .run(tauri::generate_context!())
              .expect("Tauri application failed");
      }
      ```
    - Files to Create/Edit:
      - `Cargo.toml`: Add workspace membership while retaining root package.
      - `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/src/lib.rs`,
        `src-tauri/src/main.rs`: Tauri crate and launch.
      - `src-tauri/tauri.conf.json`: Build, bundle, CSP, and window config.
      - `src-tauri/capabilities/main.json`: Minimal main-webview capability.
      - `frontend/package.json`, `frontend/package-lock.json`,
        `frontend/tsconfig.json`, `frontend/vite.config.ts`,
        `frontend/index.html`: Frontend toolchain.
      - `frontend/src/main.tsx`, `frontend/src/app/App.tsx`: Initial application.
      - `.github/workflows/*` or current CI files: Linux Tauri prerequisites and
        frontend gates (exact files determined by current CI layout).
      - `docs/development/build-and-test.md`: Transitional build commands.
    - References:
      - `src/launch.rs`, `src/ipc.rs`, `src/bin/clay-server.rs`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - Tauri command smoke: status command returns typed connected/disconnected
      states.
    - Capability negative test: Main webview cannot invoke unregistered
      filesystem/shell commands.
    - Process lifecycle integration: Spawn/connect/shutdown leaves no orphan.
    - Linux production build smoke: Frontend and Tauri bundle compile.

- [x] Phase 3 — Implement the typed Tauri bridge and session bootstrap

  Completed 2026-08-23. Evidence:

  - **Protocol serde layer** (`src/protocol/*`): 165 types now carry blanket
    `serde::Serialize/Deserialize` beside their rkyv derives — one semantic
    definition, two encodings. Envelopes are adjacently tagged camelCase
    (`{"family":…,"payload":…}` / `{"kind":…,"data":…}`); unit enums stay
    plain strings; nested payloads inherit camelCase via container attrs.
    Menu session ids cross as strings through `menu_session_id_serde`
    (high-bit ids exceed JS safe integers). `clay::client::ClientConnectionEvent`
    and `ClientResyncSnapshot` gained the same derives; the client queue grew
    a verbatim `enqueue_raw` for non-edit messages.
  - **Bridge modules** (`src-tauri/src/bridge/{mod,dto,errors,forwarder,
    session}.rs`, documented in `docs/wiki/modules/desktop-typed-bridge.md`):
    - `session.rs`: one live `clay::client` session per shell; idempotent
      `bootstrap()` (cached while connected, concurrent → busy), reconnect
      that aborts the old pump before the new handshake (stale stream data is
      structurally impossible; generation increments each session) with tab
      reclaim-or-new from observed `TabRegistry` bindings.
    - `forwarder.rs`: bounded ordered delivery — FIFO live lane (512, natural
      backpressure to the socket) plus latest-wins slots for viewport-
      resynthesizable decoration/folding families keyed by
      `(document, provenance, kind)` with a coalesced counter; disconnected
      notices bypass both lanes; failed sinks self-remove.
    - `errors.rs`: sanitized length-capped `BridgeError { code, message }`;
      requests are size-capped raw JSON (`MAX_REQUEST_BYTES = 512 KiB`)
      parsed strictly in Rust; `Hello` is forbidden (handshake is bridge
      owned); edits route through the optimistic edit queue; every other
      variant is identity-stamped by an exhaustive compile-guarded matcher.
  - **Commands**: `session_bootstrap`, `session_subscribe` (Tauri channel),
    `session_unsubscribe`, `session_reconnect`, `session_request`,
    `session_stats`; Phase 2 supervision commands unchanged. The webview sees
    no archive bytes, frames, sockets, or protocol versions and cannot forge
    its identity.
  - **Frontend facade** (`frontend/src/bridge/{client,types,subscriptions,
    errors}.ts`, `frontend/src/state/connection-store.ts`): typed envelope/
    bootstrap/error mirrors with branded menu-session/document ids, Channel
    subscription, per-kind event dispatch (unknown kinds ignored, forward
    compatible), dependency-free observable connection store; `App.tsx`
    bootstraps + subscribes on mount and offers session reconnect.
  - **TauRPC decision**: evaluated 2.0.0/specta v2 against native commands +
    serde-derived types; rejected with rationale and exact pins recorded in
    `docs/development/taurpc-spike.md`. Native path keeps protocol semantics
    server-side while giving React fully typed commands/channels — the plan's
    "one bridge module hides whether TauRPC is used" property holds because
    only `frontend/src/bridge/client.ts` touches IPC.
  - **Tests** (all green): `tests/dto_roundtrips.rs` — JSON round trip for
    every `ClientMessage` variant + constructible `ServerMessage` families,
    exhaustive family matchers as compile guards, menu-id string assertion,
    theme/typography/tab-registry round trips; `src-tauri/src/bridge/
    forwarder.rs` unit tests — latest-wins coalescing, live ordering,
    lifecycle bypass; `tests/bridge_session.rs` — real-server end-to-end:
    bootstrap completeness, TabRegistry delivery, typed TabCommand round trip
    (registry revision bump), disconnect notice on server death, reconnect to
    generation 2 with fresh identity; frontend Vitest — store transitions,
    dispatcher routing, error normalization.
  - **Verification on this host (AerynOS)**: `cargo fmt --check` PASS;
    `cargo clippy --workspace --all-targets -- -D warnings` PASS; desktop
    crate 20 tests PASS; root package unchanged vs Phase 1 baseline (same
    single pre-existing bundled @clay/chat failure); frontend lint/format/
    11 Vitest tests/build all PASS (bundle ~199 kB, ~63 kB gzip); live window
    smoke clean (adopted running server, zero orphans).
  - Acceptance Criteria:
    - Functional: Bootstrap, documents, tabs, runtime, menu, SDUI, theme,
      diagnostics, language, and agent server families reach React through
      typed commands/channels with reconnect and cancellation.
    - Performance: Streams are bounded and ordered; ordinary edits avoid full
      snapshots and per-token global event fan-out.
    - Code Quality: Server protocol semantics remain separate from frontend
      DTOs. One bridge module hides whether native commands or TauRPC is used.
    - Security: Archived bytes are validated in Rust; malformed, oversized,
      stale, unknown, or unauthorized messages fail before frontend install.
      Large IDs are strings in JavaScript.
  - Approach:
    - Documentation Reviewed:
      - [Tauri calling Rust](https://v2.tauri.app/develop/calling-rust/)
      - [Tauri channels](https://v2.tauri.app/develop/calling-rust/#channels)
      - [Tauri IPC](https://v2.tauri.app/concept/inter-process-communication/)
      - [TauRPC](https://github.com/MatsDK/TauRPC)
      - `docs/wiki/modules/protocol-codec.md`
      - `docs/wiki/modules/server-ipc-skeleton.md`
    - Options Considered:
      - Replace `rkyv` server protocol with JSON: large unrelated rewrite.
        Rejected.
      - Expose archived bytes to browser: unsafe and couples frontend to codec.
        Rejected.
      - Rust translation boundary with generated/bounded DTOs: selected.
    - Chosen Approach:
      - Run a bounded spike for exact TauRPC/Tauri/Specta versions. Record
        generated output, channel behavior, typed errors, cancellation, build
        reproducibility, and maintenance risk. Keep only if it reduces code.
      - Export IDs as branded strings. Export text positions as line + UTF-16
        column where editor-facing; convert at one boundary.
    - API Notes and Examples:
      ```rust
      #[tauri::command]
      async fn subscribe(
          state: tauri::State<'_, BridgeState>,
          events: tauri::ipc::Channel<FrontendEvent>,
      ) -> Result<(), FrontendError> {
          state.subscribe(events).await
      }
      ```
      ```ts
      export type DocumentId = string & { readonly __documentId: unique symbol };
      ```
    - Files to Create/Edit:
      - `src-tauri/src/bridge/mod.rs`, `codec.rs`, `commands.rs`, `channels.rs`,
        `dto.rs`, `errors.rs`: Server↔frontend bridge.
      - `frontend/src/bridge/client.ts`, `types.ts`, `subscriptions.ts`,
        `errors.ts`: Frontend bridge facade.
      - `frontend/src/state/connection-store.ts`: Bounded connection/bootstrap
        projection.
      - `src/protocol/*`: Only generic protocol fixes proven necessary by the
        spike; otherwise unchanged.
      - `docs/development/taurpc-spike.md`: Decision evidence and exact pins.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
  - Test Cases to Write:
    - DTO round trips for every bridged message family.
    - Oversized frame and malformed archive rejection.
    - Unknown enum/version rejection with sanitized frontend error.
    - Slow frontend channel backpressure/coalescing and cancellation.
    - Reconnect installs one complete latest state and rejects stale stream data.

- [x] Phase 4 — Build React shell, component registry, theme runtime, and accessibility foundation

  Completed 2026-08-23. Evidence:

  - **Shell + router** (`frontend/src/app/{App,router,use-clay-session}.tsx`,
    `layout/{app-shell,tab-bar,working-area}.tsx`): `createMemoryRouter` for
    `/workspace` only; DEV `/fixture/:id` for visual states. Landmarks are
    `header`/`main`/`footer`. Router is memoized on session generation;
    production routes subscribe to the session store so events do not remount
    the tree. `WorkingArea` hosts an optional left slot via
    `react-resizable-panels` (ratios 0.05–0.95).
  - **Component registry** (`frontend/src/components/*`): catalog kinds
    `button`, `label`/`text`, `textInput`, `dropdown`, `list`, `collapse`,
    `modal`, plus badge/kbd/divider chrome. React Aria supplies behavior;
    CSS Modules read only `--clay-*` tokens. Zero radius on chrome.
  - **Theme runtime**: Rust `resolve_theme_token_snapshot` (91 core tokens +
    contrast gate) → `ThemeSnapshotDto` on bootstrap and live `ActiveTheme`
    events → `themeStore` writes CSS vars once per revision. Naming rule
    `token.name.sub` → `--clay-token-name-sub`. Density pre-scales spacing;
    `motion.*` emits ms; `z.*` emits stacking integers 0/10/20/40/50.
    Host fallbacks live in `frontend/src/styles/tokens.css`.
  - **Tests**: 32 vitest (theme adapter, keyboard/focus, landmarks, render
    count, connection store). Production gzip 128 kB / 160 kB budget
    (`npm run check:budget`). Bundled-theme contrast remains
    `tests/theme_packages.rs` + `src/shell/theme.rs`.
  - **Docs**: `docs/wiki/modules/react-shell.md`, catalog mapping
    "Implemented (Phase 4)" table, `docs/development/build-and-test.md`
    Phase 4 baselines.
  - **Visual/a11y review** (headless Chrome CDP against `vite` DEV server;
    Tauri invoke absent so workspace shows the disconnected recovery state):
    - `code-reviews/screenshots/2026-08-23-tauri-react-phase4/workspace-disconnected.png`
      — header brand + tablist, `main` "Session lost" + Reconnect, footer.
      Roles: `tablist` "Window tabs", `main` "Clay workspace", `button`
      "Reconnect session".
    - `code-reviews/screenshots/2026-08-23-tauri-react-phase4/fixture-states.png`
      — loading / empty / error.
    - `code-reviews/screenshots/2026-08-23-tauri-react-phase4/fixture-controls.png`
      — buttons, badge, kbd, field, dropdown, list.
    - `code-reviews/screenshots/2026-08-23-tauri-react-phase4/fixture-modal.png`
      — dialog "Confirm" + Dismiss scrim; dialog stays opaque.
    Findings fixed in the same pass: leftover Phase 2 `styles.css` was still
    imported and restyled every `button` (removed); button base class was not
    composed onto variants; modal scrim opacity faded the dialog (now
    `color-mix` on the fill); `z.modal` emitted the string `modal` (invalid
    CSS `z-index`). computer-use-linux not used; CDP accessibility snapshot
    stood in. Live themed Connected workspace needs the Tauri webview (Phase 2
    binary) — not re-captured this pass.

  - Acceptance Criteria:
    - Functional: Routes, shell landmarks, controls, overlays, loading/error/
      empty states, responsive desktop layouts, current themes, appearance,
      typography, density, and live theme updates render correctly.
    - Performance: Components subscribe to minimal derived state; heavy
      renderers are code-split; long collections virtualize only after measured
      need; theme resolution is not repeated per frame.
    - Code Quality: Components are composable, focused, typed, colocated with
      tests, and styled only through Clay semantic tokens/CSS custom properties.
    - Security: Theme/package data is validated in Rust; no raw package CSS,
      scripts, unsafe URLs, or secret-bearing DOM attributes. Native semantics
      are preferred over unnecessary ARIA.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - [React Router modes](https://reactrouter.com/start/modes)
      - [React Router `createMemoryRouter`](https://reactrouter.com/api/data-routers/createMemoryRouter)
      - [React Aria Components](https://react-spectrum.adobe.com/react-aria/components.html)
      - `.agents/skills/clay-ui/references/tokens.md`
    - Options Considered:
      - Browser history routes: needs packaged SPA fallback for no product gain.
        Rejected.
      - Framework/SSR mode: unnecessary desktop complexity. Rejected.
      - Data Mode + memory router: selected.
      - Tailwind/shadcn as package contract: conflicts with Clay tokens. Rejected.
    - Chosen Approach:
      - Use React Router for top-level surfaces only. Tabs, panes, documents,
        menus, and overlays remain application state.
      - Use CSS Modules and generated `--clay-*` variables. One theme adapter
        maps resolved Rust theme snapshots to CSS and CodeMirror inputs.
      - Build a fixture route within development/test builds for deterministic
        visual states; it is not a production second shell.
    - API Notes and Examples:
      ```ts
      const router = createMemoryRouter(routes, {
        initialEntries: ["/workspace"],
      });
      ```
      ```css
      .button {
        color: var(--clay-color-text-primary);
        background: var(--clay-color-surface-control);
      }
      ```
    - Files to Create/Edit:
      - `frontend/src/app/router.tsx`, `routes/*`, `layout/*`: Application shell.
      - `frontend/src/components/*`: Clay component registry and tests.
      - `frontend/src/theme/*`: Theme snapshot, CSS variable, typography, and
        CodeMirror adapters.
      - `frontend/src/styles/tokens.css`, `global.css`: Host-owned styles.
      - `frontend/src/testing/ui-fixtures/*`: Deterministic visual states.
      - `src-tauri/src/bridge/dto.rs`: Theme/typography DTOs.
      - `docs/development/react-ui-catalog-mapping.md`: Implemented mapping.
    - References:
      - `.agents/skills/project-patterns/references/ui-modernization.md`
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
  - Test Cases to Write:
    - Keyboard/focus tests for buttons, tabs, dialogs, menus, inputs, and errors.
    - Theme parity tests for all bundled themes and token overrides.
    - Contrast rejection and invalid-theme fallback tests.
    - Narrow/wide/large-typography layout tests.
    - Render-count and frontend bundle-size budget checks.

- [x] Phase 5 — Port CodeMirror editing and optimistic document synchronization

  Completed 2026-08-23. Evidence:

  - **Editor host** (`frontend/src/editor/{ClayEditor,create-editor,transactions,compartments}.ts*`):
    one `EditorView` per `documentId`; compartments for read-only, theme,
    keymap, language, behavior, decorations. User/undo txs emit; resync/
    programmatic do not. Lazy-loaded from the workspace route.
  - **Sync** (`frontend/src/editor/sync/*`, `state/document-store.ts`):
    metadata only in React. Edits go `session_request` → existing
    `ClientEditQueue` (bridge still assigns `base_version`/lease). Ack,
    stale reject → `requestResync`, snapshot/open/reload replace the view,
    save/close update chrome.
  - **Position map**: `src/editor/position_map.rs` ↔
    `frontend/src/editor/position-map.ts` (shared golden vectors, mid-unit snap).
  - **Bridge**: `src-tauri/src/bridge/editor.rs`; `ClientResyncSnapshot` now
    camelCase.
  - **Tests**: 47 vitest; rust `editor::position_map` 4/4; desktop
    `bridge::editor` 2/2. Shell gzip 147/160 kB, total 222/400 kB.
  - **Docs**: `docs/wiki/modules/react-codemirror-editor.md`.
  - **Visual/a11y review** (headless Chrome CDP, `vite` DEV; computer-use-linux
    not used). Screenshots:
    `code-reviews/screenshots/2026-08-23-tauri-react-phase5/`
    - `workspace.png` — disconnected recovery unchanged.
    - `fixture-editor.png` — chrome (notes.md, v1 clean editable, Save/
      Reload/Close, path field, Open) + CM `textbox` "fixture document".
      Roles: `region` "Editor notes.md", `textbox` "Open path",
      `textbox` "Start typing". Toolbar compacted after first pass
      (native path input replaced labeled TextField). Live Connected
      edit/save against Tauri webview not recaptured this pass.

  - Acceptance Criteria:
    - Functional: Open, edit, save, reload, close, reject, correct, and resync
      work against the existing server with exact dirty/read-only/version state.
    - Performance: CodeMirror applies local transactions before IPC; edit
      batching remains within current keypress-to-local-paint budget and avoids
      React rerenders for each keystroke.
    - Code Quality: One adapter owns `EditorView`; behavior, keymap, language,
      theme, read-only, and decoration changes use compartments rather than
      editor recreation.
    - Security: Server versions, leases, document IDs, edit ranges, and payload
      limits are revalidated. Frontend cannot bypass server document authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - [CodeMirror system guide](https://codemirror.net/docs/guide/)
      - [CodeMirror reference](https://codemirror.net/docs/ref/)
      - `docs/wiki/flows/client-edit-emission.md`
      - `docs/wiki/flows/versioned-text-synchronization.md`
      - `docs/wiki/modules/server-document-state.md`
    - Options Considered:
      - Store editor text in React/Zustand: rerender and dual-authority risk.
        Rejected.
      - Invoke Rust on every keystroke before applying: breaks latency. Rejected.
      - CodeMirror transaction first, bounded asynchronous edit queue: selected.
    - Chosen Approach:
      - Mount one `EditorView` per visible pane through a lifecycle component.
        Use transaction annotations to distinguish user, correction, resync,
        remote, undo, and programmatic operations.
      - Define one UTF-16 position mapping module and property-test against the
        Rust UTF-8 rope conversion.
    - API Notes and Examples:
      ```ts
      view.dispatch({
        changes: { from, to, insert },
        annotations: clayTransaction.of(transactionId),
      });
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/ClayEditor.tsx`, `create-editor.ts`,
        `transactions.ts`, `compartments.ts`, `position-map.ts`.
      - `frontend/src/editor/sync/*`: Shadow state, queue, ack/reject/resync.
      - `frontend/src/state/document-store.ts`: Metadata/session projection only.
      - `src-tauri/src/bridge/editor.rs`: Editor DTO conversion.
      - `src/server/document.rs` and `src/protocol/*`: Only generic boundary
        changes identified by primitive review.
      - `frontend/src/editor/*.test.ts`: Editor integration tests.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - Unicode/emoji/combining-mark/CRLF UTF-16↔UTF-8 property tests.
    - Local typing remains immediate with blocked IPC consumer.
    - Ordered ack, stale reject, correction, and full resync.
    - Dirty/save/reload conflict and read-only lease behavior.
    - Editor lifecycle preserves state across theme/behavior updates.

- [x] Phase 6 — Port panes, splits, tabs, per-tab workspaces, and persistence

  Completed 2026-08-23. Evidence:

  - **Split tree** (`frontend/src/shell/split-tree.ts`): 4-pane cap, 0.05–0.95
    clamp, equal comb, close-merges-sibling, reading-order move. Nested
    `react-resizable-panels` in `PaneTree.tsx`.
  - **Tabs** (`workspace-controller.ts` + `BridgeState` session map): each tab
    is a real `connect_with_workspace_root` client. `session_request` stamps
    that tab's `client_id`. Events are `Routed { clientId, tabId, event }`.
  - **Dirty close**: Clay modal Save all / Discard / Cancel. Last tab refused.
  - **Duplicate open** focuses the owner pane in the same tab.
  - **Persist**: `layout_load`/`layout_save` reuse `parse_window_state`.
    Hostile files rejected; trees without pane 1 degrade.
  - **Tests**: 62 vitest; clay-desktop 14 lib + bridge_session + 3 security +
    7 dto. Shell gzip 162 / 180 kB.
  - **Docs**: `docs/wiki/modules/react-tabs-and-splits.md`.
  - **Visual/a11y review** (headless Chrome CDP, `vite` DEV; computer-use-linux
    not used). Screenshots:
    `code-reviews/screenshots/2026-08-23-tauri-react-phase6/`
    - `workspace.png` — disconnected recovery + tablist unchanged.
    - `fixture-splits.png` — two panes, `separator` at 50, pane 1 editor,
      pane 2 empty "No document". Divider thickened to `dimension.border.thin`
      after first pass. Live Connected multi-tab Tauri recapture not this pass.

  - Acceptance Criteria:
    - Functional: All current split, pane, tab, workspace, focus, resize,
      reorder, close, reconnect, and restore workflows match the parity ledger.
    - Performance: Pane paint and tab-switch replacement budgets meet current
      limits or stricter measured web equivalents; inactive views are bounded.
    - Code Quality: Stable IDs preserve editor/focus/scroll state. Split/tab
      models remain independent of pane content type.
    - Security: Tabs remain independent server clients; roots, documents,
      leases, capabilities, and package scopes cannot cross tab boundaries.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/wiki/modules/masonry-shell.md`
      - `docs/wiki/modules/tabs-and-clients.md`
      - `docs/wiki/modules/pane-document-views.md`
      - `docs/reference/primitives/shell-layout-strategy.md`
      - [react-resizable-panels](https://github.com/bvaughn/react-resizable-panels)
    - Options Considered:
      - Reuse native shell through embedded surface: maintains two UI trees.
        Rejected.
      - Build a speculative general docking framework: unnecessary. Rejected.
      - Port current bounded split-tree semantics with accessible separators:
        selected.
    - Chosen Approach:
      - Keep current server tab registry and persisted schema where compatible.
        React projects tab/split state; Tauri bridge carries existing commands.
      - Use an installed accessible panel library only if its exact version
        satisfies current nested split, stable ID, keyboard separator, and
        persistence requirements; otherwise implement the current small bounded
        split tree directly.
    - API Notes and Examples:
      ```text
      tab -> split tree -> pane leaf -> pane content
      pane content = editor | package entry | core fallback
      ```
    - Files to Create/Edit:
      - `frontend/src/shell/tabs/*`, `panes/*`, `splits/*`, `slots/*`.
      - `frontend/src/state/tab-store.ts`, `layout-store.ts`.
      - `frontend/src/persistence/layout.ts`.
      - `src-tauri/src/bridge/tabs.rs`, `layout.rs`.
      - `src/server/tab_registry.rs`, `src/protocol/*`: Compatibility-only
        changes if required.
      - `test-plan/13-window-splits.md`, `test-plan/14-tabs.md`: Transitional
        target steps.
    - References:
      - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - Four-pane cap, equal split, resize, reorder, focus, close.
    - Duplicate open focuses existing pane.
    - Tabs use independent connections/workspaces/documents/modes.
    - Dirty close and save/discard confirmation.
    - Corrupt/hostile persisted layout fallback.
    - Cross-tab root/document/lease/capability denial.

- [x] Phase 7 — Port full editor interaction, rendering, completion, and language-intelligence parity

  Completed 2026-08-23. Evidence:

  - **Generic CodeMirror adapters** (`frontend/src/editor/extensions/*`):
    manifest enter/tab/pairs/electric/chords, native movement/multi-selection,
    chunked syntax/semantic/link/inlay decorations, source-keyed lint,
    server folds, asynchronous completion/snippets, hover/definition/signature/
    code-action projection, textobject/smart-select requests, and accessibility.
  - **Rust authority preserved**: visible UTF-8 viewport requests only; exact
    document/behavior versions; server provenance/budgets/LSP/provider ranking
    unchanged. No CodeMirror language parser or browser LSP client.
  - **Theme parity**: Rust `StyleRegistry` resolves 35 token families plus
    SearchMatch/InlayHint layers into `ThemeSnapshotDto.editor_styles`; the
    adapter installs color/background/attributes/scale CSS variables once.
  - **Security**: Tauri stamps nested completion/intelligence/selection client
    identity; stale versions drop; hover uses `textContent`; links allow only
    same-document or safe relative targets; inlays stay out of editable a11y
    text; code-action edits remain inert previews.
  - **Markdown boundary correction**: the existing `@clay/markdown` preview is
    bounded package SDUI, not HTML. Phase 7 ports its decorated source editor;
    Phase 8 projects the unchanged preview panel. No unified/rehype dependency
    or duplicate Markdown authority was added.
  - **Verification**: frontend lint/build/budget PASS; 72 Vitest PASS. Desktop
    tests/clippy PASS. Existing Rust suites PASS: completion 27, language 31,
    decoration 20, syntax 73, editor hot-path 40.
  - **Performance**: shell 162.4/180 kB gzip; lazy editor 106.0 kB gzip; total
    269.4/400 kB. 1 MiB local typing and 1,000-span viewport budgets PASS.
  - **Visual/a11y**: `code-reviews/screenshots/2026-08-23-tauri-react-phase7/`
    contains default and large-type intelligence fixtures. CDP exposes one
    `textbox` named `Document editor`; inlay text is absent from its value,
    fold gutter is not a tab stop, and diagnostics include non-color icon
    presentation. First pass fake full-line indent grid and inlay AT leakage
    were fixed; second pass fixed large-type chrome wrapping. Linux AT-SPI
    could enumerate the Chrome frame but window-target resolution failed and
    development keyboard input was unavailable; CDP supplied the bounded
    editor tree/visual proof. Live provider interaction remains in the manual
    test plan.
  - **Docs**: `docs/wiki/modules/react-codemirror-editor.md`, catalog mapping,
    parity ledger (five rows → `ported`), and build baselines updated.

  - Acceptance Criteria:
    - Functional: Every current core-editing, movement, selection, multi-cursor,
      syntax, diagnostics, completion, folding, link, inlay, Markdown, and LSP
      ledger row passes in CodeMirror.
    - Performance: Large-file, typing, scrolling, decoration, completion, and
      intelligence budgets are measured; work is cancellable and viewport-
      bounded; heavy preview modules load on demand.
    - Code Quality: Existing Rust/package language authority is reused through
      generic CodeMirror adapters. No language-specific frontend architecture
      branch or duplicate browser LSP client is introduced.
    - Security: Link/navigation intents, completion commands, package
      decorations, HTML/SVG/Markdown preview, and language results are bounded,
      sanitized, provenance-checked, and stale-version rejected.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - CodeMirror reference sections for state, view, decorations,
        autocompletion, lint, language, panels, gutters, and merge.
      - `docs/wiki/modules/decoration-transport.md`
      - `docs/wiki/modules/completion-snippet-expansion.md`
      - `docs/wiki/modules/language-intelligence.md`
      - `docs/wiki/modules/folding-ranges.md`
      - `docs/wiki/modules/first-party-language-packages.md`
    - Options Considered:
      - Use CodeMirror language packages as canonical parser/LSP source:
        duplicates server and package authority. Rejected.
      - Keep native editor hidden for advanced features: permanent dual client.
        Rejected.
      - Adapt existing inert results into CodeMirror extensions: selected.
    - Chosen Approach:
      - Implement separate adapters for behavior manifest, keymap/transforms,
        decorations, diagnostics, completion, folding, intelligence, and
        accessible editing. Keep adapters generic and independently tested.
      - Preserve the current Markdown preview as validated package SDUI and
        project it with package UI in Phase 8. Do not add a second browser
        Markdown/HTML renderer, notebook, or LaTeX scope.
    - API Notes and Examples:
      ```ts
      const decorationExtension = EditorView.decorations.compute(
        [serverDecorationField],
        state => projectDecorations(state.field(serverDecorationField)),
      );
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/extensions/behavior.ts`, `keymaps.ts`,
        `decorations.ts`, `diagnostics.ts`, `completion.ts`, `folding.ts`,
        `intelligence.ts`, `accessibility.ts`.
      - `frontend/src/editor/testing/*`: Feature fixtures and performance checks.
      - `src-tauri/src/bridge/language.rs`, `decorations.rs`, `completion.rs`.
      - Existing `src/server/*` and `packages/*`: Only parity defects or generic
        adapter DTO support; mode logic stays package/server-owned.
    - References:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - Movement/selection/multi-cursor/undo/IME/clipboard/key-sequence matrix.
    - Decoration layer ordering, stale replacement, provisional continuity.
    - Completion ranking/snippets/exclusive/disable/stale/error behavior.
    - Folding/link/inlay/hover/definition/signature/code-action behavior.
    - Markdown source/preview sanitization and theme parity.
    - Large-file and slow-language-provider responsiveness.

- [x] Phase 8 — Port SDUI/package UI and preserve package trust domains

  Completed 2026-08-23. Evidence:

  - **Complete package UI wire snapshot** (`src/protocol/runtime.rs`, protocol
    v26): generation-stamped empty-tab content, fixed panels, overlays,
    standalone/status components, input routes, action targets, and
    host-stamped package provenance/trust labels. Bounds are 4 panels, 16
    overlays, 64 routes, unique IDs, allowed slots/anchors, and 16 KiB parsed
    component trees.
  - **Server authority** (`src/server/ui.rs`, `src/server/mod.rs`):
    `PackageUiRegistrySnapshot::wire_snapshot` is built from already-validated
    manifest records. Trusted labels require an exact enabled bundled record
    and runtime domain; missing/spoofed records fail to third-party.
  - **Narrow Tauri adapter** (`src-tauri/src/bridge/dto.rs`): runtime snapshots
    retain client/tab routing, resolve theme/typography in Rust, and parse
    component JSON into inert values before the webview sees it. Raw theme
    overrides, package modules, V8 values, callbacks, CSS, and scripts do not
    cross.
  - **Stable React projection** (`frontend/src/sdui/*`): targeted SDUI
    snapshot/update state, stale-base denial, all 15 implemented component
    kinds, semantic token-only styles, typed action intents, inert hostile
    text, and stable keys preserving input/disclosure/dropdown/focus/scroll
    state. `table` remains reserved.
  - **Package shell** (`frontend/src/packages/PackageWorkspace.tsx`,
    `frontend/src/shell/*`): mandatory main plus top/left/right/bottom slots,
    contained overlays, package status, SDUI editor slot, generic empty-tab
    package landing, and core Open File/Open Folder fallback. Package renderer
    is code-split and loads only when SDUI/package state exists.
  - **Trust domains unchanged**: package graph 18, package loading 47,
    cross-domain 7, package UI conformance 10, primitive docs 29 — PASS.
    Third-party replacement remains third-party; adoption, revoke, rollback,
    stale-generation, internal-op/module, raw-style, and Tauri capability
    denials remain enforced. No isolated arbitrary third-party React surface
    was added because the parity ledger identifies no current consumer.
  - **Verification**: frontend lint/typecheck/build/budget PASS; 79 Vitest PASS.
    Desktop all-target tests PASS (16 lib, bridge session, 3 security, 7 DTO).
    Rust fmt/check/clippy, package/SDUI suites, and protocol v26 round trips
    PASS. Full `cargo test --all-targets --no-fail-fast` retains exactly two
    Phase 1 baseline failures: bundled `@clay/chat` extension-point scope and
    Command Centre session call-site count. Startup shell 164.3/180 kB gzip; package renderer 27.8
    kB; total 299.3/400 kB.
  - **Visual/a11y**:
    `code-reviews/screenshots/2026-08-23-tauri-react-phase8/` contains wide,
    narrow, large-type, and open-dropdown states. CDP exposes complementary
    Workspace, CodeMirror textbox, Settings region, labelled controls,
    ListBox/Option dropdown semantics, and package status. First pass fixed
    optional-grid slot overflow, full-height working-area loss, narrow main
    starvation, and panel containment. `computer-use-linux` exposed only the
    Chrome frame and omitted Chrome-for-Testing from compositor targets, so
    desktop keyboard claims remain blocked; CDP supplied bounded DOM/a11y
    evidence. Fresh Impeccable reviewer disposition: `ship`; detector: clean.
  - **API/config review**: no Clay JS API, facade, configuration option,
    `examples/init.js` line, package manifest field, permission, component kind,
    token, or overlay anchor changed. Existing one-line loads are the contract.
  - **Docs/manual parity**: package guide, component catalog, React mapping,
    bridge/SDUI wiki, new React package UI wiki, build baselines, test-plan
    P32-P36/Q28-Q30, and parity ledger updated. At Phase 8 completion those
    three rows were `ported`; Phase 12 later verified them before native deletion.
  - **Phase boundary**: `settings.open` persistence/native-dialog/configuration
    workflows remain Phase 9; Chat transcript/streaming remains Phase 10. Phase
    8 renders their validated declarative surfaces without duplicating those
    authorities.

  - Acceptance Criteria:
    - Functional: Existing SDUI and package UI snapshots/updates, component
      kinds, pane contents, slots, overlays, settings, Git, file browser, and
      Chat landing render through stable-ID React reconciliation.
    - Performance: Updates are bounded and targeted; unchanged nodes retain
      state; package JavaScript never runs in render/layout/input hot paths.
    - Code Quality: `package.json` contributions remain the single data source;
      one-line `loadPackage` defaults remain; target registry is generic and
      documented.
    - Security: Trusted classification uses compiled provenance/integrity, not
      names. Third-party runtime lacks internal ops/modules/Tauri access.
      Cross-domain values remain typed, inert, bounded, generation-checked, and
      revocable. Same-realm arbitrary third-party React is denied.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/wiki/modules/server-driven-ui.md`
      - `docs/wiki/modules/masonry-sdui-region.md`
      - `docs/wiki/modules/slot-aware-package-ui.md`
      - `docs/wiki/modules/third-party-runtime-authority.md`
      - `docs/reference/packages/creating-packages.md`
      - `.agents/skills/project-patterns/references/package-manifest-single-source.md`
    - Options Considered:
      - Let packages return JSX: breaks validation and isolation. Rejected.
      - Keep native Masonry package UI in a second window: permanent dual UI.
        Rejected.
      - Stable-ID declarative React registry with isolated advanced surfaces:
        selected.
    - Chosen Approach:
      - Preserve component kinds and semantic styles during parity. React keys
        use SDUI node IDs; host state keyed by node ID survives snapshots.
      - First-party trusted frontend modules are build-time registrations only.
        Package logic still runs in server `deno_core`.
      - Do not implement isolated custom third-party surfaces unless the Phase 1
        ledger identifies a current feature requiring them.
    - API Notes and Examples:
      ```tsx
      const Component = componentRegistry[node.kind];
      return <Component key={node.id} node={node} sendIntent={sendIntent} />;
      ```
    - Files to Create/Edit:
      - `frontend/src/sdui/*`: Schema projection, reconciler, registry, state,
        action routing, tests.
      - `frontend/src/packages/*`: Pane content, panels, overlays, provenance UI.
      - `src/protocol/runtime.rs`, `src/server/{ui,mod}.rs`: Complete bounded
        package UI snapshot, action-target freshness, and host-stamped trust.
      - `src-tauri/src/bridge/{dto,session,forwarder}.rs`: Rust-parsed atomic
        runtime projection; no parallel bridge modules were needed.
      - `packages/*`: No schema or package source migration was required.
      - `docs/reference/packages/creating-packages.md`: Transitional and final
        React renderer contract.
      - `.agents/skills/clay-ui/references/components.md`: Target renderer state.
    - References:
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
  - Test Cases to Write:
    - Stable node identity preserves scroll/focus/input/collapse state.
    - Snapshot/update bounds, stale generation, unknown kind/style rejection.
    - Third-party internal op/module/Tauri denial.
    - Adopt/revoke/replace/rollback removes UI and executable authority.
    - One-line first-party package loading and unload-to-fallback.

- [x] Phase 9 — Port Command Centre, path browsing, configuration, settings, and desktop workflows
  - Acceptance Criteria:
    - Functional: Command mode, path mode, pickers, native dialogs, settings,
      themes, typography, configuration reload, status/recovery, file browser,
      and Git workflows match current behavior and keyboard operation.
    - Performance: Menu open/filter, path listing, configuration reload, and
      settings updates meet current budgets; filesystem work stays off render.
    - Code Quality: One Command Centre and one configuration authority remain;
      no duplicated dropdown/picker or frontend-only preference store.
    - Security: Browse grants, file/folder grants, secret inputs, path
      sanitization, package command provenance, and runtime generation install
      remain fail-closed.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/wiki/modules/transient-menu-round-trip.md`
      - `docs/wiki/modules/centered-command-centre-surface.md`
      - `docs/wiki/modules/path-browser.md`
      - `docs/wiki/modules/configuration-runtime.md`
      - `docs/wiki/modules/phase20.6-theme-segregation-settings-ui.md`
    - Options Considered:
      - Replace server-owned menu sessions with local fuzzy lists: authority and
        generation drift. Rejected.
      - Keep existing server sessions and port accessible projection: selected.
    - Chosen Approach:
      - React renders bounded server-owned menu snapshots and emits typed
        query/selection/activate/cancel/backspace intents.
      - Tauri commands open OS dialogs; selected paths return to existing
        server grant paths. Secret setup fields bypass snapshots and logs.
    - API Notes and Examples:
      ```text
      server menu session -> bounded snapshot -> React dialog/listbox
      -> typed intent -> shared server command executor
      ```
    - Files to Create/Edit:
      - `frontend/src/command-centre/*`, `settings/*`: Shared command/path/
        picker projection and trusted bundled settings presentation.
      - `frontend/src/shell/{workspace-controller,WorkspacePanes}.ts*`,
        `app/layout/app-shell.tsx`: Per-tab menu/diagnostic/settings state,
        closed client-command dispatch, status, and native new-tab flow.
      - `frontend/src/editor/{sync/session.ts,extensions/controller.ts}`:
        Reuse active CodeMirror command and explicit clipboard paths.
      - `src-tauri/src/{commands,lib}.rs`, `bridge/session.rs`: Existing native
        dialog backend on blocking pool, direct capability conversion, no
        extra bridge modules or broad plugin capabilities.
      - `src/server/connection/runtime.rs`, `command_execution.rs`,
        `ops/typography.rs`: Client-UI projection and atomic typography
        validation/persistence.
      - `test-plan/02-configuration-init-js.md`,
        `03-files-and-workspace.md`, `09-packages-and-modes.md`,
        `10-keybindings-and-commands.md`, `11-performance.md`.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
  - Test Cases to Write:
    - Command filtering, activation, stale generation, package provenance.
    - Path descend/ascend/jump/open/workspace grant and traversal denial.
    - Modal focus trap, Escape, focus restoration, result announcements.
    - Secret setup values absent from snapshots/logs/DOM accessible names.
    - Config success/failure atomic reload and old-generation preservation.
  - Completion Evidence (2026-08-23):
    - `frontend/src/command-centre/CommandCentre.tsx` renders command, Path
      Browser, and picker snapshots through one React Aria dialog/textbox/
      listbox. Query, semantic Backspace, relative selection, primary/
      secondary activation, and cancel remain opaque server intents.
    - `workspace-controller.ts` owns one menu/diagnostic/settings visibility
      state per tab and a closed dispatcher for exact pane/tab/editor/dialog/
      settings client commands. Unknown sibling IDs are inert.
    - Tauri reuses `clay::client` native dialog backends in `spawn_blocking`;
      selected paths go directly into `ClientEditQueue` single-use capability
      helpers and never enter React/package DOM. Main capability stays
      `core:default`; security tests confirm no privileged plugin.
    - The compiled trusted `SettingsPanel` is selected only for exact bundled
      `@clay/settings`, composes existing catalog controls/tokens, and sends
      current-version intents. Complete three-profile/seven-ratio typography
      now validates before atomic persistence and again on reload.
    - Runtime diagnostics project per tab into shell status. Reload, watcher,
      package/Git/file-browser/path authority and generation rollback remain
      existing server paths; no frontend catalogue, fuzzy matcher,
      configuration evaluator, filesystem listing, or preference authority was
      added.
    - UI review started with `computer-use-linux_get_app_state`; CDP verified
      active/empty/narrow Command Centre, Path Browser, settings
      collapsed/expanded/narrow/invalid states, focus roles/labels, and live
      result status. Evidence and `ship` finish verdict:
      `code-reviews/screenshots/2026-08-23-tauri-react-phase9/`.
    - Verification PASS: frontend format/lint/typecheck, 84 Vitest tests,
      production build/budgets (startup 156.5/180 kB gzip, total
      304.9/400 kB), Rust fmt/check/clippy, menu sessions 17, Path Browser 34,
      settings 10, protocol 202, desktop 16 + bridge/session/security/DTO.
      Full all-target run retains the two documented Phase 1 baseline
      failures; one concurrent sandbox fixture hit transient Linux `ETXTBSY`
      and passed its exact single-thread rerun.
    - Clay JS API/configuration review found no new public API or option:
      existing `controlCenter.*`, documents/workspace client commands,
      `runtime.reloadConfiguration`, and `theme.*` APIs remain authoritative;
      `examples/init.js` stays valid and unchanged. Wiki/catalog/package guide,
      parity ledger, and test-plan modules 02/03/09/10/11 are updated.

- [x] Phase 10 — Adopt AG-UI over Tauri channels and port current Chat behavior
  - Acceptance Criteria:
    - Functional: `@clay/chat` supports landing, provider/model/agent/setup,
      prompt, streaming, cancellation, transcript, session list/resume/delete,
      thinking, usage, and error states through one AG-UI client stream.
    - Performance: Agent deltas are batched/reduced without per-token global
      rerenders; transcript retention and event payloads remain bounded; editor
      input does not wait on agent work.
    - Code Quality: Prism daemon protocol remains internal. One Rust adapter
      produces AG-UI; React uses a custom `AbstractAgent` transport and does not
      maintain a parallel Clay-only event reducer.
    - Security: Credentials never enter events, snapshots, logs, DOM
      attributes, or accessibility names. Packages cannot spawn/speak to daemon
      or acquire Tauri authority. ACP remains absent.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - [AG-UI architecture](https://docs.ag-ui.com/concepts/architecture)
      - [AG-UI state](https://docs.ag-ui.com/concepts/state)
      - `docs/wiki/modules/clay-agent.md`
      - `docs/wiki/modules/phase25-agent-protocol.md`
      - `docs/wiki/modules/phase25-agent-process-manager.md`
      - `.agents/skills/project-patterns/references/agent-host.md`
    - Options Considered:
      - `HttpAgent` over localhost SSE: unnecessary listener. Rejected.
      - AG-UI directly between daemon and React: bypasses server authority.
        Rejected.
      - Server adapter + Tauri channel `AbstractAgent`: selected.
    - Chosen Approach:
      - Map existing Prism/Clay event union to AG-UI lifecycle, text, tool,
        snapshot/delta, raw/custom, finish, and error events. Tool variants may
        remain unused for current Chat but must not expose future authority.
      - Keep session/profile/provider/model selection server-owned. React owns
        presentation and local composer interaction only.
    - API Notes and Examples:
      ```ts
      class TauriClayAgent extends AbstractAgent {
        run(input: RunAgentInput): Observable<BaseEvent> {
          return runOverTauriChannel(input);
        }
      }
      ```
    - Files to Create/Edit:
      - `src/server/agent_agui.rs`: Bounded internal-event→AG-UI adapter.
      - `src-tauri/src/bridge/agent.rs`: Run commands and event channels.
      - `frontend/src/agent/TauriClayAgent.ts`, `events.ts`, `state.ts`.
      - `frontend/src/chat/*`: Landing, composer, transcript, sessions, setup.
      - `frontend/package.json`, lockfile: Exact AG-UI packages.
      - `packages/chat/*`: Presentation schema/registration migration as needed.
      - `docs/reference/packages/creating-packages.md`: Agent surface contract.
    - References:
      - `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
      - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
  - Test Cases to Write:
    - Event mapping for lifecycle/text/state/error/cancel and reserved tools.
    - Snapshot then ordered RFC 6902 delta application and resync recovery.
    - Slow renderer batching/backpressure and transcript bounds.
    - Credential redaction across Rust, Tauri DTO, JS logs, DOM, and a11y.
    - Package disable/replacement removes landing/profile but not host security.
  - Completion Evidence (2026-08-24):
    - `src/server/agent_agui.rs` maps Clay agent wire messages to standard
      AG-UI events. Tools/permissions/overflow stay inert `CUSTOM` events;
      pickers stay Command Centre; credentials have no mapped field.
    - `src-tauri/src/bridge/agent.rs` fans adapted events over a Tauri
      channel. Prompts/cancel/sessions reuse `session_request`. Raw Clay
      agent frames never enter the webview envelope stream.
    - `TauriClayAgent` extends `@ag-ui/client` `AbstractAgent`. React has no
      parallel Clay event reducer; out-of-run snapshots use `setMessages`/
      `setState`. Store notifies at most once per animation frame.
    - `ChatPanel` mounts only for exact `@clay/chat` empty-tab provenance.
      Landing copy/buttons come from the package tree. Transcript, thinking,
      usage, error, sessions, and composer/cancel are host-presented.
    - UI review: `/fixture/chat` landing, conversation, streaming, error,
      and narrow states. Evidence:
      `code-reviews/screenshots/2026-08-23-tauri-react-phase10/`.
    - Verification PASS: frontend format/lint/typecheck, 96 Vitest tests,
      production budgets (shell 160.4/180 kB gzip, chat 37.1 kB gzip, total
      342.9/400 kB), Rust fmt/check/clippy, `agent_agui` 6, desktop lib 20,
      bridge session + 3 security + 7 DTO. Ledger row `agent.chat.prism` is
      `ported`. No new public Clay JS API or `init.js` option.

- [x] Phase 11 — Harden remote operation, platforms, packaging, updates, security, and performance
  - Acceptance Criteria:
    - Functional: Local, remote, container, multi-client, packaged install,
      launch, update-test-channel, and uninstall workflows operate through one
      client bridge. Linux is blocking; Windows/macOS regressions are avoided
      where practical.
    - Performance: Startup, memory, bundle, edit, tab, menu, SDUI, and agent
      budgets are measured in development and packaged builds.
    - Code Quality: Reproducible release artifacts include matching server and
      agent binaries, versions, licenses, and diagnostics.
    - Security: Tauri capability/CSP regression tests, updater signing,
      dependency audits, SBOM/license checks, secret storage, remote endpoint
      authentication, and process cleanup pass.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - [Tauri distribution](https://v2.tauri.app/distribute/)
      - [Tauri updater](https://v2.tauri.app/plugin/updater/)
      - [Tauri security](https://v2.tauri.app/security/)
      - `docs/development/security.md`
      - `docs/development/performance.md`
      - `docs/development/windows.md`
    - Options Considered:
      - Ship development web assets/server separately: not a production
        artifact. Rejected.
      - Bundle reviewed artifacts with explicit versions and signing: selected.
    - Chosen Approach:
      - Add packaged-build smoke before native deletion. Updater uses a test
        endpoint/channel first; signing keys stay outside repository.
      - Verify WebKitGTK behavior on Linux early and continuously. Do not defer
        Linux WebView issues until final packaging.
    - API Notes and Examples:
      ```text
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      npm --prefix frontend run typecheck
      npm --prefix frontend test
      npm --prefix frontend run build
      npm --prefix clay-agent test
      ```
    - Files to Create/Edit:
      - `src-tauri/tauri.conf.json`, `capabilities/*`: Release configuration.
      - `src-tauri/icons/*`: Application icons.
      - CI/release workflow files: Linux packages, audits, signing-input checks,
        smoke tests.
      - `scripts/package-smoke.sh`, `scripts/security-audit.sh`: Deterministic
        release checks.
      - `docs/development/build-and-test.md`, `security.md`, `performance.md`,
        `windows.md`: Transitional operational docs.
    - References:
      - `AGENTS.md` Linux blocking validation rules.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Packaged Linux install/launch/edit/reconnect/uninstall.
    - Capability and CSP negative suite.
    - Server/agent version mismatch and missing-artifact diagnostics.
    - Remote/container reconnect and multi-client isolation.
    - Updater rejects unsigned/wrong-target/wrong-version payloads.
  - Completion Evidence (2026-08-24):
    - `src-tauri/src/release.rs` owns local-only `CLAY_ENDPOINT`, sidecar
      lookup (`clay-server-<triple>`), typed missing-binary status, version
      identity, and `accept_update` (unsigned / wrong-target / non-newer).
      `tauri-plugin-updater` is not compiled; signing keys stay out of tree.
    - Supervisor adopts a live local server (container / `clay server`) and
      reaps only children it spawned. Network URLs never become IPC endpoints.
    - `tauri.conf.json` ships `icons/icon.png` and Linux `deb`/`rpm`/`appimage`
      targets. Capability/CSP suite now also pins version identity and the
      absence of updater artifacts.
    - `scripts/security-audit.sh` and `scripts/package-smoke.sh` are the
      release checks; CI runs clay-agent tests + package-smoke after the
      frontend production build. Install/uninstall is the host package
      manager; `CLAY_TAURI_BUNDLE=1` is the opt-in `.deb` build.
    - Docs: `security.md`, `performance.md` (180/400 kB gzip pins),
      `windows.md` (Tauri/WebView2, Linux-blocking), `build-and-test.md`
      Phase 11 baselines, wiki `desktop-release-hardening.md`.
    - Ledger: `performance.budgets.feel` and `platform.windows` → `ported`
      (not verified; Windows packaging is not a Linux CI gate).
    - Verification PASS: desktop lib 27, config_security 4, protocol
      documentation_coverage + wiki index, `tauri_react_bundle_budgets`,
      `scripts/package-smoke.sh` (incl. clay-agent 8 tests), `cargo fmt`,
      `cargo check --all-targets`, `clippy -D warnings`.

- [x] Phase 12 — Certify parity, cut over launch, and remove the native client
  - Acceptance Criteria:
    - Functional: Every parity-ledger row is `verified` or has a separately
      approved removal decision. Tauri becomes default desktop launch; server
      remains standalone. No production native UI path remains.
    - Performance: Replacement budgets pass in packaged Linux build; native
      benchmark removal occurs only after equivalent target coverage exists.
    - Code Quality: Masonry/Vello/Parley/winit/native client modules, local
      native UI patches, dead compatibility layers, and obsolete tests/fixtures
      are deleted. No permanent dual-client abstraction remains.
    - Security: Final audit confirms no broad webview capability, raw package
      UI execution, stale native bypass, secret leak, invalid archive access,
      or missing cleanup path.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/tauri-react-parity-ledger.md`
      - `Cargo.toml` native dependency/patch list.
      - `docs/development/architecture-ownership.md`
      - Current source maps under `src/client`, `src/driver`, `src/editor`,
        `src/shell`, and `src/masonry_*`.
    - Options Considered:
      - Keep native client behind a feature forever: doubles maintenance and
        hides parity gaps. Rejected.
      - Delete native client before parity: removes oracle and risks regression.
        Rejected.
      - Certify, cut over, then delete in one bounded phase: selected.
    - Chosen Approach:
      - Close ledger first. Switch launch/default-run. Delete native source and
        dependencies in reviewable groups while all target tests stay green.
      - Preserve historical plans/decision logs. Current docs are rewritten;
        history is marked superseded rather than altered.
    - API Notes and Examples:
      ```text
      Keep: clay server, server protocol, packages, deno_core, clay-agent
      Delete after parity: Masonry client, native editor/shell/driver,
      Vello/Parley/winit dependencies, local native UI patches
      ```
    - Files to Create/Edit:
      - `Cargo.toml`, `Cargo.lock`: Remove native UI dependencies/patches and set
        target launch structure.
      - `src/main.rs`, `src/launch.rs`, `src/cli.rs`: Tauri/default launch cutover.
      - `src/masonry_*`, `src/editor/*`, `src/shell/*`, `src/driver/*`,
        `src/client/*`, `src/app_driver.rs`: Delete or retain only server-neutral
        pieces moved during prior phases; exact list is ledger-driven.
      - `vendor/masonry_core`, `vendor/accesskit_atspi_common`,
        `vendor/accesskit_unix`: Delete after no dependency remains.
      - `benches/*`, `tests/*`, fixtures/scripts: Replace native-only coverage
        before deletion.
      - `docs/development/tauri-react-parity-ledger.*`: Record final evidence.
    - References:
      - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - Native-dependency/source/path absence guard.
    - Every ledger row requires passing evidence.
    - Default launch and standalone server smoke.
    - Full Linux Rust/frontend/Node/security/package gate.
  - Completion Evidence (2026-08-24):
    - All 18 parity-ledger capabilities are `verified` with named automated and
      retained manual/phase-review evidence. The ledger and primitive migration
      matrix now identify former native paths as historical inputs.
    - `clay` and `clay client` already route to `clay-desktop`; launch tests pin
      that behavior while `clay server` and smoke routes remain standalone.
    - Deleted Masonry shell/widgets, driver, native editor implementation,
      native clipboard/runtime compatibility, native-only integration tests and
      benchmarks, local Masonry/AccessKit patches, and their dependency graph.
      Renderer-neutral protocol/theme/package/layout compatibility modules remain.
    - Added `removed_native_client_modules_cannot_return`; converted catalog,
      typography, agent hot-path, API metadata, and suite inventory guards to
      React/Tauri ownership. Clay JS registry paths now point at retained command
      declarations instead of deleted native implementations.
    - Wiki: `docs/wiki/modules/tauri-react-cutover.md`. Current build, parity,
      migration, API inventory, generated registry, and package-extension docs
      reflect the cutover; historical plans/wiki records remain intact.
    - Verification PASS: Rust fmt/check/clippy; all Rust targets (1117 lib,
      30 presentation, 184 protocol, 68 runtime, 130 security, launch and server
      benchmarks); frontend typecheck, 96 Vitest tests, production build and
      budgets (shell 160.4/180 kB gzip, total 342.9/400 kB); clay-agent 8 tests;
      `scripts/security-audit.sh`; `scripts/package-smoke.sh`.
    - Security audit found no vulnerabilities and retained 19 explicitly allowed
      unmaintained/unsound transitive warnings, primarily Tauri Linux GTK3 plus
      existing V8/runtime dependencies. No removed native dependency or patch is
      present.

- [x] Perform visual screenshot and accessibility review of the complete Tauri/React UI
  - Acceptance Criteria:
    - Functional: Real Linux build exercises launch, loading, error/recovery,
      file/editor, completion, command/path centre, settings/package UI,
      tabs/splits, theme, Chat, empty, and disabled states at narrow/wide sizes.
    - Performance: Review records visible jank, slow transitions, editor input,
      long-list behavior, and agent streaming; observed regressions block parity.
    - Code Quality: Screenshot paths, accessibility dumps, steps, findings, and
      resolutions are retained under a dated review artifact directory.
    - Security: Review confirms secrets/absolute paths do not appear in UI,
      DOM accessibility names, or screenshots; focus cannot escape modals into
      unauthorized surfaces.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
      - `docs/development/accessibility.md`
      - `docs/wiki/modules/ui-review-harness.md`
    - Options Considered:
      - Structural tests only: not visual proof. Rejected.
      - Manual screenshots without accessibility tree: incomplete. Rejected.
      - Real representative UI plus screenshots, keyboard, and accessibility
        inspection: selected.
    - Chosen Approach:
      - Reload the complete mandatory project-local UI skill stack for this
        execution task. Launch packaged/debug Linux build. Start computer use
        with `get_app_state`, inspect accessibility
        tree, drive keyboard/pointer interactions, capture every listed state,
        and re-check focus/roles/names/states after transitions.
      - If tooling is unavailable, record exact blocker and leave acceptance
        unresolved.
    - API Notes and Examples:
      ```text
      code-reviews/screenshots/2026-xx-xx-tauri-react-parity/
        review-log.md
        editor-dark/
        editor-light/
        command-centre/
        tabs-splits/
        chat-streaming/
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/<dated-tauri-react-review>/*`: Evidence.
      - `docs/development/accessibility.md`: Implemented review contract.
      - `test-plan/*`: Findings and step results.
    - References:
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
  - Test Cases to Write:
    - React semantic companion tests for roles, labels, disabled state, live
      status, modal focus containment, and typed intent routing.
    - Keyboard-only focus-order and modal-restoration tests where a safe host
      input backend exists; this run records the host blocker instead of a
      false pass.
    - Reduced-motion, contrast, zoom/large typography, and screen-reader label
      checks through existing token/typography suites plus the retained AX
      snapshots; no new axe dependency was added.

  - Completion Evidence (2026-08-24):
    - Real Linux Tauri/React AT-SPI dumps are retained for the welcome shell,
      opened editor, tabs/splits, and Chat landing under
      `code-reviews/screenshots/2026-08-24-tauri-react-parity/`.
    - 20 app-only CDP fixture screenshots plus paired AX snapshots cover wide
      1440×900 and narrow 780×900 editor, intelligence, package UI, settings,
      Command Centre active/empty, Path Browser, Chat, splits, and combined
      loading/empty/error states. Full-desktop portal screenshots containing
      unrelated terminal/window content were deleted; no retained screenshot
      contains secrets or absolute paths.
    - Findings fixed: `ClayEditor` now sanitizes absolute fallback labels to a
      basename (`frontend/src/editor/ClayEditor.tsx`, regression in
      `frontend/src/test/editor.test.tsx`); the shell connection status is a
      polite live region (`frontend/src/app/layout/app-shell.tsx`, regression
      in `frontend/src/test/shell.test.tsx`); the path fixture no longer uses
      an absolute display path. The low-priority `Theme Theme` placeholder
      announcement is recorded as a follow-up.
    - `scripts/capture-ui-review.sh` now recognizes the `clay-desktop` AT-SPI
      application and the current `Clay workspace` landmark, and tracks the
      Tauri child so cleanup cannot leave a review-owned desktop process.
      Component, bridge, server, package, and security checks remain green.
    - Verification PASS: Rust fmt/check/clippy and all targets (1117 lib,
      4 launch, 30 presentation, 184 protocol, 68 runtime, 130 security);
      frontend format/lint/typecheck and 99 Vitest tests; frontend budgets
      160.6/180 kB shell gzip and 343.2/400 kB total; security/package smoke;
      Clay Agent 8/8.
    - Interactive acceptance is explicitly `UNRESOLVED`, not inferred: this
      host denied `/dev/uinput`, has no `xdotool`/`ydotool`, and its Wayland
      portal cannot target Clay. Completion, Command Centre/path activation,
      native dialogs, settings actions, and tab/pane keyboard re-runs require
      a safe keyboard/window-targeting backend. Current role/name/disabled/
      containment semantics pass static AX and React component tests.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Every public behavior changed by migration retains or gains a
      documented Clay JS API; frontend/Tauri internals do not become accidental
      public APIs.
    - Performance: Public APIs preserve asynchronous/bounded behavior and do not
      expose editor hot-path synchronous calls.
    - Code Quality: Core IDs use bare `<domain>.<name>`; package IDs use package
      prefixes; docs include names, bindings, properties, examples, errors,
      permissions, Rust/op/facade paths, and lookup tags.
    - Security: APIs do not expose Tauri handles, webview internals, raw ops,
      filesystem/process handles, credentials, or package trust promotion.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
    - Options Considered:
      - Expose Tauri commands as public package API: wrong trust boundary.
        Rejected.
      - Preserve server facades and keep frontend bridge internal: selected.
    - Chosen Approach:
      - Inventory changed `pub` Rust functions. Expose only true public
        capabilities through explicit `deno_core` ops and stable facades; make
        implementation helpers private or `pub(crate)`.
      - Update Markdown-authoritative docs and generated registry.
    - API Notes and Examples:
      ```text
      Core command ID: runtime.reloadConfiguration
      Package command ID: chat.submit
      Import specifier: clay:configuration
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`: Updated API pages.
      - `docs/reference/clay-js-api/api-inventory.toml`: Inventory.
      - `docs/index.md`: Links.
      - `docs/generated/clay-js-api-registry.json`: Regenerated artifact.
      - `runtime/js/*.js`, `runtime/js/*.d.ts`: Facades/declarations if changed.
      - `src/server/ops/*`, `src/packages/manifest.rs`: Backing ops/reserved
        domains only when required.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - Registry freshness and complete metadata.
    - Rust public-function/API coverage.
    - Frontend/Tauri internals absent from package API inventory.
    - Lookup by ID/name/tag/path returns migrated APIs.

  - Completion Evidence (2026-08-24):
    - Verification: the generated registry, Markdown frontmatter, facades, and
      ops were audited against the retained (React/Tauri) owner set. All
      registry-backed APIs keep valid `facade_path`, `deno_op_path`,
      `backing_rust`, and documentation paths; no Tauri command, webview
      channel, or frontend type became a Clay JS API; no raw op name leaks
      into js exports; lookups by ID, js export, user-facing name, lookup tag,
      kind/owner, and key binding all resolve to the migrated entries
      (`src/docs/registry.rs`, `tests/clay_js_api_inventory.rs`,
      `tests/clay_js_doc_registry.rs`, `tests/primitives_docs.rs`).
    - Migration cleanup: `api-inventory.toml` previously still cited deleted
      native-client modules in its unvalidated metadata. All 64
      `current_rust_owner` rows referencing `masonry_*`, `editor/surface`,
      `editor/buffer|viewport|layout`, `app_driver`, and `driver/restore`
      now name the retained renderer-neutral owner
      (`src/client_commands.rs::EditorClientCommand`/`ShellClientCommand`,
      `src/server/*`, `src/shell/layout.rs`) plus the React/CodeMirror
      executor (`frontend/src/editor/extensions/controller.ts`,
      `frontend/src/shell/workspace-controller.ts`, `frontend/src/shell/
      PaneTree.tsx`, `frontend/src/sdui/renderer.tsx`,
      `frontend/src/theme/adapter.ts`, `frontend/src/app/layout/tab-bar.tsx`)
      where the behavior is client-local; one stale `backing_rust`
      (`configuration.setModePreference` → `src/server/modes.rs`) points at
      `src/packages/modes.rs::ModeRegistry`; the internal
      `masonry-paint-layout-hot-path` runtime path and its hot-path policy
      are now `client-paint-layout-hot-path`.
    - API page cleanup: all 94 `docs/reference/clay-js-api/**` pages now have
      zero references to removed native modules, renderer names (Masonry,
      Parley, Vello), `reconcile_pane_hosts`, or native widget components;
      "Backing implementation" sections name the retained Rust owner and the
      React/CodeMirror executor. `docs/generated/clay-js-api-registry.json`
      was regenerated with `cargo run --bin update-doc-registry` (frontmatter
      security/guidance wording flows into the derived registry).
    - Durable guard: `tests/clay_js_api_inventory.rs::
      inventory_rust_paths_name_existing_source_files` asserts every
      `src/*.rs` path named in inventory `backing_rust`/`current_rust_owner`
      exists, so a future edit cannot resurrect deleted native-module
      references in the inventory.
    - Verification PASS: `cargo fmt --check`, `cargo clippy --all-targets`
      `-- -D warnings`, and `cargo test --all-targets` (1117 lib, 4 launch,
      30 presentation, 185 protocol, 68 runtime, 130 security; protocol suite
      includes the new guard).

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Existing `init.js` configuration, package loads, keybindings,
      themes, typography, appearance, editor behavior, and migration-relevant
      options retain documented behavior.
    - Performance: Configuration reload remains background/candidate-based and
      installs one atomic bounded frontend snapshot.
    - Code Quality: Every behavior-changing option is a documented Clay JS API
      custom property; no undocumented frontend local-storage preference is
      introduced.
    - Security: Configuration does not implicitly grant Tauri filesystem,
      network, shell, process, extension, AI mutation, or workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/wiki/modules/configuration-runtime.md`
      - `docs/reference/clay-js-api/configuration.md`
    - Options Considered:
      - Add a frontend-only settings store: splits authority. Rejected.
      - Keep `init.js`/server preferences canonical and project into React:
        selected.
    - Chosen Approach:
      - Audit every settings control and frontend option against canonical
        configuration APIs. Persist only existing approved UI-session state;
        version any browser storage and keep it non-authoritative.
    - API Notes and Examples:
      ```js
      loadPackage("@clay/markdown");
      loadPackage("@clay/chat");
      ```
    - Files to Create/Edit:
      - `runtime/js/configuration.js`, `configuration.d.ts`: APIs if changed.
      - `src/server/configuration.rs`, `config_watch.rs`: Canonical behavior.
      - `frontend/src/configuration/*`: Projection only.
      - `docs/reference/clay-js-api/configuration*`: Public docs.
      - `docs/generated/clay-js-api-registry.json`: Regenerated.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - Successful/failed reload atomicity and old-generation preservation.
    - Unknown/hidden option denial.
    - Settings UI and `init.js` precedence parity.
    - Frontend storage cannot grant authority or override canonical config.

  - Completion Evidence (2026-08-24):
    - Verification across the four test cases:
      - Reload atomicity/old-generation preservation: `reload_runtime_generation`
        (`src/server/mod.rs`) returns `active_generation_id = previous_generation_id`
        with `reloaded: false` when configuration evaluation or candidate
        validation fails, and the config watcher debounces rapid saves and
        reloads changes landing during a reload (`src/server/config_watch.rs`).
        Tests: `reload_runtime_generation_swaps_only_after_successful_
        configuration_load`, `reload_reruns_init_js_package_load_in_fresh_
        generation_and_preserves_old_on_failure`, `watcher_debounces_rapid_
        saves_into_one_reload`, `watcher_reloads_for_a_change_that_lands_
        during_a_reload`, `configuration_watcher_reloads_changed_root_without_
        command_intent`.
      - Unknown/hidden option denial: `src/server/configuration.rs` rejects
        non-local specifiers, prohibited authority values, watcher-control
        keys, editor-default keys, and hidden/ad-hoc/raw-authority package
        options (tests: `package_option_configuration_rejects_hidden_ad_hoc_
        and_raw_authority_keys`, `phase28_editor_defaults_are_not_configuration_
        keys`, `configuration_rejects_watcher_control_keys`, `plan060_internal_
        security_and_performance_controls_are_not_configurable`); the
        preferences store is a closed 8KiB-bounded three-key object
        (theme/appearance/typography) whose unknown, corrupted, oversized, or
        non-object payloads are dropped field-by-field/wholesale with
        diagnostics.
      - Settings UI ↔ `init.js` precedence parity: `apply_persisted_
        preferences` runs immediately after `init.js` evaluation on every
        startup/reload, so UI choices win for the three preference keys
        (`ui-session` > `init-js` > canonical/package default); explicit
        `setTheme` beats appearance-derived defaults. Tests:
        `explicit_set_theme_wins_over_appearance`, `explicit_set_theme_wins_
        over_canonical_default`, `set_typography_replaces_all_profiles_
        atomically`, `absent_init_js_loads_no_runtime_theme`, and the
        `settings.setTypography` persist+reload assertion in
        `src/server/connection/mod.rs`. `settings.setTheme`/`setAppearance`/
        `setTypography` validate → persist atomically (tmp+rename) → reload
        the runtime generation; `settings.reset` clears the store and reloads;
        `settings.open`/`settings.close` acknowledge only
        (`src/server/connection/runtime.rs::persist_settings_change`). The
        React SettingsPanel is projection-only: it sends typed bounded intents
        (`settings.setTheme id`, `settings.setAppearance id`,
        `settings.setTypography { typography: JSON }`, `settings.reset`) and
        disables Apply on incomplete/invalid input
        (`frontend/src/settings/SettingsPanel.tsx` + tests).
      - Frontend storage cannot grant authority: new durable guard
        `frontend_has_no_browser_storage_authority` in
        `tests/documentation_coverage.rs` forbids `localStorage`,
        `sessionStorage`, `indexedDB`, and `document.cookie` in
        `frontend/src` and `src-tauri/src`; a live scan confirms zero browser
        storage today — all persistent UI state flows through typed `settings.*`
        intents into the server-authoritative `preferences.json`.
    - Stale documentation fixed: `docs/reference/clay-js-api/configuration.md`
      still claimed `settings.setTypography` "does not yet persist" (pre-
      Phase 9 wording). Updated the precedence table and the settings command
      flow paragraph to the implemented behavior (parse → full revalidation →
      atomic persist → reload). No other stale claims found in the API docs or
      `docs/wiki/modules/configuration-runtime.md`.
    - Registry/docs surface: `clay:configuration` module and the three
      configuration-entrypoint APIs (`configuration.loadConfigurationModule`,
      `configuration.getConfigurationState`, `setPackageOption`) remain
      documented and registry-generated; docs bindings/closed-option checks
      pass (`phase28_configuration_apis_have_documented_bindings_and_closed_
      options`, `denied_configuration_authorities`, `set_typography_api_doc_
      has_required_configuration_metadata` in `tests/clay_js_doc_registry.rs`).
      No new configuration API was needed: watcher behavior is deliberately
      automatic server behavior with no `watch*` JS API, and appearance/theme/
      typography stay `clay:theme` exports; the `clay:configuration` module
      remains closed.
    - Verification PASS: `cargo fmt --check`, `cargo clippy --all-targets
      -- -D warnings`, `cargo test --all-targets` (30 presentation, 186
      protocol — includes the new storage guard, 68 runtime, 130 security;
      lib 1117), frontend 99/99 Vitest.

- [x] Update the canonical example configuration (`examples/init.js`)
  - Acceptance Criteria:
    - Functional: Canonical example contains each supported migrated
      configuration surface exactly once, including first-party package loads,
      keybindings, themes, typography, and agent setup guidance.
    - Performance: Heavy/environment-specific package and LSP setup stays
      commented; default copy does not trigger unnecessary work.
    - Code Quality: Option names, types, defaults, enums, and ordering match
      server validators and API inventory; JavaScript parses successfully.
    - Security: Active copy-safe section contains no credentials, broad grants,
      remote endpoints, or unsafe package adoption.
  - Approach:
    - Documentation Reviewed:
      - `examples/init.js`
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `.agents/skills/create-plan/references/clay.md`
    - Options Considered:
      - Create separate web-client config: duplicates canonical config.
        Rejected.
      - Keep one `init.js` example and annotate frontend-neutral behavior:
        selected.
    - Chosen Approach:
      - Update existing ordered sections and remove native-only wording. Keep
        migration internals out of user config.
    - API Notes and Examples:
      ```js
      loadPackage("@clay/chat");
      loadPackage("@clay/markdown");
      ```
    - Files to Create/Edit:
      - `examples/init.js`: Canonical configuration.
      - `examples/packages/first-party.js`: First-party package examples.
      - `examples/packages/third-party.js`: Trust-domain examples if affected.
    - References:
      - `decision-logs/2026-08-04-1623-canonical-example-config-and-plan-maintenance-duty.md`
  - Test Cases to Write:
    - `node --check` for every example JavaScript file.
    - Canonical example/API inventory option parity.
    - Active default configuration loads without environment-specific grants.

  - Completion Evidence (2026-08-24):
    - Added agent setup guidance: `examples/init.js` gains section 12
      documenting the server-owned clay-agent host (@clay/chat landing loaded
      in `packages/first-party.js`; no `agent*`/`chat*`/`provider*` config API
      exists; credentials live in the encrypted vault/OS keychain and must
      never be pasted into config; LSP tooling setup referenced to
      first-party.js). Every configuration surface still appears exactly
      once: theme (one active `setTheme`), typography (one active atomic
      `setTypography`), keybindings (batch + single-form tables), editor
      cursor style (one active call), editor layout (one active call), pane
      focus policy, syntax engine preference, modular config loads, package
      loads — as pinned by the existing doc-registry tests.
    - Removed native-only wording: scan of `examples/` shows zero Masonry /
      Parley / Vello / winit / native-client terms; no migration internals in
      user config.
    - `node --check` gate: new step in `scripts/package-smoke.sh` runs
      `node --check` on `examples/init.js`, `examples/packages/first-party.js`,
      and `examples/packages/third-party.js` (skips with a notice when node is
      absent locally; runs in CI which provides node 24). All three parse.
    - Option/enum/inventory parity: new test
      `canonical_example_cross_checks_remaining_configuration_options_
      against_inventory` (tests/clay_js_doc_registry.rs) cross-checks
      `clientSetCursorStyle` (shape/blink enums + widthPx/heightPct/hollow/
      stopBlinkOnTyping), `setPaneFocusPolicy` (click|cursor), and
      `setSyntaxEnginePreference` (target/tier; native|wasm|javascript|js)
      annotations against `custom_properties` in api-inventory.toml; extends
      the existing editor-layout cross-check. Registry-driven, not prose.
    - Copy-safe active config: new test
      `canonical_example_active_configuration_is_copy_safe` strips comments
      and asserts every EXECUTED line of all three example files contains no
      remote endpoints (`://`), credentials (password/api_key/apikey/secret/
      Bearer/BEGIN), or raw-adoption idiom (`github:`). Active config is
      thereby grant-free; language-server grants stay in the fault-isolated
      optional module, wrapped in try/catch, failing closed when tooling or
      roots are absent (no credentials, broad grants, or unsafe adoption in
      the copy-safe path). Active default configuration loads without
      environment-specific work: grants stay inert until
      `startLanguageServerSession` matches them, and the chat load lines are
      pinned by existing tests (`first_party_example_loads_chat_with_one_
      uncommented_line`).
    - Verification PASS: `node --check` (3 files), `cargo fmt --check`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`
      (188 protocol — now includes both new example tests, 30 presentation,
      68 runtime, 130 security, 1117 lib), and `scripts/package-smoke.sh`
      (includes the new example-syntax step). One transient ETXTBSY sandbox
      fixture failure observed and confirmed unaffected on rerun (known Linux
      flake, also recorded in Phase 9 evidence).

- [x] Execute and update the manual test plan (`test-plan/`)
  - Acceptance Criteria:
    - Functional: Every affected module executes on a real Linux Tauri build;
      new web/Tauri behavior has numbered steps, expected results, negative
      checks, and known ceilings. No existing behavior step is weakened or
      deleted to claim parity.
    - Performance: Module 11 records packaged/editor/menu/tab/agent measurements
      and subjective interaction checks against documented budgets.
    - Code Quality: Index coverage matrix maps Tauri/React changes to modules;
      native-only setup is removed only after target replacements exist.
    - Security: Manual checks cover capability denial, secret/path redaction,
      remote/tab/package isolation, modal focus, and untrusted UI boundaries.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `test-plan/index.md`
      - `test-plan/01-launch-and-connection.md` through `14-tabs.md`
      - `docs/development/tauri-react-parity-ledger.md`
    - Options Considered:
      - Add one migration-only test file: hides feature-level parity. Rejected.
      - Update existing behavior modules and add a packaging module only if
        needed: selected.
    - Chosen Approach:
      - Execute modules 01–11, 13, and 14 on Linux; module 12 remains Windows
        guidance unless Windows execution is explicitly available. Add a new
        packaging/update module only if current modules cannot hold those steps.
      - Record exact screenshot/accessibility/performance evidence paths.
    - API Notes and Examples:
      ```text
      Step E-Tauri-1: Type Unicode text while bridge consumer is paused.
      Expected: Local glyph/caret update is immediate; queued edit later acks.
      Negative: No full-document frame and no renderer freeze.
      ```
    - Files to Create/Edit:
      - `test-plan/index.md`: Coverage matrix and execution record.
      - `test-plan/01-launch-and-connection.md`
      - `test-plan/02-configuration-init-js.md`
      - `test-plan/03-files-and-workspace.md`
      - `test-plan/04-core-editing.md`
      - `test-plan/05-movement-and-selection.md`
      - `test-plan/06-multi-cursor.md`
      - `test-plan/07-caret-and-typography.md`
      - `test-plan/08-syntax-and-textobjects.md`
      - `test-plan/09-packages-and-modes.md`
      - `test-plan/10-keybindings-and-commands.md`
      - `test-plan/11-performance.md`
      - `test-plan/12-platform-windows.md`
      - `test-plan/13-window-splits.md`
      - `test-plan/14-tabs.md`
      - `test-plan/15-packaging-and-updates.md`: Create only if packaging steps
        cannot be represented cleanly in module 01.
    - References:
      - `decision-logs/2026-08-04-1645-manual-test-plan-folder-and-per-plan-duty.md`
  - Test Cases to Write:
    - Manual execution itself is the test; automated companions are linked per
      step and must pass before a step is marked complete.

  - Completion Evidence (2026-08-24):
    - Fresh execution on the real Linux Tauri build via
      `scripts/capture-ui-review.sh` (isolated config/data/workspace):
      `ui-review-default` PASS (full AT-SPI structure: `clay-desktop` frame,
      `Clay workspace`, `Window tabs` tab list, `Pane 1`, named Open
      File/Folder actions, status bar) and `ui-review-large-typography` PASS.
      `ui-review-command-centre` UNRESOLVED (interactive state needs a TTY —
      documented host keyboard ceiling). `ui-review-error` UNRESOLVED with a
      NEW finding: WebKitGTK does not expose static text inside the footer/
      live region as AT-SPI accessible names or Text-interface content
      (verified with a targeted AtspiText probe; even the `Connected` status
      text is invisible), so name-based dumps can never see the sanitized
      runtime diagnostic; diagnostic delivery itself stays covered by
      automated tests. Recorded in index.md with an AT-SPI exposure follow-up.
    - Module 11 re-measured on a fresh production build: shell 160.6/180 kB
      gzip, total 343.2/400 kB gzip, no budget raised; suite timings recorded;
      clay-agent structural record added.
    - Missing post-cutover records added for modules 05 (movement/selection),
      06 (multi-cursor): automated-equivalent PASS + keyboard-UNRESOLVED
      status; module 12 (Windows): NOT EXECUTED on Linux host per policy.
    - Stale native-era references replaced with current equivalents — no step
      weakened: module 01 L20/L21 and module 07 T18/T19 (deleted Masonry
      rescale/live-smoke tests → manual scale checks + retained captures),
      module 04 E35 evidence pointer (removed native editor a11y test →
      CodeMirror built-ins + frontend editor tests), module 13 a11y
      equivalents (deleted TreeUpdate tests → React shell live-region
      assertions + new static-text AT-SPI ceiling note), module 14 deep
      reference (`react-tabs-and-splits.md`) and overflow automated-
      equivalents (tab-bar CSS clamp + Phase 12 captures), module 10 SDUI
      region deep reference (`react-sdui-package-ui.md`), index.md wording.
    - Index coverage matrix extended with Plan 097 rows mapping Phases 5–7
      and 10–12 to modules (8/9 rows already existed); new dated execution
      record section summarizing fixture results, budgets, findings, and
      follow-up.
    - Verification PASS: `cargo fmt --check`, `cargo clippy --all-targets
      -- -D warnings`, `cargo test --all-targets` (1537 tests total across
      lib/presentation/protocol/runtime/security suites, including the
      ledger-coverage tests that parse every test-plan step table).

- [x] Rewrite current-state architecture, development, package, security, performance, and contribution documentation
  - Acceptance Criteria:
    - Functional: Current documentation describes only the implemented
      Tauri/React architecture, separate server, CodeMirror editor, React SDUI,
      AG-UI transport, theme projection, package trust model, build/test flow,
      and platform behavior. Historical plans/logs remain historical and are
      marked superseded where necessary.
    - Performance: Docs state measured frontend/bridge/editor/bundle budgets and
      hot-path ownership; obsolete Masonry paint budgets are removed only after
      replacement coverage.
    - Code Quality: Source paths, commands, diagrams, examples, API references,
      package authoring instructions, and ownership tables match final code.
      Deterministic checks reject stale current-state claims.
    - Security: CSP, Tauri capabilities, server authority, third-party UI
      isolation, package trust domains, secrets, remote endpoints, and updater
      trust are documented truthfully.
  - Approach:
    - Documentation Reviewed:
      - `docs/index.md`
      - `docs/development/*`
      - `docs/reference/*`
      - `.agents/skills/clay-ui/*`
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/*`
      - `AGENTS.md`, `README.md` if present, `concept.md`, package READMEs.
    - Options Considered:
      - Bulk search/replace Masonry→React: preserves false ownership details.
        Rejected.
      - Rewrite by documented behavior/owner and enforce stale-term checks:
        selected.
    - Chosen Approach:
      - Maintain a documentation migration inventory linked to parity rows.
        Rewrite current-state pages after corresponding code lands. Final phase
        scans all non-historical docs for obsolete architecture claims and dead
        source paths.
      - Decision logs remain immutable. Add supersession notes through new
        decisions/index metadata rather than editing historical rationale.
    - API Notes and Examples:
      ```text
      Current-state forbidden after cutover:
      - Masonry/Vello/Parley client ownership claims
      - package UI maps to native Masonry widgets
      - first-party path has no AG-UI
      - native source paths removed from repository
      ```
    - Files to Create/Edit:
      - `docs/index.md`
      - `docs/development/architecture-ownership.md`, `build-and-test.md`,
        `accessibility.md`, `security.md`, `performance.md`, `windows.md`,
        `launch-and-gui-smoke.md`, and affected workflow docs.
      - `docs/reference/ui-components.md`, `docs/reference/primitives/**`,
        `docs/reference/packages/creating-packages.md`, affected Clay JS API docs.
      - `.agents/skills/clay-ui/SKILL.md`, `references/components.md`,
        `references/tokens.md`.
      - `.agents/skills/create-plan/references/clay.md`.
      - `.agents/skills/project-patterns/references/*`: Remove transitional
        native wording after cutover while preserving stable rules.
      - `AGENTS.md`, `concept.md`, `clay-agent/README.md`, package docs/readmes,
        and root README if one exists at execution time.
      - `tests/documentation_coverage.rs`, `tests/primitives_docs.rs`,
        `tests/clay_js_doc_registry.rs`: Drift checks.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - All current doc links/source paths resolve.
    - Generated registry is fresh.
    - Current docs contain no obsolete architecture assertions.
    - UI catalog/token/component code and package guide remain synchronized.
    - Historical files are excluded only by explicit path classification.

  - Completion Evidence (2026-08-24):
    - Rewrote by behavior/owner, not search/replace. Current-state docs now
      describe only the implemented Tauri/React architecture:
      `architecture-ownership.md` client table replaced with the desktop
      ownership map (Supervisor, Tauri bridge modules, AppShell,
      WorkspacePanes/workspace controller, ClayEditor/extensions, SduiRenderer/
      PackageWorkspace, ChatPanel transport, retained neutral Rust modules)
      with current hot-path/budget table (incl. frontend bundle budgets);
      `ui-observability.md` rewritten around frontend Vitest suites +
      server-side SDUI validation + capture-ui-review harness (with the
      WebKitGTK static-text AT-SPI ceiling recorded); security.md dependency
      posture rebuilt for the post-cutover lockfile (0 vulnerability
      exceptions; quick-xml RUSTSEC-2026-0194/-0195 retired; classified GTK3
      Linux shell chain + deno_core chain warnings); `.cargo/audit.toml`
      ignores emptied accordingly (test updated to allow the zero-exception
      state); performance.md stale Masonry benches/tests replaced with
      historical notes plus retained deterministic gates and a reconstructed
      Tauri/React budgets section (shell 160.6/180 kB, total 343.2/400 kB).
    - Reference docs updated to the React/renderer-neutral boundary:
      rendering-strategy.md attachment points + Phase 26 axes carriers,
      shell-layout-strategy.md substrate/runtime baselines, ui-chrome-
      primitives.md state-color helpers as CSS-class mapping, registry.md,
      audit.md, backlog.md, package-security.md, package-loading.md,
      typography.md, diagnostics.md, parse-update-strategy.md,
      implementation-gate.md, markdown-mode-requirements.md,
      ui-components.md, creating-packages.md, docs/index.md, windows.md,
      file-open-save-reload-workflow.md, launch-and-gui-smoke.md platform
      tables (Tauri dialog commands, webview clipboard/IME). concept.md
      carries an explicit historical-vision banner.
    - Wiki: index entries + in-page banners classify removed native modules
      as historical (masonry-sdui-region, driver, pane-document-views,
      editor-chrome-and-layout, rendering-primitives, shell-primitives
      already marked) and point at their React successors;
      client-file-dialog.md notes the retained neutral types vs replaced
      native backends. Dated historical records elsewhere remain dated.
    - New deterministic drift guard:
      `current_state_docs_reject_removed_native_architecture_terms` in
      tests/documentation_coverage.rs scans README/docs current-state set
      for masonry/vello/parley/winit/accesskit/ClayShellWidget/EditorWidget/
      PaneDocumentView/PackageOverlayHost/EditorSurface; exclusions exist
      only via an explicit historical list whose banners are asserted.
      documentation-contracts.json stale marker updated
      ("no package JavaScript runs on the client hot path").
    - Verification PASS: cargo fmt --check, cargo clippy --all-targets
      -- -D warnings, cargo test --all-targets (1538 tests), protocol suite
      189 incl. registry freshness + ledger coverage, frontend typecheck,
      99 Vitest tests, production build within budgets (160.6/180 kB shell,
      343.2/400 kB total gzip), scripts/package-smoke.sh.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The code wiki is updated after all implementation tasks are
      complete and teaches the final Tauri/React architecture, bridge, editor,
      shell, SDUI, package UI, themes, AG-UI, security, testing, and migration
      outcome.
    - Performance: Wiki pages document hot paths, queues, render boundaries,
      bundle/loading policy, and measured budgets without adding runtime work.
    - Code Quality: Pages explain responsibilities, data flow, state machines,
      invariants, tradeoffs, source/test paths, and extension guidance; every
      page is linked from `docs/wiki/index.md`.
    - Security: Pages document capabilities, CSP, server authority, package
      trust domains, third-party UI isolation, agent/credential boundaries, and
      updater/remote trust without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/project-wiki/references/page-template.md` if present.
      - Existing relevant pages linked from `docs/wiki/index.md`.
    - Options Considered:
      - Update after each subtask: likely to describe transitional code as
        final and churn heavily.
      - Update once after implementation, parity, and docs pass: selected.
    - Chosen Approach:
      - Rewrite or replace native implementation pages with final module/flow
        pages, preserve historical context through links, and update the master
        index once after tests pass.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/tauri-desktop-shell.md
      docs/wiki/modules/react-client-bridge.md
      docs/wiki/modules/codemirror-editor.md
      docs/wiki/modules/react-sdui-renderer.md
      docs/wiki/flows/frontend-edit-synchronization.md
      docs/wiki/flows/ag-ui-tauri-stream.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Final navigation.
      - `docs/wiki/modules/tauri-desktop-shell.md`
      - `docs/wiki/modules/react-client-bridge.md`
      - `docs/wiki/modules/codemirror-editor.md`
      - `docs/wiki/modules/react-shell-tabs-panes.md`
      - `docs/wiki/modules/react-sdui-renderer.md`
      - `docs/wiki/modules/frontend-theme-runtime.md`
      - `docs/wiki/modules/ag-ui-agent-transport.md`
      - `docs/wiki/flows/frontend-edit-synchronization.md`
      - `docs/wiki/flows/ag-ui-tauri-stream.md`
      - Existing wiki pages whose current ownership/source paths changed.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Wiki navigation test: Every wiki page is linked and every link resolves.
    - Source/test path validation for new pages.
    - Manual onboarding review: A new developer can trace launch, edit, SDUI,
      package, theme, and agent flows from the index.

  - Completion Evidence (2026-08-24):
    - New final-architecture pages: `modules/tauri-desktop-shell.md`
      (src-tauri process shell: adopt-or-spawn Supervisor, narrow invoke
      surface, dialog mutexes, fail-closed endpoint, CSP/capability posture),
      `modules/react-client-bridge.md` (frontend bridge client, single-flight
      bootstrap, connection view model, envelope routing),
      `modules/frontend-theme-runtime.md` (snapshot→`--clay-*` projection,
      density/z-level/editor-style rules, install-once policy);
      `flows/frontend-edit-synchronization.md` (keystroke→CodeMirror→
      origin-filtered optimistic edit→ack/reject/resync with invariants) and
      `flows/ag-ui-tauri-stream.md` (agent wire→AG-UI adaptation in Rust→
      relay channels→AbstractAgent; validated request path for prompts).
    - Existing pages already teaching editor/shell/SDUI/chat/cutover were
      verified current (react-codemirror-editor, react-tabs-and-splits,
      react-sdui-package-ui, react-agui-chat-stream, desktop-typed-bridge,
      tauri-react-cutover); native-era flow pages (client-edit-emission,
      client-server-edit-ack, versioned-text-synchronization,
      document-leases-and-region-locks) now carry explicit historical banners
      pointing at the current flow pages; index updated with the three new
      module entries and annotated flow entries.
    - New deterministic guard `wiki_navigation_is_complete_and_current_
      page_paths_resolve` in tests/documentation_coverage.rs: every wiki page
      is linked from the master index, every intra-wiki `.md` link resolves,
      and the five new/current pages may only name existing source/test
      paths. Fixed two pre-existing broken links it caught
      (test-plan deep link depth, clay-ui components.md depth).
    - Verification PASS: cargo fmt --check, clippy --all-targets -D warnings,
      cargo test --all-targets (1539 tests incl. the new guard).

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
