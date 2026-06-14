---
date: 2026-06-09 02:19
status: approved
decision_about: "Explicit init.js package loading with one-line defaults"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: JS packages are explicitly loaded from init.js with one-line defaults when possible

## Decision

Clay JavaScript packages must be explicitly loaded from `~/.config/clay/init.js`. The preferred default setup for a package is a single user-facing load command, such as `loadPackage("@clay/markdown")` or an equivalent documented Clay JS API; after that command, the package's default mode behavior should usually work without additional user code.

Packages may expose optional customization APIs for package or mode behavior, but customization is not mandatory for the default path. If Clay's current primitive/API limitations make a one-line default impossible for a specific package, that package may require more setup temporarily, but the extra setup should be documented as a fallback/limitation rather than treated as the preferred convention.

## Context

The Phase 19 Windows Markdown smoke fixture proved that Markdown file opening and Markdown mode activation could work, but the fixture `init.js` contained a large amount of boilerplate: inline package metadata, multiple Clay facade imports, manual mode registration, activation, command registration, parse handler registration, decoration publication, and SDUI panel publication. That script was useful for deterministic smoke validation, but it is not acceptable as the end-user Markdown setup.

The user clarified the desired package-loading model while reviewing the Phase 20 Markdown end-user plan: packages should not be silently active by default, but enabling a package in user configuration should be concise. End users should be able to opt into `@clay/markdown` explicitly with one default load command most of the time, while advanced users can add package-specific customization only when they need it.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: The user said, "You can go ahead and create a decision log" and specified the decision points: each JS package is explicitly loaded in `init.js`; the default should be a one-liner like `loadpackage(@clay/markdown)`; further customization should be possible but not mandatory; and multi-line package setup should be treated as an exception caused by Clay limitations rather than the preferred design.

## Alternatives Considered

1. **Auto-load first-party packages without `init.js` opt-in** — Rejected. It would make package activation implicit, reduce user control, and weaken the configuration model that treats `init.js` as the user-owned entry point for behavior-changing setup.
2. **Keep large fixture-style setup as the normal user path** — Rejected. It forces users to understand package manifests, Clay facade plumbing, parser/decorations registration, and test-only UI setup before Markdown can work.
3. **Require every package customization to be specified up front** — Rejected. Defaults should be sufficient for common use; customization should be optional and package-specific.
4. **Adopt one-line package loading as a hard universal rule** — Rejected as too rigid. Some packages may need additional setup until Clay primitives mature, but that should be documented as a limitation or fallback.
5. **Use an explicit one-line load command by default, with optional package customization APIs** — Selected. It preserves opt-in package activation while making ordinary setup usable.

## Rationale and Evidence

- `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md` establishes `~/.config/clay/init.js` as the configuration entry point and requires configuration behavior to be expressed through documented Clay JS APIs rather than hidden keys.
- `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md` separates package installation from execution and requires Clay to own the package contract while avoiding a custom package manager.
- `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md` requires mode/package behavior to remain package-owned and primitive-first, with no Markdown-specific Rust branches.
- `tests/fixtures/configuration/windows-markdown-open/init.js` and `tests/fixtures/configuration/markdown-mode/init.js` demonstrate the current fixture boilerplate that should be collapsed behind a package-owned/default loader for normal users.
- `packages/markdown/dist/load.js` already centralizes much of the Markdown package's mode registration, command registration, editor rules, and parse handler registration, indicating that a concise package-level default load surface is feasible.
- `plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md` records the immediate implementation need: make Markdown mode usable through end-user configuration rather than a smoke-fixture-only script.

## References

- `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md` — `init.js` and configuration-through-Clay-JS-APIs decision.
- `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md` — package distribution and installation/execution separation.
- `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md` — primitive-first package/mode ownership.
- `tests/fixtures/configuration/windows-markdown-open/init.js` — current smoke fixture boilerplate that motivated the decision.
- `tests/fixtures/configuration/markdown-mode/init.js` — current Markdown fixture boilerplate.
- `packages/markdown/dist/load.js` — current package-owned Markdown load helper.
- `docs/reference/clay-js-api/configuration.md` — current configuration model and Phase 19 review.
- `docs/wiki/modules/first-party-markdown-package.md` — current Markdown package implementation notes.
- `plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md` — plan that should implement this direction for Markdown.

## Consequences

- Future package plans should include a task or acceptance criteria for the default `init.js` package-load experience.
- The default setup target is a one-line explicit package load command; users should not need to paste manifests or register every primitive manually for ordinary package use.
- Package customization remains possible through documented Clay/package JS APIs, but it should be optional for common use.
- If a package cannot support one-line default loading because Clay lacks a needed generic primitive, the plan should identify the generic primitive gap and document the multi-line setup as a temporary fallback.
- The create-plan project requirements and project pattern memory should be updated so future AI agents apply this convention when planning package work.
- This decision does not grant packages new authority: package loading remains server-side, explicit, validated, and deny-by-default for filesystem, network, shell, AI, WASM, raw ops, client-side JavaScript, and package-manager execution authority unless a future documented permission-bearing API says otherwise.
