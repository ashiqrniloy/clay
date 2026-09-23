# Plan 125 — Composer Palette Stage Flows and Centered Sheet Retirement

## Objectives

- Retire the window-centered transient sheet. The composer-anchored `/` palette
  above the agent lane is the **one** transient selection/input surface for
  shell-owned flows: the command catalogue, its path mode, the agent/provider/
  model/session pickers, and the multi-stage provider setup (list → auth method
  → secret / URL / OAuth).
- Keep the palette's current approved design (no colored border, no own focus
  ring — the composer box stays the boundary) and replace its drop shadow, and
  the `@` mentions menu's, with a subtle even **halo** (DESIGN.md language
  change ⇒ approved artifact + design-system package data + host fallbacks).
- Adopt the old centered sheet's *composition* where it earned its keep — the
  visible session/stage prompt, the results rows, the stage-aware foot with
  `↑↓ / ↵ / Esc` hints and the result count — inside the palette, without
  reintroducing the centered position or its focus behavior.
- Close the review-round lane affordances: a tab with no agent auto-adopts the
  default agent type (`coding` when the host lists it, else the first listed
  type), and the composer field is always typable so the lane is never a dead
  end.
- Confine the lane to the view pane (user direction, 2026-09-18): the lane spans
  the **middle pane only**, both rails keep the working area's full height, and
  hiding a rail hands its width back to the lane — so with both rails hidden the
  lane is the working area's whole width again. The palette sheet is the lane's
  inner width and follows it. This supersedes plan 124's full-width chrome strip
  (its review defect D1) and the `DESIGN.md` §12 sentence that produced it.
- Leave one normative, testable story: no producer anywhere creates a centered
  session; exactly one palette renderer exists; DESIGN.md, the design-system
  package, catalogs, the manual test plan, and the wiki agree with the code.

## Expected Outcome

- `frontend/src/command-centre/CommandCentre.tsx` and its `.surface`/centered
  projection are deleted; `TransientMenuOriginData::Centered` and
  `PackageOverlayAnchor::Centered` are retired (kept decodable on the wire, never
  produced), pinned by a test.
- Selecting a palette row that opens a picker (`Configure Provider`,
  `Choose Model`, `Resume Session`, `Search Sessions`, `Choose Agent`,
  `Choose Provider`) keeps the *same* bottom-anchored sheet: stage prompt,
  filtered rows, `Esc`/`Alt+←` back, `Alt+↵` secondary action, result count.
- The `Secret` stage never echoes typed characters: it is entered in a shielded
  field inside the sheet, never in the composer draft.
- Palette and mentions carry hairline border + subtle halo; no drop shadow.
- The agent lane is the **view pane's** chrome strip: it never covers the files
  rail or the agent rail (both run to the status bar at full height), it takes
  the working area's whole width when both rails are hidden, and the palette
  sheet is the lane's inner width — it cannot cross into a rail, and it widens
  and narrows with the lane.
- A tab whose host lists agent types shows the default agent attached without a
  user pick, and the composer is typable in every provider/agent state.
- DESIGN.md, `@clay/design-instrument`, `.agents/skills/clay-execution/
  references/components.md`, `docs/reference/ui-components.md`,
  `docs/reference/packages/creating-packages.md`, `test-plan/` and
  `docs/wiki/` all describe the single palette surface.

## Tasks

- [x] Establish the baseline, inventory every centered session producer/consumer, and freeze the retirement scope
  - Completion Evidence (2026-09-18, task 1):
    - Baseline before any change (HEAD `de4c4ec`, branch `change/control_center`,
      protocol v31; tree = uncommitted plan-124 wiki/doc edits + this plan):
      `cargo test --test protocol` → 219 passed / 0 failed (53.9s);
      `cargo test --test presentation` → 61 passed / 0 failed (0.5s);
      `npx vitest run` (frontend) → 51 files / 478 tests passed.
    - Completeness probes (the "no producer escapes" check):
      `grep -rn 'TransientMenuOrigin::' src/ --include=*.rs` → the only
      `Centered` producer is `src/server/agent_picker.rs:211`; catalogue
      (`src/server/control_center.rs:195`) and path
      (`src/shell/path_browser.rs:317`) are `CommandPalette`; every other hit is
      the closed mapping/decoder (`src/shell/transient_menu.rs:178,248-254,283,487`,
      `src/server/menu_sessions.rs:488-494`) or a test.
      `grep -rn 'TransientMenuOriginData::Centered' src/ src-tauri/` → the
      protocol variant + decoder remap + the synthetic codec test
      (`src/protocol/menu.rs:307,315`) only — no live producer.
      `grep -rn '"centered"' frontend/src` → `CommandCentre.tsx:66` plus tests
      and the fixture only.
    - **A. Server-owned menu sessions** (one active session per connection,
      `src/server/menu_sessions.rs:66-70`):
      - A1 Command catalogue — `open_control_center`
        (`src/server/menu_sessions.rs:60-79`), `ControlCenter::session()`
        (`src/server/control_center.rs:172-196`, prompt `"Commands"`, scoped
        rows): origin `CommandPalette` → **stays** (it is the palette).
      - A2 Path browser — `open_path_browser` (`:113-124`),
        `src/shell/path_browser.rs:317` (prompt `"Browse workspace"`): origin
        `CommandPalette` → **stays**.
      - A3 Agent picker — `open_agent_picker` (`:88-112`),
        `AgentPicker::session()` (`src/server/agent_picker.rs:201-211`; prompt
        per kind/stage `:368-383`): origin **`Centered`** → **moves to the
        palette** (tasks 6/7).
      - A4 Wire projection — `snapshot_from_session`
        (`src/server/menu_sessions.rs:446-495`): 5 origins → 4 wire values;
        the `Centered` arm **retires**, the wire variant stays decodable.
      - A5 Clay→hosted-package-UI projection —
        `TransientPackageOverlay::from_menu_session`
        (`src/shell/package_ui.rs:563-610`), `overlay_observations` (`:419-436`),
        `PackageOverlayAnchor::Centered` (`:113,608`),
        `rect_with_centered_width` (`:792-830`): the `Centered` mapping arm,
        the anchor, and the `result_count` gate (`:597`) **retire**;
        `centered_rect` (`:1007`) **stays** (the `Pointer` anchor uses it).
    - **B. Picker entry points** (every command id that reaches a picker;
      `picker_kind_for_command` `src/server/agent_picker.rs:657-666`, open path
      `src/server/connection/menus.rs:87-136`, registration
      `src/server/command_execution.rs:775-780`):
      - `agent.clientOpenAgentPicker` → `Agent`: List.
      - `agent.clientOpenProviderPicker` → `Provider`: List.
      - `agent.clientOpenModelPicker` → `Model`: List (also the `/model`
        built-in, `frontend/src/shell/AgentLane.tsx:215-220`).
      - `agent.clientOpenProviderSetup` → `ProviderSetup`: List(providers) →
        `AuthMethods` → `Secret` | `Url`; `oauth` → `StartOauth` → `Stage::Oauth`.
      - `agent.clientOpenSessionPicker`, `coding-agent.resume` → `Session`: List
        (secondary activation = delete).
      - `agent.clientOpenSessionSearchPicker` → `SessionSearch`: List (FTS hits
        installed per query by the connection).
      - No command maps to `OmObservation`/`OmReflection`; their list is always
        empty (`src/server/agent_picker.rs:534-537`, Plan 109 I8 panel
        dropdowns) → **never reaches the palette**.
      - The picker commands are visible palette rows (routing-policy filtered),
        asserted at `src/server/control_center.rs:549`.
    - **C. Client consumers/renderers** (`frontend/src`):
      - C1 Palette gate `runtime.menu?.origin === "commandPalette"`
        (`shell/WorkspacePanes.tsx:119-121`) → stays.
      - C2 Centered gate + lazy `CommandCentre` (`shell/WorkspacePanes.tsx:31-33,271-284`)
        → **deleted**.
      - C3 `command-centre/CommandCentre.tsx` (173 lines; centered flag `:66`,
        own `<input>` `:107-124`, local draft `:35-36` holding the real secret,
        prompt-heuristic `:65`, empty-draft `menuBackspace` `:70-73`) →
        **deleted**; its secret handling becomes the shielded stage (task 7).
      - C4 `command-centre/CommandCentre.test.tsx` (sheet render/close/masked-key
        flush) → **deleted**; the masked-key + key-routing cases migrate to
        `CommandPalette.test.tsx`.
      - C5 `routes/fixture.tsx` `CommandCentreFixture` + `command-centre{,-empty,-path,-menu}`
        states + `menuMode` (`:17,96-105,378-503`) → centered/menu states
        **deleted**; picker-stage fixture added (task 8).
      - C6 Centered stubs in tests: `shell/shell-chords.test.tsx:232`,
        `shell/workspace-controller.test.ts:669,734` → retarget to a
        palette/picker snapshot.
      - C7 `coding-agent/Composer.tsx` palette owner: `ComposerPalette`
        `:105-121`, `paletteFilter` `:124`, sigil rule `:200-225`, key routing
        `:340-380`, submit routing `:430-450`, menu slot `:500-520` → stays;
        grows `activate(secondary)`/`back()` (task 7).
      - C8 `shell/AgentLane.tsx:215-232` (`/model`, `/resume` → picker
        commands) → stays; the resulting session now renders in the palette.
      - C9 `bridge/types.ts:224-233` (origin union, `focusPolicy`) and
        `shell/workspace-envelope.ts:215-217` (cast: additive DTO fields are
        type-only) → gains `mode?: string` (task 6).
      - C10 CSS `command-centre/command-centre.module.css`: centered-only
        `.surface`, `.menu`, `.surface:focus-within`, `.head`,
        `.input`, `.input::placeholder`, `.results`, `.hintKeys` → **deleted**;
        `.prompt` **stays** (corrected by task 2: it is exactly the stage
        prompt line);
        `.searchIcon`, `.empty`, `.emptyHint`, `.foot`, `.hint`, `.spacer`,
        `.count`, `.pal*` **stay** (the palette uses them).
      - C11 CSS-consumption gates: `test/overlay-composition.test.ts:108-122`
        asserts "the palette is bounded/scrolls/one focus ring" against
        `.surface`/`.input` → retarget to `.palette`;
        `test/design-system-consumption.test.ts:280-300` family lists
        (`popover`, `menu`, `commandCentre`) lose the centered-only entries.
    - **D. Explicit non-targets** (frozen as *stays*):
      - Package UI overlays — `PackageOverlayDto.anchor`
        (`src-tauri/src/bridge/dto.rs:762-770`) rendered by
        `frontend/src/packages/PackageWorkspace.tsx:92-146`; anchors are
        package-declared strings, and `"centered"` is not a parseable anchor
        value (`src/shell/package_ui.rs:792-800` maps unknown → `WorkingArea`),
        so retirement cannot break the package authoring contract.
      - `TransientMenuOrigin::{ContextMenu, MenuBar, Completion}`: no producer on
        either side of the wire (grepped `src/` and `src-tauri/`; hits are the
        closed mapping/decoder and synthetic tests only) → out of scope; their
        client renderer is the same `CommandCentre` being deleted.
      - `ClayModal` usage for unsaved changes
        (`frontend/src/shell/WorkspacePanes.tsx:285-320`) stays.
      - `menu.focus_policy` is **not** consumed by the React client (only
        fixtures/stubs set it) — the picker's `Modal` default is inert there;
        recorded so it is not mistaken for a behavior change.
    - **E. Frozen retirement scope** (the call this task makes):
      - **Deleted:** the centered projection (`CommandCentre.tsx`/`.test.tsx`,
        its CSS block, the `WorkspacePanes` mount + lazy import, the centered
        fixture state, the centered test stubs), `TransientMenuOrigin::Centered`
        production, `PackageOverlayAnchor::Centered` + its geometry arm + the
        centered `result_count` gate.
      - **Kept decodable** (additive-only protocol rule):
        `TransientMenuOriginData::Centered` + its codec round-trip test
        (`src/protocol/menu.rs:303-317`), re-documented as
        retired-never-produced; `src/protocol/mod.rs:53` gains the v32 note.
      - **Kept:** catalogue/path/picker sessions (all `CommandPalette`), package
        UI overlays and anchors, `ClayModal`, and the palette's shared CSS
        classes/recipe families.
    - **F. Gaps found** (inputs later tasks must close):
      - G1 (root cause of the reported defect): opening any picker replaces the
        palette session (one active session per connection,
        `src/server/menu_sessions.rs:66-70`) and the picker's `Centered` origin
        routes the client to `CommandCentre` → a window-centered modal. Tasks 6+7+8.
      - G2: the palette has no stage/prompt line — `CommandPalette` renders
        `menu.prompt` only as the dialog's accessible name, so picker prompts
        (`Agents`, `Configure provider`, `API key (hidden)`, `Authorize
        provider`) would be invisible. Task 7.
      - G3: there is no stage-back — `AgentPicker` keeps one `stage`
        (`src/server/agent_picker.rs:56`) and `backspace()` only pops query text
        (`:142-145`); the centered sheet's back is "Backspace on empty draft"
        (`CommandCentre.tsx:70-73`). The palette's back must derive the previous
        stage (`Secret`/`Url`/`Oauth` → `AuthMethods` → `List`; `ProviderSetup`
        → `List`) — no trail field needed. Tasks 6/7.
      - G4 (security): the centered sheet's secret input is a plain text
        `<input>` bound to a local draft holding the real value
        (`CommandCentre.tsx:35-36,107-124`); the composer is a textarea. Nothing
        masks the local draft today — the shielded stage is new work. Task 7.
      - G5: no typed presentation exists; the client guesses "secret" from the
        prompt string (`/api key|hidden|base url/i`, `CommandCentre.tsx:65`).
        Task 6 adds `mode`.
      - G6: `result_count` is built only for `Centered`
        (`src/shell/package_ui.rs:597`) — after the move the hosted package-UI
        projection loses the palette's count; the `CommandPalette` origin should
        carry it. Task 8 (while deleting the arm).
      - G7: drift to fix while editing — `open_agent_picker`'s doc block reads
        "Opens a new Path Browser session" (`src/server/menu_sessions.rs:88-95`);
        `test/overlay-composition.test.ts:108-122` names the centered sheet "the
        palette" while asserting `.surface`.
      - G8: the picker stages add no new recipe family — the palette already
        consumes `commandCentre.default.*`, `list.default.*`, `seg.default.*`,
        `kbd`, `divider.default.*`, `key-hint.default.*`; the shielded well adds
        the shipped `textInput` shell only. Consistent with task 2's reuse table.
  - Acceptance Criteria:
    - Functional: A written inventory (appended to this plan and to the task evidence) lists every producer of a `centered` menu session, every renderer, every command id that reaches a picker, and every picker kind/stage — each row marked *moves to the palette*, *stays a popover*, or *deleted*, with the reason.
    - Performance: The inventory records the query/activation paths of the palette (`menuQuery` → `command_center.rs`, `menuActivate` → `agent_picker.rs`) and states that no new per-keystroke work is introduced by moving pickers (row filtering stays server-side).
    - Code Quality: The inventory names exact `file:line` anchors for producers, renderers, protocol variants, CSS classes, and tests, so later tasks cite spans instead of re-discovering them.
    - Security: The inventory names the secret-bearing stages (`Stage::Secret`, `Stage::Oauth`), states where their characters currently render (centered sheet input; server echoes `•`), and records that no current path masks the local draft — the gap the design phase must close.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5 (geometry), §6 (materials/shadow recipes), §9 (focus), §11 (`commandCentre.*`, `menu.*` recipes), §12 (shell, lane, palette), §14 (retired patterns).
      - `.agents/skills/clay-execution/references/ui.md`; `references/components.md`; `references/tokens.md`; `docs/reference/ui-components.md`.
      - `design-artifacts/README.md` (prototype/approved contract); `design-artifacts/approved/agent-lane-palette/README.md`.
    - Options Considered:
      - Full grep-led sweep per later task: cheap now, but each implementation task re-derives scope and risks missing a producer.
      - One recorded inventory task first: one extra task, every later task cites it. (Chosen.)
    - Chosen Approach:
      - Read the producers end to end (`src/server/agent_picker.rs`,
        `src/server/control_center.rs`, `src/shell/path_browser.rs`,
        `src/shell/package_ui.rs`, `src/server/menu_sessions.rs`,
        `src/shell/transient_menu.rs`, `src/protocol/menu.rs`) and the client
        consumers (`frontend/src/command-centre/{CommandCentre,CommandPalette}
        .tsx`, `frontend/src/coding-agent/Composer.tsx`,
        `frontend/src/shell/WorkspacePanes.tsx`,
        `frontend/src/shell/workspace-controller.ts`,
        `frontend/src/bridge/types.ts`), then write the table.
    - API Notes and Examples:
      ```bash
      # producers
      grep -rn 'TransientMenuOrigin::' src/ --include=*.rs
      # picker entry points
      grep -rn 'picker_kind_for_command' -A 10 src/server/agent_picker.rs
      # renderers
      grep -rn 'origin === "commandPalette"\|origin === "centered"' frontend/src
      # baseline gates
      cargo test --test protocol
      npm --prefix frontend test -- --run
      ```
    - Files to Create/Edit:
      - `plans/125-Composer-Palette-Stage-Flows-and-Centered-Sheet-Retirement.md`: append the inventory table under *Compromises Made* → renamed *Evidence* is not allowed; append it to this task's evidence list in the task body instead.
      - No product code.
    - References:
      - `src/server/agent_picker.rs:211` (`with_origin(TransientMenuOrigin::Centered)`), `:378` (prompt per kind/stage), `:657` (`picker_kind_for_command`).
      - `src/server/control_center.rs:188-196` (catalogue session, `CommandPalette` origin).
      - `src/shell/path_browser.rs:317`.
      - `src/shell/package_ui.rs:113-125` (`PackageOverlayAnchor`, `Centered` is Clay-internal only), `:597-608`, `:822` (`centered_rect`), `:1229`.
      - `src/server/menu_sessions.rs:488-494` (origin mapping), `:301` (`backspace`).
      - `frontend/src/command-centre/CommandCentre.tsx` (centered sheet, own input, `secretPrompt` heuristic).
      - `frontend/src/command-centre/CommandPalette.tsx` (composer palette), `command-centre.module.css` (`.surface`, `.palette`).
      - `frontend/src/coding-agent/Composer.tsx` (`/`-sigil rule, submit routing), `coding-agent.module.css:432` (mentions shadow).
      - `frontend/src/shell/WorkspacePanes.tsx` (mounts `CommandCentre`; `paletteMenu` filter), `workspace-controller.ts:619` (`menuActivate(secondary)`).
  - Test Cases to Write:
    - Inventory completeness check: `grep` for `TransientMenuOrigin::Centered` and `CommandCentre` returns only entries the table lists (a shell one-liner recorded in the evidence).
    - Baseline: `cargo test --test protocol` and the frontend suites pass before any change.

