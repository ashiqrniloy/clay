---
date: 2026-06-09 14:31
status: approved
decision_about: "Clay-owned shell layout and package UI contribution model"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Clay owns the application shell, layout slots, and package UI contribution model

## Decision

Clay will define a Clay-owned declarative application shell for package-controlled UI instead of allowing packages to directly create Masonry widgets, mutate native layout, provide raw CSS, or run JavaScript in the client. Packages will declare inert, validated contributions for UI/layout, input, actions, logic, data/state, configuration, and style/theme tokens; Clay will validate and compose those declarations server-side and the Rust client will render them through Clay-owned Masonry widgets and native rendering paths.

The shell model is based on a working area containing a pane/split tree. Each leaf pane has a fixed slot layout: one mandatory `main` container plus optional `left`, `right`, `top`, and `bottom` panel slots. Panels may be fixed or transient. Package components live inside those Clay-defined containers and route interactions through registered command/action intents. Styling is centralized through Clay themes, typed component style variables, and semantic style tokens that Clay maps to Masonry properties and native render styles; raw CSS and direct JavaScript styling are not package-facing APIs.

Clay's implementation should use Masonry as the primary native widget/layout substrate: introduce a root Clay shell/working-area widget over Masonry's `RenderRoot`, use existing Masonry widgets such as `Split`, `Flex`, `Grid`, `ZStack`, and `Portal` where they fit, and add Clay-owned custom container widgets where the working-area/pane/container semantics need stronger invariants. Taffy may be used later as an internal helper inside Clay-owned widgets if needed, but it is not the package-facing layout model and is not the first implementation target.

## Context

While preparing to make the Markdown mode usable through a concise package-owned load path, the user identified a more fundamental missing layer: Clay needs a consistent UI/layout/package structure before individual modes define their own panels, previews, actions, and styling. The user proposed concepts for a working area, multiple split windows, fixed per-window containers, package-controlled components/elements, package styling, and standard package interfaces for UI, input, logic, actions, and data.

The initial analysis recommended a Clay-owned declarative shell and primitive system. The user then asked to verify Masonry documentation before moving forward because Masonry is the actual Rust GUI substrate. Local Cargo documentation and source review confirmed that Masonry is a foundational widget-tree framework for building higher-level GUI abstractions, not the intended user-facing/package-facing API. Masonry's model supports a Clay-owned higher-level shell: `RenderRoot` owns a widget tree, container widgets explicitly lay out children, existing widgets include `Split`, `Flex`, `Grid`, `ZStack`, and `Portal`, widget properties are typed styling data, input events target/bubble through widgets, and widget actions are submitted to the app driver.

This makes the architecture a prerequisite for further Markdown mode cleanup. Markdown should consume the shell/package UI primitives rather than inventing a Markdown-specific left panel, preview model, layout model, or styling mechanism.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: After reviewing the Masonry-informed recommendation, the user said, "Okay. I agree with the recommended approach. Keep a detailed decicion log for this using @.agents/skills/create-decision-log/" and requested roadmap/doc updates to make these Clay modifications happen before further Markdown mode work.

## Alternatives Considered

1. **Let packages directly create Masonry widgets or mutate the Masonry tree** — Rejected. It would leak native implementation details into package APIs, make package behavior hard to validate, and conflict with Clay's server-authoritative/client-native boundary. Packages should not receive direct native widget mutation authority.
2. **Keep the current `EditorWidget` as the whole app shell and overlay SDUI panels manually** — Rejected as the long-term direction. It was acceptable for early SDUI smoke validation, but it cannot express consistent working-area panes, fixed/transient slot panels, package/user layout composition, or multi-pane mode workflows cleanly.
3. **Expose raw CSS or CSS-like strings for package styling** — Rejected. Clay is a native Masonry/Vello/Parley application, not a DOM/CSS runtime. Raw CSS would require a parser/cascade/security model and would undermine centralized theme consistency. Package styling should be typed/tokenized and mapped to Masonry/native properties.
4. **Use Taffy as the primary layout architecture before Masonry shell work** — Rejected for the first implementation path after reviewing Masonry. Taffy can compute flex/grid/block layouts and may be useful internally, but Masonry's widget pass system is the actual native UI substrate. Clay should first build a Masonry-native shell and use Taffy only as an internal helper if later justified.
5. **Make Markdown mode define its own layout/panel conventions now** — Rejected. Markdown should not become the source of global Clay UI conventions. The package shell/layout/theme/action model should be generic and reusable by future packages and modes.
6. **Clay-owned declarative shell with package contributions compiled to native Masonry UI** — Selected. It keeps packages easy to author, keeps users' configuration coherent, preserves Clay's authority boundaries, and fits Masonry's intended role as a foundation for higher-level GUI libraries.

## Rationale and Evidence

