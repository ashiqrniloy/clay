# Persistent Agent Lane and Slash Command Palette (Control Center Consolidation)

## Objectives

- Make Clay agent-native at the shell level: the coding agent's bottom lane —
  message composer plus the session-environment foot (workspace root, git repo
  and branch, extensions, MCP servers) — becomes a persistent shell surface at
  the bottom of the window, present in **both** of a tab's views (workspace and
  agent), not only inside the agent view.
- Make the lane hideable with a chord (`Ctrl+X Ctrl+P` default), with the
  visibility persisted like the workspace rail.
- Consolidate the Control Center into the composer: every option the Control
  Center modal offers today (the server command catalogue: executable commands,
  built-ins, key-binding and provenance details, fuzzy filtering) is reachable
  by typing `/` in the lane's input — merged with the daemon-registered slash
  commands and the client built-ins (`/model`, `/resume`).
- Redesign the `/` menu to the Control Centre aesthetic: when the palette is
  open, the sheet is elevated and the background is blurred, exactly like the
  current Control Centre modal opening; the small inline completions list is
  retired for the command-palette case (mention completions keep their dropdown).
- Retire the centered Control Centre modal as a separate surface; the `/`
  palette in the lane is the one command palette.

## Expected Outcome

- Every window shows the agent lane at the bottom of the working area (above
  the shell status bar) in the workspace view and the agent view; `Ctrl+X Ctrl+P`
  toggles it, and the choice survives restarts.
- `Ctrl+X Ctrl+P` no longer opens a centered modal; `controlCenter.open`
  dispatch focuses the lane, opens the `/` palette over a blurred backdrop, and
  lists the same items the Control Centre listed today (server-authoritative,
  routing-policy-filtered, fuzzy-scored), plus the agent slash commands.
- Selecting a palette item dispatches the same server command intents the
  Control Centre dispatched today; no new authority is created.
- The titlebar Control Centre button either opens the palette or is removed
  (prototype decision recorded in the approved artifact); no second landing or
  second palette exists.
- `frontend/src/command-centre/` renders menu sessions (`contextMenu`/`menuBar`)
  unchanged in their narrower popover; only the palette origin is redesigned.