- [x] Review Clay UI catalog and plan primitive/component reuse before UI work
  - Completion Evidence (2026-09-18, task 2):
    - Documentation read for this task: `DESIGN.md` §5/§6/§7/§9/§11/§12/§14
      (the surface, elevation, focus-ring, palette and shell rules the reuse
      table is checked against); `.agents/skills/clay-execution/references/
      components.md` (the surface→slot index and "Rules for Adding
      Components"); `references/tokens.md`; `references/ui.md`;
      `docs/reference/ui-components.md`;
      `docs/reference/primitives/shell-layout-strategy.md`;
      `docs/development/ui-design-system-recipe-matrix.md` (property bounds,
      the whole-shell family list, pre-bootstrap core set).
    - Commands run: the task's two API-note greps
      (`recipeAttributes(...)` in `frontend/src/command-centre` +
      `coding-agent/Composer.tsx`; `--clay-ds-command-centre`/`--clay-ds-menu`
      in the two CSS modules), a `python3` dump of
      `packages/design-instrument/package.json` (165 shipped keys) for the
      shadow values, and the conformance check below.
    - Reuse table (flow/element → cataloged owner → where it is consumed today
      → added?):
      - Sheet frame (radius 16, `spacing.sm` padding, opaque `surface.overlay`,
        one hairline, 240ms spring) → `commandCentre` root, a whole-shell family
        (host CSS consumer, not a package kind) →
        `frontend/src/command-centre/command-centre.module.css:198-232`
        (`.palette`) → **reused**.
      - Rows, selection, detail line → `list.root/row/rowTitle/rowDetail` →
        `CommandPalette.tsx:127-155` (`recipeAttributes("list", …)`) over
        `.palRow` → **reused**; every picker stage's rows already exist
        server-side (`Stage::Secret` → "Store API key", `Stage::Url` →
        "Save base URL", `Stage::Oauth` → "Open in browser"/"Copy URL",
        prompts at `src/server/agent_picker.rs:368-383`), so a stage is the same
        sheet with a prompt and fewer rows.
      - Scope chips → `seg.root`/`seg.item` (`CommandPalette.tsx:103-118`) →
        **reused** (already consumed; nothing stage-specific).
      - Chords and hints → `kbd.root` + the `keyHint` gap vars in the foot →
        `ClayKbd` in rows/foot → **reused** (stage-back and secondary-action
        hints are more `ClayKbd`s in the same foot).
      - Empty/no-match state → `empty.root` +
        `commandCentre.default.empty.rest` → `.empty`/`.emptyHint` → **reused**
        (a stage with no rows uses the same bare centred text).
      - Veil → `modal.scrim` (`surface.scrim` @0.5 + blur 3) →
        `frontend/src/shell/workspace-panes.module.css` `.veil` → **reused**,
        untouched.
      - **Stage prompt line (gap G2)** → host class `.prompt` (caption size,
        letterspaced, `text.muted`) written for exactly this case
        (`command-centre.module.css:101-108`) → **reused — must be kept, not
        deleted**; this corrects task-1 evidence C10 (and task 8's deletion
        list, fixed there).
      - **Shielded secret field (gap G4)** → `textInput.field/label/input/
        description/error`; the `input` slot already ships `rest`/`hover`/
        `focus`/`disabled`/`invalid` states → `components/text-field.module.css`
        → **reused**; the single host-side addition is a
        `type?: "text" | "password"` prop on `ClayTextField` (pass-through to
        React Aria `<Input>`), which is a host *prop* — not a kind, slot, key,
        token, or CSS family.
      - Focus ring → the composer box stays the boundary (the sheet draws
        none) → unchanged, `DESIGN.md` §14.4 one-ring rule.
    - Statement (AC): the stage flows need **no new component kind, recipe
      slot, recipe key, token, or CSS family**. The named gaps and their
      justification:
      - (a) `ClayTextField type="password"` — a bespoke masked input would
        duplicate the `textInput` family and its `aria-describedby`/error-slot
        wiring.
      - (b) The foot's hint strings are host-owned literals, not design-system
        data → stage flows only make them mode-dependent (`↵ run`,
        `↵ store`, `Alt+↵ delete`, `Esc back`).
      - (c) The three picker stages add no surface: rows, prompts and intents
        are already in the picker session.
      - (d) `.palette` carries no `data-clay-component` projection attributes,
        consistent with the mentions menu and the rest of the shell chrome:
        `commandCentre`/`menu`/`popover` are whole-shell families, are absent
        from `KNOWN_COMPONENT_KINDS`, and design-system variables are installed
        globally on `document.documentElement`
        (`frontend/src/state/design-system-store.ts:59-66`), so package recipes
        reach the sheet through the CSS variable lookup without attributes.
    - Halo (the one shadow change) is a **value change on two existing keys,
      zero new names**: today
      `commandCentre.default.root.rest.shadow` =
      `[{y:24,blur:60,spread:-16,text.primary@0.42},
      {y:2,blur:10,spread:-4,@0.22}]` and
      `menu.default.root.rest.shadow` =
      `[{y:14,blur:34,spread:-14,@0.34},{y:1,blur:3,spread:-1,@0.16}]`
      (`packages/design-instrument/package.json`, mirrored as static fallbacks
      at `frontend/src/styles/tokens.css:252-256,512-515`). The halo replaces
      those two `shadow` values only, inside the existing `shadow` bound
      (≤3 layers, blur ≤64, |x|/|y| ≤32, spread ±16, theme role + opacity, and
      blur > 0 so it never becomes a banned hard offset shadow).
      `popover.default.root.rest` still ships `menu`'s values — it is not in
      the user's request (it backs trigger-anchored dropdown popovers, which
      have no veil), so it keeps the drop shadow; recorded as a deliberate
      divergence for tasks 3/4 to confirm against the prototype.
    - Performance (AC): the palette's render budget is bounded end to end.
      Server: one snapshot per query update, fuzzy-scored over the installed
      candidate list (≤ `TRANSIENT_MENU_MAX_ITEMS` = 256) with no document or
      filesystem work — advisory p95 budgets 50ms open / 4ms per update
      (`src/perf/budgets.rs:294-302`), and the catalogue snapshot is not rebuilt
      per query (`src/server/control_center.rs:861`). Wire: per-row caps (label
      128, detail 256, scope 16, ≤4 bindings × 32 chars, a11y label 256) pinned
      by the CI test `worst_case_transient_menu_snapshot_stays_inside_the_frame_cap`
      (`src/perf/baselines.rs:398-405`). Client: the palette filters, scores and
      sorts nothing — it renders `menu.items` as-is; a keystroke updates one
      string prop (the field echo) and re-renders at most 256 rows, then the
      composer's `menuQueryUpdate` brings the new snapshot. No virtual scrolling
      is needed at 256 rows; if profiling ever shows the echo-only re-render,
      memoizing the row list on `menu.items` identity is the one-line upgrade
      path (no new machinery).
    - Code Quality (AC): the sheet, rows, chips and foot are owned by
      `frontend/src/command-centre/command-centre.module.css` **alone** after
      retirement (once `CommandCentre.tsx` is deleted, `CommandPalette.tsx` is
      its only importer); adjacent surfaces keep their own modules — veil
      `shell/workspace-panes.module.css`, lane `shell/agent-lane.module.css`
      (plus the composer's `coding-agent/coding-agent.module.css`), `@` mentions
      `coding-agent/coding-agent.module.css` (`menu.*`). Marked for deletion, not
      reuse: `.surface`, `.menu`, `.surface:focus-within`, `.head`, `.input`,
      `.input::placeholder`, `.results`, `.hintKeys`; with `.menu` gone the
      module stops consuming `popover.*` (11 refs), so the `popover` family
      entry loses `command-centre/command-centre.module.css` in
      `frontend/src/test/design-system-consumption.test.ts` (`:286-289`), and
      `test/overlay-composition.test.ts:108-122` retargets from
      `.surface`/`.input` to `.palette`. `.prompt` **stays** (stage prompt
      line). `--clay-dimension-overlay-centered-width` **stays** — two live
      consumers (`components/modal.module.css:32`,
      `packages/package-workspace.module.css:125`) — and only its two
      `command-centre.module.css` uses (`:27,29,52`) are swept.
    - Security (AC): the shielded stage adds no authority. The value travels the
      existing path: shield → `menuQuery` intent → session query →
      `AgentPicker::set_query` → (on `Stage::Secret`) `merge_secret_query`
      masks the echoed snapshot query (`src/server/agent_picker.rs:625`, pinned
      by `secret_is_not_in_snapshot_query_or_labels` `:753`) → `menuActivate` →
      `AgentPickerActivate::PutSecret { provider, name, secret }` →
      `host.put_credential(...)` in `src/server/connection/menus.rs:642-695`,
      the existing server-side credential authority, which emits
      `agent.credential_stored` and cancels the session. No package code, hook,
      or JS API is involved, the composer draft is never the secret transport
      (task 7's invariant), and the wire echo stays masked.
    - Conformance check (AC test): `npm --prefix frontend test -- --run
      design-system-consumption overlay-composition` → **27 passed** (19 + 8) at
      the pre-change tree. The reuse list adds no `--clay-ds-*` name (the halo
      changes two existing values), so no adoption-backlog growth is expected;
      the only consumption-list edit is the `popover` entry above.
  - Acceptance Criteria:
    - Functional: The task records which cataloged primitives already carry the flows (the `commandCentre` sheet, `list` rows, `seg` scope chips, `menu`/`popover` surfaces, `textInput` shells, `kbd`, `statusItem`) and states explicitly that no new component kind, recipe key, token, or CSS family is needed for the stage flows; any gap is named with its justification.
    - Performance: The review states the palette's render budget (bounded rows from the server; no client-side filtering, scoring, or re-render per keystroke beyond the echoed query string).
    - Code Quality: The review lists the exact CSS modules that own the palette after retirement (`command-centre.module.css` only) and marks `CommandCentre`'s `.surface`/`.input`/`.menu` classes for deletion rather than reuse.
    - Security: The review confirms the shielded secret stage needs no new authority: the value still travels through the existing `menuQuery` intent to the existing provider-credential path, and no package code is involved.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §6/§11/§12/§14; `.agents/skills/clay-execution/references/components.md`; `references/tokens.md`; `docs/reference/ui-components.md`; `docs/reference/primitives/shell-layout-strategy.md`.
    - Options Considered:
      - New `picker.*` recipe family for the stage flows: rejected — the flows are the same sheet with the same rows.
      - Reuse `commandCentre.default.*` + `list.default.*` + `seg.default.*` as-is. (Chosen.)
    - Chosen Approach:
      - Catalogue the existing recipe usage (verified in this plan: `shell.default.footer.rest`, `commandCentre.default.root/empty/status`, `modal.default.scrim.rest`, `list.default.row.*`, `seg.default.*`, `kbd`, `textInput` shell) and prove the stage flows fit inside them.
    - API Notes and Examples:
      ```bash
      grep -rn 'recipeAttributes(' frontend/src/command-centre frontend/src/coding-agent/Composer.tsx
      grep -rn '--clay-ds-command-centre\|--clay-ds-menu' frontend/src/command-centre/command-centre.module.css frontend/src/coding-agent/coding-agent.module.css
      ```
    - Files to Create/Edit:
      - `plans/125-…`: the reuse table in the task evidence.
    - References:
      - Plan 124 task 7/8 evidence (palette rendering split, no new recipes).
      - `frontend/src/command-centre/command-centre.module.css:24-90` (`.surface`, `.menu`, `.surface:focus-within`).
  - Test Cases to Write:
    - Conformance check: `npm --prefix frontend test -- --run design-system-consumption` stays green with the reuse list (no new `--clay-ds-*` names outside the halo task's single addition).

- [x] Build the HTML prototype for the palette stage flows and the halo in `design-artifacts/prototypes/composer-palette-stages/`
  - Completion Evidence (2026-09-18, task 3):
    - Artifact set (`design-artifacts/prototypes/composer-palette-stages/`):
      `palette-stages.html` (1183 lines: the plan-124 page's shell chrome —
      title bar, sidebar, status bar, inspector, page `<style>` — taken
      verbatim, plus the new scenes, the stage-aware sheet and the halo board),
      `palette-stages.css` (plan 124's `lane.css` verbatim plus a marked
      plan-125 block), `README.md`, and the kit
      (`theme.css`, `ds-quiet.css`, `components.css`, `pages.css`, `ds.js`)
      copied **byte-identically** (md5s checked pairwise against
      `../agent-lane-palette/`).
    - Scenes (18, every one reachable by keyboard in the page): `lane`,
      `lane-railed` (both rails active — the lane stops at the inspector's edge),
      `lane-full` (both rails hidden — the lane is the working area's width),
      `catalogue`, `catalogue-empty`, `catalogue-no-match`, `catalogue-scopes`,
      `catalogue-chords`, `stage-provider`, `stage-auth`, `stage-secret`,
      `stage-url`, `stage-oauth`, `stage-model`, `stage-session`, `stage-back`,
      `mentions`, `halo` — covering AC (a)–(o), the long model list and the
      stage-back affordance included.
    - Verification: `design-artifacts/tools/capture-palette-stages.mjs` driven
      over `file://` — **144 matrix runs** (18 scenes × 4 shipped themes × 2
      widths) + **15 keyboard steps**, **159 checks, 0 failures** (`PASS 159
      checks across 144 matrix runs + 15 keyboard steps`). Per run it asserts:
      theme and scene applied, no console error, no non-`file://` request, no
      horizontal overflow, `backdrop-filter: none` on the sheet and the
      mentions menu (the veil alone carries `blur(3px)`, open exactly while a
      menu is), the sheet's geometry (below), the stage prompt vs the catalogue
      head, scope chips only for the catalogue, the foot's verbs and back
      affordance, the session row's `Alt+↵ delete`, the secret field's masking,
      the halo board's four cells and the live toggle between both values.
      Evidence: `design-artifacts/screenshots/composer-palette-stages/report.json`
      (full matrix + the keyboard walk) and 30 review frames (every scene at
      gruvbox-material-dark/wide, the halo board and the secret stage at four
      themes, the rails in a light theme, and the narrow frames that change
      geometry). The tool reproduces all 144 frames; `--filter=<regex>`
      re-captures a subset after a visual fix and its exit status is the gate.
      `--page=<file>` runs the same assertions against the frozen set.
    - Geometry verified (AC): the sheet is the field's **border box** width
      (`delta 0`) and sits **6px** above it, capped at `min(52vh, 420px)` with
      internal scroll where rows overflow (12 models → the list scrolls, the
      foot stays). The page reaches that through a `.field-slot` wrapper — the
      prototype's equivalent of the shipped `ClayTextField.fieldSlot`; the
      plan-124 page's sheet is a child of the shell's padding box and drifts by
      2px/1px (its own tool tolerated ±2px), so this page fixes the anchor
      rather than inheriting it.
    - Lane containment (added in the approval round, drawn here): the lane is
      the view pane's chrome strip, both rails run the working area's full
      height, and hiding a rail hands its width back to the lane — with both
      rails hidden the lane is the working area's whole width again. The page
      encodes it structurally (three explicit grid columns, a hidden rail's
      track collapses to `0px`, the lane lives inside `.pane.main`; the view
      pane's column is explicit because a `display: none` rail leaves the grid
      and auto-placement would slide the pane into the collapsed track). The
      tool asserts it in every run: lane == view pane within 1px, no overlap
      with a visible rail, the rail visibility the scene names, the full
      working-area width when no rail shows, and the sheet inside the lane at
      the lane's inner width (inset by the composer's own padding). Two scenes
      carry it (`lane-railed`, `lane-full`); the kit's own responsive default
      collapses the inspector under 1240px, so the page re-asserts a scene's
      rail state on `load` — the scene is the review's subject, not the kit's
      window-size heuristic.
    - Stage model as drawn (AC d, f–m): `catalogue` needs the `/` sigil and its
      query is the composer draft after it; `list` stages (provider, auth,
      model, session) keep the composer as a **sigil-free** filter; `secret` /
      `field` (base URL) own a `textInput` field **inside the sheet** and the
      composer draft stays empty; `oauth` is text (user code + verification
      URI) plus two action rows. `↵` is `run` / `choose` / `resume` / `store` /
      `save` per mode, `Esc` and `Alt+←` walk the stage trail back
      (`secret → auth → provider → close`, derived, no trail field), `Alt+↵`
      deletes the selected session. The prompt line is the kept `.prompt`
      micro-label (task 2's correction), and a stage with an empty filter or a
      value-owning field shows no query echo (the echo duplicated the prompt
      before that fix).
    - Halo as proposed (AC o): one value for both keys —
      `0 0 14px -2px` at 14% + `0 0 3px 0` at 8%, both layers offset 0,
      `text.primary` — replacing the values of
      `commandCentre.default.root.rest.shadow` and
      `menu.default.root.rest.shadow` (no new key/token; inside the existing
      `shadow` bound; blur > 0, so not a banned hard offset shadow; the border
      is untouched). The page's `--halo` holds the candidate while
      `--shadow-overlay` / `--shadow-pop` stay beside it, and the board renders
      all four cells (palette and mentions × today and candidate) on the veiled
      backdrop the sheets really sit over. Both stated caveats are in the
      README: `popover.default.root.rest` (no veil behind it) is left alone, and
      the kit's drawn today-values differ from the shipped recipe's in spread
      (`-20px`/`-14px` vs `-16px`/`-14px`), so the board's captions cite the
      shipped recipe numbers.
    - Security (AC): the secret scene is a `type="password"` field holding a
      literal bullet string; the assertions prove the value is bullets only,
      the composer draft is empty, and no row, label or meta repeats it. The
      README states that no real credential is rendered and that the approved
      behavior must not echo typed characters into the composer; the wire echo
      the page mirrors is the server's bullet mask (`merge_secret_query`, task 2
      evidence).
    - Code Quality (AC): the page's markup carries the catalog vocabulary in
      `data-clay-ds` hooks (`commandCentre.default.root.rest` on the sheet,
      `list.*` rows, `seg.*` chips, `kbd` chips, `empty`, `textInput.*` on the
      stage field, `label.default.root.rest` on the prompt) and the README maps
      each element to its owner plus the no-authority statement (implementation
      cites `design-artifacts/approved/` + `DESIGN.md` only).
    - README also records the six decisions the reviewer faces (halo values and
      their single shared value; `popover` divergence; keyboard-only back
      affordance; the field-vs-composer filter split; the un-virtualized model
      list; and the carried veil drawing divergence — the kit's canvas tint vs
      the shipped `modal.scrim` recipe), so approval can accept or reject each
      one explicitly.
  - Acceptance Criteria:
    - Functional: A self-contained artifact set (openable over `file://`, no build step) renders, at the composer's real width and 6px gap: (a) catalogue rows with a query, (b) empty query, (c) no-match empty state, (d) scope chips, (e) per-row chord chips, (f) picker stage — provider list, (g) picker stage — auth method, (h) **shielded secret** stage, (i) URL stage, (j) OAuth stage (user code + URI), (k) model picker with many rows (internal scroll), (l) session picker with `Alt+↵` delete hint, (m) stage-back affordance (`Esc`/`Alt+←`), (n) the `@` mentions menu, (o) the halo side-by-side against the current drop shadow.
    - Performance: The page switches themes without reloading scripts and stays under the artifact budget (no images beyond none, no network); the palette caps at `min(52vh, 420px)` and scrolls internally at every tested width.
    - Code Quality: Every state is reachable by keyboard in the page; the page's markup uses the catalog vocabulary in `data-*` hooks (`data-clay-ds`, `component.variant.slot.state` names) so approval maps 1:1 onto catalog entries. The README states explicitly that a prototype has **no authority** and that implementation cites only `design-artifacts/approved/` plus `DESIGN.md`.
    - Security: The prototype's secret scene shows masked characters only; its README states that no real credential value is rendered and that the approved behavior must not echo typed characters into the composer.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5/§6/§7/§9/§11/§12/§14; `design-artifacts/README.md`; `design-artifacts/approved/agent-lane-palette/README.md`; `design-artifacts/approved/quiet-instrument-migration/command-centre.html` + `ds-quiet.css:830-880` (the retired sheet's composition).
      - `.agents/skills/clay-execution/references/ui.md`; `references/components.md`; `references/tokens.md`.
      - The four project-local design skills for this substantial surface work: `impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend`.
    - Options Considered:
      - Reuse the frozen `agent-lane-palette` page and add scenes in place: forbidden — approved artifacts are append-only, and the working copy is the reviewed snapshot's source.
      - New prototype folder that imports the shared kit (`components.css`, `ds.js`, `ds-quiet.css`, `theme.css`, `pages.css`) and adds one new page + stylesheet. (Chosen.)
    - Chosen Approach:
      - Copy the shared kit files byte-identically into the new folder, add `palette-stages.html` + `palette-stages.css`, and drive scenes with the kit's existing scene switcher; add a `halo` side-by-side toggle that renders the palette and the mentions menu under both shadow recipes.
    - API Notes and Examples:
      ```html
      <!-- scene switch, theme switch and halo toggle stay URL/`data-` driven -->
      <section class="palette" data-clay-ds="commandCentre.default.root.rest"
               data-stage="list" data-halo="true"> … </section>
      ```
    - Files to Create/Edit:
      - `design-artifacts/prototypes/composer-palette-stages/README.md`: scope, scene list, four-theme coverage, how to open, catalog mapping.
      - `design-artifacts/prototypes/composer-palette-stages/palette-stages.html`: the scenes.
      - `design-artifacts/prototypes/composer-palette-stages/palette-stages.css`: page styles (halo candidate values live here, not in the kit).
      - `design-artifacts/prototypes/composer-palette-stages/{components.css,ds.js,ds-quiet.css,theme.css,pages.css}`: kit copies, unchanged.
      - `design-artifacts/tools/capture-palette-stages.mjs`: scene × theme × width capture tool (mirrors `capture-lane-palette.mjs`).
    - References:
      - `design-artifacts/tools/capture-lane-palette.mjs` (74-check capture harness to mirror).
      - `design-artifacts/prototypes/agent-lane-palette/lane.css` (palette geometry now shipped).
      - `frontend/src/command-centre/command-centre.module.css` (shipped palette geometry/typography).
  - Test Cases to Write:
    - `capture-palette-stages.mjs` scene × theme matrix (4 shipped themes × 2 widths × every scene): zero console errors, no horizontal overflow, masked secret scene, halo/shadow toggle present.
    - Keyboard pass in the tool: `↑↓` selection, `↵` run, `Esc` back/close, `Alt+↵` secondary, chip activation.

- [x] Obtain explicit user approval and freeze design-artifacts/approved/composer-palette-stages/
  - Completion Evidence (2026-09-18, task 4):
    - Approval statement, verbatim (the record quotes it in full):
      "The artifacts are approved. One thing to note here is that the agent lane
      should not go over the left and right side panes. The lane should only be
      spanning the middle pane if both left and right panes are active. If they
      are hidden then it should again take the whole width. And the composer
      palette should match the width of the lane. You need to make this
      explicit in the artifact while confirming the design. Otherwise all
      good".
    - The approval carried **one required amendment** (the lane's containment),
      so the amendment was drawn into the working copy *before* the freeze
      (`lane-railed` / `lane-full` scenes, the three-explicit-column grid, the
      `0px` hidden-rail tracks, the lane inside `.pane.main`, the sheet's
      lane-inner-width assertion) and is in the frozen set; the approved
      README's §1.1 states exactly what it resolves (plan 124's D1 reading of
      `DESIGN.md` §12 is superseded) and what it reads "match the width of the
      lane" as (the lane's inner width — the composer box — with the outer-edge
      alternative named as a change to approved plan-124 geometry, not a tweak).
    - Frozen set: `design-artifacts/approved/composer-palette-stages/` — 8 files
      (`palette-stages.html` `454f33360f6806c9`, `palette-stages.css`
      `87d574284d0816b2`, `README-prototype.md` `4a004c030755ebec`, and the kit
      `theme.css` `b1a39839be745cdb`, `ds-quiet.css` `c2aee13d84b0944a`,
      `components.css` `01f30073b77148f6`, `pages.css` `697a63cdee3d8065`,
      `ds.js` `31f53b20a194a826`), byte-identical to the working copy at freeze
      time, with `README.md` recording date/approver/scope/evidence/coverage,
      the exemption list (the tool + PNGs stay in the working copy;
      `DESIGN.md` is the amendment task's; the retired centered page is
      superseded, not edited), and the 9 binding implementation requirements
      (one sheet twice-vocabularies, the stage prompt, the shielded secret,
      mode-aware foot/rows, one refraction, the halo values verbatim, the
      lane's containment + veil coverage, no new design-system surface, plan
      124 unchanged elsewhere).
    - Kit ownership: the five kit files hash-match
      `approved/agent-lane-palette/` pairwise — this approval adds **no
      design-language value**; plan 125's ink is the page and
      `palette-stages.css` (stage furniture, halo values, rail/lane block).
    - Gate evidence: the tool passes against the frozen copy itself
      (`--page=design-artifacts/approved/composer-palette-stages/palette-stages.html`:
      `PASS 159 checks across 144 matrix runs + 15 keyboard steps`) and against
      the working copy (`PASS 159 checks across 144 matrix runs + 15 keyboard
      steps`); evidence PNGs pruned to the 30 documented review frames plus
      `report.json` (all 144 runs + the keyboard walk) under
      `design-artifacts/screenshots/composer-palette-stages/`.
    - Documentation: `design-artifacts/README.md`'s contents table gained the
      approved set, the prototype, the captures and the tool; the approved
      README's §3 item 7 binds the implementation geometry (columns, tracks,
      lane inside the pane, rails at full height, the veil covering the rails'
      lower part with the lane above it at z 41 over z 40).
  - Acceptance Criteria:
    - Functional: The chosen scenes are copied byte-identically into `design-artifacts/approved/composer-palette-stages/` with a `README.md` recording the approval date, the user's quoted statement, the chosen variant, requested changes, superseded variants, the exact surface/state/theme/width coverage, the sha256[:16] hash table, and the binding file list.
    - Performance: No runtime work — artifact only.
    - Code Quality: Losing variants stay under `design-artifacts/prototypes/composer-palette-stages/` marked *not approved*; no approved file is edited after the freeze.
    - Security: The approval record states the shielded-secret behavior explicitly, because it is a security-relevant UI decision (typed characters never rendered, never echoed into the composer draft).
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/README.md` (approved = binding, append-only); `DESIGN.md` §14 (retired patterns) to reject anything the prototype tempted.
    - Options Considered:
      - Freeze without a user statement: refused — the task stays unchecked.
      - Freeze on approval with hashes. (Chosen.)
    - Chosen Approach:
      - Present scenes + the halo toggle with the open decisions called out (stage-back chord, secret-stage focus, foot hints, halo values), record the statement verbatim, copy the files.
    - API Notes and Examples:
      ```bash
      sha256sum design-artifacts/approved/composer-palette-stages/* | cut -c1-16
      ```
    - Files to Create/Edit:
      - `design-artifacts/approved/composer-palette-stages/README.md` (approval record).
      - `design-artifacts/approved/composer-palette-stages/*` (frozen copies).
    - References:
      - `design-artifacts/approved/agent-lane-palette/README.md` (record shape).
  - Test Cases to Write:
    - Hash drift check: any later diff of an approved file is visible in `git diff` against the recorded hashes.

- [x] Amend DESIGN.md and the design-system package for the single palette surface and the halo
  - Completion Evidence (2026-09-18, task 5):
    - `DESIGN.md` (AC a–g), by chapter:
      - **§6** — the materials table's palette row now reads **`halo`** (rises
        from its own edge, §7) and the popover/menu row names the exception (the
        composer's `/` and `@` menus take the halo); the shadow-recipe list
        gains `halo` = `[{0, 0, blur 14, spread -2, text.primary @0.14},
        {0, 0, blur 3, spread 0, text.primary @0.08}]` with the recorded
        user direction ("remove the drop shadow, replace with a subtle halo"),
        the reason (a directional shadow over the shared veil reads as a hole in
        the scrim), the zero-offset/`§14.1` note, "a **value** on the two
        existing keys, not a new key — the 165-key set is unchanged", and
        "`popover.root` keeps `pop`".
      - **§9** — the accent halo's owners now include the palette's own well when
        a stage owns one (§12).
      - **§11** — `commandCentre.root` carries `halo`; it is "the only transient
        selection or input surface in the app (§12)", spans "the lane's inner
        width (the composer box it answers to)", owns no input/no ring, and names
        the shielded stage as the deliberate exception; the `menu` clause states
        the composer's own `/`/`@` menus take the halo with the palette.
      - **§12** — the lane paragraph is swept to the approved amendment: "the
        **view pane's** own chrome strip … spans the middle pane only and never
        sits over a rail, while both rails … keep the working area's full height"
        (the "end at its top edge"/"full working-area width" reading is gone),
        "a hidden rail's width goes back to the lane", and the ≤1000px drawer
        case. The command-surface paragraph states the sheet as the lane's inner
        width and names the outer-edge reading as a re-approval, not a tweak, and
        the veil now covers the views *and* the rails' full height with the lane
        above it (z 41 over 40). Two new paragraphs carry AC (a)–(c) and the
        security rule: "One surface for every picker" (stage prompt line,
        mode-aware foot incl. `alt+←` back, derived from the session's stage, the
        `/` sigil belonging to the catalogue alone, a value-owning field showing
        no echo) and "A credential is never typed into the composer" (shielded
        field inside the sheet, `type="password"`, never the draft or the lane's
        persisted state, server bullet-mask echo, sheet as that stage's boundary).
      - **§13** — invariant 10 unchanged; new invariant 11 "A credential is never
        echoed" (never rendered, never in the draft/persisted state, credential
        path alone). **§14.4** — the shielded stage is named as not a violation
        (the sheet's own well is the boundary; the box carries no focus state).
      - **§14** — item 14 is extended from the centred *command* sheet to the
        whole centred *selection* family (agent-type picker, provider setup, the
        sign-in method, the model list, the session picker, package overlay menus
        — AC (e)), with `modal.dialog` keeping its callers; new item 16 forbids a
        drop shadow under the palette or the mentions menu and states the halo as
        a value on the two keys.
      - **§15** — four checklist items added (halo + veil-up, every picker stage a
        palette session with the shielded credential, the lane's containment, and
        the existing chips item kept). **§16** — the plan-124 note now records
        plan 125's four amendments (lane containment, the retired centred
        selection family, the halo as two changed values, the sheet's inner
        width) and keeps "the key set stays exactly as counted"; the overlay-family
        bullet is swept from "overlay shadow"/"the sheet's boundary turns accent"
        to the halo and to the field-under-the-sheet ring ownership.
    - `packages/design-instrument/package.json` — the two recipes'
      `shadow` arrays are the halo (a scripted, parsed-JSON-checked edit: only
      `commandCentre.default.root.rest` and `menu.default.root.rest` differ from
      HEAD; `len(recipes) == 165` asserted).
    - `src/shell/design_system.rs` — **required by the "one language"
      invariant** (not in this task's original file list): a new
      `Elevation::Halo` stack (`0.0, 0.0, 14.0, -2.0, text.primary, 0.14` +
      `0.0, 0.0, 3.0, 0.0, …, 0.08`) and the core `commandCentre` fallback moved
      from `Elevation::Overlay` to it, so the pre-bootstrap paint equals the
      package (`plan118_core_fallbacks_match_the_shipped_language` was red
      before the change and green after). `menu.*` is not in the core subset, so
      nothing there carries it.
    - `frontend/src/styles/tokens.css` — the two `--clay-ds-*-root-rest-shadow`
      fallbacks are the projection of the new values (byte-equal after the host
      block's normalisation; `plan118_host_fallback_block_matches_the_resolved_package`).
    - Texts: `packages/design-instrument/docs/index.md` (§1 one paragraph, §4
      materials rows — popover/tooltip `pop`, composer menus + palette `halo` —,
      the shadow list's `halo` entry) and `docs/wiki/modules/ui-design-system-runtime.md`
      (the `shadow` line names the halo). No other shipped-value text carried the
      two shadows (`docs/reference/ui-design-systems.md` states the schema, not
      the values).
    - New pin: `tests/package_ui_conformance.rs::plan125_palette_and_mentions_take_the_halo_not_a_drop_shadow`
      — both keys equal DESIGN.md §6's halo, all layers zero-offset with blur > 0,
      the two keys identical, `popover` (y 14) and `modal.dialog` (y 24)
      unchanged, 165 keys. Prototype-vs-spec: the frozen
      `approved/composer-palette-stages/palette-stages.css` draws exactly these
      numbers (`0 0 14px -2px` @14% + `0 0 3px 0` @8% in `text.primary`), so the
      pin needs no divergence entry; `DESIGN.md` remains normative if they ever
      part (approved README §1).
    - Gates: `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
      clean; `cargo test --test presentation` **62 passed, 0 failed** (incl. the
      new pin, the core-fallback parity and the host-block parity);
      `npm --prefix frontend test -- --run` **478 passed / 51 files**; the
      targeted `design-system-consumption` + `surface-adoption` runs green.
    - Deliberately left to their own tasks: no host-CSS value edits (the two
      consumers read the recipe variables, `command-centre.module.css:38/74/216`
      and `coding-agent.module.css:432`, and the `.surface` centred class is
      deleted by the retirement task), no lane/layout implementation (task 6), no
      behaviour change for the secret stage (tasks 7/8), and the `@`/`/` menus
      keep their `border.hairline` (the colored focus border stays withdrawn).
  - Acceptance Criteria:
    - Functional: `DESIGN.md` states (a) the composer palette is the only transient selection/input surface — the window-centered sheet is retired; (b) palette sessions carry a stage/prompt line, stage-aware foot hints, and the shielded secret stage; (c) the composer box remains the boundary and the sheet draws no ring of its own; (d) the palette's and the `@` mentions menu's shadow is the **halo value** on the existing `commandCentre.default.root.rest` / `menu.default.root.rest` keys (a value change — no new recipe key, and `popover.default.root.rest` keeps its drop shadow); (e) §14 gains the retired centered pattern; (f) **the lane is the view pane's chrome strip** — it spans the middle pane only, the workspace sidebar and the agent inspector keep the working area's full height, a hidden rail's width goes back to the lane (so with both rails hidden the lane is the working area's whole width), and the chapter's sentence saying the rails "end at its top edge" and the lane spans the full working-area width is swept (§12 + the §16 plan-124 note), with the veil covering the rails' full height and the lane above it; (g) the chapter states the sheet's width as the lane's inner width (the composer box it answers to) and names the outer-edge reading as a re-approval, not a tweak.
    - User direction recorded: "Keep the current design and remove the drop shadow. Replace with subtle halo. Include this as a task in the plan" (2026-09-18) and the earlier "The center palette has to be retired and everything has to happen with the new palette position" — the colored focus border was explicitly withdrawn ("skip the colored border part for composer palette as it creates confusion in terms of focus"), so the palette keeps `border.hairline` and draws no ring of its own.
    - Performance: The added `halo` recipe stays inside the bounded shadow grammar (≤2 layers, spread ≥ −16, blur ≤ 64, opacity ≤ 0.5) and adds no backdrop blur.
    - Code Quality: `packages/design-instrument/package.json` carries the halo values for `commandCentre.default.root.rest` and `menu.default.root.rest`, `frontend/src/styles/tokens.css` mirrors them as host fallbacks, and no recipe key is added or removed (the fixed 165-key set stays fixed).
    - Security: The secret-stage rule is normative text: typed credential characters are never rendered (masked echo only) and never written into the composer draft or the lane's persisted state.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5/§6/§7/§9/§11/§12/§13.7/§14/§14.4; `design-artifacts/approved/composer-palette-stages/README.md` (binding); `.agents/skills/clay-execution/references/tokens.md`.
      - `tests/package_ui_conformance.rs:1645-1675` (static-surface/no-shadow rules), `:2655-2670` (hard-offset rule), `:1005-1012` (shadow color roles).
    - Options Considered:
      - New recipe keys (`halo.*`): rejected — the key set is frozen at 165 and the halo is a value change on two existing keys.
      - Change the two existing shadow values to the halo layers in both the package and the host fallbacks. (Chosen.)
      - Host-CSS-only override: rejected — the design system is the authority for elevation; a host override makes the package lie.
    - Chosen Approach:
      - Write the halo as two even (zero-offset) layers, e.g.
        `{0, 0, blur 14, spread −2, text.primary @0.14}` and
        `{0, 0, blur 3, spread 0, text.primary @0.08}`, chosen in the approved
        prototype; apply to `commandCentre.default.root.rest` (palette) and
        `menu.default.root.rest` (mentions); keep 1px `border.hairline`.
    - API Notes and Examples:
      ```json
      "menu.default.root.rest": {
        "backgroundColor": "surface.overlay",
        "borderColor": "border.hairline",
        "borderWidth": 1,
        "borderRadius": 12,
        "shadow": [
          { "x": 0, "y": 0, "blur": 14, "spread": -2, "colorRole": "text.primary", "opacity": 0.14 },
          { "x": 0, "y": 0, "blur": 3, "spread": 0, "colorRole": "text.primary", "opacity": 0.08 }
        ]
      }
      ```
      ```css
      /* frontend/src/styles/tokens.css (fallback must match the package) */
      --clay-ds-menu-default-root-rest-shadow:
        0px 0px 14px -2px color-mix(in srgb, var(--clay-text-primary) 14%, transparent),
        0px 0px 3px 0 color-mix(in srgb, var(--clay-text-primary) 8%, transparent);
      ```
    - Files to Create/Edit:
      - `DESIGN.md`: §6 materials rows + the `halo` value on the two keys; §11 `commandCentre.root` / `menu.root` lines; §12 single-surface rule + stage flows + the lane's containment (view pane only, rails at full height, the sheet the lane's inner width); §14 retired centered sheet; §9/§14.4 boundary note for the shielded stage; §16 (the plan-124 amendment note) swept for the superseded full-width lane sentence at `DESIGN.md:562-568`.
      - `packages/design-instrument/package.json`: the two recipe shadow values.
      - `packages/design-instrument/docs/index.md`: provider/reference text for the changed recipes.
      - `frontend/src/styles/tokens.css:252,512`: mirrored fallbacks.
      - `src/shell/design_system.rs` (`Elevation::Halo` + the core
        `commandCentre` fallback): the same values, because `@clay/core` must be
        the host-consumed subset of one language (§16).
      - `frontend/src/command-centre/command-centre.module.css:38,73`: palette keeps `commandCentre` shadow → now halo; `.surface` centered class deleted in the retirement task (keep this task to values only).
      - `frontend/src/coding-agent/coding-agent.module.css:432`: mentions shadow → halo.
    - References:
      - `tests/package_ui_conformance.rs` shadow-layer assertions (modal/popover 2 layers, static families 0).
      - `frontend/src/test/design-system-consumption.test.ts` (provenance + no-literal gates).
      - `packages/design-instrument/README.md` (declarative data, no literals).
  - Test Cases to Write:
    - `cargo test --test presentation package_ui_conformance::` — package validates, key set unchanged, shadow bounds hold, and the new `plan125_palette_and_mentions_take_the_halo_not_a_drop_shadow` pin holds the halo against `DESIGN.md` §6.
    - `npm --prefix frontend test -- --run design-system-consumption` and `surface-adoption` — halo variables are backed, no raw colors.
    - Prototype-vs-spec: the halo values in the package equal the approved prototype's accepted values (asserted by a doc/registry pin added in the authoring task).

- [x] Confine the agent lane and the palette to the view pane (rails at full height)
  - Completion Evidence (2026-09-18, task 6):
    - `frontend/src/routes/workspace.module.css` — three placements, as the plan
      chose:
      - the lane: `.view [data-clay-ds="shell.default.footer.rest"]` is
        `grid-column: 1; grid-row: 2` (was `grid-column: 1 / -1`), so it shares
        column 1 with the views and can never paint over the rail's column;
      - the rail: `.rail` is `grid-row: 1 / -1` (was `1`), so it — and the
        workspace sidebar, which is the working area's left split panel outside
        this grid — runs the working area's full height;
      - the veil: `.view [data-panes="veil"]` is `grid-area: 1 / 1 / -1 / -1`
        (was `1 / 1 / 2 / -1`), so it covers the views, the lane's own cell and
        the rail's whole height; the lane stays above it at z 41 over z 40.
      `display: contents` on `.viewMain`/`.host` is unchanged, every item is
      explicitly placed, and the ≤1000px block keeps its single column with the
      rail out of flow (fixed drawer) — the lane stays column 1, row 2 there.
    - Comment sweep in the same sheet plus `frontend/src/shell/workspace-panes.module.css`
      (the host/veil rationale): the lane is "the view pane's strip" rather than
      "the working area's own chrome strip", the veil's box is the whole grid,
      and the two-row/column-2 rationale names the rails' full height. No new
      element, no JS, no second boundary: the lane keeps `border-top` + z 41, the
      veil keeps one element at z 40 (the mechanism the plan-124 tests already pin).
    - Tests (`frontend/src/test/workspace-composition.test.tsx`): the plan-124
      pin was retargeted — "keeps the lane in the view pane's column and the
      rails at full height" now asserts `grid-column: 1;` + `grid-row: 2;`, that
      no `grid-column: 1 / -1` rule exists for the lane, `.rail` `grid-row: 1 / -1`,
      the veil's `grid-area: 1 / 1 / -1 / -1`, and keeps the z-order pins; the new
      case "places every grid item explicitly, so a hidden rail cannot move the
      lane" asserts all four placements, that the sheet uses no `grid-auto-flow`,
      and that `.view[data-rail="collapsed"]` drops the rail's track. Suite: **479
      passed / 51 files** (was 478).
    - Real-layout verification (temporary CDP probe, not committed — the plan
      defers the fixture/live capture matrix to the review task): the DEV fixture's
      real `.view` grid was driven in headless Chromium at **1500 and 1024** with
      the shell's own item set (the fixture's auto-placed pane root marked
      `data-panes="view-area"` so the item set is view area + rail + lane + veil).
      **12/12 checks passed**, measured boxes: rail `y 40 h 872 == .view` at both
      widths (and `w 340`/`312`); exactly **two tracks** (`"800px 72px"`, no
      implicit row) with and without the rail; lane `x 0, w 1160` (1500) / `712`
      (1024) with `right == rail.x` and `bottom == .view.bottom`; veil box == the
      `.view` box; rail collapsed → lane `w == .view.w` at the **same** `y`/`h` and
      the pane still in row 1 (`y == .view.y`, `h == .view.h - lane.h`, and it
      takes the rail's width back). The palette sheet needed no change: it is a
      child of the field shell (plan 124's anchor), so it tracks the lane 1:1 and
      cannot cross into the rail.
    - Fixture-fidelity finding for the **review task**: no DEV fixture renders the
      lane inside the real shell grid (`WorkspacePanes` fixtures sit in a flex
      `.fixture` box; `?fixture=package-ui` is the only real `.view` grid and its
      pane root is auto-placed — in the first probe run that auto-placed item was
      pushed into an implicit third row once the lane existed). Marking that pane
      root `data-panes="view-area"` (or adding a shell-grid fixture scene) is what
      the capture matrix will need before it can assert the containment.
    - Documentation: the four wiki sentences plan 124's full-width reading left
      behind were swept (`react-shell.md` grid paragraph + veil paragraph + the
      composition-test list, `control-center.md`'s veil/lane paragraph,
      `ui-review-harness.md`'s plan-124 record now names the plan-125
      supersession, `index.md`'s react-shell summary). The wiki task still owns the
      full post-implementation pass.
    - Formatting: `frontend/src/styles/tokens.css` (task 5 leftovers) and the
      plan-124-era `frontend/src/components/text-field.tsx` /
      `frontend/src/routes/workspace.tsx` were reformatted, so
      `npm --prefix frontend run format:check` is clean again.
    - Gates: `npm --prefix frontend test -- --run` 479/479, `run typecheck` clean,
      `run lint` clean, `run format:check` clean; `cargo test --test presentation`
      62/62; `cargo test --test protocol` 219/219 (its first run flaked on
      `agent_protocol::run_set_options_forwards_to_daemon`, green on rerun and in
      isolation — the known environment-dependent flake class); the
      component-conformance audit stays 18/18.
  - Acceptance Criteria:
    - Functional: The lane occupies `.view`'s first column only (never the rail's track): with the agent rail expanded it stops at the rail's left edge, with the rail collapsed it takes the rest of `.view`, and with the workspace sidebar hidden too it is the working area's whole width. The agent rail spans both of `.view`'s rows, so it and the workspace sidebar run the working area's full height and the lane's hairline stops at their inner edges. The palette sheet keeps its plan-124 anchor (the composer field's border box, `delta 0`, 6px above it) and therefore follows the lane 1:1 — it can never cross into the rail — and the `@` mentions menu follows the same field. Behaviour of the lane itself (composition, per-tab visibility, chords, run signal) is unchanged.
    - Performance: No runtime work beyond the existing layout — one grid re-flow per rail toggle, no observer, no measurement, no JS.
    - Code Quality: `.view`'s composition stays declarative in one stylesheet (`frontend/src/routes/workspace.module.css`): the lane's placement is explicit (`grid-column: 1; grid-row: 2`) rather than a full-row span, the rail spans `grid-row: 1 / -1`, and every item that can be `display: none` keeps an explicit placement so a hidden rail cannot shift another item's track through auto-placement. The veil keeps one element and covers the panes plus the rail's full height; the lane stays above it (z 41 over z 40) and draws no second boundary (`border-top` only, as approved).
    - Security: No new surface or authority: a layout change only; the field remains the palette's query owner and the shielded secret stage is untouched (task 6/7 own its behaviour).
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/approved/composer-palette-stages/README.md` (binding: §1.1 the amendment and what "the lane's width" means, §3 item 7 the geometry and the veil's new coverage) and its `README-prototype.md` (the `lane-railed` / `lane-full` scenes, the containment assertions); `DESIGN.md` §12 (the sentence task 5 sweeps: `DESIGN.md:562-568`); `docs/wiki/modules/react-shell.md:185` (the composition test it names).
      - `frontend/src/routes/workspace.module.css:26-70` (`.view` grid, `display: contents` on `.viewMain`, the veil's `grid-area: 1 / 1 / 2 / -1`, the lane's `grid-column: 1 / -1`, the rail's `grid-column: 2; grid-row: 1`), `frontend/src/shell/agent-lane.module.css:1-10` (the lane's z 41 and hairline), `frontend/src/app/layout/working-area.tsx` (the sidebar is the working area's left split panel — outside `.view`, already full height).
    - Options Considered:
      - Keep the lane's `grid-column: 1 / -1` and give the rail `grid-row: 1 / -1` only: rejected — the lane would still paint over the rail's column and veil it.
      - Move the lane into `.viewMain` (the route's own wrapper) so it is a child of the view area: rejected — the lane is the tab's chrome owned by `WorkspacePanes`, and `display: contents` is what lets it be this grid's item without a second wrapper.
      - Implicit grid placement (drop the explicit `grid-column` lines): rejected — a `display: none` rail leaves the grid and auto-placement slides the view pane and the lane into its track (found in the plan-125 prototype, where the lane collapsed to 0px width).
      - Explicit placement, the rail spanning both rows, and the veil covering all rows. (Chosen.)
    - Chosen Approach:
      - `frontend/src/routes/workspace.module.css`: `.view [data-clay-ds="shell.default.footer.rest"] { grid-column: 1; grid-row: 2; }`; `.rail { grid-column: 2; grid-row: 1 / -1; }`; `.view [data-panes="veil"] { grid-area: 1 / 1 / -1 / -1; }`; comment sweep so the sheet's geometry paragraph names the view pane rather than the full working area (the lane is no longer "the working area's own chrome strip"). The ≤1000px block keeps its single-column rule and its fixed-drawer rail; the lane stays in column 1, row 2 there.
    - API Notes and Examples:
      ```css
      /* the lane is the view pane's strip: it shares column 1 with the view area */
      .view [data-clay-ds="shell.default.footer.rest"] { grid-column: 1; grid-row: 2; }
      /* the rail is full height, so it and the sidebar never end at the lane */
      .rail { grid-column: 2; grid-row: 1 / -1; }
      /* the veil covers the panes and the rail's whole height; the lane rides above it */
      .view [data-panes="veil"] { grid-area: 1 / 1 / -1 / -1; }
      ```
    - Files to Create/Edit:
      - `frontend/src/routes/workspace.module.css` (placement + veil area + comments).
      - `frontend/src/test/workspace-composition.test.tsx` (the pins for the new composition: the lane in column 1 / row 2, the rail `1 / -1`, the veil all-rows, `display: contents` kept, z 41 over 40 kept).
    - References:
      - Plan-124 review defect D1 (the full-width change this supersedes) and D2 (the veil's coverage) in `code-reviews/screenshots/2026-09-17-plan124-agent-lane-palette/review-log.md`.
      - `design-artifacts/prototypes/composer-palette-stages/palette-stages.css` (the rail/lane block this mirrors) and its two containment scenes.
  - Test Cases to Write:
    - `npm --prefix frontend test -- --run workspace-composition` — the composition pins (lane's column/row, the rail's row span, the veil's area, the z-order) fail if any of the three placements regresses.
    - New case in the same file: a hidden rail does not move the lane (assert the lane's placement is explicit in the stylesheet, i.e. the auto-placement trap stays closed).
    - `npm --prefix frontend test -- --run WorkspacePanes` / `AgentLane` — unchanged behaviour of the lane's mount, per-tab visibility and z-index.
    - Live-app / fixture check on the review task's list: rails both visible (lane stops at the rail), rail collapsed (lane wider), both hidden (lane the working area's width), palette open in each (sheet inside the lane, the rail veiled top to bottom) at 1500 and 1024 — added to the manual test plan with this task's step IDs.

- [x] Make every picker a composer-palette session behind a typed presentation field
  - Completion Evidence (2026-09-18, task 7):
    - Protocol (v32, additive like v31's `scope`):
      - `src/perf/budgets.rs`: `TRANSIENT_MENU_MAX_MODE_CHARS = 16`.
      - `src/protocol/menu.rs`: `TransientMenuSnapshotData.mode: Option<String>` —
        documented as the closed vocabulary (`catalogue` | `path` | `picker` |
        `secret` | `url` | `oauth`) and as "absent decodes as the catalogue" —
        plus `with_mode`, which clamps. `new()` leaves it absent, so no existing
        call site changed.
      - `src/protocol/mod.rs`: `PROTOCOL_VERSION = 32` with its version note.
      - `src/perf/baselines.rs`: the worst-case snapshot helper now sets a
        max-length mode, so the frame-cap baseline covers the new field (still
        inside the cap).
    - Shell session: `TransientMenuSession.mode` + clamped `with_mode` +
      `mode()`; `from_snapshot_data` carries it; the `TransientMenuOrigin::Centered`
      doc now records that plan 125 retired its last live producer.
    - Projection: `snapshot_from_session` attaches the session's mode when set.
    - Producers: `AgentPicker::session()` is `CommandPalette` +
      `Stage::mode()` (`List`/`AuthMethods` → `picker`, `Secret` → `secret`,
      `Url` → `url`, `Oauth` → `oauth`); the catalogue declares `catalogue`; the
      path browser declares `path` on both the normal and the error branch.
      No producer emits `Centered` anymore.
    - Stage-back (gap G3): `Stage::previous()` (`Secret`/`Url`/`Oauth` →
      `AuthMethods` → `List`) and `AgentPicker::backspace` ascends one stage when
      the filter is empty (popping a character otherwise, as before);
      `ProviderSetup` unwinds to the `Provider` list, and the ascent clears the
      query, so a typed secret never rides the way out. Derived from the stage —
      nothing stored, nothing added to the wire.
    - Tests: `every_picker_stage_is_a_palette_session_with_its_mode` (mode table
      + the pin that exactly one stage claims `secret`, every stage-less kind on
      `CommandPalette`/`picker`, and a walk that reaches `secret`, `url` and
      `oauth` through the real activation chain);
      `stage_back_derives_the_previous_stage_and_drops_the_secret` (the previous
      table plus the walk, including the provider-list floor); catalogue and path
      browser mode assertions (the path one covers the error branch);
      `palette_mode_round_trips_and_tolerates_absence` in `protocol::menu` (rkyv +
      JSON, present and absent) and the clamping assertion folded into
      `constructor_clamps_every_bounded_field`.
    - Pins: the three handshake pins moved to 32
      (`tests/window_management_protocol.rs`, `tests/agent_protocol.rs`,
      `tests/editor_intelligence_protocol.rs`).
    - Client: `frontend/src/bridge/types.ts` gains `mode?: string | null`
      (type-only — the envelope already casts, so no parse change and no origin
      special-casing beyond it). The lane already routes every `commandPalette`
      session to the sheet, so a picker now renders there; its stage line and
      shield are task 8's rendering work.
    - Documentation: `docs/wiki/modules/protocol-codec.md` (version list + the
      handshake-pin line) swept; the wiki task owns the full pass.
    - Gates: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
      clean, `cargo test --test protocol` 219/219, `--test presentation` 62/62,
      `--test runtime command_execution` 20/20, `cargo test --lib` 1383 pass with
      one pre-existing flake
      (`server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`,
      a shared registration-queue test that also fails at HEAD in a clean
      worktree — 5 failures there including this one — while the whole
      `server::js_runtime` module alone is 221/221 green); frontend 479/479,
      typecheck/lint/format clean; the component-conformance audit is unchanged
      (no design-system surface was touched).
  - Acceptance Criteria:
    - Functional: `AgentPicker::session()` emits `origin = CommandPalette` with a bounded `mode` (closed vocabulary: `picker`, `secret`, `url`, `oauth`); the command catalogue emits `catalogue`; the path browser emits `path`. Every picker kind and stage keeps its current rows, selection, filtering, and activation semantics; `controlCenter.open` still opens the catalogue. The session shapes match the states frozen in `design-artifacts/approved/composer-palette-stages/` (stage prompts, modes, row shapes).
    - Performance: No new per-keystroke work: the same single menu session per tab, the same server-side scoring, one intent per keystroke, unchanged snapshot size ceilings (`TRANSIENT_MENU_*` budgets).
    - Code Quality: The presentation field is added to the protocol snapshot additively at version 32 with a bounded setter (`with_mode`, `TRANSIENT_MENU_MAX_MODE_CHARS = 16`), a decode path that tolerates absence (defaults to `catalogue` for `CommandPalette` snapshots), and no client-visible origin special-casing beyond it.
    - Security: The `secret` mode is set only by `Stage::Secret`; no other stage can claim it (pinned by a test), and the server keeps masking its echoed query. Stage-back support (task-1 gap G3) is derived from the stage, not stored: `Secret`/`Url`/`Oauth` back to `AuthMethods`, `AuthMethods`/`List` back to `List`; `ProviderSetup` back to `List`.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/protocol-perf.md`; `docs/reference/primitives/shell-layout-strategy.md`; `DESIGN.md` §12/§14; plan 124 task 7/8 evidence (protocol v31 additive pattern); `design-artifacts/approved/composer-palette-stages/README.md` (binding stage/mode coverage).
    - Options Considered:
      - Keep two origins and teach the client to render both in the palette: rejected — the user's requirement is one surface, and two origins keep a centered code path alive.
      - Add a typed `mode` to the existing `CommandPalette` origin and stop producing `Centered`. (Chosen.)
    - Chosen Approach:
      - Mirror the v31 `scope` pattern exactly: bounded string on the snapshot, closed vocabulary, clamped setter, round-trip tests; the picker keeps its own stage machine so no behavior moves.
    - API Notes and Examples:
      ```rust
      // src/server/agent_picker.rs — stage → mode
      fn mode(&self) -> &'static str {
          match self.stage {
              Stage::Secret => "secret",
              Stage::Url => "url",
              Stage::Oauth => "oauth",
              _ => "picker",
          }
      }
      TransientMenuSession::new(self.session_id, self.prompt())
          .with_items(items)
          .with_origin(TransientMenuOrigin::CommandPalette)
          .with_mode(self.mode())
      ```
    - Files to Create/Edit:
      - `src/server/agent_picker.rs`: origin + mode; stage tests.
      - `src/server/control_center.rs`: `with_mode("catalogue")`.
      - `src/shell/path_browser.rs`: `with_mode("path")`.
      - `src/shell/transient_menu.rs`: `mode` field + `with_mode` + clamping.
      - `src/protocol/menu.rs` + `src/protocol/mod.rs`: additive field, `PROTOCOL_VERSION = 32`, round-trip test.
      - `frontend/src/bridge/types.ts`: `mode?: string` on the menu snapshot DTO.
    - References:
      - `src/protocol/menu.rs:57-99` (`scope`/`with_scope`, `TRANSIENT_MENU_MAX_SCOPE_CHARS`) — the pattern to copy.
      - `src/server/menu_sessions.rs:488-494` (origin mapping).
      - `src/server/agent_picker.rs:181-215,378,500-545,640-668`.
      - `frontend/src/shell/workspace-envelope.ts` (frontend cast: additive DTO fields are type-only).
      - `design-artifacts/approved/composer-palette-stages/README.md` — binding coverage for the picker modes.
  - Test Cases to Write:
    - `agent_picker::tests`: every kind/stage snapshot reports `CommandPalette` + the right mode; only `Stage::Secret` reports `secret`.
    - `protocol::menu` round-trip: `mode` survives rkyv and JSON codecs; absent mode decodes to `None`; >16 chars clamps.
    - `control_center`/`path_browser` suites: catalogue/path sessions report their modes.
    - Regression: `src/server/agent_picker.rs` activation tests (provider setup chain, session resume/delete, search hits) unchanged.

- [x] Render every palette session through one sheet: stage line, shielded secret stage, stage-aware keys
  - Completion Evidence (2026-09-18, task 8):
    - Rendering (`frontend/src/command-centre/CommandPalette.tsx`): the sheet is
      mode-aware and still display-only (no session of its own). Non-catalogue
      sessions draw the session's own prompt as the stage line (`styles.prompt`,
      the kept `.prompt` micro-label) and keep the field's echo; the catalogue's
      head/segment/chord chips are unchanged, and a stage draws no scope
      segment (its rows are unscoped — the segment exists to *fill* a scope, so
      one that cannot is not drawn). The shielded stage draws the sheet's own
      field (`ClayTextField type="password"`, `autoComplete="off"`,
      `spellCheck={false}`, `aria-label` = the prompt), suppresses the echo
      entirely — the server's bullet mask is not a value to read back — and
      takes focus when it opens. Foot verbs follow the mode
      (`run`/`choose`/`resume`/`store`/`save`), `Esc`/`Alt+← back` is stated,
      and a row that carries the secondary binding adds the `Alt+↵` chip plus
      the alternate verb (`delete`). The empty state says "goes back a stage"
      for a stage and "dismisses the palette" for the catalogue. The listbox
      sets `aria-activedescendant` to the selected option's id.
    - Client plumbing (`frontend/src/coding-agent/Composer.tsx`,
      `ComposerPalette`): `activate(secondary?: boolean)` and `back()` (already
      routed by `WorkspacePanes.tsx` → `menuActivate`/`menuBackspace`). The
      sigil rule keys on the mode: only `catalogue`/`path` seed `/`, cancel on
      its loss and filter through `paletteFilter`; a stage's field is its filter
      verbatim, and emptying it is clearing the filter, not leaving the flow. A
      stage transition (or a new session) resets the chip to `All` and clears
      the draft, so no catalogue text seeds a picker. `@`-mentions stay shut
      while a stage is up, the composer box is non-typable during the shielded
      stage (a paste cannot reach the draft), the shield's keys are routed
      through the same handler, and the shield's characters are the only source
      of its `query` intents (`onSecret`).
    - Server (`src/server/agent_picker.rs`, `menu_sessions.rs`,
      `connection/menus.rs`): `at_flow_entry()` (the picker list is the flow's
      entry; a typed filter is still the same stage) and `ascend()` (one stage
      per press, `Stage::previous` first, then the `ProviderSetup` unwind into
      the provider list) give `Esc` its "one step back, close at the floor"
      rule; `MenuEdit.close` lets the connection answer a floor `menuBackspace`
      with `TransientMenuClosed`, so the client never has to model a trail. The
      session list's rows carry the `Alt+↵` binding (the delete chord the sheet
      names).
    - Supersedes the task-7 record of `backspace`: the picker's Backspace is
      **always** stage-back now (the field deletes its own characters, so a
      filter never rides an ascent), and the entry answers `close` instead of
      popping a character. Same rule the approved set's §3.4 states ("derived
      from the session kind — no trail field on the wire").
    - Divergences (all recorded in the approved set's README §3, "recorded after
      the freeze"): the `url` stage keeps the composer field (DESIGN §12 gives
      an owned input to the shielded credential alone); the OAuth device code is
      drawn from the session's rows, not a `.stage-code` block; the composer box
      is disabled during the shielded stage; the shield's echo is not drawn.
    - Tests: `CommandPalette.test.tsx` — the picker stage's prompt/echo/keys and
      no scope segment, the verb table per mode with the shield as the only
      input, the shield's `type`/`autocomplete`/`spellcheck`/name plus the
      assertion that no secret value reaches any text node or attribute, the
      shield's clearing on a stage change and on a new session, the session
      row's binding + `Alt+↵` chip + alternate verb, `aria-activedescendant`,
      the stage empty-state copy, and the closed mode vocabulary
      (`paletteModeOf`). `Composer.test.tsx` — `Esc`/`Alt+←` walk a stage back
      (never cancelling it), a stage's field is sigil-free and does not close on
      empty, `Alt+↵` activates secondary, an empty stage's `↵` neither dismisses
      nor submits, the shield's keystrokes become the session's query while the
      composer draft stays empty (and a paste into it is dropped), and `@` stays
      shut. `WorkspacePanes.test.tsx` — this host draws a picker session on the
      one sheet, with the picker's filter and the shield's credential reaching
      the session payload (`menuQueryUpdate`) and the veil/session lifecycle
      unchanged.
    - Gates: frontend `495/495` (51 files) plus typecheck, lint and
      `format:check`; `cargo test --test presentation` `62/62`;
      `cargo test --test protocol` `219/219`; `cargo test --test runtime`
      `75/75`; the touched lib suites `38/38` (`agent_picker`, `menu_sessions`,
      `control_center`, `path_browser`, `transient_menu`, and the connection's
      menu tests); `cargo clippy --all-targets -- -D warnings` and
      `cargo fmt --check` clean.
  - Acceptance Criteria:
    - Functional: `CommandPalette` renders all sessions: `catalogue` keeps today's head/scope chips/chord chips exactly; `path` and `picker` add the session prompt as a visible stage line and keep the field echo; `secret` renders a shielded field *inside the sheet* (focus moves to it, `aria-label` from the prompt, characters never rendered and never written to the composer draft); `url`/`oauth` render their rows with the stage's hint. `Esc` goes back one stage when the session has a stage trail, else cancels; `Alt+↵` sends a secondary activation (session delete); `↵` runs the selected row.
    - Performance: No client-side filtering; the sheet is a pure function of the snapshot; the composer field's keystrokes still produce exactly one intent each, and the shielded stage produces one intent per keystroke with the same ceiling.
    - Code Quality: `ComposerPalette` grows `activate(secondary?: boolean)` and `back()`; the composer's `/`-sigil rule keys on `mode === "catalogue"` (no sigil seeding or cancellation for picker sessions); `CommandPalette` stays display-only and continues to own no session; rendered states match `design-artifacts/approved/composer-palette-stages/`.
    - Security: The composer draft is never used as the secret transport; the shielded field's value is cleared when the stage ends or the sheet closes; the field is `aria-label`led and `autocomplete="off"`, `spellcheck="false"`, and is not mirrored into any announcement. Task-1 gap G4 is the binding input for this: today's centered sheet renders the typed secret in the clear (`CommandCentre.tsx:35-36,107-124`), so this stage is the only masking the UI has ever had — `mode === "secret"` (not a prompt-string heuristic, gap G5) selects it, and focus moves from the composer field into the shield when it opens.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5/§7/§9/§11/§12/§14.4; `design-artifacts/approved/composer-palette-stages/README.md` (binding — every rendered stage matches the frozen scenes); WAI-ARIA combobox/dialog guidance; `.agents/skills/clay-execution/references/ui.md`.
    - Options Considered:
      - Palette stays a list-only sheet and pickers render below it as a second panel: rejected — two surfaces again.
      - One sheet with a mode-driven head/foot plus one shielded input for the secret stage. (Chosen.)
    - Chosen Approach:
      - Keep the composer field as the query for list-shaped stages; the secret stage alone takes focus into the sheet, because no masking exists on the composer's textarea and the draft is persisted state.
    - API Notes and Examples:
      ```tsx
      const catalogue = menu.mode === undefined || menu.mode === "catalogue";
      const shielded = menu.mode === "secret";
      // composer, catalogue only:
      if (catalogue && value.startsWith("/")) paletteIntents.current.request();
      // activate from the field:
      onKeyDown={(event) => { if (event.key === "Enter") palette.activate(event.altKey); }}
      ```
      ```tsx
      <input
        aria-label={menu.prompt}
        value={secret}
        type="password"
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => { setSecret(event.target.value); palette.query(event.target.value, "all"); }}
      />
      ```
    - Files to Create/Edit:
      - `frontend/src/command-centre/CommandPalette.tsx`: stage line, shielded stage, stage-aware foot, `activate(secondary)`.
      - `frontend/src/command-centre/command-centre.module.css`: stage line, shielded well (reuse `textInput.default.input.*`), no new recipes.
      - `frontend/src/coding-agent/Composer.tsx`: `ComposerPalette.activate(secondary)`/`back()`; sigil rule on `mode`; no `/` seeding for pickers; clear draft only when the catalogue ran a row.
      - `frontend/src/shell/WorkspacePanes.tsx`: pass the new intents through (`menuActivate(secondary)`, `menuBackspace()` mapped to `back`).
      - `frontend/src/shell/workspace-controller.ts`: expose `menuBackspace` on the `ComposerPalette` bridge if needed.
    - References:
      - `design-artifacts/approved/composer-palette-stages/README.md` — binding composition and key hints.
      - `frontend/src/coding-agent/Composer.tsx:200-230` (session catch-up + `/` seeding), `:400-430` (submit routing), `:470-520` (menu slot).
      - `frontend/src/command-centre/CommandPalette.tsx` (current head/list/foot).
      - `frontend/src/command-centre/CommandCentre.tsx:72` (`menuBackspace` on empty) — behavior to preserve as `back`.
      - `src/server/menu_sessions.rs:301` (`backspace` semantics).
  - Test Cases to Write:
    - `CommandPalette.test.tsx`: stage line per mode, shielded secret (value never in the composer, cleared on close), `Alt+↵` secondary, `Esc` back vs cancel, catalogue head unchanged (snapshot test), empty state.
    - `Composer.test.tsx`: picker session is not seeded with `/` and is not cancelled by text without a sigil; catalogue behavior unchanged; submit routes rows and `Alt+↵`.
    - `WorkspacePanes.test.tsx`: one palette sheet for every `commandPalette` session; veil/lane lifecycle unchanged.
    - Accessibility: listbox `aria-activedescendant` for list stages; shielded input named by the prompt; no secret value in any DOM text node or ARIA attribute (unit assertion over the rendered container).

- [x] Delete the centered projection and retire the centered origin
  - Completion Evidence (2026-09-18, task 9):
    - Deleted: `frontend/src/command-centre/CommandCentre.tsx` and
      `CommandCentre.test.tsx` (the whole centered sheet: its own head field,
      its `ClayModal` wrapper, the secret-prompt heuristic that rendered a typed
      credential in the clear, and the five tests that pinned it).
    - Host: `frontend/src/shell/WorkspacePanes.tsx` loses the lazy import and
      the `centredMenu` projection — the host's transient surface set is exactly
      the lane's composer palette now (the `contextMenu`/`menuBar` origins belong
      to the package UI renderer, not the shell).
    - CSS: `command-centre.module.css` drops `.surface`, `.surface:focus-within`,
      `.menu`, `.head`, `.input`, `.input::placeholder`, `.results` and the
      `.hintKeys` selector, and with them every use of
      `--clay-dimension-overlay-centered-width` (the token itself stays: the
      modal and package-workspace modules still consume it). `.prompt`,
      `.searchIcon`, `.empty`, `.emptyHint`, `.foot`, `.hint`, `.spacer` and
      `.count` stay — the palette renders all of them. The module header now
      records the retirement instead of the two-surface split.
    - Design system: the retired menu-origin popover was the only host consumer
      of `popover.default.root.rest`, so its nine projected fallbacks left
      `frontend/src/styles/tokens.css` (a projected fallback with no consumer is
      dead) and the key returned to
      `frontend/src/test/fixtures/design-system-adoption-backlog.json` with the
      note updated (19 → 20 keys). The package recipe and its pins are untouched
      (`tests/package_ui_conformance.rs` still asserts its cast shadow and the
      165-key count): a recipe is design-system vocabulary, the backlog is where
      a surface-less root is recorded.
    - Package UI projection (`src/shell/package_ui.rs`): `PackageOverlayAnchor::Centered`,
      `rect_with_centered_width`/`centered_rect(working_area, 640.0, …)` and the
      `overlay_observations` centering branch are gone; the retired wire origin
      falls in with the palette's `Bottom` anchor and never panics; the a11y
      `result_count` gate moved from `Centered` to `CommandPalette` (the palette
      is the counted surface now).
    - Wire/decoding: `TransientMenuOriginData::Centered` stays decodable
      (`protocol::menu`'s round-trip test keeps covering it) and is documented as
      retired-never-produced in `src/protocol/mod.rs` (v32 note), `src/protocol/menu.rs`,
      `src/shell/transient_menu.rs` and `frontend/src/bridge/types.ts`. The
      shell enum keeps the variant so an older peer's snapshot projects instead
      of failing to compile.
    - Fixtures/tooling: `frontend/src/routes/fixture.tsx` drops the
      `command-centre-menu` state, the `menuMode` branch and the `CommandCentre`
      import (the remaining states — `command-centre`, `command-centre-empty`,
      `path-browser` — were already the palette and stay);
      `design-artifacts/tools/capture-overlays.mjs` measures
      `[data-testid="command-palette"]`/`.palList`/`.palHead`, asserts the sheet
      draws no ring of its own (the composer box is the boundary — plan 124) and
      drops the retired menu-origin scene (its palette scenes are verified in the
      review task together with `capture-lane-palette.mjs`). Doc pins that name
      the deleted component (`docs/development/react-ui-catalog-mapping.md`, the
      recipe matrix, the wiki's centered-surface module) are the authoring and
      wiki tasks' sweep, as the plan assigns them.
    - Tests: `src/shell/package_ui.rs`'s centered-geometry test became
      `the_retired_centered_origin_still_projects_without_a_centered_anchor`
      (no centering, no count for the retired origin; the palette origin gets
      both); `src/server/menu_sessions.rs` gains
      `no_session_constructor_produces_the_retired_centered_origin` (every
      constructor's snapshot reports `CommandPalette` — the "no producer" pin);
      `frontend/src/shell/WorkspacePanes.test.tsx` gains
      `draws no centred sheet for any origin the wire can carry` (centered,
      contextMenu and menuBar snapshots render no sheet and no
      `command-centre` node); `overlay-composition.test.ts` retargets its two
      centered-sheet pins to the palette (bounded `min(52vh, 420px)` with rows
      scrolling, and no focus ring of its own);
      `frontend/src/shell/shell-chords.test.tsx`'s chord test now proves the
      snapshot lands on the palette.
    - Gates: frontend `489/489` (50 files, one file fewer after the centered
      tests left) plus typecheck, lint and `format:check`;
      `cargo test --test presentation` `62/62`; `cargo test --test protocol`
      `219/219`; `cargo test --test runtime` `75/75`;
      `cargo test --test security` `151/152` with the one failure
      (`agent_session_isolation::two_workspaces_keep_their_agent_writes_in_their_own_root`)
      passing in isolation on re-run — the recorded daemon/environment flake
      class (`npm` spawn unavailable in the harness, shared-socket races);
      `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.
  - Acceptance Criteria:
    - Functional: `CommandCentre.tsx` and its fixture states are deleted; `WorkspacePanes` mounts no centered projection; no producer creates `TransientMenuOrigin::Centered`; `PackageOverlayAnchor::Centered`/`centered_rect` and the `overlay_observations` centering branch are removed; the wire enum value stays decodable and is documented as retired.
    - Performance: One fewer lazy chunk and one fewer component in the shell graph; no palette-path change.
    - Code Quality: `.surface`/`.input`/`.menu`/`.results`/`.head`/`.hintKeys` centered CSS is deleted (`.prompt` stays — it is the stage prompt line, task 2), the `command-centre.module.css` uses of `--clay-dimension-overlay-centered-width` (`:27,29,52`) are removed while the token itself stays (live consumers: `components/modal.module.css:32`, `packages/package-workspace.module.css:125`), `frontend/src/bridge/types.ts` annotates `centered` as retired-never-produced; the approved artifact set contains no centered surface, so nothing in `design-artifacts/approved/composer-palette-stages/` is left unimplemented by this deletion. Task-1 inventory C2–C6/C10/C11 is the deletion checklist: lazy mount + import, both centered tests, the four fixture states and `menuMode`, the two centered test stubs, the eight centered CSS classes, and the two CSS-consumption gates (`overlay-composition.test.ts:108-122` retargets to `.palette`; the `design-system-consumption` family lists drop centered-only entries). The `result_count` gate (`src/shell/package_ui.rs:597`, gap G6) moves to the `CommandPalette` origin in the same sweep, and `open_agent_picker`'s stale doc block (gap G7) is corrected.
    - Security: Deleting the centered input removes the only place where a secret was rendered in the clear; a test asserts the secret path now renders masked.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12/§14 (amended in the DESIGN task); `design-artifacts/approved/composer-palette-stages/README.md` (binding: no centered scene exists in the approved set); `docs/wiki/modules/centered-command-centre-surface.md` (retirement record); `.agents/skills/clay-execution/references/ui.md`.
    - Options Considered:
      - Keep `Centered` as a live-but-unused origin: leaves a second renderer path one config away from returning.
      - Delete the renderer, retire the producers, keep the wire variant for decode compatibility. (Chosen — protocol enums are additive-only.)
    - Chosen Approach:
      - Remove producers and the client projection; pin "no producer" with a test that greps/asserts the server constructors.
    - API Notes and Examples:
      ```rust
      // src/protocol/menu.rs
      /// Retired in v32: never produced. Kept so older peers decode.
      Centered,
      ```
      ```bash
      grep -rn 'TransientMenuOrigin::Centered' src/ --include=*.rs   # expect: protocol enum + decode only
      ```
    - Files to Create/Edit:
      - `frontend/src/command-centre/CommandCentre.tsx` (deleted), `frontend/src/command-centre/CommandCentre.test.tsx` (deleted).
      - `frontend/src/command-centre/command-centre.module.css`: `.surface`, `.menu`, `.input`, `.prompt`, `.results`, `.empty` retention decided by actual consumers.
      - `frontend/src/shell/WorkspacePanes.tsx`: drop the `CommandCentre` mount/lazy import.
      - `frontend/src/routes/fixture.tsx`: drop `commandCentre`/`command-centre-*` fixture states (and `menuMode` if unused).
      - `src/shell/package_ui.rs`: remove `PackageOverlayAnchor::Centered`, `centered_rect`, the `overlay_observations` branch, and the test at `:1229`.
      - `src/shell/transient_menu.rs`, `src/server/menu_sessions.rs`, `src/protocol/menu.rs`: retire/en‑document the variant.
      - `frontend/src/bridge/types.ts:232`: mark `centered` retired.
    - References:
      - `frontend/src/command-centre/CommandCentre.tsx` (whole file), `WorkspacePanes.tsx:31-33,269-300`.
      - `src/shell/package_ui.rs:113-125,597-608,822,1229`.
      - `src/protocol/menu.rs:156-178`.
  - Test Cases to Write:
    - `src/shell/package_ui.rs` / `src/server/agent_picker.rs`: no constructor builds `Centered` (assert over the produced snapshot's origin).
    - Frontend: `WorkspacePanes.test.tsx` asserts no centered sheet is rendered for any origin; fixture route list drops centered states (documentation pin updated in the authoring task).
    - Protocol: decode of a `Centered` snapshot still succeeds (compatibility).

- [x] Adopt the default agent on agent-less tabs and keep the composer typable
  - Completion Evidence (2026-09-18, task 10):
    - Default agent: `defaultAgentType(entries)` in
      `frontend/src/coding-agent/Composer.tsx` (the coding agent when listed —
      `coding-agent`, its data-root name; `coding` for a shortened install —
      else the first listed type, else `null`), pinned by
      `AgentLane.test.tsx`. `AgentLane` gains the adoption effect next to the
      existing session-agent effect: it fires only for a tab with no agent and
      no server-reported session agent, so a resumed session's own agent wins
      and a user's pick is never replaced; the listing is the one
      `listLauncherEntries` call the lane already makes (no second fetch), and
      the intent is a single server-validated `setAgent`.
    - Always-typable composer: `disabled={shieldOpen}` — a missing agent no
      longer makes the field inert (only the shielded credential stage moves the
      input into the sheet). The agent-less placeholder now reads "Type a prompt
      — attach an agent to send it", and the lane foot distinguishes the two
      reasons nothing would send: "no agent types listed · nothing would send"
      (an empty listing) vs "no agent on this tab · pick one to send". A refused
      submit still keeps the draft, which the new tests assert directly.
    - The launcher landing: auto-attachment made `tabUncommitted`'s
      `&& !tab.agent` clause wrong — a folder-less tab would have lost its
      landing (and `openWorkspace` would have opened a second tab instead of
      filling the launcher tab in place). The rule is now the workspace alone
      (`!tab.workspaceRoot`), with the doc comment recording why the agent is
      not a commitment about where the editor is; the landing therefore survives
      the auto-attached agent, and a folder-less launcher tab still fills in
      place when a folder or file is picked.
    - Tests: `AgentLane.test.tsx` (the agent-less state types and states its
      reason), `Composer.test.tsx` (`keeps the field typeable with no provider
      and with no agent`: placeholder, typing, refused submit keeps the draft),
      `WorkspacePanes.test.tsx` (`adopts the default agent for a tab with none,
      once the listing lands` — asserts the `tabCommand.setAgent` payload and
      the attached trigger, with no second intent; `leaves an already-attached
      tab alone, with no extra pick`; `keeps the launcher landing when the
      default agent auto-attaches`), `tab-store.test.ts` (the uncommitted rule).
    - Gates: frontend `492/492` (50 files) plus typecheck, lint and
      `format:check`. No Rust change was needed (the default rides the existing
      `setAgent` intent, which the server validates against its own listing).
  - Acceptance Criteria:
    - Functional: A tab with no agent adopts the default type as soon as the server's agent listing arrives — `coding` when listed, else the first listed type, else nothing; the lane shows the attached state without a user pick. The composer field is typable in every state (no agent, no provider, streaming), and a refused submit leaves the draft intact and keeps the lane foot stating the reason. User direction recorded: "an agent should be auto selected, coding agent for now so that user is not confused" (2026-09-18).
    - Performance: One `setAgent` intent per tab, only when the tab has no agent; no re-pick loop when the server snapshot already names the session's agent; no extra `listLauncherEntries` call.
    - Code Quality: The default choice lives in one exported helper (e.g. `defaultAgentType(entries)`) with a unit test; the lane's effect does not fight the existing session-agent adoption effect; `tabUncommitted` semantics are preserved so the launcher landing is unchanged.
    - Security: The default is applied only to a name the server listing returned (already validated by `resolve_agent_type`); the client never invents an agent type.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12 (agent-less state, lane composition); `design-artifacts/approved/agent-lane-palette/README.md` and `design-artifacts/approved/composer-palette-stages/README.md` (binding lane/palette states); `.agents/skills/clay-execution/references/ui.md`.
    - Options Considered:
      - Server-side default in `TabRegistry::create_tab`: the default is client-owned/persisted (`layout.json` `agent`), and the registry snapshot carries no agent field — a server default would need a second surface.
      - Launcher-only attach: leaves ordinary tabs agent-less.
      - Client effect in `AgentLane` keyed on the server's agent listing. (Chosen.)
    - Chosen Approach:
      - Add the effect next to the existing "session agent adopts into an agent-less tab" effect, using the listing the lane already fetches once per tab.
    - API Notes and Examples:
      ```tsx
      export function defaultAgentType(entries: readonly { name: string }[]): string | null {
        if (entries.length === 0) return null;
        return entries.find((entry) => entry.name === "coding")?.name ?? entries[0].name;
      }
      ```
    - Files to Create/Edit:
      - `frontend/src/shell/AgentLane.tsx`: default-agent effect + always-typable composer (drop `disabled={agentless}`), foot reason wording for "no agent types listed".
      - `frontend/src/coding-agent/Composer.tsx`: `agentless` no longer disables the field (it still hides the model/effort controls).
      - `frontend/src/shell/AgentLane.test.tsx`: default attach, typable-in-every-state, refused-submit draft survival.
    - References:
      - `frontend/src/shell/AgentLane.tsx:130-150` (listing effect), `:160-176` (session-agent adoption), `:290-320` (submit gates).
      - `frontend/src/coding-agent/Composer.tsx:505-520` (`disabled={agentless}`).
      - `src/server/launcher.rs:163-186` (`list_agents`), `:197` (`resolve_agent_type`).
      - `frontend/src/shell/tab-store.ts:67` (`tabUncommitted`) — unchanged by this task.
  - Test Cases to Write:
    - `AgentLane.test.tsx`: listing `[reviewer]` → attaches `reviewer`; listing `[reviewer, coding]` → attaches `coding`; empty listing → stays agent-less and typable; already-attached tab → no extra `pickAgent` call.
    - `Composer.test.tsx`: typing with `agentless` produces a draft; submit with no agent/provider leaves the draft and the foot reason visible.

- [x] Update the package UI/layout authoring contract, catalogs, and documentation pins
  - Completion Evidence (2026-09-18, task 11):
    - Authoring contract (`docs/reference/packages/creating-packages.md`): new
      "### Plan 125 authoring contract: one palette for every picker, and the
      halo" section — the centered sheet is retired (the Phase 24.4 section is
      retitled "…, retired by Plan 125" and states no live producer emits
      `centered`, `dimension.overlay.centered.width` now only sizes package
      `modal` dialogs, and `PackageOverlayAnchor` no longer has a centered
      option at all), **the composer palette is the only transient selection
      surface** with its bounded `mode` field and stage flows, **the shielded
      stage is Clay-owned, and only Clay-owned** (masked snapshots, `Enter`
      flush, `host.put_credential`, no package API, a11y name/role only), and
      the halo/veil prohibitions — "packages cannot open or drive the palette",
      "No package can request or style the veil, request the halo for its own
      surfaces, mount root layers, or intercept menu input". The Plan 124 lane
      paragraph now says the lane is the *view pane's* bottom row and records
      the Plan 125 confinement; the Plan 087/Plan 088 bullets and the
      "do not" list were updated (`centered` decode-only, `commandPalette`
      Clay-internal, shielded stage off-limits).
    - Navigation page (`docs/reference/ui-components.md`): the
      Command-Centre/Path-Browser bullet became one composer-palette bullet
      ("since plan 125 the composer-anchored palette is the only transient
      selection surface"), with the veil geometry, the halo, the six modes, the
      shielded stage, and the retired `centered` value; the Plan 124 section
      gains "### Plan 125 continuation: one sheet for every picker, and the
      halo" (every picker stage, the shielded stage, the halo, package dialogs
      and the surviving width token) and its agent-lane bullet now states the
      view-pane confinement; the Plan 088 contract bullet and the
      "Rules for Changing the UI Surface" note were swept.
    - Catalog + token catalog + shell vocabulary:
      `.agents/skills/clay-execution/references/components.md` (origins, the
      palette projection, the React implementation line, and the Agent
      lane/Transient menu/Package overlays/`paint_scrim`/Command palette rows),
      `.agents/skills/clay-execution/references/tokens.md` (the halo values
      `0 0 14px -2px` @0.14 + `0 0 3px 0` @0.08 as a recipe value rather than a
      token, the scrim rows, and `dimension.overlay.centered.width` re-scoped to
      package `modal` dialogs), and
      `docs/reference/primitives/shell-layout-strategy.md` (status header, the
      transient-panel and transient-menu sections, and the Phase 24.4 paragraph
      rewritten as retired).
    - Master index (`docs/index.md`): both entries now name the Plan 125 single
      palette surface, the retired centered sheet, and the shielded stage.
    - Centered-surface sweep (the AC's "no doc still offers a centered transient
      surface"): also updated `docs/development/accessibility.md` (the modal
      dialog section became the palette/mentions a11y contract — modeless
      relative of the field, listbox + live count, the shielded stage's focus
      move, the veil's lane exclusion), `docs/development/performance.md`,
      `docs/development/launch-and-gui-smoke.md` (the `ui-review-command-centre`
      fixture now opens the palette), `docs/reference/clay-js-api/configuration.md`,
      `docs/reference/primitives/ui-chrome-primitives.md`, the
      `commandCentre` rows of `docs/development/ui-design-system-recipe-matrix.md`
      (consumers retargeted to `CommandPalette.tsx`, retired slots called out),
      and the `docs/development/react-ui-catalog-mapping.md` Command-Centre row.
    - Pin: `tests/primitives_docs.rs` extended in place (the plan's chosen
      approach — one contract test, not a parallel one) with the Plan 125 marker
      sets for `creating-packages.md`, `ui-components.md`, `components.md`,
      `tokens.md`, `shell-layout-strategy.md`, and `docs/index.md`; the Plan 087
      and Plan 088 pins were kept satisfied by phrasing the `completion` /
      `centered` origin claim so the older literals still hold.
    - Gates: `cargo test --test protocol` **219/219** (includes
      `primitives_docs` 35/35, `documentation_coverage` 11/11 — parity ledger
      still maps every manual-step id and public API — and
      `manual_smoke_docs` 26/26), `cargo test --test presentation` 62/62,
      `cargo test --test runtime` 75/75, `cargo fmt --check`, and
      `cargo clippy --all-targets -- -D warnings`. Documentation only: no
      frontend or server behavior changed in this task.
  - Acceptance Criteria:
    - Functional: `docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`, `.agents/skills/clay-execution/references/components.md`, `docs/reference/primitives/shell-layout-strategy.md` and `docs/index.md` describe the single palette surface, the retired centered sheet, the picker stage flows, and the halo; no doc still offers a centered transient surface.
    - Performance: Documentation only.
    - Code Quality: A documentation pin (`tests/primitives_docs.rs`) fails when the new markers disappear; the parity ledger keeps every manual-step id and public API mapped.
    - Security: `creating-packages.md` states that packages cannot open the palette, request the veil, or claim the shielded stage; no package-facing anchor vocabulary gains a centered option.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`; existing Plan 124 pin (`plan124_agent_lane_and_composer_palette_authoring_contracts_are_pinned`) as the pattern.
    - Options Considered:
      - Fold into the wiki task: rejected — the authoring contract is package-facing and is gated by its own test.
      - One contract task before implementation review. (Chosen.)
    - Chosen Approach:
      - Extend the existing pin with Plan 125 markers rather than adding a parallel test.
    - API Notes and Examples:
      ```rust
      // tests/primitives_docs.rs — marker set for plan 125
      const PLAN_125_MARKERS: &[(&str, &str)] = &[
          ("docs/reference/ui-components.md", "composer-anchored palette is the only transient selection surface"),
          ("docs/reference/packages/creating-packages.md", "packages cannot open or drive the palette"),
          ("docs/index.md", "composer palette"),
      ];
      ```
    - Files to Create/Edit:
      - `docs/reference/ui-components.md`; `docs/reference/packages/creating-packages.md`; `docs/index.md`; `.agents/skills/clay-execution/references/components.md`; `.agents/skills/clay-execution/references/tokens.md` (halo values); `docs/reference/primitives/shell-layout-strategy.md`; `tests/primitives_docs.rs`.
    - References:
      - Plan 124 task 10 evidence and the existing pin test.
      - `docs/development/tauri-react-parity-ledger.json` (step ids added by the manual-test-plan task).
  - Test Cases to Write:
    - `cargo test --test presentation primitives_docs::` — all Plan 124 + Plan 125 markers present.
    - Documentation coverage: `cargo test --test protocol documentation_coverage::` passes after the parity-ledger update.

- [x] Perform the visual and accessibility review of the single palette surface
  - Completion Evidence (2026-09-18, task 12): review log
    `code-reviews/screenshots/2026-09-18-plan125-palette/review-log.md`, with
    `fixture-layer.json` (291 checks, 0 failures, + the console log),
    `geometry.txt` (per-theme pixel means), `drive.txt` (the live action
    sequence), `accessibility.txt` (AT-SPI extracts) and `screenshots/*`
    (12 window-cropped live captures + full AX trees, and the fixture-layer
    per-scene captures in the sibling scene folders).
    - **Two layers.** Deterministic: `design-artifacts/tools/capture-lane-palette.mjs`
      extended for this task — every picker stage seeded through the DEV fixture
      route (`?fixture=command-centre&stage=providers|auth|secret|url|oauth|models|sessions`,
      rows/prompts/modes mirrored from `src/server/agent_picker.rs`), assertions
      for each stage's mode, prompt, foot verb, the halo's computed layers, the
      mentions menu's halo and its missing scrim, the shielded field's privacy,
      the 40-row list inside the 420px cap, the session row's `Alt+↵`, and a new
      console-message recorder. Live: `clay server` + `clay client` from
      `npm run build` + `cargo build -p clay -p clay-desktop`, isolated root
      `/tmp/plan125-review` (plan-097 privacy: HOME/XDG/TMPDIR inside, canonical
      `examples/config` copied with only `setTheme` rewritten, window-cropped
      captures only), driven through on-screen affordances because this host has
      no keyboard/pointer synthesis.
    - **Four themes, verified by pixel**: the canvas sample equals each theme's
      own `shellBg` (gruvbox-material-dark 40,40,40; gruvbox-material-light
      251,241,199; modus-operandi 255,255,255; modus-vivendi 0,0,0) — the
      multi-theme sweep plan 124 could not complete. Two widths
      (1500×940 for all four, 1024×900 for modus-operandi).
    - **Geometry, measured live**: `dialog "Commands" 44,531,1124x420` — the
      sheet is the composer box's own width (field shell 1124 = entry 1106 +
      2×9px), sits the approved 6px above it (951 + 6 = 957), and hits the 420px
      cap exactly with the ~95-row catalogue scrolling inside (list box 306px).
    - **Veil**: sidebar −6.0, pane −6.0, rail −5.7 and the rail's lower half
      −4.7 in the dark theme (light −56.3/−77.3/−79.0, operandi
      −89.0/−126.0/−127.0, vivendi +10.0 where the scrim and the surface are both
      black) — the views *and* the rail's full height, with the lane's strip, the
      composer box and the status bar at delta 0.0.
    - **Accessibility (AT-SPI)**: the sheet is a `dialog` named by the session
      prompt, its rows are `list item`s in a `list box` named by the prompt, the
      scope chips are `toggle button`s, and the composer `entry` holds focus with
      the seeded `/` draft. Host ceilings (unchanged since plan 124, recorded in
      `drive.txt`): no input synthesis, no `EditableText` on the composer, and no
      actions on palette rows — so typing, row activation and the picker flow's
      stage navigation are covered by the fixture layer and the unit tests.
    - **Defect D8, fixed in this review**: `.lane` took `background: inherit` and
      the veil is placed over the lane's own cell, so a transparent strip showed
      the scrim and its blur through the chrome (measured −6/−56/−89 lum in
      three themes) while its content stayed crisp — contradicting the grid's own
      comment and the approved drawing. `.lane` now paints
      `--clay-ds-shell-default-root-rest-background-color`, pinned in
      `frontend/src/test/workspace-composition.test.tsx`; the live re-run shows
      0.0 deltas in all four themes.
    - **Defect D7, escalated**: the lane spans the workspace sidebar's column and
      the sidebar ends at its top hairline, while DESIGN §12 (normative) says the
      lane spans the middle pane only and both rails keep the working area's full
      height (the approved drawing shows the lane's chrome spanning the sidebar
      column with its content inset, so drawing and wording already disagreed).
      Root cause: the sidebar is a package-contributed *panel* inside the view
      slot (`packageUi.panels` slot `left`,
      `frontend/src/packages/package-workspace.module.css`), while the lane is a
      sibling of the view slot — so aligning them needs either the shell
      mirroring the host-owned sidebar column (~25 lines of CSS + a presence
      signal + tests) or re-approving the drawing's reading and amending §12 and
      the approved README. Recorded in Further Actions for the user's decision;
      the palette's own geometry is unaffected either way.
    - **Console hygiene**: typing produces no product warning or error (136
      messages captured; 58 are three named, pre-existing fixture-harness
      artifacts — the DEV `detached()` warn and its rejection from the route's
      inert `workspace.send` stub, and React's render-phase-update error from the
      fixture seeding its snapshot while rendering; 0 product warnings).
  - Acceptance Criteria:
    - Functional: The review exercises, on a real Linux build with the canonical config plus a configured provider stub where needed: catalogue with/without query, each picker stage (list, auth method, secret, URL, OAuth), model picker with many rows, session picker + secondary delete, path mode, the `@` mentions menu, and the halo against the approved artifact — at narrow and wide widths, in all four shipped themes.
    - Performance: The review records the palette's measured height cap, internal scrolling, and that no console/perf warning appears while typing (server round-trip per keystroke unchanged).
    - Code Quality: Every deviation from `design-artifacts/approved/composer-palette-stages/` is recorded with a disposition (fixed, or explicitly re-approved); the evidence lives under `code-reviews/screenshots/2026-…-plan125-palette/screenshots/`.
    - Security: The secret stage is inspected specifically: characters masked, no value in the DOM, no value in the composer draft, focus returns to the composer when the stage ends; AT-SPI exposes no secret text (name/role only).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md` + the visual-review duty in `create-plan/references/clay.md`; `DESIGN.md` review checklist; `design-artifacts/approved/composer-palette-stages/README.md`.
    - Options Considered:
      - Browser-fixture-only capture: cannot show live themes/pickers.
      - Two layers (fixture capture for deterministic states + live Tauri build for shell/AT-SPI), as Plan 124 task 9 did. (Chosen.)
    - Chosen Approach:
      - Reuse the plan-124 harnesses: `design-artifacts/tools/capture-lane-palette.mjs` for deterministic AX/geometry checks and the AT-SPI helper scripts for the live pass; note the known host ceilings (no keyboard/pointer synthesis; composer `EditableText` unreachable) and use the status-bar fallbacks plus fixtures for those paths.
    - API Notes and Examples:
      ```bash
      node design-artifacts/tools/capture-lane-palette.mjs --out design-artifacts/screenshots/plan125
      # live: isolated root, canonical config, window-cropped captures (plan 097 privacy rule)
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-09-18-plan125-palette/review-log.md` + `screenshots/*` + `accessibility.txt` + `geometry.txt`.
    - References:
      - Plan 124 task 9 review log (`code-reviews/screenshots/2026-09-17-plan124-agent-lane-palette/review-log.md`); `test-plan/artifacts/124-agent-lane/drive.txt`.
  - Test Cases to Write:
    - Capture matrix: every stage × theme × width renders, no clipping, correct veil/lane stacking (lane above veil).
    - AT-SPI: palette dialog named by the session prompt; rows exposed as options; shielded stage exposes a password-ish entry with no value; `Esc` returns focus to the composer.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Completion Evidence (2026-09-18, task 13): audit + documentation + pins; no new
    facade was needed and no runtime API work was added.
    - **Audit — ids stable and unchanged.** All six ids in the task's list are present,
      spelled identically, in the built-in table
      (`src/server/command_execution.rs::builtin_commands!` → `BUILTIN_SERVER_COMMAND_IDS`,
      provenance `clay`, `RoutingPolicy::ServerFirst`, no permissions, no default chords),
      paired with their shipped display names (`Choose Agent`, `Choose Provider`,
      `Choose Model`, `Configure Provider`, `Resume Session`, `Search Sessions`), and each
      maps onto a picker kind in `src/server/agent_picker.rs::picker_kind_for_command`
      (`Provider`, `Model`, `Agent`, `ProviderSetup`, `Session`, `SessionSearch`). The
      palette keeps listing them (`src/server/control_center.rs` tests) and
      `builtin_server_command_ids()` still feeds the generation-stamped catalogue
      (`src/server/mod.rs::command_catalogue_snapshot`, pinned by `src/server/tests.rs`),
      now rendered as composer-palette stages (`TransientMenuOrigin::CommandPalette`,
      plan 125 tasks 7/8) instead of the retired centered sheet.
    - **Gap found — no facade needed, and none added.** The family behaves exactly like
      `controlCenter.open`/`controlCenter.openPath`: fixed built-in command ids with no
      `clay:*` export, no op, and no `api-inventory.toml` entry (`shell.toggleAgentLane`
      remains the only facade in this area, and it is a client command id). Verified that
      built-ins resolve through the command boundary as inert ids — new unit test
      `server::command_execution::tests::builtin_picker_commands_stay_inert_command_ids`
      executes all six and asserts `ServerFirst` + `Accepted` with no session work —
      and that sessions open only on a user command intent
      (`src/server/connection/runtime.rs` → `menus.rs::open_command_centre_session`).
      The new integration pin asserts none of the six has a generated registry entry and
      that `runtime/js/agent.js` carries no picker-opening export, so no facade can ever
      expose the shielded secret stage or menu internals.
    - **Second gap found and recorded, not fixed.** The ids are **not** `bindKey` targets:
      `src/server/ops/keybindings.rs::is_runtime_bindable_command` is an explicit allowlist
      that contains the palette family but not the picker family, so
      `bindKey("Ctrl+X Ctrl+M", "agent.clientOpenModelPicker")` fails with
      `keybindings.unknown_command` while the palette lists the same id. Shipping chords
      for picker stages is a deliberate follow-up, not an inferred default (see Further
      Actions); the docs now state the current status and the pin fails if bindability
      changes silently.
    - **Package-UI action targets documented.** The ids stay in the
      `CLIENT_DIALOG_ACTIONS` allowlist (`src/server/ui.rs`) because they are built-in Clay
      commands rather than registered package commands, and `@clay/coding-agent` uses that
      route for its model/session picker rows
      (`packages/coding-agent/package.json`). The documented contract is that this is an
      inert trigger: the package gains no session, stage, query, or credential access, and
      cannot open, populate, filter, or intercept the palette. Observed nuance for the
      record: because plan 125 makes every picker a `CommandPalette` session, a
      package-triggered picker replaces the user's open palette session — the intended
      single-surface behaviour, not a second surface.
    - **Docs.** `docs/reference/clay-js-api/keybindings/bind-key.md` gains
      "## Plan 125 picker command IDs" (table of the six ids with names, palette stage,
      no facade, no default binding, no custom properties, no permissions, plus the
      activation/backing-path, Clay-owned-session, bindability, and action-target notes)
      and the `command` option text now points at it;
      `docs/reference/clay-js-api/commands/server-register-command.md` gains an
      "**Agent picker stages**" bullet in the Phase 18.8 boundary list. No inventory row,
      `docs/index.md` registry link, or generated-registry change was added, so the
      registry freshness gate is unaffected.
    - **Pins.** `tests/clay_js_doc_registry.rs::plan125_palette_picker_command_ids_are_stable_audited_and_documented`
      checks ids + names in the built-in table, the picker-kind mapping, the UI
      action-target allowlist, both doc sections' markers, absence from the generated
      registry, the absent facade, and the current (non-)bindability status.
    - **Gates.** `cargo test --test protocol` 220/220 (includes
      `clay_js_*`, `generated_registry_is_current`, api-inventory and documentation-coverage
      gates), `cargo test --test presentation` 62/62, `cargo test --lib
      server::command_execution` 25/25, `server::agent_picker` 20/20,
      `server::control_center` 17/17, `cargo fmt --all --check`, `cargo clippy
      --all-targets -- -D warnings`.
    - **AC check.** Functional: audited + stable ✓ (no rename; facade added only if a gap
      appeared — none did). Performance: registry generation stays a build-time step
      (`src/bin/update-doc-registry.rs` untouched, no runtime work) ✓. Code Quality:
      inventory/index/reference pages agree (no inventory entry needed for a facade-less
      command id, matching `controlCenter.open`) and the `clay_js_*` suites pass ✓.
      Security: no facade reaches the shielded stage or menu internals; the ids stay inert
      intents routed by the server ✓.
  - Acceptance Criteria:
    - Functional: The task audits the picker command ids (`agent.clientOpenProviderSetup`, `agent.clientOpenAgentPicker`, `agent.clientOpenProviderPicker`, `agent.clientOpenModelPicker`, `agent.clientOpenSessionPicker`, `agent.clientOpenSessionSearchPicker`) and confirms each remains a documented public command with an unchanged stable id; any new facade needed to open the palette flows is added following the existing pattern (a JS function returning a stable command id).
    - Performance: Registry generation (`src/bin/update-doc-registry.rs`) stays a build-time step; no runtime API work added.
    - Code Quality: `docs/reference/clay-js-api/api-inventory.toml`, the registry source list in `docs/index.md`, and the reference pages agree; `tests/clay_js_*` pass.
    - Security: No facade exposes the shielded secret stage or menu internals; commands stay inert ids routed by the server.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`; `docs/reference/clay-js-api/schema.md`; plan 124 task 11/12 evidence.
    - Options Considered:
      - Add new facades for each picker command: rejected unless a gap appears — they are already registered commands.
      - Verify + document. (Chosen.)
    - Chosen Approach:
      - Verify the inventory rows and docs; add a pin test if the docs were silent about palette-only rendering.
    - API Notes and Examples:
      ```js
      // clay:runtime — unchanged id, now renders as a composer-palette stage
      runtime.runCommand("agent.clientOpenProviderSetup");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**` (only where the palette rendering is described); `tests/clay_js_doc_registry.rs` (pin).
    - References:
      - `runtime/js/*.js` facades; `docs/reference/clay-js-api/api-inventory.toml`; `docs/development/tauri-react-parity-ledger.json`.
  - Test Cases to Write:
    - `cargo test --test protocol clay_js_` — inventory/index/registry/pin consistency.
    - Parity ledger: each touched command id appears in exactly one capability row.

- [x] Create or verify Clay configuration APIs
  - Completion Evidence (2026-09-18, task 12): verify-only configuration review completed; no new configuration API, option, preference, permission, or authority grant is needed.
    - **Configuration boundary confirmed.** Plan 125 reuses `clay:keybindings` only. `shell.toggleAgentLane` remains a bounded client-UI command with `custom_properties = []`; `controlCenter.open` remains the fixed server-first palette command. `bindKey`/`unbindKey` override or remove `Ctrl+X Ctrl+P` and `Ctrl+X Ctrl+O`, and `listKeyBindings("global")` inspects them.
    - **Per-tab state confirmed.** `laneVisible` is Clay-owned layout persistence in `frontend/src/shell/persist.ts` / `layout.json`; absent means visible. It is not a global setting, package option, or `init.js` property, so configuration cannot overwrite another tab's lane state. Default agent selection remains the existing code path (`defaultAgentType`: `coding-agent`, then `coding`, then first listing entry), not a new configuration key.
    - **Palette ownership confirmed.** Agent picker stages remain `TransientMenuOrigin::CommandPalette` sessions. Their stage, query, rows, navigation, placement, sizing, filtering, and shielded secret input remain Clay-owned. The six picker IDs are palette rows/package-UI action targets, not `bindKey` targets or public configuration facades.
    - **Security boundary confirmed.** Configuration can change launch chords only. It cannot open, style, position, filter, dismiss, populate, or intercept the palette; select picker credentials; grant package menu access; or expose the shielded stage. No filesystem, network, shell, extension, AI, workspace, package, raw-op, native-widget, or client-JavaScript authority was added.
    - **Documentation.** Added `## Plan 125 composer palette and picker configuration review` to `docs/reference/clay-js-api/configuration.md`, documenting the existing binding example, `custom_properties = []`, per-tab persistence, picker IDs, rejected hidden keys, and the no-new-API decision. The prior Plan 124 configuration section remains the normative binding reference; picker API/command details remain in `bind-key.md` and `commands/server-register-command.md`.
    - **Pin coverage.** Added `plan125_configuration_contract_keeps_picker_flows_palette_owned` to `tests/clay_js_doc_registry.rs`. It pins the configuration section, both shipping chords, all six picker IDs, rejected-key markers, canonical example rebinding snippets, and empty inventory custom-property metadata.
    - **Verification.** `node --check examples/config/init.js`; `cargo test --test protocol clay_js_doc_registry` — 55 passed; `cargo test --lib configuration_default_agent_lane_binding_is_present_and_overridable` — 1 passed; `cargo test --test runtime` — 75 passed; targeted frontend tests (`AgentLane.test.tsx`, `WorkspacePanes.test.tsx`) — 44 passed; `cargo check --all-targets`; `cargo fmt --all --check`; `cargo clippy --all-targets -- -D warnings`.
    - **AC check.** Functional: no new key is needed; existing binding coverage and per-tab state are documented ✓. Performance: no configuration evaluation or hot-path work added ✓. Code Quality: inventory metadata, docs, canonical example, and registry pins agree ✓. Security: configuration remains chord rebinding only and cannot reach palette/session credentials ✓.
  - Acceptance Criteria:
    - Functional: The task confirms the plan needs no new configuration key: picker/agent-lane behavior is layout state (per-tab) and keybindings, not settings; the review records the decision and the existing `bindKey` coverage for any chord involved.
    - Performance: No configuration evaluation work added.
    - Code Quality: Any touched `custom_properties` stay documented; the configuration reference reflects the palette-only surface.
    - Security: Configuration never gains the ability to open the palette or grant package menu access.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md`; `docs/reference/clay-js-api/configuration.md`; plan 124 task 12 evidence.
    - Options Considered:
      - Add a "default agent" option: rejected for now — the user asked for `coding` by default, and the default belongs to the lane; adding a key without a requirement contradicts YAGNI. Record as a follow-up if the user asks.
      - Verify-only. (Chosen.)
    - Chosen Approach:
      - Verify and document; leave the default-agent choice as a code constant with a test.
    - API Notes and Examples:
      ```js
      // unchanged: lane visibility is per-tab layout state, not config
      bindKey("shell.toggleAgentLane", "Ctrl+X Ctrl+P");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md` (single-surface note); `tests/clay_js_doc_registry.rs` (only if a marker is added).
    - References:
      - `src/server/js_runtime/tests.rs` (binding overrides); `frontend/src/shell/layout-state.ts`.
  - Test Cases to Write:
    - `cargo test --test protocol clay_js_doc_registry` and the runtime binding tests pass unchanged.

- [x] Update or verify the canonical example configuration
  - Completion Evidence (2026-09-18, task 14): canonical config reviewed and minimally annotated; no active configuration or chord changed.
    - **Canonical bindings remain correct.** `examples/config/init.js` keeps the idempotent active defaults `Ctrl+X Ctrl+P` → `shell.toggleAgentLane`, `Ctrl+X Ctrl+O` → `controlCenter.open`, and `Ctrl+X Ctrl+F` → `controlCenter.openPath`. The existing commented rebinding examples remain aligned with the validated APIs (`unbindKey` then `bindKey`) and the active batch table remains copy-safe.
    - **Plan 125 behavior is documented exactly once.** Added one comment block beside the Control Center/lane/Path Browser sections stating that provider, auth, secret, URL, OAuth, and session pickers stay in the same composer-anchored `/` palette; picker stages have no key bindings or configuration properties; the shielded secret field never becomes `init.js` or composer-draft state; and the canonical config intentionally declares no picker binding.
    - **No stale centered-surface or retired chord remains.** The file contains no centered-sheet instruction, no old `Ctrl+X Ctrl+P` palette assignment, and no active picker binding. Existing Plan 124 wording continues to describe the shipped `CommandPalette` route and per-tab lane visibility.
    - **Cross-checks.** The active command IDs/chords match `docs/reference/clay-js-api/configuration.md`, `docs/reference/clay-js-api/keybindings/bind-key.md`, `docs/reference/clay-js-api/api-inventory.toml`, and the default manifest; `shell.toggleAgentLane` remains `custom_properties = []` and no picker ID is promoted into the example as a configuration API.
    - **Verification.** `node --check examples/config/init.js`; `cargo test --test protocol plan125_configuration_contract_keeps_picker_flows_palette_owned` — 1 passed; `cargo test --test protocol example_config_control_center_chord` — 3 passed. The full protocol suite later passed 221/221; no Rust or frontend behavior changed.
    - **AC check.** Functional: canonical active bindings and one palette/picker annotation are correct ✓. Performance: syntax check passes and active config remains copy-safe ✓. Code Quality: IDs/chords/options match validated docs and inventory ✓. Security: no grant, key, trust declaration, picker credential access, or new authority introduced ✓.
  - Acceptance Criteria:
    - Functional: `examples/config/init.js` remains complete and correct for the surfaces this plan touches; if the palette/picker behavior needs an annotation (single surface, palette-only pickers), it is added exactly once in the right section; no commented default becomes active.
    - Performance: `node --check examples/config/init.js` passes; the active part stays copy-safe.
    - Code Quality: Option names/enums/defaults in the touched sections match the validated parsers and `api-inventory.toml`.
    - Security: No new grant, key, or trust declaration is introduced.
  - Approach:
    - Documentation Reviewed:
      - `create-plan/references/clay.md` (example-config duty); `examples/config/init.js` section comments; `docs/reference/clay-js-api/configuration.md`; `docs/reference/clay-js-api/api-inventory.toml`.
    - Options Considered:
      - Skip: the plan changes no configuration surface; but the example config is the launch-test subject and must state the shipping behavior (chords, palette). Update the comments only.
      - Verify + annotate. (Chosen.)
    - Chosen Approach:
      - Re-read the file end to end, update only statements the plan invalidated, keep ordering constraints intact.
    - API Notes and Examples:
      ```bash
      node --check examples/config/init.js
      ```
    - Files to Create/Edit:
      - `examples/config/init.js` (comments/sections touched by the retirement).
      - `tests/example_config_control_center_chord.rs` (only if a chord/command statement changes — expected unchanged).
    - References:
      - Plan 124 task 14 evidence (chords `Ctrl+X Ctrl+P` / `Ctrl+X Ctrl+O`).
  - Test Cases to Write:
    - `node --check` + `cargo test --test protocol example_config_control_center_chord` pass.

- [x] Launch-test the app with the canonical example config
  - Completion Evidence (2026-09-18, task 15): live Linux GUI run on the canonical `examples/config/` tree (unmodified), build + evidence in `test-plan/artifacts/125-palette/` (`launch.txt`, `accessibility.txt`, `geometry-halo.txt`, `screenshots/01..05*.png` + `.ax.txt`, `launch-drive.sh`, `launch-check.py`).
    - **Launch + Connected + configuration generation.** Fresh `npm run build` + `cargo build --bin clay` (embedded assets, no dev server) in an isolated root (`HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`TMPDIR`/socket/workspace all under `/tmp/plan125-launchtest`). Server reached `listening`, the client painted a 1500x1151 window, the server registered the agent profile, and **no `clay server configuration failed`** diagnostic or shell configuration diagnostic appeared. The configuration is demonstrably live, not defaulted: the canvas samples the canonical theme's own background (gruvbox-material-dark `40,40,40`; the default would be modus-vivendi) and the palette lists the config-loaded package rows (`/branch … @clay/coding-agent@0.1.0`, `@clay/rust`, `@clay/typescript`, `@clay/markdown`, `@clay/settings`).
    - **Lane + palette.** State 01: lane with composer box (`entry "Message"`, agent-type and model combos), hint row, `Session environment` foot, and the shell hints carrying the shipped chords (`hide lane Ctrl X P`, `palette Ctrl X O`, `files Ctrl B`, `hide outline Ctrl I`, `filter /`). State 02: the titlebar `Control Center` trigger seeds `/` and opens the **bottom-anchored** sheet `dialog "Commands" 44,531 1124x420` — width = the lane inner width and the composer box width, exactly **6 px** above the field shell (sheet bottom border row 950, field shell top border row 957), `95 results`, scope chips, mode-aware foot. State 03: the status-bar `palette` hint (the same `controlCenter.open` client command as `Ctrl+X Ctrl+O`) reproduces the state exactly; the literal Global chords are pinned by `cargo test --test protocol example_config_control_center_chord`.
    - **Halo + veil + D6.** Halo verified numerically across the sheet's top border (rows 525→530: `39→42→46→48→51` against a veiled background of 38–39 and a rest canvas of 40, i.e. brighter than the un-veiled canvas at the edge; rows 951–953 below the border are 53/50/47 over a 40 lane background) — symmetric falloff, **no offset**, no dark drop-shadow band, and the sheet interior (29,32,33) is darker than the veiled canvas. Veil: sidebar −1.7 / pane −4.7 / rail −5.0 luminance with lane foot strip, composer box and status bar at **0.0** (D8's fix holds live). States 04/05 (status-bar `hide lane` / `lane`): hiding the lane removes the strip, the sheet, and the veil (0 `Agent lane`/`Commands` nodes, working area back to `40,40,40`) and re-showing it restores the same rect and the retained `/` draft — defect D6 verified live.
    - **Performance.** Window visible **267 ms** after client spawn (plan 124 baseline 312 ms) — no regression beyond noise; measured by polling the compositor's window list before the harness's own activate/size stabilisation.
    - **Host ceilings recorded (not hidden).** This host has no keyboard/pointer synthesis (`can_send_development_input: false`), the composer exposes no `EditableText` over AT-SPI, and palette rows report no usable action, so the picker stages were not driven live: they stay covered by the fixture layer (`capture-lane-palette.mjs`, 291 checks over catalogue/path/providers/auth/secret/URL/OAuth/models/sessions), `src/server/agent_picker.rs`, and `src/server/command_execution.rs`; the live run does show the picker rows themselves (`Configure Provider`, `Choose Agent/Model/Provider`, `Resume Session`) inside the same sheet, plus the inert no-provider model combo box (disabled by design). This is the plan's existing "Live picker-stage driving" Further Action, not a new defect.
    - **Verification.** Canonical config copied byte-for-byte (`launch-drive.sh` asserts the shipped `setTheme` line before launching); five-state AX + pixel evidence retained; `python3 launch-check.py` reproduces every number; captures are window-cropped (plan 097 privacy rule, no host paths).
    - **AC check.** Functional: canonical config reaches Connected, commits a generation with no failure diagnostic, and shows the lane, the titlebar-triggered palette, the picker rows inside the one sheet, and the halo (chord dispatch and picker-stage rendering covered by the pinned tests + fixture layer as recorded) ✓. Performance: 267 ms vs 312 ms baseline, no regression ✓. Code Quality: drive sequence and numeric checks are retained and reproducible; ceilings and pre-existing diagnostics documented ✓. Security: isolated root, no host profile touched, window-cropped captures, no new grant/key/trust declaration ✓.
  - Acceptance Criteria:
    - Functional: A real Linux GUI build launched from a copied isolated config root reaches Connected, commits a configuration generation with no `configuration failed` diagnostics, and shows: the lane with the palette opening on the titlebar trigger and `Ctrl+X Ctrl+O`, a picker row (`Configure Provider`) keeping the bottom-anchored sheet, and the palette/mentions halo visible under the shipped design system.
    - Performance: Window visibility time recorded (plan 124 baseline 312 ms) with no regression beyond noise.
    - Code Quality: The launch command, scratch root, and observations are recorded as task evidence; degraded behavior is a defect or a prioritized follow-up.
    - Security: The launch uses an isolated HOME/XDG root; screenshots are window-cropped (no host paths, no unrelated windows).
  - Approach:
    - Documentation Reviewed:
      - `create-plan/references/clay.md` (live launch-test duty); `test-plan/11-performance.md` (startup budgets); the plan-124 launch script pattern.
    - Options Considered:
      - Trust the automated suites: rejected — the retirement is a shell-level projection change.
      - Isolated live launch with the canonical config. (Chosen.)
    - Chosen Approach:
      - Reuse `/tmp/launch-canonical-review.sh`'s shape (isolated roots, compositor-level window signal, cropped captures) with the review workspace as the server cwd.
    - API Notes and Examples:
      ```bash
      cp -r examples/config/. "$root/home/.clay/"
      "$repo/target/debug/clay" server "$root/review.sock" &
      "$repo/target/debug/clay" client "$root/review.sock" &
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/125-palette/screenshots/*`, `launch.txt`, `accessibility.txt`.
    - References:
      - Plan 124 launch evidence (`test-plan/artifacts/124-agent-lane/`); plan 097 privacy rule (window-cropped captures).
  - Test Cases to Write:
    - Launch assertions: connected, no configuration failure, palette rows present, picker stage keeps the sheet anchored, halo rendered (numeric pixel check around the border), lane above the veil.

- [x] Execute and update the manual test plan (test-plan/)
  - Completion Evidence (2026-09-18, task 16): modules **10** (K100–K108: picker rows in the one sheet, `Esc`/`Alt+←` stage back, `Alt+↵` secondary activation, the shielded credential stage, draft hygiene across modes, the retired centered anchor, inert picker command ids; the centered-era K54–K59/K70/K75 supersession extended), **11** (Q10/Q13/Q31 re-aimed at the shipped palette surface, new Q39 halo profile and Q40 stage transitions), **13** (S47 amended to the confined lane and left **FAIL live — defect D7**, new S50 rail/lane independence), **14** (new T83 default-agent adoption, T84 folder-less landing), **16** (new A23 palette authority + retired centered anchor, A24 agent-less truth), **17** (C59/C62/C64 amended, new C65 picker flows, C66 adoption, C67 typable-with-no-agent, C68 no stage/secret survives), plus notes in **03** (the path session is `mode=path` on the one sheet) and **15** (UI-DS-37 measures the palette, not the retired centered fixture drawing). Every new step is numbered per module, executed at least once with a PASS/PARTIAL/UNRESOLVED record and an evidence path, and referenced exactly once in `docs/development/tauri-react-parity-ledger.json`.
    - **Execution.** Live legs on a fresh build with the canonical config in an isolated root (window-cropped captures + AT-SPI dumps in `test-plan/artifacts/125-palette/`) plus the repo capture harness (`scripts/capture-ui-review.sh --fixture ui-review-workspace --size 1500x950 --drive …`, `PASS`, viewport 1500x1104) and the fixture layer (`design-artifacts/tools/capture-lane-palette.mjs`, 291 checks over the picker stages). Live PASS: K100 row presence + 0 centered nodes, K106 sigil per mode, K107 no centered sheet, Q10 sheet/narrow geometry, Q13 open + internal scroll, Q39 halo numbers, S48 sheet + veil, S50 rail independence, T83/T84 adoption + landing, A24 agent-less truth, C59/C62/C66/C67 states, C64 (D6 holds). Automated/fixture PASS: K101–K105/K108, Q40, A23, C65, C68. UNRESOLVED live (recorded, not claimed): anything needing a typed query, arrow moves, `Enter`/`Alt+Enter`/`Escape`, row activation, or a configured provider — the standing host ceilings.
    - **Defects found and dispositions.** **D9** — toggling the lane also hid the workspace rail and the agent inspector, because `frontend/src/shell/layout-state.ts` shared one `visibleByTab` map/listener set across the three `createVisibility()` stores. Fixed in this task (state moved inside the factory), pinned by `frontend/src/shell/layout-state.test.ts` (3 new tests; 495 frontend tests pass), re-verified live in both directions and re-captured with the repo harness; the pre-fix capture is retained as `screenshots/04a-lane-hidden-rail-collapsed.*`. **D7** (lane covers the sidebar's column) reconfirmed live and now has a normative failing step (module 13 S47); it still needs the user's decision recorded in Further Actions.
    - **Index/ledger.** `test-plan/index.md` gained the “Plan 125 manual-test-plan execution record” section and plan-125 clauses on the module-map rows for 03/10/11/13/14/15/16/17; the parity ledger carries the 18 new step ids (K100–K108, Q39–Q40, S50, T83–T84, A23–A24, C65–C68). `cargo test --test protocol documentation_coverage` passes (11/11), including the every-step-covered-exactly-once and no-stale-reference checks.
    - **AC check.** Functional: every module named in the task is executed/amended on a real build and updated ✓ (with the picker-stage typing legs recorded as host ceilings rather than passes). Performance: startup 267 ms vs the plan-124 baseline 312 ms and the palette's geometry/scroll/halo numbers are recorded against the module's budgets ✓. Code Quality: new ids numbered per module, no step deleted or weakened, failing steps recorded as defects (D7) or ceilings, `test-plan/index.md` module map + record updated, ledger maps each new id exactly once ✓. Security: isolated root, no real profile touched, window-cropped captures, and the credential-stage privacy legs assert no plaintext reaches the composer draft or the wire ✓.
  - Acceptance Criteria:
    - Functional: Affected modules are executed on a real build and updated: `10-keybindings-and-commands.md` (centered-sheet steps K54–K57/K70/K75 amended to the palette; new steps for picker stages, `Esc` back, `Alt+↵`), `11-performance.md` (Q10/Q11/Q13/Q31), `13-window-splits.md`, `14-tabs.md`, `16-agent-host.md`, `17-coding-agent-parity.md` (C59 `/model`, picker stages, default agent), plus new steps for the default-agent and halo behaviors.
    - Performance: Startup and palette-open timings recorded against the module's budgets.
    - Code Quality: New step ids are numbered per module; failing steps are defects or documented ceilings, never deleted; `test-plan/index.md` module map/coverage matrix updated; `docs/development/tauri-react-parity-ledger.json` maps every new step id exactly once.
    - Security: Evidence artifacts keep the isolated-root discipline (no real profile, no host paths in captures).
  - Approach:
    - Documentation Reviewed:
      - `create-plan/references/clay.md` (manual test plan duty); `test-plan/index.md`; the plan-124 execution pattern (AT-SPI-driven steps + fixtures where synthesis is unavailable).
    - Options Considered:
      - Automated-only verification: rejected — the retirement changes where users type.
      - Execute what the host allows, record the host ceilings, and add normative steps. (Chosen.)
    - Chosen Approach:
      - Use the AT-SPI helpers (`/tmp/atspi_action.py` shape) for click/type where possible; record unresolved interactive legs explicitly (keyboard synthesis and the composer `EditableText` remain this host's ceilings).
    - API Notes and Examples:
      ```bash
      # AT-SPI helper shape used by the plan-124 run (find/click/settext over the a11y bus):
      python3 "$helper" find combobox "Attach an agent"
      python3 "$helper" click list-item "Configure Provider"
      ```
    - Files to Create/Edit:
      - `test-plan/10-keybindings-and-commands.md`, `11-performance.md`, `13-window-splits.md`, `14-tabs.md`, `16-agent-host.md`, `17-coding-agent-parity.md`; `test-plan/index.md`; `docs/development/tauri-react-parity-ledger.json`; `test-plan/artifacts/125-palette/**`.
    - References:
      - Plan 124 steps K92–K99, S47–S49, T79–T82, A21–A22, C57–C64 (to amend where the surface moved).
  - Test Cases to Write:
    - Each new step is executed at least once with a PASS/PARTIAL/UNRESOLVED record and its evidence path.

- [x] Update and verify the code wiki after implementation
  - Completion Evidence (2026-09-18, task 18): fifteen pages swept in one pass after the implementation gates went green (chosen over per-task edits so the sweep read the final code, not intermediate states).
    - **Pages and what each gained.** `centered-command-centre-surface.md` was rewritten as the **removal record** (what Phase 24.4 did, what plan 124 re-anchored, what plan 125 deleted, and what deliberately survives — the wire variant, `parse("centered")` → `WorkingArea`, the bottom-anchor mapping); `control-center.md` gained the mode table (catalogue/path/picker/secret/url/oauth with query source and shipped foot verb), the stage-key semantics, the shielded stage's path to `host.put_credential`, the halo, and the lane/veil/z-index geometry; `transient-menu-session.md` and `transient-menu-round-trip.md` gained the one-sheet rule, `MenuEdit { close }`, `secondary`, protocol v32's bounded `mode`, and the retired-origin mapping; `react-command-centre-desktop-workflows.md` and `react-shell.md` replaced the deleted `CommandCentre` with the `CommandPalette`-only story (modes, stage keys, halo, lane confined to the view column, rail-covering veil); `path-browser.md` records the path session as `mode="path"` (`/` sigil kept, `Esc` cancels, backspace ascends); `command-registry.md` records the six picker ids as palette rows that are *not* `bindKey` targets or package-executable; `clay-agent.md` records the palette-owned pickers, default-agent adoption, and the typable agent-less/no-provider composer; `react-tabs-and-splits.md` records adoption plus the `tabUncommitted` folder-less landing rule; `react-agui-chat-stream.md` records that pickers are palette stages, not AG-UI events; `react-sdui-package-ui.md`, `slot-aware-package-ui.md`, and `ui-review-harness.md` record the plan-125 authority boundary (no package lane/composer/veil/halo/stage), the removed `PackageOverlayAnchor::Centered`, and the fixture/host-ceiling reality of the review; `docs/wiki/index.md` rows updated for all fourteen.
    - **Pin.** New `tests/documentation_coverage.rs::plan125_wiki_pages_describe_the_one_palette_surface`: positive per-page markers (mode table, removal record, “second, centered renderer”, shield, halo, `PackageOverlayAnchor::Centered`, default-type adoption, `tabUncommitted`) plus a recursive negative scan asserting no evergreen page in `docs/wiki/modules` or `docs/wiki/flows` repeats the plan-124-era claims plan 125 falsified (`Centered \`CommandCentre\` remains`/`rendering is retained`/`rendering survives`, `Centered picker/package sessions`, `Non-palette origins continue through`); the removal record is the one page exempt, because it explains them. Two markers failed on first run because the sweep had paraphrased them — the pin now fixes the exact wording for later sweeps.
    - **Gates.** `cargo test --test protocol` **222/222** (221 + the new pin; `wiki_navigation_is_complete_and_current_page_paths_resolve` and `documentation_coverage` included), `cargo test --test presentation` **62/62**, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean. No frontend or protocol source changed in this task, so the earlier task gates (495 frontend tests, conformance 18/18) still stand.
    - **Manual wiki review.** `docs/wiki/index.md` links every changed page (test-enforced), the `grep -i centered` sweep over `modules/`+`flows/` now returns only retirement records and historical Masonry-era test names, and no page instructs a centered transient surface.
    - **AC check.** Functional: every page named in the task (plus `react-tabs-and-splits.md`/`react-agui-chat-stream.md`/`command-registry.md`, the plan-124 sweep's leftovers) describes the single palette surface, the stage flows, the shielded secret stage, the halo, and default-agent behavior ✓. Performance: wiki-only ✓. Code Quality: each page states what changed, how it works, the invariants (one sheet, no second renderer, shield, authority boundary), its tests, and code examples; the master index is updated; the retirement record is kept in `modules/` as a navigational removal record rather than archived — recorded in *Compromises Made* ✓. Security: the shielded stage and the authority boundary are documented without a credential value, a package internal, or a host path ✓.
  - Acceptance Criteria:
    - Functional: The wiki is updated after all implementation tasks complete: `docs/wiki/modules/centered-command-centre-surface.md` becomes the retirement record, `command-registry.md`, `transient-menu-session.md`, `transient-menu-round-trip.md`, `control-center.md`, `path-browser.md`, `react-shell.md`, `react-command-centre-desktop-workflows.md`, `clay-agent.md`, `react-sdui-package-ui.md`, `ui-review-harness.md` and `docs/wiki/index.md` describe the single palette surface, the stage flows, the shielded secret stage, the halo, and the default-agent behavior.
    - Performance: Wiki-only.
    - Code Quality: Pages explain what changed, how it works, invariants/tradeoffs, tests, and examples; the master index links each page; completed-phase records go to `docs/wiki/archive/` if a page is retired.
    - Security: The shielded secret stage and the palette's authority boundary are documented without exposing secrets or package internals.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md` (wiki workflow, quality bar, archive policy).
    - Options Considered:
      - Per-task wiki edits: noisy; once after tests pass. (Chosen.)
    - Chosen Approach:
      - Follow the plan-124 wiki sweep, editing the module pages that named the centered sheet as live.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol   # after the wiki/doc edits
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md` plus the module pages listed above.
    - References:
      - Plan 124 wiki task evidence; `docs/wiki/modules/centered-command-centre-surface.md` (the page this plan completes).
  - Test Cases to Write:
    - Manual wiki review: master index links the changed pages; no page still instructs a centered transient surface.

## Compromises Made

- **The retirement record stays in `docs/wiki/modules/`, not `docs/wiki/archive/`.**
  The wiki task's code-quality line says completed-phase records are archived when
  a page is retired. `centered-command-centre-surface.md` is not a phase record:
  it is the navigational answer to “why is there no centered sheet, and what still
  decodes `TransientMenuOriginData::Centered`?” — the surviving wire variant,
  `parse("centered")` → `WorkingArea`, and the bottom-anchor mapping are live code
  a reader must be able to reach from `control-center.md`. Phase records proper
  (the plan, the review log, the MTP artifacts) are where they belong
  (`plans/`, `code-reviews/`, `test-plan/`).
- **The palette's own live interaction (typing a query, arrow/`Enter`, row
  activation, picker-stage navigation) is not driven live.** The host has no
  keyboard/pointer synthesis, WebKitGTK exposes no `EditableText` for the
  composer, and palette rows report no usable AT-SPI action, so those legs are
  carried by the named unit suites and the fixture tool and are recorded as
  UNRESOLVED in the modules and the index record — never as passes.
- **Provider-dependent stages are fixture-verified.** The canonical config
  configures no provider by design, so the credential store, the URL stage and
  the OAuth device-code poll were exercised in the fixture layer and by
  `src/server/agent_picker.rs` tests rather than against a live provider.
- **D7 was not fixed inside this plan's MTP task.** The lane-vs-sidebar
  architecture needs a user decision (Further Actions); the task recorded the
  failing normative step (module 13 S47) instead of quietly widening the spec to
  match the implementation.
- **The review harness's AT-SPI/capture helpers live in a per-run temp root.**
  The repo harness (`scripts/capture-ui-review.sh`) regenerates them, and the
  plan-125 artifacts retain the driver the live legs used
  (`test-plan/artifacts/125-palette/atspi.py`); a future review that hand-rolls
  its own driver should check it in rather than re-deriving the AX/crop offsets.

## Further Actions

- **D7 — RESOLVED (2026-09-18, on the user's decision).** Task 12 found that the
  lane's strip spanned the workspace sidebar's column — the sidebar ended at the
  lane's top hairline — while DESIGN §12 (normative) and the approved drawing say
  the lane spans the middle pane only with both rails at the working area's full
  height. **The user decided the spec's reading**: "the lane strip should not span
  the sidebars. Meaning the left and right side bar should take the full height of
  the window and agent lane should be only spanning the middle part leaving the
  space for left and right side bar". Root cause found while implementing: the
  workspace sidebar is the SDUI tree's region sized `dimension.sidebar.default`
  (`src/shell/file_browser.rs`) rendered *inside* the pane, so the only grid that
  can give it the working area's full height — the shell's, whose second row is
  the lane — never saw it. Fix: the region is **host-placed**
  (`frontend/src/sdui/renderer.tsx` exports `WORKSPACE_SIDE_SIZE`,
  `hostRailRegionId()` and `SduiRegion`, and `WorkspacePanes.tsx` renders it in
  the shell's own `.side` rail at `grid-column: 1; grid-row: 1 / -1`, with the pane
  tree rendered without it), the working-area grid is three tracks (files rail ·
  pane · inspector rail) with the lane in the pane's column and row 2, and both
  rails become fixed drawers below 1000px exactly as §12 says. No DESIGN amendment
  was needed — the implementation was the side that disagreed with §12 and the
  approved drawing; §5/§12 gained one clause naming the placement so the next
  reader does not re-derive it. Verified live at 1500×950 (`Agent lane
  270,946 916x200` — starts at the sidebar's inner edge, ends at the inspector;
  the sidebar's content runs through the lane's row to the status bar) and
  deterministically at 1500/1024/900 with `capture-sidebar.mjs` (3/3 widths; the
  ≤1000px legs are drawers and the lane keeps the pane's full width), plus 497
  frontend tests (`workspace-composition.test.tsx`, `WorkspacePanes.test.tsx`).
  Evidence: `code-reviews/screenshots/2026-09-18-plan126-rails/review-log.md` and
  `report.json`, `test-plan/artifacts/126-rails/`; the plan-125 run's own D7
  evidence stays in `code-reviews/screenshots/2026-09-18-plan125-palette/review-log.md`
  (§D7), `drive.txt` step 01, `geometry.txt`.
- **Fixture-harness console noise (priority low).** The DEV fixture route seeds
  its session snapshot during render against an inert `workspace.send`, which
  makes React log a render-phase-update error and `detached()` log a refused
  fire-and-forget call (with its unhandled rejection). Task 12's tool now records
  and classifies these three patterns so a *product* warning still fails the run;
  a later harness pass could seed through `useEffect` + a subscription instead,
  removing the need for the allowlist.
- **Picker command ids are not `bindKey` targets (priority low, needs a decision if
  wanted).** Task 13's audit found that `agent.clientOpenAgentPicker`,
  `clientOpenProviderPicker`, `clientOpenModelPicker`, `clientOpenProviderSetup`,
  `clientOpenSessionPicker`, and `clientOpenSessionSearchPicker` are built-in
  server-first command ids that the palette lists, but
  `src/server/ops/keybindings.rs::is_runtime_bindable_command` does not accept them, so
  a user chord for a picker stage is rejected with `keybindings.unknown_command` while
  the palette family (`controlCenter.open`, `controlCenter.openPath`) is bindable.
  Rationale for not changing it now: no requirement asks for picker chords, the palette
  is the shipped discovery surface (`/model`, `/resume`, …), and adding ids to that
  allowlist is a deliberate capability decision with its own test/doc sweep. If chords
  for picker stages are wanted, the change is one id per allowlist entry plus rows in the
  Plan 125 picker table in `bind-key.md`. Evidence: plan task 13 completion notes, pin
  `tests/clay_js_doc_registry.rs::plan125_palette_picker_command_ids_are_stable_audited_and_documented`.
- **Live picker-stage driving (priority low, host ceiling).** Palette rows expose
  no AT-SPI actions and this host has no input synthesis, so picker stages are
  only exercised against the fixture layer. If a future review needs live stage
  navigation, the cheapest route is a protocol-level test client that speaks
  `menuActivate`/`menuQueryUpdate` directly (the session is server-owned), not
  more UI synthesis. The MTP task's live legs used the repo harness
  (`scripts/capture-ui-review.sh`, whose AT-SPI probe/capture helpers it writes
  into a per-run temp root) plus the retained
  `test-plan/artifacts/125-palette/atspi.py` (dump with extents + action click);
  a future review should reuse those two instead of re-deriving a driver.
- **D9 (fixed during the MTP task).** Executing module 13's rail geometry step
  found that toggling the lane also hid the workspace rail and the agent
  inspector: `frontend/src/shell/layout-state.ts` declared `listeners`,
  `visibleByTab` and `activeTab` at module scope, so the three
  `createVisibility()` stores shared one map. Fixed by moving the state inside
  the factory, pinned by `frontend/src/shell/layout-state.test.ts`, re-verified
  live in both directions and re-captured with the repo harness. Evidence:
  `test-plan/artifacts/125-palette/04a-lane-hidden-rail-collapsed.*` (pre-fix),
  `rail-independence.ax.txt` (post-fix),
  `code-reviews/screenshots/2026-09-18-plan125-palette/review-log.md` (§D9).