- Masonry 0.4.0 documentation describes Masonry as a foundational Rust GUI framework for building higher-level GUI libraries, with the user-facing abstraction intentionally left to downstream crates. This supports Clay defining a package-facing UI/layout abstraction above Masonry rather than exposing Masonry directly.
- Masonry's composition root is `RenderRoot`, and `masonry_winit` starts OS windows with `NewWindow` and a root widget. Clay currently starts with one root `EditorWidget`, which means the editor is acting as the app shell. The new architecture should introduce a Clay shell/working-area root widget and make editor views components inside it.
- Masonry container documentation requires containers to call `LayoutCtx::run_layout` and `LayoutCtx::place_child` for every child during layout. This means package layout updates must be applied as validated state before/through widget mutation/update passes, not by running package logic during layout or paint.
- Masonry's `Split` widget already provides horizontal/vertical split areas, ratios, draggable resizing, min sizes, and pointer handling, making it a strong implementation substrate for Clay pane splits.
- Masonry's `Flex`, `Grid`, `ZStack`, and `Portal` widgets provide native building blocks for component layout, overlays/transient UI, grid placement, and scrollable areas. Clay can use these internally while exposing stable Clay component names to package authors.
- Masonry widget properties are typed data used mostly for styling, such as background, border, padding, content color, and corner radius. That aligns with Clay exposing typed style variables and semantic tokens rather than CSS or callbacks.
- Existing Clay docs already require packages to contribute inert declarations and server-side handlers, while the Rust client renders validated behavior manifests, SDUI trees, protocol updates, and decoration data without client-side JavaScript. The new shell/layout decision extends that rule to the full package UI model.
- Existing Clay rendering docs already use style tokens for decorations (`markup.heading.1`, `diagnostic.error`, etc.) rather than arbitrary style strings; the shell style model should follow the same tokenized approach for containers and components.
- Deno documentation confirms the importance of explicit permission boundaries and clarifies that module loading/execution authorities must be controlled. Clay's package runtime should continue to run server-side through Clay facades and deny raw operations/client JavaScript by default.

## References

- `C:\Users\ashiq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\masonry-0.4.0\README.md` — Masonry overview, role as foundational GUI framework, widget tree/testing/debugging notes.
- `C:\Users\ashiq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\masonry-0.4.0\ARCHITECTURE.md` — Masonry goals, `RenderRoot`, `AppDriver`, widget hierarchy, passes, properties, and testing.
- `C:\Users\ashiq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\masonry_core-0.4.0\src\doc\pass_system.md` — event/rewrite/render passes, layout/compose semantics, mutation timing, and event bubbling.
- `C:\Users\ashiq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\masonry_core-0.4.0\src\core\widget.rs` — `Widget` trait, layout/paint/accessibility/input hooks, focus/pointer behavior, and child invariants.
- `C:\Users\ashiq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\masonry-0.4.0\src\doc\implementing_container_widget.md` — container layout requirements and child registration invariants.
- `C:\Users\ashiq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\masonry-0.4.0\src\doc\widget_properties.md` — typed properties as styling data, not behavior.
- `C:\Users\ashiq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\masonry-0.4.0\src\widgets\split.rs` — implementation substrate for split panes.
- `C:\Users\ashiq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\masonry-0.4.0\src\widgets\flex.rs`, `grid.rs`, `zstack.rs` — native layout/overlay building blocks.
- `src/main.rs` — current Clay root widget launch path using `EditorWidget` as the root widget.
- `src/masonry_editor.rs` and `src/masonry_sdui.rs` — current editor-root and SDUI overlay implementation that should evolve into a Clay shell/component model.
- `docs/wiki/modules/server-driven-ui.md` — existing inert SDUI schema, command-intent action validation, and native Masonry reconciliation.
- `docs/wiki/modules/rendering-primitives.md` and `docs/reference/primitives/rendering-strategy.md` — inert rendering/style-token model and no-package-JS-in-paint invariant.
- `docs/reference/primitives/package-security.md` — package security, provenance, prohibited authorities, and package primitive validation.
- `docs/wiki/flows/client-behavior-routing.md` — behavior manifest input routing and client UI command routing.
- `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md` — configuration through `init.js` and documented Clay JS APIs.
- `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md` — package distribution, package identity/prefix, and install/enable/runtime separation.
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md` — explicit package loading from `init.js` with one-line defaults.
- Context7 `/dioxuslabs/taffy` docs — Taffy supports flex/grid/block layout trees and `compute_layout`, but this decision limits Taffy to possible internal helper use after Masonry shell work.
- Context7 `/denoland/docs` docs — Deno secure runtime/permission model informs server-side package runtime authority boundaries.
- Commands run for verification: `CARGO_TARGET_DIR=target/pi-doc cargo doc -p masonry --no-deps --quiet` and `CARGO_TARGET_DIR=target/pi-doc cargo doc -p masonry_winit --no-deps --quiet`.

## Consequences

- New roadmap phases must be inserted immediately after Phase 18 and before other feature work to implement the shell/layout/package UI foundations.
- `EditorWidget` should become an editor component/view inside a Clay shell rather than remaining the long-term root application shell.
- Current SDUI should evolve from a fixed/sidebar overlay model into a slot-aware declarative package UI/component snapshot model applied by a Clay shell widget.
- Package author documentation must explain the standard package surfaces: manifest, explicit `init.js` loading, UI/layout contributions, input declarations, command/action declarations, logic/runtime handlers, data/state, configuration, style/theme tokens, permissions, docs, tests, and security constraints.
- Every shell/layout/package UI phase must update the package authoring documentation as APIs and examples become implemented, so docs do not lag behind architecture changes.
- The Markdown end-user loading/UI cleanup plan must be revisited after the shell/layout phases land; Markdown should then be updated to consume the implemented package layout APIs instead of relying on fixture-style SDUI panels.
- Raw CSS, arbitrary client-side JavaScript, direct Masonry widget mutation, native widget handles, Vello/Parley callbacks, raw Deno ops, filesystem/network/shell/AI/WASM authorities, and package-manager execution remain denied to package UI contributions unless a future approved decision introduces explicit permission-bearing APIs.