- All Rust (`cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, relevant Linux tests) and
  frontend (vitest) gates pass.

## Tasks

- [x] Review Clay UI catalog and plan primitive/component reuse before UI work
  - Completion Evidence (2026-09-16, task 1):
    - Documentation read in full for this task: `DESIGN.md` §1/§4/§5/§6/§7/
      §8/§11/§12/§13/§14/§15; `.agents/skills/clay-execution/references/ui.md`,
      `references/components.md`, `references/tokens.md`;
      `docs/reference/ui-components.md`. Code verified against source:
      `src/shell/transient_menu.rs` (L100–112), `frontend/src/components/modal.tsx`
      + `modal.module.css`, `frontend/src/command-centre/command-centre.module.css`,
      `frontend/src/command-centre/CommandCentre.tsx`,
      `frontend/src/coding-agent/Composer.tsx` + `CodingAgentPanel.tsx`,
      `frontend/src/shell/use-shell-chords.ts` + `WorkspacePanes.tsx`,
      `src/server/control_center.rs` (skeleton), `src/protocol/mod.rs`
      (`default_keymaps` L539–676, `default_commands` L726–802).
    - Reuse inventory (catalog vocabulary → implementation task):
      - **Palette sheet:** existing `commandCentre` surface family — recipe
        slots `commandCentre.{scrim,dialog,input,listBox,item,status,empty}`
        (host slots; `status`/`empty` already recipe-declared), shipped
        `commandCentre.default.root.rest` recipe (radius 16, overlay shadow,
        opaque `surface.overlay` fill) already consumed by
        `command-centre.module.css` via `--clay-ds-command-centre-*` vars.
        The redesigned `/` palette re-anchors this surface to the lane —
        no new component kind or recipe slot is required unless the approved
        prototype introduces a genuinely new variant (e.g. bottom-anchored
        entrance geometry), which would then be additive design-system data
        (task 9 stays conditional).
      - **Blur/backdrop:** `ClayModal` scrim already projects the native
        `paint_scrim` contract from the `modal.default.scrim.rest` recipe
        (`surface.scrim` @ `opacity.scrim`, `backdrop-blur` 3,
        `--clay-ds-modal-default-scrim-rest-backdrop-blur`), with the
        reduced-transparency opaque fallback (§13.7). Palette-open blur
        reuses this scrim recipe; blur outside scrim/toast stays retired
        (§14.5, conformance-tested `backdropBlur == 0` on canvas/rows).
      - **Lane composition:** `ClayTextField` `variant="composer"`
        (`textInput.field`/`input` recipes, radius 12 multiline composer,
        focus = accent border + spread-3 inset halo, §14.4 one-ring rule),
      `ClayList` (`list.row.*` — selected = accent @0.15 fill only,
        §14.13), `ClayKbd` (`kbd` recipe, 18px, radius 5), `ClayIconButton`
      (≥24px hit target, icon slot), `ClayDropdown` `agentPicker` family
        (`agentPicker.trigger.*`), `ApprovalStrip`, plus the shipped
        `composer`/`composerHints`/`composerArea`/`agentFoot`/`stateStrip`
        host styles in `coding-agent.module.css` (extract, do not redraw).
        `statusItem`/`statusDot.{success,warning,error,muted,busy}` recipes
        carry the lane's environment/status marks; mono-for-data with
        tabular figures is mandatory for git/MCP/paths (§8).
      - **Server plumbing:** `TransientMenuOrigin::CommandPalette` —
        bottom-anchored command-palette origin, Clay-internal, already in
        `src/shell/transient_menu.rs` L100–112 (Plan 087);
        `TransientMenuOrigin::Centered` (Phase 24.4) is the modal the plan
        retires for the palette case. `TransientPackageOverlay::
        from_menu_session` maps origins to `PackageOverlayAnchor` (`Bottom`
        exists). `CommandCentre.tsx` already branches on
        `menu.origin === "centered" || "commandPalette"` — the palette
        branch is the re-anchor point. `controlCenter.open` →
        `ControlCenter::open_catalogue` keeps the generation-stamped
        `CommandCatalogue`, fuzzy scoring (`src/shell/fuzzy.rs`), and
        routing-policy filter (`is_executable_from_control_center`).
      - **Lane toggle:** rail-toggle precedent (`workspaceRail` in
        `layout-state.ts` + `persist.ts`, chord handling in
        `use-shell-chords.ts` L~140) is the pattern for lane visibility.
    - Performance reuse path (recorded for tasks 6–8): palette filtering
      rides the existing menu-session query flow — the catalogue snapshot is
      built once per session open and queries only re-score
      (`catalogue_snapshot_is_not_rebuilt_for_query_updates`,
      `src/server/control_center.rs` L729–753, stays green); the lane is
      mounted once per tab in `WorkspacePanes` (both view slots stay mounted,
      plan 118 task 33 pattern), so view switches never remount it; theme/
      recipe resolution is cached at install time (`--clay-ds-*` projection,
      no per-frame resolution).
    - Catalog-vocabulary booking for implementation tasks: every task-6/8
      component is a cataloged kind or shipped recipe above; the only
      not-yet-cataloged piece is the lane slot itself, which is **Clay-native
      internal surface** (like the status bar / tab bar in
      `components.md` → Clay-Native Surfaces) — to be added to that internal
      table (and `docs/reference/ui-components.md`) by the authoring-
      contract task, not a package-facing kind.
    - Security: palette activation reuses server menu intents
      (`menuQuery`/`menuMove`/`menuActivate`) and
      `workspace.dispatchServerCommand`; the catalogue remains
      routing-policy-filtered server-side (`is_executable_from_control_center`);
      no new client-side command authority, no package can open/drive the
      palette (`centered`/`commandPalette` are Clay-internal origins).
  - Acceptance Criteria:
    - Functional: An inventory exists (task evidence) of the cataloged
      primitives this plan must reuse — `ClayModal`/scrim behaviour,
      `ClayList`, `ClayTextField` (`variant="composer"`), `ClayKbd`,
      `ClayIconButton`, `ClayDropdown` (`agentPicker` family), the
      `composer`/`composerHints`/`agentFoot`/`stateStrip` styles, the
      `PackageOverlayAnchor::Bottom` overlay plumbing, and the
      `TransientMenuSession`/`CommandCatalogue` machinery in
      `src/server/control_center.rs`.
    - Performance: The inventory names the reuse path for the palette so no
      per-keystroke catalogue rebuild happens (the existing
      `catalogue_snapshot_is_not_rebuilt_for_query_updates` invariant is
      preserved).
    - Code Quality: The plan's implementation tasks cite catalog vocabulary
      (`component.variant.slot.state`, kind names, token names); any component
      outside the catalog is justified in that task's `Options Considered`.
    - Security: No new client-side command authority is proposed; palette
      activation still routes through server-owned menu intents and
      `dispatchServerCommand`.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` — §1 five laws, §4 design-system values, §11 component
        recipes, §12 composition rules for shells, §13 accessibility
        invariants, §14 retired patterns, §15 review checklist.
      - `.agents/skills/clay-execution/references/ui.md`,
        `references/components.md`, `references/tokens.md`,
        `docs/reference/ui-components.md`.
      - `src/protocol/menu.rs` (overlay anchors, centered origin),
        `src/server/control_center.rs` (catalogue, fuzzy scoring, routing
        policy), `frontend/src/shell/use-shell-chords.ts` (Global server-first
        chords), `frontend/src/coding-agent/Composer.tsx` (slash/mention
        completion state machine).
    - Options Considered:
      - Treat the lane/palette as new bespoke components: rejected — the shell
        layout contract and catalog already cover composer, lists, modals,
        and anchored overlays.
      - Reuse-first over the existing catalog: chosen.
    - Chosen Approach:
      - Inventory and record; every later implementation task builds on a
        cataloged or already-shipped primitive.
    - Files to Create/Edit:
      - None (evidence recorded in this task's completion notes).
    - References:
      - `design-artifacts/approved/quiet-instrument-migration/` (approved shell
        baseline this plan extends).
      - Plans 108/117/118/119 (coding-agent surface, slash registry, two-view
        tab model, agent store per tab).

- [x] Build the HTML prototype for the agent lane and slash palette in design-artifacts/prototypes/agent-lane-palette/
  - Completion Evidence (2026-09-16, task 2):
    - Artifact: `design-artifacts/prototypes/agent-lane-palette/` —
      `lane-palette.html` (the whole shell: titlebar, sidebar, both views,
      lane, palette, inspector, status bar, scenes and the live keyboard path),
      `lane.css` (the two new compositions, theme roles + kit geometry only),
      `README.md` (scope, variants incl. the rejected ones, the eight decisions
      awaiting approval, coverage, catalog mapping, named language additions,
      security, how to open), plus the approved kit copied verbatim
      (`theme.css`, `ds-quiet.css`, `components.css`, `pages.css`, `ds.js`) so
      the page opens over `file://` with no build step.
    - Coverage: 11 scenes — `idle` (lane in the workspace view), `agent`,
      `streaming` (Stop in the composer; the run's signal is the window mark —
      no cue in the lane), `approval` (ApprovalStrip in
      the lane, Allow
      focused), `no-agent`, `disabled` (no provider: composer live, foot
      note), `hidden` (lane toggle
      state), `palette`, `palette-agent` (lane does not move with the view),
      `palette-empty` (no-results), `mention` (`@` completions over the same
      veil as the palette) — at
      four shipped themes and 1500/1024 widths.
    - Review revision (user, 2026-09-16; this is the reviewed state):
      (1) the palette spans the composer's own box — the same width as the `@`
      mentions menu — instead of a 640px centered sheet; (2) the tab's agent
      controls (agent type, model, reasoning effort, context meter) moved out
      of the agent view's header into the lane, inside the composer box, so
      they are reachable from either view; (3) the agent-type dropdown joined
      them. The agent view therefore has **no header**: transcript, state
      strip and inspector remain. Consequences recorded: the composer box owns
      both rows (field + control toolbar) and keeps one ring; the palette
      above it can cover nothing in the lane; the agent-less tab keeps only
      the agent picker (labelled `Attach an agent`); the model picker is
      disabled with `Configure a provider` when no provider is configured;
      the lane's menus open upward (react-aria popover flip in the real
      component, drawn with `.popover--up` here).
    - Review revision (user, 2026-09-17; this is the reviewed state):
      (1) the composer has **no Send button** — `↵` sends and the hint row
      says so, and the slot the island button filled is Stop's while a run is
      live; (2) **no provider never blocks typing** — the composer stays live,
      the model trigger stays disabled reading `Configure a provider`, and the
      reason nothing will send is a warning-toned note in the lane's foot
      (`no provider configured · Settings → Providers`), visible in both views;
      (3) the **run's signal is the window mark** — the accent dot inside
      `Clay` in the title bar pulses while the active tab's agent works (same
      1.1s the tab marker uses) and is a steady dot otherwise; the lane's foot
      **states the environment only** — the stream cue drawn in the round above
      is removed; and the agent view's state strip above the composer swaps its
      tone dot for **three accent working bars** (1.1s, 160ms offsets) beside
      `Working` while a turn is in flight, the dot returning when it ends, so
      no dot blinks anywhere but the mark and "working" reads as motion in the
      words themselves (third round of the review); (4) **the `@` mentions menu gets the
      palette's blurred veil** — one veil
      for both composer menus, the lane still above it (the earlier "mentions
      stay unveiled" reading is superseded); (5) **the title bar is taken from
      the app as implemented** (`app-shell.tsx` + `shell.module.css`): the mark,
      the tab strip hugging it (label + the mono agent word), the spacer, then
      the window actions at the right edge — the icon-only Control Center
      trigger and the tab's view switcher — with no window buttons and no
      inspector/files toggles (those live in the status bar's hint family in
      the app; the prototype keeps the inspector toggle as such a hint).
    - Gate: `design-artifacts/tools/capture-agent-lane.mjs` — 88 asserted runs
      (11 × 4 × 2) plus a keyboard-only walk (`Ctrl+X Ctrl+P` toggles the lane,
      `Ctrl+X Ctrl+O` opens the palette, `/` opens it from the composer,
      `/zzz` shows the empty state, `/compact` narrows, `Esc` closes, `@`
      mentions, the effort menu opens with its levels and a pick writes into
      the trigger, the agent menu likewise) recorded in `report.json`.
      Asserts the scene/theme really applied, lane visibility, palette bottom
      6px above the composer box with the palette's left/right edges on the
      composer's (±2px, and never narrower), veil `blur(3px)` + translucent
      fill with the lane above it (for the palette **and** the mentions menu),
      no Send button anywhere while Stop tracks streaming, the lane's foot free
      of any run indicator (and of the word `Working`), the window mark's dot
      pulsing at the tab's 1.1s exactly while streaming and steady otherwise,
      the state strip's tone dot present with no turn in flight and replaced by
      exactly three 1.1s bars beside `Working` while one is (never both), the
      tab marker carrying its state by colour
      without motion, the title bar laid out
      as the app has it (mark, one window tab in the strip, spacer, the Control
      Center trigger and the view switcher inside `Application controls` at the
      right edge, no window buttons),
      per-scene Stop/disabled, per-mode
      agent controls (no-agent: only the agent picker; no provider: live
      composer, disabled
      model picker, the foot note, no effort/meter; otherwise model
      `provider/model`, effort
      and meter present), `backdropBlur == 0` on editor/transcript/sidebar/
      rows (§14.5), no agent header in the agent view, no horizontal overflow
      or frame overflow at either width, no console error, no remote request.
      Result: 88 runs, 0 failures, 68 PNGs (66 scene shots + `menu-effort`
      per theme) written to `design-artifacts/screenshots/agent-lane-palette/`.
    - Catalog findings (the inventory's prediction held): no new recipe key is
      required. The palette sheet reuses `commandCentre.default.root.rest`,
      the veil reuses `modal.default.scrim.rest`, the foot reuses
      `shell.default.footer.rest`, the composer reuses the
      `textInput.default.field/input` pair, rows reuse `list.default.row.rest`,
      the three lane pickers consume the shared `dropdown`
      trigger/popover/item recipes (the agent-type trigger additionally
      carries `agentPicker.default.trigger.*`, as it does today).
      Named additions for the approval task: (1) the lane as a Clay-native
      internal shell surface (host composition, not a package-facing kind);
      (2) a §7 motion entry for a bottom-anchored sheet entrance (rising
      `translateY(10px) scale(0.99)`, same 240ms `spring-snappy`);
      (3) palette placement geometry: the composer box's width, 6px above it
      (the `@`-completion anchor — no `dimension.overlay.centered.width`
      involvement any more); (4) the protocol chord swap already
      in this plan (`shell.toggleAgentLane` on `Ctrl+X Ctrl+P`,
      `controlCenter.open` on the palette chord, drawn as `Ctrl+X Ctrl+O`
      pending approval); (5) the review of 2026-09-17 adds no recipe either —
      the window mark's dot pulses with the tab marker's own `agentPulse` (the
      dot itself is host chrome drawn by `shell.module.css`, so this is a §7
      motion sentence and a §12 composition sentence, not a new value), the
      no-provider note is warning-toned text like the approval
      strip's, and the veil on the mentions menu is the same
      `modal.default.scrim.rest` a second surface now uses (the shipping
      `button.primary` island button is simply no longer drawn in the lane).
    - Decisions put to the reviewer (README §3): the lane spans the view pane
      (inspector keeps full height); the lane stays above the veil as the
      palette's input; the palette header echoes the composer's draft instead
      of owning an input; the titlebar `Commands` trigger opens the palette
      (Decision A) versus removing it; the chord swap; the agent-less tab keeps
      an inert lane with its agent picker; no provider keeps the same shape;
      approval keeps the shipped strip, moved into the lane; the agent
      controls live inside the composer box; no Send button (`↵` sends); the
      `@` menu shares the palette's veil; the run's signal is the window mark's
      dot — the foot states the environment only — which leaves the app's tab
      marker (plan 118 T33) and the brand dot both marking "working": the
      prototype draws the tab marker steady (colour only) and asks whether the
      pulse should live at the mark alone, at the tab marker alone, or at both;
      the title bar is the app's composition, with the prototype's own window
      buttons and titlebar toggles dropped.
    - Security: placeholder data only (`/workspace`, `feat/agent-lane`,
      repo-relative session-file names); no credentials, no absolute machine
      paths; rows are inert markup and activation only raises a toast.
    - Reproduce: `node design-artifacts/tools/capture-agent-lane.mjs`
      (`--no-shots --quiet` asserts without writing).
  - Acceptance Criteria:
    - Functional: A self-contained HTML artifact (opens over `file://`, no
      build step) covers: the persistent lane at the bottom of the workspace
      view and of the agent view; the lane hidden/toggle state; the `/` palette
      open over a blurred backdrop (the Control Centre aesthetic) anchored to
      the lane; the palette closed; mention completions (`@` dropdown) keeping
      their shipped geometry **over the same blurred backdrop**;
      an agent-less tab's lane state; streaming (Stop + the lane's animated
      cue) vs idle (**no Send button**: `↵` sends); no provider configured
      (**typing stays open**, the reason stated in the lane's foot); palette
      empty/no-results; approval strip in the
      lane.
    - Performance: Narrow (1024px) and wide (1440px+) layouts included; no
      horizontal overflow or clipping at either width.
    - Code Quality: Renders against the four shipped content themes
      (`@clay/theme-modus-operandi`, `@clay/theme-modus-vivendi`,
      `@clay/theme-gruvbox-material-dark`, `@clay/theme-gruvbox-material-light`)
      with a theme switcher; reuses catalog vocabulary so approval maps 1:1 onto
      catalog entries; a needed catalog addition (e.g. a palette-sheet or
      backdrop-scrim recipe) is named explicitly; a `README.md` states scope,
      variants, coverage, and how to open.
    - Security: Prototype contains no credentials or real workspace paths.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §4/§11/§12/§14; `.agents/skills/clay-execution/references/ui.md`,
        `references/components.md`, `references/tokens.md`.
      - Four design skills for this substantial new surface: `impeccable`,
        `full-output-enforcement`, `high-end-visual-design`,
        `design-taste-frontend`.
    - Options Considered:
      - One combined prototype (lane + palette in one page set): chosen — the
        palette is anchored to the lane; separating them would fake the
        geometry.
      - Per-surface prototypes: rejected for the same reason.
    - Chosen Approach:
      - Shared theme stylesheet and language CSS (the approved kit, copied) plus
        ONE page with a scene switcher for the eleven states — the migration
        set's own convention, which keeps the lane unmounted/remounted questions
        out of the page and lets a scene be addressed as `?scene=` for capture;
        screenshots (scene + theme + width) and a keyboard-only pass recorded in
        evidence; explicit statement that the prototype has no authority.
    - Files to Create/Edit:
      - `design-artifacts/prototypes/agent-lane-palette/` (HTML + `lane.css` +
        README + the copied kit), `design-artifacts/screenshots/agent-lane-palette/`
        (capture + `report.json`), `design-artifacts/tools/capture-agent-lane.mjs`
        (the assertion gate).
    - References:
      - `frontend/src/coding-agent/coding-agent.module.css`
        (`composerArea`, `agentFoot`, `stateStrip`),
        `frontend/src/command-centre/command-centre.module.css` (current
        Control Centre composition), `frontend/src/components` catalog.

- [x] Obtain explicit user approval and freeze design-artifacts/approved/agent-lane-palette/
  - Completion Evidence (2026-09-17, task 3):
    - **Approved**: user, 2026-09-17 — "Okay. Design artifact now approved",
      after the third review round (the state strip's working bars). The three
      rounds and their requested changes are in the prototype's `README.md`
      §1a, every rejected variant in §2.
    - **Frozen**: `design-artifacts/approved/agent-lane-palette/` —
      byte-identical copies of the reviewed page (`lane-palette.html`
      `92795cd7a097deac`), its stylesheet (`lane.css` `0c6548f3fd38cf10` — the
      review-time `4e8509eea65a6061` plus a comment-only fix of two stale
      `2026-06-24` dates, no rule changed, gate re-run), the
      kit it renders over (`ds-quiet.css` `c2aee13d84b0944a`, `theme.css`
      `b1a39839be745cdb`, `pages.css` `697a63cdee3d8065`, `components.css`
      `01f30073b77148f6`, `ds.js` `31f53b20a194a826`) and the working copy's
      README at freeze time (`README-prototype.md` `dcee87539f9e292d`), with
      the approval record in the frozen `README.md` (date, approving statement,
      scope, what the approval binds, open questions drawn but not approved,
      reproduction + hash re-check).
    - The kit hashes are the ones the Quiet Instrument sets froze with — this
      approval adds a page and a stylesheet and **no design-language value**.
    - Baselines stay with the working copy
      (`design-artifacts/screenshots/agent-lane-palette/`, 68 PNGs +
      `report.json`); the working-copy `README.md` now carries the approved
      banner and stays live for iteration.
    - Indexed: `design-artifacts/README.md` lists the frozen set under
      *approved/* and marks the prototype as the working copy.
  - Acceptance Criteria:
    - Functional: The prototype is presented with the decisions called out
      (lane composition in the workspace view, titlebar Control Centre
      trigger's fate, palette anchor and blur treatment, agent-less tab
      state); the review of 2026-09-16 has already been folded in (palette
      spans the composer box; agent type/model/effort/meter live in the lane's
      composer box and the agent view keeps no header), so the statement due
      here covers the reviewed state, not the first draft; approval is a real
      user statement recorded in
      `design-artifacts/approved/agent-lane-palette/README.md` (date, quoted
      statement, chosen variant, requested changes, coverage).
    - Performance: N/A (documentation task).
    - Code Quality: Approved artifacts are append-only copies; losing variants
      stay in `prototypes/` marked not approved; no implementation task starts
      before this directory exists.
    - Security: N/A.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` — UI Prototype and
        Explicit User Approval Task.
    - Options Considered:
      - Infer approval from review silence: forbidden.
      - Explicit user statement: chosen (and mandated).
    - Chosen Approach:
      - Freeze the chosen variant into `approved/agent-lane-palette/`; later
        deviations re-enter the loop as a new variant, never in-place edits.
    - Files to Create/Edit:
      - `design-artifacts/approved/agent-lane-palette/` (copied artifacts +
        README).
    - References:
      - `design-artifacts/README.md` (contract page).

- [x] Amend DESIGN.md for the persistent agent lane and palette (approved-language change)
  - Completion Evidence (2026-09-17, task 4):
    - **§12 (composition)**: a new *title bar is the app's own composition*
      paragraph (mark + accent dot, tab strip, spacer, window actions — palette
      trigger then the tab's view switcher; **no window buttons**; the app as
      implemented governs where the earlier migration artifact drew its own
      chrome); a new *the lane is the shell's one bottom section* paragraph
      (one instance per tab below the view slots, both views, sidebar /
      inspector full height, approval strip → composer box (field + the tab's
      agent-type/model/effort controls + context meter + hint row) →
      session-environment foot; `Ctrl+X Ctrl+P` toggle persisted per tab as a
      state; typing never blocked with no provider — the foot states the
      reason; no Send button, `↵` sends, Stop takes the slot while a run is
      live; the foot never carries a run cue); a new *the command surface*
      paragraph (Control Centre consolidated into the composer's `/` palette,
      `Ctrl+X Ctrl+O`, scope chips `All · Session · Shell · Files`, spans the
      composer box 6px above it, rises from its own edge, owns no input, shares
      one veil with `@`, `controlCenter.openPath` keeps its chord and stays a
      row); the switcher paragraph now says the tab marker carries its state in
      accent **without** the pulse (the pulse is the window mark's); the
      launcher paragraph points at the lane's agent picker; the workspace
      surface gains the lane's composer; the agent surface loses its header and
      its own composer (transcript + state strip + inspector, the working bars
      in the strip while a turn is in flight); *Universal* replaces the stale
      `⌘K` palette claim with the one command surface and adds the lane to the
      persisted layout state.
    - **§7 (motion)**: the bottom-anchored sheet entrance row
      (`translateY(10px) scale(0.99)`, 240 `spring-snappy`); the running-work
      row now names the mark's dot and the working bars; a rule paragraph fixes
      running work at exactly two places (mark's dot + bars) and specifies the
      bars (3 × 4×11px accent bars, pill radius, 1.1s with 160ms offsets).
    - **§9 (state)**: the halo paragraph now names the composer box (the
      palette owns no input) and a new *running work* rule: motion once per
      window, the tab marker a colour step with no pulse.
    - **§6 (materials)**: chrome strips row includes the agent lane; a
      bottom-anchored sheet row; the lane-is-chrome paragraph (above the veil).
    - **§11 (recipes)**: `modal.scrim` records its second caller (the `/` and
      `@` menus over the working area only, lane above it); `commandCentre.root`
      records the palette's anchor, width, 6px gap, `min(52vh, 420px)` cap,
      internal scroll and no-input rule; a `shell.default.footer.rest` bullet
      describes the lane's boundary and the composer box/toolbar inside it.
    - **§5 (geometry)**: the command palette's width is the composer box's own
      width; the `dimension.overlay.centered.width` token no longer applies to
      it.
    - **§13**: new invariant 10 — a menu whose input lives outside itself keeps
      that input usable (the veil is never a barrier over the field it serves;
      `listbox` + `aria-controls`/`aria-activedescendant`; `esc` returns focus).
    - **§14 (retired)**: 14.14 the window-centred command sheet as a standing
      surface (the composer's local `/` list is now the palette, not a second
      surface; `modal.dialog` keeps its callers and only the centred *command*
      sheet retires) and 14.15 a second blinking dot for the run.
    - **§15 (checklist)**: a run-state-appears-once item.
    - **§16 (shipped state)**: a lead-in note marking the composer, agent-header
      and centred-command-sheet sentences superseded by plan 124 until the
      implementation tasks rewrite them, with the recipe key set unchanged
      (the lane and the palette add **no key** — `shell.default.footer.rest`,
      `commandCentre.*`, `modal.scrim`, `shell.default.brand.rest` and
      `statusDot.*` all already ship).
    - **Header**: a shell-composition-amendment status line and the approved
      artifact added to Provenance.
    - **Cross-references swept**: `grep` for the superseded claims (agent view
      header, composer anchored at the bottom, the strip marker pulsing, `⌘K`,
      "one command palette", Control Center trigger) returns nothing outside the
      §16 note; markdown table column counts verified even (0 uneven tables);
      §14 numbering extended, no index referenced elsewhere moves.
    - **Also updated**: `.agents/skills/clay-execution/references/ui.md` shell
      layout model (lane + palette, plan-124 reference) so future planning reads
      the current composition; `design-artifacts/approved/agent-lane-palette/README.md`
      §5 records the amendment as landed plus the **one recorded disagreement** —
      §11 names the shipped `modal.scrim` recipe for the composer menus
      (`surface.scrim` @0.5 + `blur 3`) where the drawing filled the veil with
      the canvas at 66% + `blur 3`; the specification wins and the visual review
      (the visual-review task below) compares the composited result against the
      frozen PNGs.
    - **Not changed (deliberately)**: the doc's pre-existing Apple-style chord
      spellings (`⌘T`, `⌘O`, `⌘I`, `⌘⏎`) — they predate this plan and the shells
      they name are untouched by it; the shipped Ctrl-family chords are the ones
      the amended surfaces state (`Ctrl+X Ctrl+P`, `Ctrl+X Ctrl+O`, `Ctrl+1/2`).
    - **Follow-on added to task 6** (the amendment's own consequence): the tab
      marker's `agentPulse` retires (`frontend/src/components/tab-strip.module.css`)
      and the mark's dot carries the pulse (`shell.module.css`), with a test —
      otherwise §12/§7 and the app would disagree in the other direction.
  - Acceptance Criteria:
    - Functional: `DESIGN.md` §12 is updated: the shell gains the persistent
      agent lane (composer box — field, agent controls, hints — +
      session-environment foot) at the bottom of both
      views, the lane's composer box as the one place the tab's agent, model
      and reasoning effort are chosen (the agent view's header is retired),
      toggle chord `Ctrl+X Ctrl+P`, the composer's `/` palette as the
      one command palette (Control Centre consolidated) spanning the composer
      box, one veil shared by the palette and the `@` mentions menu, no Send
      button in the lane (`↵` sends), the run's signal at the window mark (the
      mark's accent dot pulses while the active tab's agent works — the same
      motion the tab marker uses — the agent view's state strip swaps its tone
      dot for the three working bars while a turn is in flight, and the lane
      foot states the environment
      only; §7 gains those sentences), the no-provider
      note, the titlebar composition as the app has it (mark, tab strip, spacer,
      window actions at the right edge; no window buttons), and the titlebar
      trigger's new role per the approved artifact; the same section records
      which title bar is normative — the app's as implemented, or the earlier
      approved migration artifact that draws window buttons and a
      `palette · ? · Inspector` action set the app does not have; §11 gains the
      palette-sheet
      and blurred-scrim recipe entries if the prototype introduced them (the
      scrim now has two callers); no
      §14 retired pattern is reintroduced (the centered palette sheet as a
      standing surface is retired in §14 if the approved design removes it).
    - Performance: No runtime impact (documentation).
    - Code Quality: Wording matches the approved artifact 1:1; cross-references
      (§12 ↔ §11) updated; the design-system package data referenced by §16
      stays consistent.
    - Security: N/A.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` current §11/§12/§14/§16;
        `design-artifacts/approved/agent-lane-palette/`.
    - Options Considered:
      - Leave DESIGN.md and treat the lane as an implementation detail:
      rejected — §12 is the normative shell composition; a standing shell
      surface contradicts it silently.
      - Amend §12 (plus §11 recipes where new): chosen.
    - Chosen Approach:
      - One edit pass after user approval of the prototype; re-approval of the
        language change is the freeze task above.
    - Files to Create/Edit:
      - `DESIGN.md` (§11, §12, §14, §16 as needed).
    - References:
      - `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md`
        (the §12 baseline being amended).

- [x] Re-plumb chords and commands: lane toggle and palette open
  - Completion Evidence (2026-09-17, task 5):
    - **Server defaults (`src/protocol/mod.rs`)**: `default_keymaps()` gained
      `shell.toggleAgentLane` on the Global ServerFirst `Ctrl+X Ctrl+P` chord
      and `controlCenter.open` moved to `Ctrl+X Ctrl+O` (comments state why:
      the P stroke the Command Centre held now toggles the lane, the palette
      keeps the same server-intent lane); `default_commands()` gained
      `CommandDeclaration::client_ui("shell.toggleAgentLane", "Toggle Agent Lane")`
      — declared so the keymap resolves against the manifest command set, the
      palette reaches it, and the server answers its intent with a
      `ShellClientCommandRequest` (authority ClientUi; **no new server
      authority**, matching the rail toggle). `src/server/ops/keybindings.rs`
      `is_runtime_bindable_command` accepts it; `command_routing_policy` falls
      through to `ServerFirst` **deliberately**, so a `bindKey` rebind compiles
      to the same route the shell matcher resolves (a ClientUiCommand-routed
      rule would leave the chord dead outside editor focus). `src/client_commands.rs`
      gained `ShellClientCommand::ToggleAgentLane` plus its
      `SHELL_CLIENT_COMMAND_CATALOGUE` row, so the palette lists it and the
      native client's deny-by-default parse accepts the id.
    - **Client route**: the shell matcher needed **no change** — the pending
      chord matcher already resolves any Global ServerFirst rule from the
      manifest, so `Ctrl+X Ctrl+P` and `Ctrl+X Ctrl+O` both work outside editor
      focus while the editor keymap owns them inside `.cm-editor`; only the
      matcher's doc comment moved to the new pair. The client command executes
      in `frontend/src/shell/workspace-commands.ts` (`direct` arm →
      `agentLane.toggle()`), which also covers the palette-row activation path
      (`serverClientCommandRequest` envelope) and the editor-focus round trip.
    - **Layout state (rail-style, per tab)**: `layout-state.ts` exports
      `agentLane` from the same `createVisibility()` factory;
      `workspace-controller.ts` subscribes it to `schedulePersist`, serializes
      `laneVisible` per tab, restores it, and keeps it in step with the active
      tab in all three tab-activation paths; `persist.ts` carries
      `laneVisible` on `PersistedTab`/`TabLayout` (absent means visible, so a
      pre-plan-124 document loads unchanged) and `src/shell/layout_persist.rs`
      round-trips it as `laneVisible` on the same v2 document (no version bump:
      the Rust parser reads each field optionally).
    - **Chrome**: `app-shell.tsx` gained the lane hint (`Ctrl X P`, label flips
      `lane`/`hide lane`, running the same `agentLane.toggle()` the chord and
      the palette row run), moved the palette hint to `Ctrl X O`, and updated
      the titlebar trigger's `shortcut` to `Ctrl+X Ctrl+O` (label stays
      `Control Center`, matching the approved artifact's trigger).
    - **Tests**: `frontend/src/shell/shell-chords.test.tsx` — the Control Center
      pair re-pinned to the real wire manifest at `Ctrl+X Ctrl+O` (shell matcher,
      modifier-noise chord, WorkspacePanes overlay, editor keymap) plus a new
      lane describe: the shell matcher dispatches the
      `shell.toggleAgentLane` intent outside editor focus and the per-tab store
      flips when the server's client-command answer arrives (both directions),
      and the editor chord keymap resolves the same rule inside CodeMirror.
      `workspace-controller.test.ts` — palette-open dispatch still sends the
      `controlCenter.open` intent for the active pane session; the lane toggle
      executes through the routed client-command lane (and stays out of the
      editor fallback); rail/inspector/lane visibility persists and restores per
      tab. `tab-store.test.ts` and `src/shell/layout_persist.rs` cover the same
      `laneVisible` round trip and the absent-field default.
      `src/test/shell.test.tsx` asserts the hint row carries a lane hint.
      Rust: `default_keymaps_contain_agent_lane_toggle_binding`,
      `default_commands_declare_agent_lane_toggle_as_client_ui`,
      `default_keymaps_ctrl_x_family_keeps_distinct_second_strokes` (P/O/F),
      the control-centre default test re-pinned to O,
      `keybindings::agent_lane_toggle_is_bindable_and_server_routed`, the
      `client_commands` allowlist test, and
      `server::connection::tests::agent_lane_toggle_projects_a_shell_client_request`
      — an over-the-wire intent that must answer with the shell-client request
      and advance no runtime generation.
    - **Gates**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test --lib` (single-threaded: 1375 passed, 1 ignored — parallel
      runs flake `js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`,
      reproduced on a stashed clean tree, pre-existing and unrelated),
      `cargo test --test protocol` (216 passed) and `--test runtime` (75 passed)
      — the example-config integration test still passes because that config's
      live `bindKey` table overrides the same sequence; frontend `tsc -b`,
      eslint, prettier and `vitest run` (49 files, 439 tests) all green.
    - **Two incidental fixes the change required**: (1) the
      `each_language_mode_registers_indent_electric_pairs_comment_triggers`
      payload budget for `@clay/markdown` (6740 → 6900 bytes: every mode layer
      now carries one more command declaration and keymap rule); (2) frontend
      tests had **no RTL cleanup** at all — the shell-chord tests leaked window
      keydown listeners, so the rail test's outcome depended on how many
      component renders preceded it in the file. `src/test/setup.ts` now
      registers `afterEach(cleanup)`, which is what the suite assumed.
    - **Follow-ups recorded for later tasks** (not done here, by plan design):
      - **Gate gap found in task 7 (2026-09-17)**: this task's gate list ran the
        lib tests and the frontend suite but not the `security` suite, where
        `tests/package_loading.rs::keypress_routing_uses_manifest_without_javascript`
        rejects a `ClientUiCommand` id that is not prefixed `shell.client`/
        `editor.client` — `shell.toggleAgentLane` is exactly that. Task 7
        replaced that name-prefix allowlist with the two Clay-owned command
        catalogues (`ShellClientCommand`/`EditorClientCommand`), which is the
        deny-by-default parse the native client already applies and is strictly
        stronger than a prefix.
      - **Intermediate state until tasks 6/8**: `controlCenter.open` is now
        routed (chord, titlebar trigger, hint row, server intent) but still
        *renders* the centered Command Centre sheet — `ControlCenter::session()`
        keeps `TransientMenuOrigin::Centered` until task 8 re-anchors it to the
        lane, which is where the acceptance clause "opens the `/` palette in the
        lane instead of the centered modal" is finally true.
      - **Task 14 (example config) — RESOLVED 2026-09-17 by the
        example-config task**: `examples/config/init.js` now binds
        `"Ctrl+X Ctrl+P": "shell.toggleAgentLane"` and
        `"Ctrl+X Ctrl+O": "controlCenter.open"` in the live Global table, and
        `tests/example_config_control_center_chord.rs` asserts that pair (the
        header note above records the pre-fix state).
      - **Tasks 12/13/17 (API docs and wiki)**: the chord appears in
        `docs/reference/clay-js-api/keybindings/{bind-key,unbind-key}.md`,
        `docs/reference/clay-js-api/commands/{server-list-commands,server-register-command}.md`,
        `docs/reference/clay-js-api/configuration.md`,
        `docs/reference/packages/creating-packages.md`,
        `docs/development/launch-and-gui-smoke.md` and
        `docs/wiki/modules/{control-center,command-registry,sequence-keybindings,transient-menu-round-trip,behavior-manifests,ui-review-harness}.md`
        — all still describe `Ctrl+X Ctrl+P` → `controlCenter.open`.
  - Acceptance Criteria:
    - Functional: `Ctrl+X Ctrl+P` toggles the agent lane from anywhere
      (editor focus, workspace view, agent view, no document open) — the
      Global server-first chord path (`use-shell-chords.ts` +
      editor keymap) and the AppShell hint row both use the new binding;
      `controlCenter.open` (and its `Ctrl+X`-family entry points, including
      the titlebar trigger per the approved artifact) now opens the `/`
      palette in the lane instead of the centered modal; `controlCenter.openPath`
      remains reachable from the palette.
    - Performance: Chord resolution stays on the existing
      pending-chord matcher; no new listeners beyond the current one.
    - Code Quality: The toggle command is a declared, bindable command
      (dotted-ID convention, e.g. `shell.toggleAgentLane`) with lane visibility
      persisted through the existing rail-style layout state
      (`frontend/src/shell/layout-state.ts` + `persist.ts`), not ad-hoc React
      state; retired bindings updated in `src/protocol/mod.rs`
      `default_keymaps`/`default_commands` and
      `src/server/ops/keybindings.rs` `is_runtime_bindable_command`.
    - Security: No new authority: the toggle is client-local like the rail
      toggle; palette open still dispatches the server command.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md` (dotted-ID
        convention, `RESERVED_CORE_API_DOMAINS`);
        `frontend/src/shell/use-shell-chords.ts`; `src/protocol/mod.rs`
        (`default_keymaps` L539–L676, `default_commands` L726–L802).
    - Options Considered:
      - Keep `Ctrl+X Ctrl+P` on `controlCenter.open` and have the palette be
        what it opens (no new command): fewer moving parts, but then the lane
        has no toggle binding and the user-requested chord is taken.
      - New `shell.toggleAgentLane` on `Ctrl+X Ctrl+P`; `controlCenter.open`
        rebound to the palette-open chord (e.g. `Ctrl+X Ctrl+O` or the `/`
        route): chosen — matches the user's binding request exactly and keeps
        every existing `controlCenter.*` doc/test target meaningful.
    - Chosen Approach:
      - Implement the second option; the exact new palette-open chord follows
        the approved artifact. As built (2026-09-17): the lane toggle ships as
        a declared **ClientUi** command whose default keymap is Global +
        **ServerFirst**, so the existing shell pending-chord matcher resolves
        it with no new listener and the editor keymap owns it inside
        `.cm-editor`; the palette-open chord is `Ctrl+X Ctrl+O`.
    - API Notes and Examples:
      ```ts
      // frontend/src/shell/workspace-commands.ts — the client half of the
      // ServerFirst intent (`Ctrl+X Ctrl+P` → server → shell client request)
      "shell.toggleAgentLane": () => agentLane.toggle(),
      ```
      ```ts
      // frontend/src/shell/layout-state.ts — rail-style per-tab visibility
      export const agentLane = createVisibility();
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: lane-toggle default command + keymap, the
        `controlCenter.open` chord move, and the default-manifest tests.
      - `src/server/ops/keybindings.rs`: bindable list (+ routing/bindability
        test); `src/client_commands.rs`: `ShellClientCommand` variant +
        catalogue row.
      - `frontend/src/shell/workspace-commands.ts`: the client command arm.
      - `frontend/src/shell/layout-state.ts`, `persist.ts`,
        `workspace-controller.ts`: lane visibility store, persistence,
        restore, active-tab hand-off.
      - `src/shell/layout_persist.rs`: `laneVisible` round trip.
      - `frontend/src/app/layout/app-shell.tsx`: hint row + titlebar trigger.
      - `frontend/src/shell/use-shell-chords.ts`: doc comment only — the
        matcher needed no code change.
      - `src/packages/commands.rs`: not needed — no id/label wording lives
        there (the built-in table is `src/server/command_execution.rs`, and the
        palette reaches the shell catalogue through `client_commands.rs`).
    - References:
      - `examples/config/init.js` L454/L458 (current chord map),
        `frontend/src/shell/shell-chords.test.tsx`,
        `frontend/src/editor/extensions/extensions.test.ts`.
  - Test Cases to Write:
    - `frontend/src/shell/shell-chords.test.tsx`: `Ctrl+X Ctrl+P` toggles lane
      visibility outside editor focus; editor keymap path still resolves.
    - `frontend/src/shell/workspace-controller.test.ts`: palette-open dispatch
      still sends the server intent with the updated command id/chord.
    - Rust `src/protocol` tests: default keymap contains the new defaults; no
      duplicate chord assignments in the `Ctrl+X` family.

- [x] Extract the persistent agent lane from the coding-agent panel and mount it shell-wide
  - Completion Evidence (2026-09-17, task 6):
    - **The lane** (`frontend/src/shell/AgentLane.tsx` + `agent-lane.module.css`,
      new): one instance per tab, mounted in `WorkspacePanes` beside the two view
      slots. Top to bottom — the approval strip while a tool call is suspended,
      the composer box (the field's row, the tab's agent controls as a second row
      *inside* that box, then the hint row), and the session-environment foot
      (`root · git <branch> · extensions …` … `MCP …`). `Ctrl+X Ctrl+P` flips its
      per-tab visibility through the shipped `agentLane` store; hiding keeps the
      lane mounted (draft and pickers survive) and takes it out of the
      accessibility tree. The lane never carries a run cue.
    - **One store per tab, owned by the host**: `WorkspacePanes` resolves it
      (`useTabAgentStore`, keyed by the tab runtime so a tab switch reads that
      tab's own store), runs the bootstrap (`listSessions` + `requestBinding` +
      `setUiVersion`) and adopts it into the tab runtime; the lane and the agent
      view both read that same instance. The agent view no longer creates the
      store (it must not depend on that view having been shown), and keeps its
      own creation + bootstrap only for a standalone mount (fixtures, tests).
    - **Agent view** (`CodingAgentPanel.tsx`): the header (agent-type picker,
      model, meter, effort, inspector reveal) and the composer/foot block are
      gone — the view is the transcript, the state strip, and the inspector. The
      strip renders three accent working bars (4×11px, pill radius, 1.1s
      `ease-in-out`, 160ms offsets, `aria-hidden` decoration over the strip's
      `role="status"` text) in place of the tone dot exactly while a turn is in
      flight, and the declared surface title now names the region.
    - **Shared derivations** (`frontend/src/coding-agent/surface-state.ts`, new):
      one `useAgentSurfaceState(store)` read of the store feeds both surfaces
      (session/config pair, branch, extensions, MCP list, models grouping, effort
      levels, meter, slash commands, skills, files, transcript rows, session file
      list, context view/detail, OM view, pending approval).
      `estimateContextTokens`/`compactTokens`/`groupModelsByProvider` moved there
      and are re-exported from `CodingAgentPanel` for their existing importers.
    - **Composer** (`Composer.tsx`): no Send button and no Close button — the
      trailing slot holds Stop exactly while a run is live and nothing at rest;
      a missing provider no longer disables the field (typing stays open, the
      submit is ignored and the draft survives) — the reason is the lane's foot
      (`no provider configured · Settings → Providers`); a tab with no agent is
      the one inert state (`Attach an agent to this tab to send a prompt`); the
      agent-control toolbar arrives through a new declared `toolbar` slot on
      `ClayTextField`'s composer variant, so the box stays the only boundary
      (`components/text-field.tsx` + `.module.css`: row wrap, `flex-basis: 100%`,
      divider from the input's own hairline colour).
    - **Run signal**: the window mark's dot pulses while the tab in view works
      (`.brand[data-busy]` in `app/layout/shell.module.css`, 1.1s
      `ease-in-out`, `prefers-reduced-motion` off; `AppShell` reads the active
      tab's `agentBusy`), the lane reports `agentBusy` (it is mounted for every
      tab, so a hidden agent view still marks its window), and the tab marker's
      `agentPulse` is deleted (`components/tab-strip.module.css` — accent colour
      + tooltip only). One looping pulse per window.
    - **DESIGN.md**: §12's lane paragraph now states the shipped geometry — the
      lane is the working area's own chrome strip (§6/§11), so it sits below both
      views and their rails (the workspace sidebar and the agent inspector end at
      its top edge). §16's profile note records the split and its superseded
      sentences mark the plan-118 header/composer description.
    - **Dev review fixture** (`routes/fixture.tsx`): the coding-agent fixture now
      draws the app's composition — the view above, the tab's lane at its foot —
      so a capture reviews what ships.
    - **Tests**: new `frontend/src/shell/AgentLane.test.tsx` (21 cases: the lane's
      composition and foot, slash completion from session state, the composer's
      `↵`-submit/steer/Stop path, typed-with-no-provider, the agent-less state,
      the effort chord + dropdown, `/model` and `/resume` intents being declared
      action targets, the agent picker's list/pick/inert states, and the
      approval strip's Allow/Deny payload + focus handoff — moved here from the
      panel suite, which can no longer reach them).
      `shell/WorkspacePanes.test.tsx` gained the lane's integration cases: one
      lane for both views with the same DOM node across a switch, visibility
      hiding with the draft surviving, the host's bootstrap on the tab's sender,
      the agent-less state, `agentBusy` true only while a run is live, and one
      store per tab across a tab switch. `CodingAgentPanel.test.tsx` now asserts
      the view renders no composer/foot, the working bars' exact lifetime, that a
      host store is never re-created (a standalone mount still bootstraps itself),
      and the Context tab's MCP detail without the foot. `Composer.test.tsx` owns
      the trailing-slot/provider-gate/toolbar-inside-the-shell assertions.
      `components/tab-strip` has no pulse left to assert, so
      `test/workspace-composition.test.tsx` pins the motion rule instead: the
      shell carries exactly one keyframe (the mark's run pulse), the tab strip
      none, workspace/editor none; `test/shell.test.tsx` pins the mark's own
      wiring (`data-busy` follows the tab runtime's `agentBusy`, at rest
      `false`). `test/design-system-consumption.test.ts`
      records `shell/agent-lane.module.css` as a consumer of `shell`,
      `agentPicker` and `statRow`.
    - **Gates**: frontend `tsc -b`, eslint, prettier and `vitest run` — 50 files,
      **448 passed**; Rust `cargo fmt --check`, `cargo clippy --all-targets -- -D
      warnings`, `cargo test --lib` single-threaded (1375 passed, 1 ignored; the
      untouched parallel flake of task 5 stays). Eyeballed the real fixture over
      CDP in the conversation, streaming, approval and workspace states: the lane
      sits under both views with one hairline,
      the composer box holds the controls row, the agent-less state shows only the
      picker, the working bars replace the dot while streaming, and Stop alone
      occupies the trailing slot.
    - **Follow-ups recorded for later tasks**: the `/` palette still lands on the
      composer in task 8 (the composer keeps its local completion list until
      then); the visual/accessibility review (below) now checks the full-width
      lane geometry against the amended §12 and the shared veil; the `Close`
      action (`coding-agent.close`) is no longer a button — it stays a declared
      command the palette lists.
  - Acceptance Criteria:
    - Functional: The lane (approval strip + composer box — field, agent
      controls, hint row — + session-environment foot: workspace root · git
      branch · extensions · MCP summary) renders at
      the bottom of the working area in both views for every tab, driven by
      that tab's agent store (plan 119 SC-6); the lane's agent controls
      (agent type, model, reasoning effort, context meter; review 2026-09-16)
      are the single place those are changed — the agent view keeps the
      transcript, state strip, and inspector and loses its header; a tab with
      no agent
      shows the lane state from the approved artifact (agent picker labelled
      `Attach an agent`, inert composer, no model/effort/meter); no provider
      configured never blocks typing — the composer stays live and the reason
      nothing will send is stated in the lane's foot (`no provider configured ·
      Settings → Providers`); the composer draws **no Send button** (`↵`
      sends, the hint row says so, and Stop takes that slot while a run is
      live); while a turn is in flight the run's signal is the window mark —
      the accent dot inside `Clay` pulses (AppShell reads the active tab's
      `agentBusy` for it) — and the lane's foot states the environment only
      (review 2026-09-17, second round: no `Working` chip in the lane; the
      agent view's state strip swaps its tone dot for the three working bars
      while a turn is in flight — third round), so the
      run reads alive from either view without a cue in the
      lane; the tab's own marker carries that tab's state in the accent colour
      with **no pulse of its own** (its `agentPulse` retires — the window shows
      one blinking dot; `DESIGN.md` §7/§9/§14.15), so work in a tab that is not
      in view stays visible; prompts typed
      in the workspace
      view target the tab's agent session and switching views mid-run steers
      the same run.
    - Performance: Switching views never remounts the lane or re-fetches the
      session STATE (the tab's store is the single source; the lane is mounted
      once per tab, not per view); no transcript re-render on lane keystrokes.
    - Code Quality: The lane is a host-owned shell component (first-party
      trusted module) composed from the existing `Composer`,
      `ApprovalStrip`, and foot markup; the agent-type/model/effort pickers
      are the shipped `ClayDropdown` (no bespoke picker), and the meter moves
      with them; `CodingAgentPanel` loses its duplicate
      bottom lane **and its header**; the tab strip's `agent[data-busy]` loses
      its `agentPulse` animation (colour only, tooltip unchanged) so the run's
      motion lives in one place (`frontend/src/components/tab-strip.module.css`,
      `frontend/src/app/layout/shell.module.css`); the lane's `Composer` instance renders
      **no Send icon button** (review 2026-09-17) — submit is `↵`
      (`onComposerSubmit` unchanged) and the icon slot holds Stop only while
      streaming; `Composer`'s provider gate stops disabling the field (typing
      is allowed without a provider; the reason moves into the lane's foot);
      no product-named pane kind is introduced.
    - Security: The lane sends through the tab's own command lane exactly as
      the composer does today (`agentCommandPayload`, `sendIntent`); no new
      Tauri capabilities.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12 (as amended); `.agents/skills/clay-execution/references/ui.md`
        (client architecture, shell layout contract);
        `references/components.md`.
      - `frontend/src/shell/WorkspacePanes.tsx` (view slots, store adoption),
        `frontend/src/coding-agent/CodingAgentPanel.tsx` (lane composition),
        `frontend/src/agent/state.ts`.
    - Options Considered:
      - Render the lane inside both views and hide the inactive one: duplicate
        composer state per view — rejected.
      - Mount the lane once in `WorkspacePanes` below the view slots (sibling
        of the two `viewSlot` divs), reading the tab's adopted store: chosen.
      - Keep the composer only in the agent view and show a collapsed input in
        the workspace view: rejected — the user asked for the same structure
        always.
    - Chosen Approach:
      - One lane component mounted at the `WorkspacePanes` host level; the
        agent view mounts lazily as today (`agentMounted`), while the lane can
        create/adopt the tab's store without mounting the transcript view.
        As built (2026-09-17): the **host** resolves the store (keyed by the tab
        runtime) and runs its bootstrap, because two creators on one tab would
        race; the lane and the view read the same instance. The lane is the
        working area's chrome strip — full width, one hairline on its inner edge
        — so the two views and their rails end at its top edge (DESIGN.md §12,
        amended in this task from the artifact's "rails keep full height"
        phrasing).
    - API Notes and Examples:
      ```tsx
      // frontend/src/shell/WorkspacePanes.tsx — one store per tab, host-owned
      const agentStore = useTabAgentStore(runtime, workspace);
      // …<View slots>…
      <AgentLane store={agentStore} uiVersion={uiVersion} … />
      ```
      ```tsx
      // frontend/src/coding-agent/Composer.tsx — the lane's controls ride the
      // field shell's own second row
      <ClayTextField variant="composer" toolbar={toolbar} endContent={stopOnly} … />
      ```
    - Files to Create/Edit:
      - `frontend/src/shell/AgentLane.tsx`,
        `frontend/src/shell/agent-lane.module.css` (new; the extracted lane and
        its chrome), `frontend/src/coding-agent/surface-state.ts` (new; the one
        shared derivation), `frontend/src/shell/AgentLane.test.tsx` (new).
      - `frontend/src/shell/WorkspacePanes.tsx` (+`workspace-panes.module.css`):
        the host store, the lane below the view slots, the view slot's own row.
      - `frontend/src/coding-agent/CodingAgentPanel.tsx` (+`coding-agent.module.css`):
        header, composer block and foot removed; working bars in the state
        strip; the shared derivative hook; dead header/meter/foot rules deleted.
      - `frontend/src/coding-agent/Composer.tsx`: Send/Close removed, the
        provider gate off the field, the `toolbar` slot.
      - `frontend/src/components/text-field.tsx` (+`.module.css`): the composer
        variant's `toolbar` row.
      - `frontend/src/coding-agent/AgentView.tsx`: the lane-owned props dropped.
      - `frontend/src/app/layout/app-shell.tsx` (+`shell.module.css`) and
        `frontend/src/components/tab-strip.module.css` (+`.tsx`): the mark's run
        pulse, the tab marker's retired pulse.
      - `frontend/src/routes/fixture.tsx` (+`.module.css`): the fixture draws the
        lane under the view.
      - Tests: `frontend/src/shell/WorkspacePanes.test.tsx`,
        `frontend/src/coding-agent/CodingAgentPanel.test.tsx`,
        `frontend/src/coding-agent/Composer.test.tsx`,
        `frontend/src/test/workspace-composition.test.tsx`,
        `frontend/src/test/design-system-consumption.test.ts`.
      - `DESIGN.md`: §12 geometry, §16 profile note and its superseded sentences.
    - References:
      - Plan 119 SC-6 (store adoption/disposal), plan 118 task 33 (view
        switching without remount), `frontend/src/agent/state.ts`
        (`createAgentSession`).
  - Test Cases to Write:
    - `frontend/src/shell/WorkspacePanes.test.tsx`: lane visible in both
      views; toggling visibility; agent-less tab state; store survives view
      switch; the window mark's dot pulses only while a run is in flight; the
      title bar matches the app's composition in the comparison.
    - `frontend/src/coding-agent/CodingAgentPanel.test.tsx`: panel no longer
      renders its own composer **or its header**; the three pickers render in
      the lane and changing one calls the same callbacks as before;
      transcript/inspector intact; the state strip draws the working bars
      instead of the tone dot exactly while streaming (third round); the Send
      button assertion (plan 112 T8) gains its successor — no `Send` button in
      the DOM, `↵` submits.
    - `frontend/src/components/tab-strip.test.tsx` (or the shell test that owns
      it): a busy tab's marker carries the accent colour and no animation, and
      the mark's dot is the only animated element in the title bar.
    - `frontend/src/coding-agent/Composer.test.tsx`: with no provider the field
      is enabled, the placeholder is the normal prompt, and the disabled model
      trigger is the only gate; no Send button rendered.
    - `frontend/src/test/chat-surface-absence.test.ts` stays green (no retired
      chat-era naming).

- [x] Route the command catalogue behind the `/` palette (server side)
  - Completion Evidence (2026-09-17, task 7):
    - **The palette shape** (`src/server/control_center.rs`): the session now
      declares `TransientMenuOrigin::CommandPalette` — the bottom anchor the
      shipped plumbing already carries (`PackageOverlayAnchor::Bottom`,
      `TransientPackageOverlay::from_menu_session`) — instead of `Centered`, and
      names itself `Commands` (the frozen artifact's own accessible name for the
      sheet and its listbox). The module doc states the contract the client
      needs: the session's query is the *filter* the field holds after the `/`
      sigil, opening shows the whole route-filtered catalogue, and every later
      keystroke arrives as a `MenuQueryUpdate`. Nothing else moved: the same
      generation-stamped `CommandCatalogue` snapshot, the same
      `is_executable_from_control_center` routing filter, the same fuzzy scoring,
      the same `ServerMenuActivation` dispatch, the same one-session lifecycle
      (open/replace/query/move/backspace/activate/cancel).
    - **The palette's path mode** (`src/shell/path_browser.rs`): the path browser
      session (the `Browse Filesystem` row's own surface) moved to the same
      bottom/composer anchor, because it is the palette's Files mode —
      DESIGN.md §14.14 retires the centred *command* sheet, and leaving the
      browser centred would pop a window sheet over the lane the palette belongs
      to. Agent-picker sessions and package/dialog menus stay `Centered`: they
      are window-level surfaces, and moving the model/session pickers into the
      lane's own popovers is not named by the artifact.
    - **No protocol change**: the origin variant already existed, so the wire
      shape, its version, and the item shape are untouched. `src/protocol/menu.rs`
      and `src/shell/transient_menu.rs` gained doc comments splitting the two
      origins by surface (composer palette vs window-level menu session).
    - **Deviations recorded** (approach text vs as-built):
      1. *Seeded query:* the approach says `controlCenter.open` opens the session
         with the draft as the initial query. As built the session opens with the
         **empty filter** and the field's text arrives through `MenuQueryUpdate`,
         because `ClientMessage::CommandIntent` carries no arguments
         (`src/protocol/mod.rs`) and DESIGN.md §12 gives the query to the lane's
         field — the `/` sigil and everything after it are the client's (task 8
         sends the filter, exactly as the retired sheet's own input did). The
         plan's "opens with a `/`-seeded query" case is therefore realized as:
         open shows the whole catalogue, a filter narrows it, clearing it restores
         it. Seeding was unnecessary, not skipped.
      2. *Prompt rename:* `Control Center` → `Commands`. The palette's accessible
         name is the artifact's (`aria-label="Commands"`), while the *trigger*
         keeps the label `Control Center` (approval requirement 8, DESIGN.md §12).
         Worth a look in the task-10 review: until task 8 replaces the surface,
         the centred sheet that still draws this session shows `Commands`.
      3. *Item shape unchanged:* rows still carry `label` + one `detail` string
         (`bindings — routing — provenance`) + `accessibility_label`. The artifact
         draws the chord as `ClayKbd` chips and the provenance as its own meta
         column, so task 8 renders both from that text; promoting bindings and
         provenance to structured item fields is an additive archive-layout
         change (protocol version) and needs a reviewed reason first — recorded
         for task 8/10.
      4. *The frontend test named by this task* (`CommandCentre.test.tsx`: palette
         items render label + detail from the session) moves to task 8, because
         that task replaces the surface which consumes the snapshot. The data it
         will render is pinned by the Rust tests below.
    - **Tests** (`cargo test --lib` single-threaded: 1376 passed, 1 ignored):
      `src/server/control_center.rs` gained
      `palette_session_opens_full_and_keeps_the_catalogue_across_queries` —
      origin `CommandPalette`, prompt `Commands`, empty query, full
      route-filtered list; `set_query` narrows it without changing the origin;
      `backspace` pops one filter character from the same session; clearing the
      filter restores the open-time id set byte-for-byte (one session held the
      whole catalogue — a keystroke re-filters and never rebuilds). The existing
      invariant `catalogue_snapshot_is_not_rebuilt_for_query_updates` now also
      asserts the palette origin survives the query update, i.e. the invariant
      runs on the new origin. `src/server/menu_sessions.rs`
      `snapshot_projection_is_inert_and_bounded` pins `prompt == "Commands"` and
      `origin == CommandPalette`. `src/server/connection/tests.rs`:
      `control_center_opens_filters_activates_and_cancels_scenario` asserts the
      open snapshot carries the bottom anchor (not a window sheet) and the
      `Commands` name, captures the open-time item count, and proves after the
      filtered update that the session id and origin are unchanged and that
      clearing the query restores the open-time count — then activates and
      cancels exactly as before (the scenario's whole-workflow 5s bound still
      holds); `path_browser_opens_from_keybinding_and_control_center_catalogue`
      asserts the path browser's snapshot carries the same anchor on both entry
      paths. `src/shell/package_ui.rs`
      `menu_session_projects_to_bottom_transient_overlay` already pins the
      `Bottom` projection for the palette origin (the "existing Bottom plumbing"
      the criteria name), so no new projection test was needed.
    - **Gate gap found and fixed (latent from task 5):**
      `tests/package_loading.rs::keypress_routing_uses_manifest_without_javascript`
      failed on `shell.toggleAgentLane` — its `ClientUiCommand` arm allowed only
      ids literally prefixed `shell.client`/`editor.client` plus
      `editor.toggleInlayHints`, and task 5's lane-toggle id (`shell.toggleAgentLane`)
      does not match a prefix. The allowlist now asks the two Clay-owned
      catalogues (`ShellClientCommand::from_command_id`,
      `EditorClientCommand::from_command_id`) — exactly the allowlists the native
      client parses deny-by-default — which keeps the test's intent (packages
      cannot request native client UI authority, not even by naming themselves
      `shell.*`) and is self-maintaining. Task 5's gate list did not run the
      `security` suite, which is how it slipped through.
    - **Gates**: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D
      warnings` clean, `cargo test --lib` 1376 passed (1 ignored), `--test
      protocol` 216, `--test runtime` 75, `--test presentation` 61, `--test
      security` 151 passed with one **pre-existing** environment failure
      (`agent_session_isolation::real_daemon_serves_one_session_per_workspace`
      times out waiting for a real agent daemon; reproduced on a stashed clean
      tree, so it is not this task's regression). Frontend untouched this task;
      its suite re-run as insurance: 50 files / 448 tests passed, `tsc -b` clean.
    - **Carried into task 8**: the client turns the palette origin into the
      lane-anchored sheet, sends the field's filter as `menuQuery`, and keeps the
      `centered`/`contextMenu`/`menuBar` sessions on their own surfaces. Two
      things that surface must handle: the path browser's session query is a
      *path string* (directory prefix + filter), so its row/scope presentation
      belongs to the palette's Files mode rather than its command list; and the
      palette's rows render the existing `label` + `detail` text (chord,
      routing, provenance inside one string) until an additive item field is
      justified.
  - Acceptance Criteria:
    - Functional: Opening `/` in the lane offers the merged list — server
      command-catalogue entries (labels, key-binding + provenance details,
      routing-policy-filtered exactly as `is_executable_from_control_center`
      does today), daemon-registered slash commands, and client built-ins —
      with the existing server-side fuzzy scoring; selecting a catalogue item
      produces the same command activation the Control Centre produced; the
      palette rides the existing menu-session lifecycle (query, move,
      backspace, activate, cancel) so cancellation/cleanup semantics are
      unchanged.
    - Performance: No catalogue snapshot rebuild per query keystroke (existing
      invariant test extended to the new origin); palette payload bounds stay
      within the current menu-session limits.
    - Code Quality: Reuses `TransientMenuSession`/`CommandCatalogue`; the
      palette session uses the `Bottom` anchor plumbing that already exists
      (`PackageOverlayAnchor::Bottom`, `TransientPackageOverlay::from_menu_session`);
      no mode-specific or product-specific Rust branches.
    - Security: Palette items remain limited to commands the routing policy
      allows from the control centre; activation still flows through the
      server command execution path with its existing validation — the client
      cannot widen the list.
  - Approach:
    - Documentation Reviewed:
      - `src/server/control_center.rs`, `src/server/menu_sessions.rs`,
        `src/server/connection/menus.rs`, `src/protocol/menu.rs` (anchors,
        centered origin), `src/shell/package_ui.rs`
        (`TransientPackageOverlay`).
    - Options Considered:
      - New protocol surface riding the agent session STATE: duplicates
        catalogue/fuzzy/routing logic client-side — rejected.
      - Reuse the menu-session RPC with a palette origin anchored to the lane
        (the client opens the session when the draft starts with `/`):
        chosen — one source of truth, existing tests mostly carry over.
    - Chosen Approach:
      - `controlCenter.open` opens the bottom-anchored palette session; the
        lane's field owns the query and every keystroke arrives as a
        `menuQuery` intent against that one session.
      - As built (2026-09-17): the catalogue and the path browser declare the
        existing `CommandPalette` (bottom) origin, the catalogue names itself
        `Commands`, and **nothing else changed** — one generation-stamped
        catalogue is held for the session's life, so a keystroke re-filters it
        and never rebuilds it, and the routing filter plus the shared dispatch
        path are untouched. No protocol change (the origin variant already
        shipped). Seeding the session's query turned out to be unreachable —
        `CommandIntent` carries no arguments — and unnecessary: the field sends
        the filter, so the session opens unfiltered. See the deviations in the
        evidence above.
    - API Notes and Examples:
      ```rust
      // src/server/control_center.rs — the palette is the catalogue's own shape
      TransientMenuSession::new(self.session_id, "Commands")
          .with_items(filtered)
          .with_selected_index(self.selected_index)
          .with_query(&self.query)
          .with_origin(TransientMenuOrigin::CommandPalette) // Bottom anchor
      ```
      ```ts
      // the client's contract (task 8): the field's filter is the query
      menu.origin === "commandPalette" // → anchor the sheet above the composer
      ```
    - Files to Create/Edit:
      - `src/server/control_center.rs`: the palette origin, the `Commands`
        prompt, the query/held-catalogue contract docs, the palette shape tests.
      - `src/shell/path_browser.rs`: the path browser (the palette's Files mode)
        onto the same anchor.
      - `src/shell/transient_menu.rs`, `src/protocol/menu.rs`: origin docs split
        by surface (no wire change).
      - `src/server/menu_sessions.rs`: the projection test's origin/prompt pins.
      - `src/server/connection/tests.rs`: the catalogue scenario's anchor,
        session-identity and restore assertions; the path browser's anchor.
      - `tests/package_loading.rs`: the client-UI allowlist now asks the two
        Clay-owned command catalogues (fixes a latent task-5 gate gap).
    - References:
      - `src/server/connection/tests.rs`
        `control_center_opens_filters_activates_and_cancels_scenario`,
        `runtime_generation_replacement_cancels_open_control_center_scenario`
        (must keep passing or gain direct successors).
      - `src/shell/package_ui.rs`
        `menu_session_projects_to_bottom_transient_overlay` (the Bottom anchor
        plumbing this task reuses).
  - Test Cases to Write:
    - Rust successor tests: palette session opens with the palette anchor and
      the `Commands` name over the whole route-filtered catalogue, filters,
      activates a command, cancels; routing-policy filtering intact; the
      catalogue is not rebuilt per query (the existing invariant extended to the
      new origin). *Landed in this task (2026-09-17).*
    - `frontend/src/command-centre/CommandCentre.test.tsx`: palette items render
      label + detail (binding/provenance) from the session. *Carried into task
      8, which replaces the surface that consumes the snapshot.*

- [x] Redesign the `/` palette surface (bottom-anchored, blurred backdrop, Control Centre aesthetic)
  - Completion Evidence (2026-09-17, task 8):
    - **The sheet is the field's own menu, in the field's own DOM.** New
      `frontend/src/command-centre/CommandPalette.tsx` draws the session the
      server opens (`origin === "commandPalette"`) as the composer box's menu:
      `ClayTextField` gained a `menu` slot (a bare, absolutely positioned child
      of the field shell — the `toolbar` slot's sibling), so `bottom: 100%`
      *is* the shell's top edge and the sheet is exactly as wide as the box it
      answers to, 6px above it, capped at `min(52vh, 420px)` with an internal
      scroll and a 240ms `spring-snappy` rise from `translateY(10px)
      scale(0.99)` (§5/§7/§11/§12). `.composer` (the shell) is now
      `position: relative`: the anchor is the box, not the form's padding.
    - **It owns no input and no ring.** The head echoes the field's filter
      (mono, ellipsised, `/` sigil drawn by CSS, `type a command` when empty),
      the sheet paints `commandCentre.root`/`.status`/`.empty` and no focus well
      — the boundary in focus stays the composer box's (§9/§14.4).
    - **Rows are the shipped row language**: a native `<button role="option">`
      (the artifact's own markup) consuming `list.default.row.rest/hover/selected/focus`
      recipes with the selected row as the fill, label + the server's one detail
      line (chord, routing, provenance — task 7's recorded item shape).
    - **The composer is the query** (`frontend/src/coding-agent/Composer.tsx`):
      a `/`-led draft *is* the palette's filter — the first slash sends
      `controlCenter.open`, later keystrokes send `menuQuery(filter)` with the
      sigil stripped, losing the slash cancels, `↑↓` send `menuSelectionMove`,
      `↵` runs the selected row and clears the field, `Esc` dismisses and keeps
      the draft. The inline `ul.completions` slash list is **gone** (the criterion):
      only `@` mentions keep the inline dropdown, with their Tab/click
      completion unchanged. When a session appears without the field saying so
      (the titlebar trigger, `Ctrl+X Ctrl+O`), the field takes the sigil and the
      focus — DESIGN §12's "puts the field in query mode".
    - **One veil, two callers** (`DESIGN.md` §6): `WorkspacePanes` renders a veil
      inside the view area — the region above the lane — carrying the shipped
      `modal.scrim` recipe attributes and values, so the global reduced-transparency
      / no-backdrop-filter / forced-colors fallbacks apply unchanged. It opens for
      the palette **and** for the `@` mentions menu (the composer reports its own
      menu up); the lane sits above it (its shipped `z-index`), which is the
      accessibility invariant: the field that *is* the query stays interactive and
      undimmed. `.viewArea` became a backdrop root (`isolation: isolate`) so the
      blur samples the working area and never the lane.
    - **The session is the one owner of "open"** (`frontend/src/shell/WorkspacePanes.tsx`):
      `CommandCentre` now renders only the window-level origins (`centered`
      pickers/package menus, and the `contextMenu`/`menuBar` popover) — the
      palette origin is filtered out before it mounts. A palette session also
      reveals a hidden lane (the artifact's chord does the same: there is nowhere
      to draw the menu otherwise).
    - **Deviations recorded** (with disposition; item 1 landed in the task right
      after this one):
      1. *Scope chips and per-row chord chips are not in this build.* DESIGN §12
         and the artifact draw an `All · Session · Shell · Files` segment and
         each row's chord as `ClayKbd` chips; the wire item is
         `{id, label, detail, accessibilityLabel}` (task 7 deliberately left it
         unchanged). Both needed new item fields (the wire spells them `scope`
         and `bindings`), which is an additive protocol change with a version
         bump — taken as the task right after this one, which is where the
         chips and the detail line's chord segment landed.
      2. *No focus trap.* The criterion says "focus is trapped, Esc returns focus
         to the lane input". The palette's query is the composer's field, so the
         correct model is the combobox one: focus never leaves the field (the
         sheet takes no focus), `↑↓` move the *session's* selection through the
         field, and Esc dismisses with the caret where it was. A hard trap would
         hold the query input outside itself — the accessibility invariant the
         approved artifact states ("`Esc` dismisses the palette and returns the
         composer"). Rows stay focusable buttons, so `Tab` into the list and
         `↵` on a focused row still work.
      3. *`↵` with nothing to run falls through.* With rows, `↵` runs the
         highlighted row (the artifact). With an **empty** result set, the
         artifact does nothing; this build submits what was typed instead, so a
         typed built-in (`/model`, `/resume`) keeps its shipped path, and the
         session closes with it.
      4. *Tab in the palette:* the artifact defines no palette Tab behaviour
         (Tab/click completion belongs to the `@` mentions), so Tab keeps its
         composer meaning. Read as "the shipped model, unchanged" — worth a
         reviewer's eye in the visual review.
      5. *Trigger over a draft:* the artifact seeds `/` even over existing text;
         this build matches it (the field is a command query at that moment).
         Recorded because it discards a composed prompt; if the review prefers
         preserving it, the fix is one guard in the Composer's seed effect.
    - **Tests** (`npx vitest run`: 51 files / 466 passed; `tsc -b` clean; `eslint`
      clean on the changed files): new `frontend/src/command-centre/CommandPalette.test.tsx`
      (the head is the field's echo and the sheet has no textbox, the server's
      selected row is the one marked, count singular/plural in a live `<output>`,
      click runs the row it names, the session's empty key inside the sheet, the
      foot's key model); `Composer.test.tsx` (`/` routes to the palette with no
      inline list left, one `request` then filters, cancel when the slash goes,
      the trigger seed + focus, `↵` runs the row and clears the field, the empty
      fall-through, `↑↓`/`Esc` intents, click moves-then-runs, the mentions
      report); `AgentLane.test.tsx` (the `/` draft no longer completes inline,
      and a palette session reveals a hidden lane);
      `WorkspacePanes.test.tsx` (the palette renders *inside* the lane's composer
      form, the veil is the one scrim element, opens with the palette and never
      covers the lane); `CommandCentre.test.tsx` (unchanged for the centred and
      popover origins — the palette origin is not drawn there). A shared
      `frontend/src/test/palette-stub.ts` builds the session + intents for them.
    - **Verified in the real React app** (Vite dev server + headless Chrome CDP,
      `design-artifacts/tools/`-style probe): at 1500×940 the sheet's width equals
      the composer box's (Δ −2px = the box's 1px hairline on each side — the
      artifact's own arithmetic), its bottom sits 6px above that box (measured 5px
      to the box's *border* box, 6px to its padding box, exactly as the artifact's
      `.lane-palette`/`.input-shell` pair computes), `max-height: 420px`, entrance
      `0.24s`, head echo `type a command`, 3 rows with the server's details, the
      first marked selected, foot `3 results ↑↓ navigate ↵ run Esc close`; the
      empty and path-mode scenes render the same sheet with 0 and 2 rows. The DEV
      fixtures were made faithful with it (`/?fixture=command-centre`,
      `-empty`, `path-browser` now mount the lane + palette; the menu scene keeps
      the centred/popover path), and the palette fixture draws the lane at the
      frame's foot so the sheet has the room it has in the app.
    - **Gates**: `cargo fmt --check` / clippy untouched (no Rust change this
      task — the Rust suite's state is task 7's); frontend `tsc -b`, `eslint`,
      `prettier --check` on the changed files, `vitest run` 466 passed. The
      `design-system-consumption` ownership table gained the two new consumers of
      existing recipes (`list` rows in `command-centre.module.css`, the scrim in
      `shell/workspace-panes.module.css`) — no recipe key was added, which is
      the recipe-boundary task's gate.
  - Acceptance Criteria:
    - Functional: With the palette open, the app behind it is blurred by the
      same scrim treatment the Control Centre modal uses today; the palette is
      an elevated sheet spanning the composer box (the width the `@` mentions
      menu uses), 6px above it — per the approved artifact — with
      the search head, scrolling results, and key-hint foot composition;
      keyboard model unchanged from the composer (`↑↓` move, `Enter` run,
      `Esc` close, `Tab`/click complete); focus is trapped, `Esc` returns
      focus to the lane input, and the results count is announced
      (`aria-live`); the inline `ul.completions` list is used only for
      non-palette completions (`@` mentions) per the approved artifact.
    - Performance: Backdrop blur uses the existing modal/scrim recipe (GPU
      `backdrop-filter`), not a JS-driven overlay; opening the palette
      triggers no workspace re-layout.
    - Code Quality: Implemented with cataloged primitives (`ClayList`,
      `ClayKbd`, `ClayText`, modal scrim recipe) and the recipes named by the
      approved artifact; no raw colors, no component-local font/point values;
      reduced-motion/transparency fallbacks per the design-system boundary.
    - Security: N/A (presentation).
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/approved/agent-lane-palette/` (binding reference —
        cited by every acceptance check below);
      - `DESIGN.md` §6 materials, §11 recipes (as amended);
        `.agents/skills/clay-execution/references/ui.md`,
        `references/components.md`, `references/tokens.md`.
    - Options Considered:
      - Keep `ClayModal` centered and restyle only: rejected — the user asked
        for the palette to live in the lane with the control-centre aesthetic.
      - Lane-anchored sheet + shared scrim: chosen.
    - Chosen Approach:
      - Refactor `CommandCentre.tsx`: palette origins render the lane-anchored
      sheet; menu-session origins (`contextMenu`/`menuBar`) keep the narrower
      popover unchanged; the composer's slash path opens the palette session
      instead of the inline list.
    - Files to Create/Edit (as built, 2026-09-17):
      - `frontend/src/command-centre/CommandPalette.tsx` (new) +
        `command-centre.module.css`: the sheet, anchored inside the field's shell.
      - `frontend/src/components/text-field.tsx`: the `menu` slot (the anchor is
        the box, so the sheet is a child of it) — not a new cataloged component:
        the veil reuses `modal.scrim` and the rows reuse `list.default.row`.
      - `frontend/src/shell/WorkspacePanes.tsx` + `workspace-panes.module.css`:
        the veil over the view area, and the palette/mentions wiring.
      - `frontend/src/coding-agent/Composer.tsx`: the field drives the session
        (open, filter, move, run, dismiss); mentions keep the inline list.
      - `frontend/src/shell/AgentLane.tsx`: threads the session + intents, and
        reveals the lane when a palette session opens.
    - References:
      - `frontend/src/components` (`ClayModal`, `ClayList`, `ClayKbd`),
        `DESIGN.md` §6/§11.
  - Test Cases to Write (as built, 2026-09-17):
    - `frontend/src/command-centre/CommandPalette.test.tsx` (new; the sheet) and
      `frontend/src/shell/WorkspacePanes.test.tsx` (palette in the lane's form,
      the one veil over the view area, never over the lane):
      `/` opens the palette, the veil is present, keyboard cycle via the field,
      Esc dismisses with the caret in the field, count announced.
    - `frontend/src/coding-agent/Composer.test.tsx`: `/` routes to the palette
      (no inline slash list); `@` mention dropdown unchanged; `↵` runs the
      session's selected row.

- [x] Carry each palette row's scope and chord as item fields (additive protocol)
  - Completion Evidence (2026-09-17, the task after task 8):
    - **The wire carries what the chips draw, and the chips filter the session.**
      `TransientMenuItemData` (protocol version 31) gained `scope` (a **closed
      vocabulary the server owns**: `session` / `shell` / `files`) and
      `bindings` (the command's effective chords in the app's spelling), and
      `ClientMessage::MenuQueryUpdate` gained `scope`, so the *session* filters
      and re-selects inside the chip instead of the client hiding rows the
      server still selects over. The client stays a renderer: it renders a chip
      per word it is handed, sends the word back, and never derives a scope from
      a command id.
    - **Where the values come from** (`src/server/control_center.rs`):
      `command_scope` classifies in precedence order — `controlCenter.openPath`
      is `files` (the palette's own path mode), shell/editor client-UI spellings
      and every `clay` built-in are `shell`, the agent package
      (`@clay/coding-agent`) is `session`, and anything else has **no** scope (it
      shows under `All` only; no guessed grouping). `binding_chords` emits one
      chord string per registered binding; `TransientMenuItem::with_scope` /
      `with_bindings` clamp them, and `ControlCenter::set_scope` falls back to
      `All` for a word outside the vocabulary rather than inventing a filter.
    - **The detail line stops restating the chord** (the task's own decision, so
      each fact has one owner): it is `routing — provenance` now, and
      `query_score` reads `bindings`, so typing a chord still finds its command
      (test: `a_command_is_findable_by_its_chord`).
    - **Frontend**: `CommandPalette` draws the `All · Session · Shell · Files`
      segment from the shipped `seg` family (the titlebar's view switcher is the
      reference) **only when the session's rows carry a scope at all** — a path
      browser's rows are paths, so the sheet offers no chip it cannot fill — and
      each row's chords as `kbd` groups (one chip per stroke, the status bar's
      own `split(" ")` spelling). The Composer holds the chip per *session*: a
      new session (a fresh open, or the catalogue swapping for its path mode)
      starts on `All`, which is the scope the server starts it with, so the
      segment never shows a filter the session is not applying.
      `workspace.menuQuery(query, scope)` sends `scope: null` for `All`.
    - **Bounds** (`src/perf/budgets.rs`): `TRANSIENT_MENU_MAX_SCOPE_CHARS = 16`,
      `TRANSIENT_MENU_MAX_BINDINGS = 4`, `TRANSIENT_MENU_MAX_BINDING_CHARS = 32`;
      the DTO and the shell item clamp at construction, `perf/baselines.rs`'s
      worst-case snapshot now includes a max-length scope and the full count of
      max-length chords, and it is asserted **in CI** (a unit test runs the
      builder; the criterion benches share it).
    - **Tests** — Rust: `items_carry_the_closed_scope_vocabulary` (all four
      classes, and a third-party package's shell/editor spelling),
      `scope_chip_narrows_the_catalogue_and_resets_the_selection` (only its own
      rows, selection reset, `files` = the path row, `All` restores, an unknown
      word is not a scope), `a_command_is_findable_by_its_chord`,
      `item_states_its_chords_and_scope_while_the_detail_keeps_provenance`,
      `constructor_clamps_every_bounded_field` (the new fields), the palette
      integration scenario in `src/server/connection/tests.rs` (a row carries
      `shell` + `Ctrl+X Ctrl+P`; the `files` chip narrows the live session to
      `controlCenter.openPath` and resets the index; an unknown chip restores the
      full list; the session id and anchor survive), plus
      `worst_case_transient_menu_snapshot_stays_inside_the_frame_cap`.
      Frontend (`vitest run`: 51 files / 472 passed): the segment draws from row
      scopes and reports a chip, marks the active chip while a query empties a
      scope, is absent when no row carries one, a row renders its chords as chips
      and an unbound row none; the Composer sends the chip with the filter it is
      filtering and starts a new session on `All`; `menuQuery` puts
      `scope: null` / `scope: "shell"` on the wire.
    - **Verified in the real React app** (DEV fixture + headless Chrome CDP):
      `/?fixture=command-centre` draws the segment in the head's trailing edge
      (`All` selected) and `Ctrl+X` `Ctrl+P` / `Ctrl+X` `Ctrl+F` chip groups at
      the rows' trailing edges; the empty scene and the path-browser scene render
      **no** segment and no chips (no row carries a scope).
    - **The seam that nearly dropped it** (recorded because it is the wiring
      this class of change hides in): `WorkspacePanes` implements the palette
      interface the field calls, and its `query` took only the filter — legal
      TypeScript, and the chip would have gone nowhere in the real app while
      every component test stayed green. It forwards the chip now, and
      `WorkspacePanes.test.tsx` asserts the *payload* the host sends on a chip
      click (`scope: null` for `All`, `"shell"` for Shell), not just the field's
      intent.
    - **Version choreography**: `PROTOCOL_VERSION` 31 with its per-version
      comment, the three pinned-version tests updated
      (`tests/editor_intelligence_protocol.rs`, `window_management_protocol.rs`,
      `agent_protocol.rs`), and `docs/wiki/modules/protocol-codec.md`'s version
      line + handshake-pin reference updated with it.
    - **Gates**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test --lib` 1379 passed (the known
      `coding_agent_clean_init_one_line_activates_working_defaults` parallel-run
      flake reproduces on the clean tree), the `protocol` (216), `presentation`
      (61) and `runtime` (75) suites passed, `npx tsc -b`, `eslint`,
      `prettier --check` on the changed files, `vitest run` 472 passed. No recipe
      key was added (the segment and the chips consume `seg.default.*` and the
      shipped `ClayKbd`).
  - Acceptance Criteria:
    - Functional: The `/` palette renders what DESIGN.md §12 names: scope chips
      (`All · Session · Shell · Files`) filtering the catalogue by the row's
      scope, and each row's chord as keyboard chips — from server data, never
      from an id heuristic or from parsing the detail line. A row with no
      binding renders no chip; a row outside the four scopes renders under
      `All` only.
    - Performance: No extra round trip per keystroke (the fields ride the same
      bounded item projection the session already sends); the snapshot stays
      inside `TRANSIENT_MENU_MAX_*` budgets and under `DEFAULT_MAX_FRAME_SIZE`,
      with `perf/baselines.rs` updated for the two added fields.
    - Code Quality: `TransientMenuItemData` gains bounded, optional fields
      (`group: Option<String>`, `bindings: Vec<String>` or one bounded
      summary), the protocol version is bumped with its comment, the catalogue
      projection fills them from the registered command (routing/scope and
      `key_bindings`), and the frontend DTO + `CommandPalette` consume them
      (chips via `ClayKbd`, the scope segment via `seg`). Existing rows keep
      working: both fields are additive and default empty.
    - Security: The item projection stays inert display data (no actions, no
      authority): strings only, truncated by the shared budgets, and the
      scope vocabulary is a closed set the server owns.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5/§11/§12 (rows are `list` rows; the palette's scope chips
        and per-row chord), `design-artifacts/approved/agent-lane-palette/`
        (`.pal-head`'s segment, `.pal-item`'s `.pal-meta` + chips).
      - Task 7's evidence: the item shape was deliberately left unchanged.
    - Options Considered:
      - Derive the scope on the client from the command id (e.g. the package
        prefix): rejected — the client would guess server semantics and
        misgroup any other agent's commands.
      - Parse the chord out of `detail` (`"<bindings> — <routing> —
        <provenance>"`): rejected — string surgery over server-authored prose.
      - Additive item fields populated by the catalogue: chosen.
    - Chosen Approach:
      - Add the two fields to `TransientMenuItemData`, bump `PROTOCOL_VERSION`
        with its per-version comment, fill them in
        `src/server/control_center.rs` (`command_to_menu_item`: the binding
        summary already computed for `detail`, and a scope from the command's
        family: the agent package's commands → `session`, shell/pane/editor →
        `shell`, `controlCenter.openPath` → `files`), and render them in
        `CommandPalette` (scope segment filtering the delivered rows client-side,
        chord chips per row). The `@` mentions menu and the path browser keep
        their current shapes.
      - `detail` stays as it is (routing + provenance are still useful text);
        the chips state the binding, so the detail can drop its binding segment
        once the chips ship — decide it in the same change, not twice.
    - Files to Create/Edit:
      - `src/protocol/menu.rs` (+ `PROTOCOL_VERSION` in `src/protocol/mod.rs`),
        `src/server/control_center.rs`, `src/perf/baselines.rs`,
        `frontend/src/bridge/types.ts`, `frontend/src/command-centre/CommandPalette.tsx`
        (+ its CSS/tests), and any protocol snapshot fixture that pins the item
        shape.
    - References:
      - `src/server/control_center.rs` `item_detail_includes_key_binding_and_provenance`
        (the binding summary's own test), `src/protocol/menu.rs` sample fixtures.
  - Test Cases to Write:
    - Rust: the projection carries the scope and the bindings for a shell
      command, an agent-package command and the path row; absent bindings stay
      empty (no fabricated chord); the archive round-trip and version guard
      pass; the palette session's item budget stays inside `perf/baselines.rs`.
    - `frontend/src/command-centre/CommandPalette.test.tsx`: the scope segment
      filters the delivered rows (and `All` restores them); a row renders its
      chord chips; a row without bindings renders none; the segment is absent
      when no row carries a scope.

- [x] Keep the typed recipe/design-system boundary for new palette recipes
  - Completion Evidence (2026-09-17, after the row-field task):
    - **Answer: no additions, and the record says why.** Mechanically: the whole
      plan changed nothing in `packages/design-instrument/package.json`,
      `frontend/src/styles/tokens.css`, or the 149-key
      `tests/fixtures/design-system-reference-keys.txt` baseline (`git diff`
      empty for all three), so the shipped recipe key set, the core tokens and
      the typed style variables are unchanged — additive-only by construction,
      and no existing package can break. The surfaces the artifact introduces
      are the palette sheet (family `commandCentre`), its rows
      (`list.default.row.*`), the scope segment (`seg.default.*`), the chord
      chips (`kbd`, painted by `ClayKbd` itself), the veil (`modal.scrim` via
      `recipeAttributes`), and the lane's foot (`shell.footer.*`). The entrance
      the Approach worried about needs no key: `commandCentre.default.root.rest`
      is one of the tool's eight *entering* surfaces (240ms spring-snappy), and
      `.palette`'s keyframe takes its duration **and** timing from that recipe's
      own variables while touching only `opacity`/`transform` — the house
      pattern (`markPulse`, `workingBar`). Only the anchor geometry is host
      spec, and DESIGN §12 names it (`6px` gap, `min(52vh, 420px)` cap).
    - **The audit found two boundary holes and closed both** (this is the task's
      real work: the plan's surfaces sat in the gap between two gates):
      1. **A phantom font role.** `command-centre.module.css` (`.palQuery` — the
         palette's field echo — and the sheet's result count) and
         `components/controls.module.css` (`.dropdownItemHint`) wrote
         `font-family: var(--clay-font-mono)`. No such token exists: `tokens.css`
         and `theme/adapter.ts` define `--clay-font-monospace`, fed from the
         **user's typography profile**. The declaration was silently dropped, so
         the echo and the count rendered in the UI font and ignored the profile —
         a concrete-font rule violation in the shape a grep for `monospace`
         never finds. All three now name the real role (no raw fallback, because
         the token is always defined).
      2. **A gate that could not see the surfaces it exists to police.**
         `verify-component-conformance.mjs`'s
         `material/no-filter-or-animation-in-host-css` scanned only
         `frontend/src/components`, so the plan's two material/motion additions
         (the palette's entrance animation, the lane veil's `backdrop-filter`)
         were outside it. It now scans every stylesheet except `tokens.css`,
         allows the two reduced-mode fallbacks that *remove* blur/animation and
         the at-rules that gate it, and still only admits recipe-driven blur and
         `opacity`/`transform` keyframes.
    - **Gates added, each red-first verified** (`frontend/src/test/design-system-consumption.test.ts`,
      the host-CSS boundary file that already owned the recipe-drift rules):
      `checkFontRoleDiscipline` (every `font-family` is `inherit` or
      `var(--clay-font-*)` naming a token `tokens.css` defines — a concrete stack
      or a phantom role fails) and `checkNoRawColors` (no colour literal in any
      stylesheet outside `tokens.css`; the tree has zero today). Both ship a
      synthetic rule test and a real-data zero-drift test, and both were shown
      failing against a deliberately reinstated offender before being trusted.
      Verified red-first for the widened tool check too (a raw `blur(4px)` in
      `shell/workspace-panes.module.css` fails it; the recipe var passes).
      Deliberately **not** added: a "no host selectors" gate — the kind/slot
      contract is already enforced by `recipeAttributes` sanitization, the
      `ComponentKind`↔catalog drift guard and the slot index, and a selector
      lint would be a heuristic with no authority behind it.
    - **Cross-theme conformance** (AC: ≥2 materially different themes): frontend
      `design-system-conformance.test.tsx` + `design-system-adapter.test.ts` +
      the consumption gate — 50 passed, covering the shipped
      `@clay/design-instrument` plus a third-party system and the Gruvbox Dark /
      Modus Operandi light content themes (recolour without recipe change,
      revocation, malicious snapshot rejection, reduced motion/transparency);
      Rust `cargo test --test presentation` 61 passed, including
      `plan118_recipe_matrix_marks_undeclared_slots`,
      `style_variable_catalog_matches_components_md` and the theme-package
      contrast floors across all four shipped themes; the offline
      `verify-component-conformance.mjs` audit **18/18** (contract 165 keys ·
      63 families · 40 kinds; entering-tier, radius ladder, single border weight,
      transient-only shadows, blur scrim/toast only).
    - **Catalogs updated** (no key/slot/token changed, so the edits are the
      surface and enforcement records): `references/components.md` — the
      transient-menu row gains the `CommandPalette` origin; the command-palette
      row states the as-built anchor and "no new recipe key"; `paint_scrim`
      gains its second caller and the "never over the lane" rule; a new
      **Typography boundary** bullet under the conformance contract (with
      DESIGN §8's role ownership). `references/tokens.md` — `surface.scrim` /
      `opacity.scrim` purposes, and the note that
      `dimension.overlay.centered.width` no longer applies to the palette.
      `references/ui.md` names the two gates. The plan-118 surface inventory
      (`design-artifacts/prototypes/quiet-instrument-migration/README.md`) gains
      a "superseded in part by plan 124" correction block instead of rewritten
      history — it is the owner inventory the consumption gate cites, so a
      reader must not trust its stale palette anchor.
    - **Security**: package-data validation untouched — `git diff` empty for
      `src/server/ui.rs`, `src/shell/design_system.rs`, `src/shell/theme.rs`,
      `src/shell/components.rs`, `src/packages/`; `cargo test --lib ui::` 66 and
      `--lib theme` 82 passed (provenance, generation, schema, bounds,
      revocation, raw-value rejection).
    - **Gates**: `cargo fmt --check`; `cargo test --test presentation` 61;
      frontend `npx tsc -b`, `eslint`, `prettier --check`,
      `vitest run` **476 passed**; conformance audit 18/18.
  - Acceptance Criteria:
    - Functional: Any new recipe (palette sheet, blurred scrim, lane foot as a
      shell surface) ships as typed, versioned design-system data mapping
      host-owned component kinds/slots/states to non-color properties and
      semantic theme-color roles; no raw CSS colors, selectors, or concrete
      fonts enter host components.
    - Performance: Conformance fixtures for the shipped first-party design
      system still validate across at least two materially different themes,
      including the new recipes.
    - Code Quality: Component kinds, slots, recipe properties, tokens, and
      style variables are additive-only; existing packages keep working;
      catalogs (`references/components.md`, `references/tokens.md`) updated.
    - Security: Package data validation unchanged (provenance, generation,
      schema, bounds, revocation).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md` (UI design-system
        packages), `references/tokens.md`, `DESIGN.md` §4/§11/§16.
    - Options Considered:
      - Hardcode the palette styling in `command-centre.module.css`: rejected
        — breaks the recipe boundary and per-theme contrast.
      - Typed recipe additions: chosen.
    - Chosen Approach:
      - Task-1 finding: existing `commandCentre.*` / `modal.scrim` recipes
        likely cover the palette; this task ships additions only for what the
        approved artifact genuinely introduces (e.g. bottom-anchored entrance
        geometry), and records "no additions" if none do.
    - Files to Create/Edit:
      - Design-system package data (per §16 profile), conformance fixtures,
        catalogs.
    - References:
      - `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.
  - Test Cases to Write:
    - Design-system conformance tests cover the new recipes in ≥2 themes;
      contrast checks pass.

- [x] Perform visual screenshot and accessibility review of changed UI
  - Completion Evidence (2026-09-17):
    - **Artifact**: `code-reviews/screenshots/2026-09-17-plan124-agent-lane-palette/`
      — `review-log.md` (method, state results, surface-by-surface comparison,
      findings, limits), `fixture-layer.json` (74/74 assertions + probes), the
      live set (`live-app/*.png` + `*.ax.txt`, 1500×940 and 1024×900), and the
      per-state fixture captures (`{palette-open,palette-empty,palette-files,lane-*}/*/fixture.png`
      + `probe.json` + `ax.txt`).
    - **Real Linux GUI build exercised** (`npm run build` → `cargo build -p clay-desktop`
      → `target/debug/clay server` + `client` in an isolated root): lane at rest,
      the `/` palette open over the working area with the veil, both at 1500 and
      1024 px. Palette geometry measured in the live window: sheet `1464×420`
      (= the field's border box, capped at `420`), `6px` above it. Lane geometry:
      lane `1500×200` spanning the working area, `Pane 1` `1160×836`, rail
      `340×836` — the sidebar *and* the rail end at the lane's top edge.
    - **Four defects found and fixed** (each with the measurement that failed
      before and passed after):
      1. **The lane was not the working area's chrome strip** — the route grid put
         the panes + lane in column 1, so the lane measured 1160 of 1500 px and
         the rail ran full height beside it (the pre-amendment reading; the
         fixture composite had hidden it). Fixed by making the working area a
         two-row grid with layout-transparent wrappers (`display: contents`) and
         explicit placement for view area / rail / lane; pinned by
         `workspace-composition.test.tsx` ("mounts the tab's lane as the working
         area's own full-width chrome row").
      2. **The veil covered the panes but not the rail** (rail pixel p99
         unchanged 202.3 → 202.3 with the palette open, against DESIGN §6). Fixed
         by placing the veil in the working area's own row (`grid-area: 1 / 1 / 2 / -1`,
         `position: relative` so its `--clay-z-modal` tier applies) with the lane
         at `z-index: 41`; re-measured rail p99 202 → 38 with titlebar, status
         bar and lane untouched and the sheet still bright.
      3. **`clay-desktop` did not compile**: the task-9 protocol change left
         `src-tauri/src/bridge/session.rs` forwarding `MenuQueryUpdate` without
         `scope` (E0063), so the plan's own GUI acceptance could not have run.
         Fixed; `cargo check --all-targets` and
         `cargo clippy --all-targets -- -D warnings` are clean again.
      4. **The `/` palette was anchored to the field's padding box** (5 px above
         the visible edge, 2 px narrower than the field). `ClayTextField` now
         wraps the shell in a `fieldSlot` box and renders its `menu` layer there.
      5. **`@` mention rows omitted the artifact's detail line** (the
         `.completionDetail` style existed but was never rendered): the rows now
         state the skill's description / the file's directory (`repository root`
         at the top), covered by a new `Composer.test.tsx` case.
    - **Accessibility**: live AT-SPI structure per state (lane `footer "Agent lane"`,
      `dialog "Commands"` + `list box "Commands"` + `output "67 results"`, scope
      chips as `toggle button` with pressed state, the composer `entry "Message"`
      enabled/inert per state, the status bar's chord buttons, rail landmark);
      palette rows announced with their chord; approval strip = `alertdialog` with
      focus on Allow; the fixture layer asserts the run announces once (three bars
      over the strip's `role="status"` words, `aria-hidden` decoration) and the
      palette's results count is a polite `output`. Recorded limit: WebKitGTK
      publishes palette rows as `list item` with no accessibility action
      (F1) — keyboard is the path, and the sheet says so.
    - **Limits recorded, not waived** (`review-log.md` §Limits): no keyboard
      backend in this session (`ydotoold` needs root, `wtype` unsupported,
      computer-use reports `can_send_development_input: false`), so the live
      keyboard-only pass is UNRESOLVED and chords/typed paths stand on
      `shell-chords.test.tsx`, `Composer.test.tsx`, `AgentLane.test.tsx` and the
      prototype gate's real key events; the live theme/JS-runtime path is not
      functional here (the harness's own shipped light fixture fails identically),
      so the four-theme live sweep is UNRESOLVED and cross-theme evidence remains
      the prototype gate (4 themes × 2 widths) plus the conformance suites;
      streaming/approval/typed states are fixture-layer evidence (no provider, no
      input backend in the live root). Two contaminated live captures (a host
      notification banner; a neighbouring window) were deleted and retaken, and
      the capture script now refuses crops below the review floor or without
      Clay's chrome.
    - **Both views**: the live agent-less tab offers only the workspace view
      (the view switcher is inert on a tab with no agent, by design), so the
      live lane was reviewed in that view; the tab's one lane, mounted beside
      both view slots and never remounted on a switch, is pinned by
      `WorkspacePanes.test.tsx` and rendered for the agent view by the fixture
      composite (`/?fixture=coding-agent`).
    - **Gates**: frontend `vitest run` **478 passed**, `tsc -b`, `eslint .`,
      `prettier`; `cargo fmt --check`, `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test --test presentation`
      61; `capture-lane-palette.mjs` **74/74**.
  - Acceptance Criteria:
    - Functional: Real Linux GUI build exercised for: lane in both views,
      toggle chord, palette open/filtered/activated/cancelled with blur,
      agent-less tab, streaming/steer/stop (the window mark's dot pulsing, the
      state strip's tone dot replaced by the three working bars beside
      `Working`, and no run cue in the lane's foot),
      approval strip, no-provider state (composer typeable, the foot note in
      both views), the `@` mentions menu over the same veil, all four themes;
      narrow and wide layouts; the running UI is compared surface-by-surface
      against `design-artifacts/approved/agent-lane-palette/` and every
      deviation is recorded with its disposition (fixed to match or explicitly
      re-approved) — a deviation that is neither fails the review. Three
      comparisons are called out because an amendment named them
      (`DESIGN.md` §11/§7/§12, approval README §5): the veil's composited fill
      (the spec's shipped `modal.scrim` recipe vs the drawing's canvas-tinted
      66%), the run's motion count (one pulsing marker — the window mark —
      plus the working bars; a tab marker or state-strip dot that still pulses
      is a defect, not a deviation), and the lane's width (task 6 shipped the
      spec's chrome-strip reading: the lane spans the working area, so the
      workspace sidebar and the agent inspector end at its top edge — the
      drawing keeps them full height beside it; the spec wins and §12 was
      amended, so the *drawing* is the deviation to record).
    - Performance: Screenshot set stored under a named artifact path; findings
      recorded in task evidence.
    - Code Quality: With `computer-use-linux` available, `get_app_state`
      before interaction, accessibility-tree checks (roles, names, states,
      focus order/trap, announcements) and a keyboard-only pass. The lane's
      modal states follow the approved artifact (no Send button to find; `↵`
      is the send path; the no-provider note is text, not a disabled field, so
      it must be readable by a screen reader; the working bars are
      `aria-hidden` decoration over the strip's `role="status"` words, so the
      run announces once, not twice).
    - Security: N/A.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/planning-checklist.md`,
        `DESIGN.md` §13/§15.
    - Options Considered:
      - Source inspection only: forbidden for UI changes.
      - Live GUI + a11y tooling: chosen.
    - Chosen Approach:
      - Per the mandatory review duty; blockers (no GUI) recorded and manual
        acceptance left unresolved rather than claimed.
    - Files to Create/Edit:
      - Review evidence under `design-artifacts/` review path (named,
      checked in).
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Manual: per-state screenshot + a11y checklist pass recorded.
    - Automated evidence already in place for the lane half (task 6):
      `design-artifacts/screenshots/agent-lane-palette/report.json` asserts the
      prototype's geometry, and the shipped lane's own suite
      (`frontend/src/shell/AgentLane.test.tsx`,
      `frontend/src/shell/WorkspacePanes.test.tsx`) pins the composition, the
      agent-less/no-provider states, the run signal and the store sharing.

- [x] Update the package UI/layout authoring contract and package guide
  - Acceptance Criteria:
    - Functional: The authoring contract reflects the shell change — the
      persistent agent lane is Clay-owned shell surface (not a package
      contribution slot), packages keep declaring inert contributions through
      the same APIs, and the palette's catalogue exposure is documented for
      package-registered commands (how a package command appears behind `/`).
    - Performance: N/A.
    - Code Quality: `docs/reference/packages/creating-packages.md`,
      `docs/reference/ui-components.md`, `.agents/skills/clay-execution/references/components.md`,
      and `docs/index.md` updated in-phase; documentation-drift `cargo test`
      gates pass.
    - Security: Trust-domain wording unchanged: third-party UI still cannot
      compile into the lane; palette items remain routing-policy-filtered.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md`,
        `docs/reference/ui-components.md`,
        `.agents/skills/clay-execution/references/ui.md` (shell layout
        contract).
    - Options Considered:
      - Document the lane as a package slot: rejected — the lane is host
        chrome per `DESIGN.md` §12.
      - Host-owned lane + documented catalogue exposure: chosen.
    - Chosen Approach:
      - One documentation pass after implementation stabilizes.
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`,
        `docs/reference/ui-components.md`, catalogs, `docs/index.md`.
    - References:
      - `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`,
        `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`.
    - Completion Evidence (2026-09-17):
      - **`docs/reference/packages/creating-packages.md`**: new
        `### Plan 124 authoring contract: the persistent agent lane and the
        composer's / palette` section. The lane is Clay-owned shell chrome (no
        `PanelContribution` slot; not a `TransientMenuOrigin` or
        `OverlayAnchor`; no package JavaScript on its paint/layout/input paths;
        a package `bottom` panel still composes inside its pane, above the
        lane), and every existing contribution path is unchanged (panels,
        component trees, overlays, input/state metadata, layout overrides, theme
        tokens, options, commands — same facades, provenance, budgets,
        precedence). Package-command exposure behind `/` is documented: declared
        under `clay.contributions.commands` or registered with
        `commands.serverRegisterCommand`; the row shows the display name, a
        `"<routing> — <package>@<version>"` detail line (built-ins read
        `"… — built-in"`), and the effective chords; scope chips come from
        Clay's closed vocabulary (`files` / `shell` / `session` — a third-party
        command carries none, so it appears under *All*); listing is
        routing-policy-filtered (client-first commands stay out); listing grants
        no authority; no package API opens, populates, filters, or drives the
        session. The Phase 24.4 "Centered Command Centre" section was swept:
        command and path sessions are now `CommandPalette` on the composer's own
        field, and `Centered` hosts the agent picker and package UI dialogs.
      - **`docs/reference/ui-components.md`**: the Plan 087 Command
        Centre/Path Browser boundary and the Plan 088 ownership list now name
        the command/path palette as the composer field's own menu (6 logical
        pixels above the box, as wide as the box, working-area veil) and add the
        persistent agent lane to the surfaces Clay owns; a new
        `## Plan 124 agent lane and composer / palette` section records both as
        internal (implementing files, origin, server row fields, not a package
        anchor) and links the guide's contract anchor.
      - **`.agents/skills/clay-execution/references/components.md`**: the
        Clay-native internal-surfaces table gains the agent lane row; the Plan
        088 contract excludes the lane and the `/` palette from package
        ownership; the Plan 097 renderer mapping splits
        `CommandPalette.tsx` (palette and path sessions) from
        `CommandCentre.tsx` (`Centered` hosts).
      - **`docs/reference/primitives/shell-layout-strategy.md`**: the shell
        vocabulary names the persistent agent lane beside the fixed-panel slot
        model and states it is not a slot, contribution, origin, or anchor; the
        transient-menu section and the Phase 24.4 paragraph record the plan-124
        re-anchoring; the status header carries the boundary; the verification
        contract's vocabulary list includes the lane.
      - **`docs/index.md`**: the UI-components and package-guide entries both
        index the plan 124 lane/palette contract.
      - **New drift pin**:
        `tests/primitives_docs.rs::plan124_agent_lane_and_composer_palette_authoring_contracts_are_pinned`
        asserts the contract markers across the guide, the UI navigation page,
        the catalog, the shell-layout reference, and the master index — the same
        shape as the plan 087/088 consistency gates. Red-first verified:
        temporarily renaming the `ui-components.md` heading failed the gate;
        restoring it passed.
      - **Gates**: `cargo test --test protocol` **217 passed** (includes
        `documentation_coverage`, `primitives_docs`, `package_loading_docs`,
        `manual_smoke_docs`, `clay_js_doc_registry`, `clay_js_api_inventory`,
        `example_config_control_center_chord`), `cargo test --test presentation`
        **61 passed** (includes `package_ui_conformance`),
        `cargo test --test runtime` **75 passed**, `cargo fmt --check` clean.
        `cargo test --test security` reports 151 passed and 1 failure —
        `agent_session_isolation::real_daemon_serves_one_session_per_workspace`,
        which spawns a real daemon requiring the JS/package runtime (the
        environment defect recorded in task 9); it is a pre-existing,
        environment-dependent failure that a markdown/docs change cannot reach.
      - **Security wording unchanged**: the trust-domain text still keeps
        third-party UI out of the lane and keeps palette items
        routing-policy-filtered; neither surface adds a kind, token, style
        variable, overlay anchor, manifest field, permission, or JS API.
  - Test Cases to Write:
    - Rust documentation-drift tests (`cargo test`) stay green with the new
      surfaces documented.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Completion Evidence (2026-09-17):
    - Added public `clay:shell` facade helper `toggleAgentLane()` with the stable ID `shell.toggleAgentLane` and typed declaration in `runtime/js/shell.js` / `runtime/js/shell.d.ts`.
    - Registered the helper in `docs/reference/clay-js-api/api-inventory.toml`, linked its complete reference at `docs/reference/clay-js-api/shell/toggle-agent-lane.md`, regenerated `docs/generated/clay-js-api-registry.json`, and indexed it from `docs/index.md`.
    - Documented `controlCenter.open` as the separate command-only palette route (`Ctrl+X Ctrl+O`) and `shell.toggleAgentLane` as the lane route (`Ctrl+X Ctrl+P`) in the keybinding, API inventory, command, and Clay JS inventory docs. No standalone palette facade, custom property, permission, or authority grant was added.
    - Added registry/facade coverage in `tests/clay_js_doc_registry.rs` and `tests/clay_js_facade_layout.rs`; added `shell.toggleAgentLane` to the parity ledger so public API coverage remains one-to-one.
    - Verification: `cargo test --test protocol` — 218 passed; `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
  - Acceptance Criteria:
    - Functional: New/changed public capabilities — lane visibility toggle
      (`shell.toggleAgentLane`-style id), palette open (`controlCenter.open`
      semantics), any lane/palette configuration — have Clay JS API docs
      (stable ID, name, default bindings, custom properties, examples,
      permissions, backing Rust path/op wrapper/JS facade, lookup tags);
      dotted-ID convention respected; registry/index regenerated.
    - Performance: No hot-path ops added; chord/config lookups stay on
      existing paths.
    - Code Quality: `cargo test` fails on missing/stale docs/registry/links;
      server-side Rust publics introduced by the plan are exposed only through
      explicit ops/facades or made private.
    - Security: No new grants: API docs state the toggle and palette open
      confer no filesystem/network/process authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`,
        `src/packages/manifest.rs` (`RESERVED_CORE_API_DOMAINS`).
    - Options Considered:
      - Leave the toggle as an undocumented chord: rejected — behavior-changing
        surface must be a documented API.
      - Documented facade: chosen.
    - Chosen Approach:
      - Extend the existing command/keybinding API docs with the two ids.
    - Files to Create/Edit:
      - `runtime/js/shell.js`, `runtime/js/shell.d.ts`.
      - `docs/reference/clay-js-api/shell/toggle-agent-lane.md`,
        `docs/reference/clay-js-api/api-inventory.toml`,
        `docs/generated/clay-js-api-registry.json`, `docs/index.md`, and
        related keybinding/command/inventory docs.
      - `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`, and
        `docs/development/tauri-react-parity-ledger.json`.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`,
        `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`.
  - Test Cases to Write:
    - Rust API-doc/registry completeness tests include the new ids.
    - `plan124_agent_lane_api_and_palette_command_are_documented` verifies the
      public registry entry, facade/type declaration, docs/index link, default
      bindings, and command-only palette boundary.

- [x] Create or verify Clay configuration APIs
  - Completion Evidence (2026-09-17):
    - Verified `~/.clay/init.js` uses the existing `clay:keybindings` APIs for both Plan 124 routes: `shell.toggleAgentLane` on `Ctrl+X Ctrl+P` and `controlCenter.open` on `Ctrl+X Ctrl+O`; `listKeyBindings("global")` reads the installed defaults and `bindKey`/`unbindKey` override or remove them.
    - Added the Plan 124 configuration review to `docs/reference/clay-js-api/configuration.md`, corrected stale Control Center chord text, documented `custom_properties = []`, and explicitly kept lane visibility out of hidden/global config: `laneVisible` is Clay-owned per-tab `layout.json` state, absent means visible, and persisted state cannot be overwritten by an `init.js` package option.
    - Added runtime coverage for rebinding the lane command and corrected the multi-stroke unbind fixture to use the current palette chord in `src/server/js_runtime/tests.rs`; added configuration-contract coverage in `tests/clay_js_doc_registry.rs`.
    - No new op, preference, package option, permission, or authority grant was added; existing keybinding validation and inert manifest routing remain the configuration boundary.
    - Verification: `cargo test --test protocol` — 219 passed; focused lane binding test passed; `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
  - Acceptance Criteria:
    - Functional: `~/.clay/init.js` can rebind the lane toggle and palette
      chords (`bindKey`) and read/override lane visibility defaults; options
      appear as documented custom properties in `api-inventory.toml`.
    - Performance: N/A.
    - Code Quality: Undocumented behavior-changing settings fail the coverage
      gate.
    - Security: Configuration grants no new authority (chord rebinding only).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md`,
        `examples/config/init.js` current chord sections.
    - Options Considered:
      - Hardcoded chord: rejected — every binding must be user-overridable.
      - Manifest/bindKey path: chosen (matches plan 109 I4 precedent).
    - Chosen Approach:
      - Register defaults in the manifest; docs + inventory updated.
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`.
      - `src/server/js_runtime/tests.rs` and
        `tests/clay_js_doc_registry.rs`.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
  - Test Cases to Write:
    - Config coverage gate: missing custom property for the new options fails.
    - `configuration_default_agent_lane_binding_is_present_and_overridable`
      verifies the shipped lane chord, unbind/rebind behavior, and
      `ServerFirst` routing.
    - `plan124_configuration_contract_uses_existing_keybinding_api` verifies
      the documented defaults, no hidden lane properties, and empty custom
      property metadata.

- [x] Update the canonical example configuration (examples/config/init.js)
  - Completion Evidence (2026-09-17):
    - `examples/config/init.js` Global batch table now binds
      `"Ctrl+X Ctrl+P": "shell.toggleAgentLane"` (new lane entry in the
      bindable-shell-command list) and `"Ctrl+X Ctrl+O": "controlCenter.open"`,
      each exactly once, with annotations for default scope/routing,
      per-tab persistence, and the distinct-second-stroke rule.
    - Swept the stale chord prose: the multi-stroke key-format example, the
      Control Center section (now the Plan 124 re-anchored composer `/`
      palette), and the commented rebinding recipes (palette on
      `Ctrl+X Ctrl+O`, lane on `Ctrl+X Ctrl+P`) all match the shipped
      `default_keymaps()`/init.js pair.
    - `tests/example_config_control_center_chord.rs` moved with the example:
      a `has_ctrl_x_chord` helper now asserts the boot manifest carries both
      `controlCenter.open` @ `Ctrl+X Ctrl+O` and `shell.toggleAgentLane` @
      `Ctrl+X Ctrl+P`, and the header note records the re-point.
    - Verification: `node --check examples/config/init.js`; `cargo test --test
      protocol example_config` — 3 passed; full `cargo test --test protocol`
      — 219 passed; `cargo fmt --check`; `cargo check --all-targets`;
      `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
  - Acceptance Criteria:
    - Functional: `Ctrl+X Ctrl+P` → lane toggle and the palette-open binding
      (with the old `controlCenter.open` comment updated to the new meaning)
      appear exactly once, in their sections, with option/default annotations
      and commented variants, matching the validated server parsers.
    - Performance: N/A.
    - Code Quality: `node --check examples/config/init.js` passes; ordering
      constraints preserved.
    - Security: No unsafe active setup.
  - Approach:
    - Documentation Reviewed:
      - `examples/config/init.js` L260–L540 (chord sections),
        `.agents/skills/create-plan/references/clay.md` (maintenance duty).
    - Options Considered:
      - Comment-only note: rejected — the example must show the real defaults.
      - Full section update: chosen.
    - Chosen Approach:
      - Edit the keybinding and any lane-visibility sections.
    - Files to Create/Edit:
      - `examples/config/init.js` (as built: Global batch table + section
        comments; also `tests/example_config_control_center_chord.rs`, which
        pins the boot manifest pair).
    - References:
      - User instruction 2026-08-03 (canonical example config duty).
  - Test Cases to Write:
    - `node --check` plus cross-check against API docs/inventory in task
      evidence.
    - As built: `example_config_boot_publishes_the_control_center_chord`
      extended to assert both the palette and lane chords from the boot
      manifest.

- [x] Launch-test the app with the canonical example config
  - Completion Evidence (2026-09-17):
    - **Launch**: fresh build (`npm run build`; `cargo build -p clay -p
      clay-desktop`, both 21:19) against a mode-700 scratch root
      `/tmp/plan124-canonical-launch` (`HOME=$root/home` with a verbatim
      `cp -r examples/config/.` as `~/.clay`, `XDG_CONFIG_HOME`/`XDG_DATA_HOME`
      /`TMPDIR` under the root, socket `$root/review.sock`). The server ran
      with cwd = the scratch workspace, so the lane foot showed
      `/tmp/plan124-canonical-launch/workspace` and nothing from the
      developer's real profile (its `~/.clay/init.js` mtime is unchanged).
      Scripts: `/tmp/plan124-final-launch.sh` (wipe + server + client) and
      `/tmp/capture-state.sh` (window-cropped captures, plan-097 privacy
      rule).
    - **Connected**: the server-driven file browser listed `notes.md`, the
      palette answered with the catalogue (95 rows), and the lane foot
      reported the scratch workspace + git state; the window mapped within
      ~0.3 s of the client spawn (compositor-visible), with no
      `Connecting…`/`Disconnected` state observed. The `Connecting…`/
      `Connected` text is not exposed through AT-SPI in this build, so the
      server-driven content is the connected-phase evidence.
    - **No configuration diagnostics**: `server.log` carries no
      `configuration.module_failed`, `theme.load_failed`, or
      `package.load_failed`; the canonical config applied (gruvbox-material-dark
      theme, `@clay/design-instrument`, phosphor-regular icons, canonical
      typography in the capture) and the first-party module evaluated
      (`@clay/coding-agent` agent profile + slash commands registered). Two
      expected scratch-root messages: store-package discovery
      (`npm list --json` with no `node_modules` → store packages unloaded;
      bundled first-party packages are unaffected) and `[agent] book.json
      write failed` (no agent config root). The status bar's `unknown workspace
      document 1` hint is a pre-existing desktop-client diagnostic (present in
      the 2026-09-01 Plan 105 captures), not a configuration diagnostic.
    - **Lane**: visible on the canonical boot (AT-SPI `footer` `Agent lane` +
      composer + `Session environment` foot). Toggled live through the status
      bar's `hide lane Ctrl X P` / `lane Ctrl X P` hint, which runs the same
      `agentLane.toggle()` the chord runs; hiding removes the lane from the
      accessibility tree and the flow, showing restores it with the draft
      intact (the hidden→shown palette still had `/` and 95 results).
    - **Palette**: the titlebar trigger (tooltip `Control Center (Ctrl+X
      Ctrl+O)`) seeds the composer with `/` and opens the bottom-anchored
      sheet at the composer's width, with scope chips `All · Session · Shell ·
      Files`, `95 results`, slash rows
      (`/branch /clone /compact /discard /fork /n /new /open-session
      /open-session-as-fork /resume /tree server-first — @clay/coding-agent@0.1.0`),
      and built-in rows with chord chips (`Toggle Agent Lane — Ctrl+X Ctrl+P`,
      `Open Control Center — Ctrl+X Ctrl+O`, `Browse Filesystem — Ctrl+X
      Ctrl+F`). Veil numerics (window-cropped mean brightness, rest →
      palette): main pane 40.0 → 35.3 (0.88), inspector rail 40.0 → 35.5
      (0.89), lane foot 50.3 → 50.3 (1.00), titlebar 1.00 — the scrim dims and
      blurs panes *and* rail and leaves the lane at full brightness.
    - **Defect D6 found and fixed (launch test)**: hiding the lane
      (`Ctrl+X Ctrl+P`) while the palette was open left the palette veil over
      the working area with its query input (the lane) gone — a stranded scrim
      with no in-area dismissal. Root cause: `WorkspacePanes` computed
      `fieldMenuOpen` from the menu session and `@` mentions only, while the
      sheet lives inside the lane. Fix: `fieldMenuOpen = laneVisible &&
      (paletteMenu !== null || mentionsOpen)` in
      `frontend/src/shell/WorkspacePanes.tsx`; regression pinned in
      `frontend/src/shell/WorkspacePanes.test.tsx` (hide → veil closes and the
      session survives; show → sheet and veil return). Re-verified live after
      `npm run build` + `cargo build -p clay-desktop`: with the lane hidden the
      pane/rail ratios return to 1.00, and showing the lane restores the
      palette with `/` and 95 results.
    - **Chord vs bindKey**: the canonical init.js Global table's
      `Ctrl+X Ctrl+P` → `shell.toggleAgentLane` and `Ctrl+X Ctrl+O` →
      `controlCenter.open` pairs ship from the boot manifest
      (`example_config_boot_publishes_the_control_center_chord` boots a copy of
      `examples/config`), the client matcher is covered by
      `frontend/src/shell/shell-chords.test.tsx`, and the live hints/keyboard
      chips render those exact chords. **UNRESOLVED (environment)**: driving
      the chord through the OS was not possible on this host — no keyboard
      synthesis reached a focused Clay window (`Atspi.generate_keyboard_event`
      reports success but the at-spi2 device-event-controller path is a no-op;
      the Computer Use remote-desktop portal `press_key` reports success but
      the focused window never receives the key; `ydotoold` needs root for
      `/dev/uinput` and there is no passwordless sudo; `wtype` is unsupported
      by GNOME). The GNOME Shell extension's window bounds also drift
      (`MoveWindow` reports success with stale coordinates, and one activate
      call dragged the window off-screen), so AT-SPI state plus
      freshly-started-window captures were the evidence path.
    - **Performance**: `npm run check:budget` (test-plan 11 budget gate) —
      shell 172.5/180 kB gzip, total 400.0/404 kB gzip; the launch observed no
      startup stall (window mapped in ~0.3 s; first a11y server-driven paint
      within the poll window).
    - **Gates**: `npx tsc -b`; `eslint`; `prettier --check`; `vitest run` —
      478 passed; `npm run check:budget`; `cargo fmt --check`; `git diff
      --check`; protocol suite 219 passed (last full run; this task changed no
      Rust).
  - Acceptance Criteria:
    - Functional: Real Linux GUI build launched against a scratch copy of
      `examples/config/init.js` (+ `examples/config/packages/` when present):
      Connected phase, no configuration diagnostics, lane visible and
      toggleable via the rebound chord, palette opens behind `/` with blur and
      lists catalogue + slash items, a rebound `bindKey` fires.
    - Performance: Startup within existing test-plan 11 bounds.
    - Code Quality: Launch command, scratch config path, and observed results
      recorded in task evidence.
    - Security: Never launched against the developer's real profile.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (launch-test duty).
    - Options Considered:
      - Automated server-only check: fallback only when GUI launch is blocked
        (record blocker, leave interactive acceptance unresolved).
      - Live GUI launch: chosen.
    - Chosen Approach:
      - Scratch HOME, server + client, exercise the changed surfaces.
    - Files to Create/Edit:
      - None (evidence in task notes). As built: `frontend/src/shell/WorkspacePanes.tsx`
        and `frontend/src/shell/WorkspacePanes.test.tsx` for defect D6.
    - References:
      - User instruction 2026-09-07.
  - Test Cases to Write:
    - Manual launch checklist recorded as evidence.
    - As recorded: the boot-manifest chord assertion
      (`example_config_boot_publishes_the_control_center_chord`) and the D6
      veil/lane coupling test in `WorkspacePanes.test.tsx`.

- [x] Execute and update the manual test plan (test-plan/)
  - Completion Evidence (2026-09-17):
    - **Step definitions added** (no existing step deleted or weakened; the
      centered-era rows keep their semantics under an explicit supersession
      note): module 10 → `K92` lane toggle, `K93` palette open, `K94`
      filter/scope, `K95` navigate/run, `K96` cancel/dismissal, `K97`
      anchoring + veil, `K98` negatives, `K99` rebinding, with `K64` re-aimed
      at the lane chord and the `Ctrl+X Ctrl+P` references in
      `K19/K29/K40/K47/K48/K49` moved to `Ctrl+X Ctrl+O` and the prompt name
      corrected to `Commands`; module 13 → `S47` lane-in-workspace-view
      geometry, `S48` sheet + veil in the working area, `S49` shell chords in
      a split layout; module 14 → `T79` per-tab lane state, `T80` draft +
      session per tab, `T81` persistence, `T82` tab a11y/session lifetime;
      module 16 → `A21` one session per tab, `A22` the lane as host chrome
      (no provider truth); module 17 → `C57`–`C64` (lane composer, effort +
      meter, model picker states, approval strip, streaming cues, agent-less
      tab, slash + mentions, negatives) with `C38` amended. Every new step
      carries its expected result, its negative check where one applies
      (palette empty state, agent-less tab, no provider, D6 orphan veil), and
      its automated leg.
    - **Executed on a real Linux build**: canonical
      `examples/config/init.js` copied verbatim into a mode-700 scratch root
      (`HOME`/`XDG`/`TMPDIR`/socket inside it, server cwd = the scratch
      workspace), fresh `npm run build` + `cargo build -p clay -p
      clay-desktop`; live state read through AT-SPI and window-cropped
      captures (plan-097 privacy rule). Evidence:
      `test-plan/artifacts/124-agent-lane/` (`drive.txt`,
      `accessibility.txt`, `geometry.txt`, `server.diagnostics.txt`, 9
      captures) and the per-module records appended to modules 10, 13, 14, 16
      and 17.
    - **Live pass/fail**: PASS live — launch/connected with no configuration
      diagnostics; lane visible and toggled (hint button runs the same
      command the chord dispatches; hidden lane leaves the flow and the a11y
      tree); palette open via the titlebar trigger and the status-bar hint,
      sheet at the composer's width, `Commands`, scope chips, `95 results`,
      `Session` scope → `14 results`; veil over panes + rail (0.88 each) and
      never the lane (1.00); hiding the lane with the sheet open closes the
      sheet **and** the veil (defect D6 fixed, pane/rail back to 1.00/0.99)
      and showing it restores the draft + scope; per-tab lane state across a
      new tab; agent-less lane keeps its place and stays typable; attaching
      `Coding Agent` from the lane picker switches to the attached lane with
      the disabled `Configure a provider` trigger and the
      `no provider configured · Settings · Providers` foot. UNRESOLVED live
      (documented host ceiling, automated legs named per step): the chord
      keystrokes themselves, free-text palette queries, `Enter` row
      activation, `Escape` cancel, `Shift+Tab` effort, prompt
      submission/streaming/approval (no provider in the canonical config),
      and AT-SPI row activation (WebKitGTK exposes rows without actions and
      the composer textarea without `EditableText`).
    - **Coverage matrix + module map updated** in `test-plan/index.md`
      (Plan 124 row: modules 10, 13, 14, 16, 17, 03, 11, 15 + launch gate),
      with the execution record above the module map; drift notes added to
      module 11 (Q-series chord/anchoring note, Q11 chord) and modules
      03/15 (path-browser and DEV-harness fixture scoping). The manual-step
      parity gate was updated with the new step IDs
      (`docs/development/tauri-react-parity-ledger.json`) and
      `parity_ledger_covers_every_manual_step_public_api_and_protocol_family`
      passes.
    - **Verification**: `cargo test --test protocol` 219 passed (includes the
      manual-step parity gate), `cargo fmt --check`, `git diff --check`;
      frontend gates unchanged from the launch test (`vitest run` 478 passed,
      `tsc -b`, `eslint`, `prettier --check`, `check:budget` shell
      172.5/180 kB · total 400.0/404 kB).
  - Acceptance Criteria:
    - Functional: Affected modules executed on a real Linux build with
      pass/fail recorded: `10-keybindings-and-commands.md` (rebound chords),
      `13-window-splits.md`/`14-tabs.md` (shell chords), `16-agent-host.md`,
      `17-coding-agent-parity.md` (lane behavior); new numbered steps added
      for lane toggle, palette open/filter/activate/cancel, blur scrim, and
      lane-in-workspace-view; `test-plan/index.md` coverage matrix updated.
    - Performance: New steps include expected results and negative checks
      (e.g. palette empty state, agent-less tab).
    - Code Quality: No existing step weakened or deleted to make a check pass;
      failures become defects or documented ceilings.
    - Security: N/A.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map/coverage matrix).
    - Options Considered:
      - Fold into the visual review task: rejected — separate duty with its
        own recording format.
      - Dedicated pass: chosen.
    - Chosen Approach:
      - Execute affected modules; add steps for the new surfaces.
    - Files to Create/Edit:
      - `test-plan/10-…`, `13-…`, `14-…`, `16-…`, `17-…`, `test-plan/index.md`.
    - References:
      - User instruction 2026-08-04.
  - Test Cases to Write:
    - The new numbered steps themselves.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki updated once after tests pass: the agent lane (shell
        surface, per-tab store, toggle state), the palette session (catalogue
        reuse, anchor, lifecycle), and the retired centered modal; linked from
        `docs/wiki/index.md`.
    - Performance: No runtime work added; performance-relevant details (no
      remount on view switch, no catalogue rebuild per query) documented.
    - Code Quality: What/how/invariants/tradeoffs/source-test paths included.
    - Security: Touched boundaries (command dispatch path, routing-policy
      filtering) documented without secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Options Considered:
      - Per-task updates: noisy.
      - Single post-test pass: chosen.
    - Chosen Approach:
      - One edit pass; archive any completed-phase review record per policy.
    - Files to Create/Edit:
      - `docs/wiki/index.md`, relevant `docs/wiki/**` pages.
    - References:
      - `.agents/skills/clay-execution/references/docs-as-code.md`.
  - Test Cases to Write:
    - Manual wiki review: index links and pages explain the changed systems.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and
  priority.
