# Phase 25 Agent Host and Pane Content Primitive Review

## Source

- `plans/096-Phase25-AI-Native-Prism-Host-and-Chat.md`
- `roadmap.md` Phase 25, Phase 22 pane-host, Phase 29
- `src/masonry_pane_host.rs`
- `src/masonry_package_region.rs`
- `src/masonry_welcome.rs`
- `src/masonry_pane_document.rs`
- `src/packages/manifest.rs` (`extends` / `replaces` / `extensionPoints`)
- `packages/settings/` (first-party SDUI package precedent)
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/wiki/modules/{primitive-architecture.md,masonry-shell.md,masonry-sdui-region.md,third-party-runtime-authority.md}`
- `/home/arn/Projects/prism/docs/{agent-session-runtime.md,agent-events.md,agent-definitions.md,credential-storage.md}`

## Overview

This review completes Phase 25 Task 1 before Prism host or agent UI work.
No runtime code or protocol was changed by this review.

2026-08-22 correction: the first draft treated Chat as an irreplaceable
Clay-native `PaneContent::Agent`. That fights Clay’s rule that product
surfaces are packages. Inventory below is the corrected split.

Reuse-first split:

- Clay already has a generic pane host, package SDUI (`PackageRegionWidget`),
  Command Centre, two package-runtime trust domains, and `extends`/`replaces`.
  Shell-layout-strategy already notes the pane-content contribution path is
  **not yet public**.
- Chat must not reuse `EditorSurface` / `PaneDocumentView` as a fake
  transcript/composer.
- Product landing and agent profiles are first-party packages:
  `@clay/chat` now, `@clay/coding-agent` in Phase 29. Third-party packages
  replace or extend them through the existing graph + user approval.
- Prism execution belongs in one Clay-core-owned Node >= 20 `clay-agent`
  child. It is **not** a Clay JS package. Packages never spawn or speak to
  it; they call documented `agent.*` APIs.

## Authority Map

| Concern | Owner | Boundary |
| --- | --- | --- |
| Pane topology, catalog widgets, tokens, Command Centre, tab bar, file dialogs | Clay core | Packages declare inert SDUI; no Masonry handles, raw CSS, client JS. |
| Empty-tab `main` content | Loaded pane-content contribution, else core fallback | One winner. `@clay/chat` is the bundled default. `replaces` withdraws it. |
| Chat greeting, chrome, composer tree, Chat `AgentDefinition` | `@clay/chat` | Trusted bundled runtime. Unload restores fallback. |
| Coding Agent profile, tools, tool UX | `@clay/coding-agent` (Phase 29) | Separate package; `extends`/`replaces` with declared points + approval. |
| Agent session identity, transcript, selected provider/model | Clay server | Client/package project snapshots; no client-side provider SDK. |
| Prism, session store, credential vault | `clay-agent` Node child | Direct stdio JSON-RPC from the Clay server only. |
| Composer buffer, focus, local paint | Native `textArea` (catalog) | Keystrokes never wait on IPC, JS, daemon. |
| Package runtime | Two Deno domains | Third-party replacement stays third-party. Core/`clay-agent` not replaceable. |
| Credentials | Host vault + Prism resolvers | No secrets in snapshots, menus, a11y, logs. |

## Existing Primitive Inventory

### Generic pane hosting

`PaneContentHost` owns one content host per leaf. `PaneContent` is currently
`Placeholder | Editor | Document`. The source comment reserves this enum for
future terminal-like content. Phase 22 documents that the **pane-content
contribution path is not public**; packages cannot contribute pane content
today.

**Reuse:** pane hosts, split/slot geometry, theme/typography, shell a11y,
`PackageRegionWidget` for SDUI.

**Gap:** add a generic `Package` (or equivalent) variant that hosts a
validated package region in `main` for empty/new tabs. Do **not** add a
product-named `Agent` kind and do not add a trait-object `Custom`. Terminal
remains a later distinct kind (PTY ≠ SDUI).

### Document/editor primitives

`PaneDocumentView` / `EditorSurface` are complete for editor panes. They are
not a transcript store or chat composer. Do not route Prism messages as
`EditOperation`s or reuse `DocumentSessionStore` for agent sessions.

**Gap:** bounded client projection keyed by server agent-session/run IDs.

### Welcome entry surface

`WelcomeWidget` is the nearest native UI precedent (token chrome, Open File /
Open Folder commands). Plan 087 made it irreplaceable product chrome.

**Reuse:** button routes, theme, a11y, file/folder flows.

**Gap / correction:** keep Welcome as the **zero-package core fallback**.
Default product landing is `@clay/chat`’s pane-content contribution. Reverse
“packages cannot replace welcome” for that product surface only. Tab bar,
Command Centre, and native dialogs stay host-owned.

### Package UI, graph, trust domains

`serverRegisterPanelContribution` covers left/right/top/bottom, not empty
`main`. `@clay/settings` shows first-party inert SDUI. Manifest already has
`extensionPoints`, `extends`, `disables`, `replaces`. Two Deno domains:
trusted bundled vs shared third-party. Replacement never enters trusted
runtime.

**Reuse:** SDUI catalog, package graph, adoption overlay, `BUNDLED_PACKAGES`.

**Gap:** pane-content contribution API; `@clay/chat` bundle + extension
points; generic multiline `textArea` because single-line `textInput` cannot
host the composer’s newline chord.

### Behavior, command, key routing

Global sequence keybindings already reach Command Centre and shell commands
without editor focus.

**Gap:** `agent.*` command IDs routed by session id, not `DocumentId`. Not
`ClientFirstPredictable` edits.

### Command Centre

`TransientMenuSession` is the picker host. Setup needs bounded non-secret
field descriptors. Secrets never in snapshots.

### Clay UI catalog reuse (Task 2)

The entry surface is a package contribution in the pane `main` slot, not a
new component kind. The reuse matrix is:

- `panel` + `flex` provide the entry composition; `label` provides the
  greeting and instructional copy; `button` provides Open File, Open Folder,
  provider/model/agent actions, and Send. Existing focus rings, button state
  palettes, and token-driven panel chrome are reused.
- `scroll` + `list` provide the bounded transcript projection and any bounded
  session/result rows. `statusItem` provides non-secret status/diagnostic
  announcements. No `agentChat`, message widget, or package dropdown is
  needed; provider/model/setup/session selection remains the Clay-owned
  Command Centre.
- `textInput` is deliberately not enough for the composer: the existing
  package widget is single-line, uses Masonry `TextArea<true>` with
  `InsertNewline::Never`, pins a one-line hit area, and treats Enter as
  commit. Phase 25 therefore needs one generic package-facing `textArea`
  kind, built on the same retained/native substrate with
  `InsertNewline::OnShiftEnter`; Enter submits and Shift+Enter inserts a
  newline. It reuses `placeholderColor`, `validationState`, typography,
  spacing, focus-ring, and text/caret/selection token handling.
- Existing tokens are sufficient: `surface.main`/`panel`/`control`/`list`/
  `selected`, `text.primary`/`muted`/`disabled`, `border.focus`,
  `spacing.xs`/`sm`/`md`, semantic typography variants, and
  `opacity.disabled`. No Chat-specific token or style variable is justified.
- `WelcomeWidget` remains the core Open File/Open Folder fallback when no
  entry package is loaded. The package guide and catalogs must be updated when
  the pane contribution and `textArea` APIs are implemented; this review does
  not add an unimplemented kind to the drift-checked implemented table.

Accessibility ceiling to carry into implementation: package declarations do
not accept arbitrary AccessKit roles. The transcript uses `list`/`scroll` plus
host-owned bounded status/live semantics; if an explicit `Role::Log` wrapper is
required, add it at the generic pane/transcript host seam, not as a raw package
role field.

The Masonry 0.4.0 local source confirms
`TextArea<true>::with_insert_newline(InsertNewline::OnShiftEnter)` exposes
`Role::MultilineTextInput` and emits `TextAction::Entered`/`Changed`, so this
is a reuse of the existing native text substrate rather than a custom editor.

### Protocol and codec

`PROTOCOL_VERSION` 23. Reuse length-prefixed rkyv, boxed large variants.

**Gap:** `src/protocol/agent.rs`; include unused tool/permission variants.

### External process

Reuse language-server child I/O mechanics. Do **not** reuse package
`language-server` grants. `clay-agent` is core-owned spawn.

### Runtime domains and facades

Reserve `agent` in `RESERVED_CORE_API_DOMAINS`. `clay:agent` is a trusted
public facade (prompt/cancel/registerProfile/pickers). No daemon handle in
either Deno domain. Package IDs use `chat.*` / later `codingAgent.*`.

### Prism 0.3.0

`createAgent` / `createAgentSession` / `AgentEvent` / omitted tools fail
closed. Chat definition is registered by `@clay/chat`, not compiled into the
daemon as the only profile.

## Generic Gaps and Chosen Handoff

1. **Pane content contribution:** public empty-tab `main` slot, one winner,
   hosted by `PackageRegionWidget`. Core fallback = file/folder Welcome.
2. **Catalog `textArea`:** multiline local buffer + submit intent using the
   existing native `TextArea<true>` substrate. No `agentChat` kind.
3. **Agent projection state:** session/run keyed, no credentials.
4. **Agent wire family:** boxed protocol, version bump, compatibility tests.
5. **Core daemon manager:** one child per server; packages multiplex via
   `agent.*`.
6. **Command Centre forms:** provider setup fields; no secret DTOs.
7. **`@clay/chat`:** bundled package, one-line load, Chat profile + entry
   tree + extension points.
8. **No-document bootstrap:** hidden `InitialDocument` sentinel may remain
   for tab binding; entry surface must not mount it as editable text.

## Rejected Shapes

- Irreplaceable native Chat/`PaneContent::Agent` as the product landing.
- Restyling empty `PaneDocumentView` as Chat (fake document).
- `ComponentKind::agentChat` or landing `dropdown` (duplicates Command Centre).
- ACP / AG-UI as the Clay bus.
- One daemon per tab.
- Package-triggered process grant for Node.
- Prism inside `deno_core`.
- Client-side provider calls or client-owned transcripts.
- Silent compiled Chat with no `loadPackage` (unlike `core.text`, this is a
  replaceable product surface).
- Hard-coded disabled Coding Agent row in core (Phase 29 package registers it).

## Hot-Path Contract

| Work | Owner/lane | Forbidden location |
| --- | --- | --- |
| Composer keystroke | Native `textArea` | No IPC, daemon, JS, provider before local paint. |
| Prompt/cancel intent | Typed client→server | Never block paint or editor typing. |
| Provider/model listing | Server/daemon snapshots | Never fetch catalogs per keystroke. |
| Prism run | `clay-agent` async stream | Bounded queue. |
| Transcript delta | Server output lane | Capped; stale dropped. |
| Package JS | Load / command / SDUI update | Never keypress, paint, layout, scroll. |

## Security and Trust Findings

- Package JS cannot spawn or communicate with `clay-agent`.
- Two Deno domains remain exactly two. `clay-agent` is a third **process**.
- Third-party `replaces` of `@clay/chat` withdraws chat contributions and
  stays in the third-party runtime. Core bootstrap and the daemon are not
  packages.
- `extends` of first-party agents requires a declared extension point and
  exact user approval.
- Chat `AgentDefinition` omits tools/skills (fail-closed). Tool event
  variants stay on the wire for Phase 29.

## Documentation and Test Handoff

Later implementation tasks must update:

- `docs/reference/primitives/shell-layout-strategy.md` and primitive registry
  for pane-content contribution
- `docs/reference/packages/creating-packages.md` (landing is replaceable;
  Command Centre is not)
- `tests/primitives_docs.rs`
- agent protocol tests, package denial of daemon handle, replace/rollback of
  `@clay/chat`
- pane-host tests: fallback vs package contribution vs Document after Open File
- final UI screenshot/accessibility review and project wiki

## Invariants and Constraints

- `PaneContentHost` remains workspace-bound but path-agnostic.
- Agent and editor content are distinct.
- No package JavaScript in Masonry paint/layout/pointer/scroll/keypress.
- All new protocol and transcript payloads are bounded.
- No ACP, AG-UI, MCP, coding tools, diffs, AI-safe mutation, PTY, or
  auto-routing in Phase 25.

## Tests

No implementation tests added for this review. Catalog and reference guards
pass:

```text
cargo test --test editor package_ui_conformance --quiet
cargo test --test editor ui_primitive_conformance --quiet
cargo test --test protocol primitives_docs --quiet
```

The future `textArea` implementation must add enum/catalog/paint parity,
Enter-vs-Shift+Enter, multiline accessibility, focus, local-edit, and bounded
layout coverage.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md)
- [Package Extension and Adoption Authority](third-party-runtime-authority.md)
- [Transient Menu Session](transient-menu-session.md)
- [Control Center](control-center.md)
- [Clay Shell and Package UI/Layout Strategy](../../reference/primitives/shell-layout-strategy.md)
- [Agent Host project pattern](../../../.agents/skills/project-patterns/references/agent-host.md)
