---
date: 2026-06-15 10:15
status: approved
decision_about: "Defer the generic one-line loadPackage(\"@clay/*\") resolver and its first-party module-loader bridge to a dedicated later phase"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Defer the generic `loadPackage("@clay/*")` resolver and first-party package-loadEntry bridge to a dedicated phase

## Decision

Phase 18.5 does **not** implement the generic one-line end-user package loader `loadPackage("@clay/markdown")` (or any equivalent generic specifier resolver). The generic loader/API gap remains explicitly documented, and the preferred end-user `~/.config/clay/init.js` setup continues to be the one-line `loadPackage("@clay/markdown")` **target**, with a documented temporary fallback that consumes implemented generic primitives and never requires users to copy package manifests into `init.js`.

The resolver, its backing op, and the constrained first-party module-loader bridge that lets the controlled server-side runtime import a declared `loadEntry` from outside the configuration root are deferred to a dedicated later phase with their own plan, security review, and tests. Phase 18.5 closes the task by verifying the gap is decision-log-backed and by shipping the cleanest package-owned fallback entry point.

## Context

- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md` establishes the convention: packages are loaded explicitly from `~/.config/clay/init.js`; the preferred default is a one-liner such as `loadPackage("@clay/markdown")`; when Clay lacks the primitive, longer setup is a documented temporary limitation, not the preferred design.
- `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md` (Task 4) chose: "Attempt to implement a generic or constrained `loadPackage` primitive. If authority, validation, or scope constraints prevent a safe implementation, document the gap in a decision log and implement the next-best package-owned default entry point with full docs/tests." Its "Further Actions" adds: "Add a decision log if the `loadPackage` generic resolver is deferred, to record the temporary fallback rationale."
- `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md` identifies `loadPackage("@clay/markdown")` as the **only** generic primitive gap blocking the Markdown replan, names the candidate surface (`clay.packages.loadPackage` / `op_clay_packages_load_package_by_specifier` + a `src/packages/service.rs` resolver), and says a safe Phase 18.5 scope is a constrained first-party `@clay/*` resolver that reuses the existing `PackageService` validation path and runs the declared `loadEntry` against the validated `PackageRecord`.

Investigation during Task 4 found that a working one-line loader cannot be a thin wrapper:

- `src/server/js_runtime.rs::ClayModuleLoader` is deny-by-default. It allows only the curated `clay:*` facades, the vendored `markdown-it` module, and configuration-root-relative modules. `src/server/configuration.rs::canonical_local_file` rejects any path that does not start with the configuration root. There is no path by which `~/.config/clay/init.js` can import a package's `loadEntry` (e.g. `packages/markdown/dist/load.js`) today.
- `src/server/ops/packages.rs` only has `op_clay_packages_validate_manifest`, `op_clay_packages_validate_permissions`, and `op_clay_packages_load_package` (which validates a caller-supplied manifest value). There is no op that resolves an installed package specifier, reads its `package.json`, enables it through `PackageService`, and executes its declared `loadEntry`.
- `src/packages/service.rs::PackageService` can validate/enable a package record from an in-memory JSON value but is not wired into the JS runtime and has no concept of a first-party packages directory the runtime can resolve `@clay/*` against.
- The Markdown package's own `loadMarkdownPackage(clay, options)` entry takes a `clay` facade object as a parameter; nothing in the runtime constructs that object or invokes the entry, so it is not yet a usable end-user fallback.

Making the one-liner actually work therefore requires a security-relevant authority expansion: the controlled runtime must be allowed to load first-party package JavaScript from outside the configuration root, gated by a validated `@clay/*` resolver. That boundary is exactly what `clay_facade_source` and the config-root canonicalization exist to protect, and the package-security and shell-layout-strategy references require such expansions to ship with their own validation, docs, registry entries, and tests rather than being folded into a Markdown replan task.

## Approval

- Proposed by: agent (during `plans/028` Task 4 execution).
- Approved by user: Yes.
- Approval evidence: The user directed execution of `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md` Task 4 ("Implement or verify the package default `init.js` loading experience ... Update the plan once done"). That plan's Task 4 "Chosen Approach" and the plan's "Further Actions" both explicitly authorize the decision-log deferral path when a safe in-phase implementation is not feasible. The investigation above established that feasibility condition.

## Alternatives Considered

1. **Implement the constrained first-party `@clay/*` resolver inside Phase 18.5 Task 4.** — Rejected as a fold-in. It requires extending `ClayModuleLoader` to load package `loadEntry` modules from outside the config root, wiring a packages-directory resolver into the JS runtime, adding a new op, and adding a `PackageService` enable/execute path. That is a security-critical authority expansion that warrants its own plan, security review, decision log, and dedicated tests, not a sub-task of a Markdown replan.
2. **Ship a Markdown-specific `MarkdownLoader` / `if package == "@clay/markdown"` Rust branch as a stopgap.** — Rejected by the primitive-first decision (`decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`) and by the Phase 18.5 primitive review. Package loading must stay generic.
3. **Keep the fixture-style inline manifest (`const markdownPackage = { ... }`) as the documented end-user path.** — Rejected by `decision-logs/2026-06-09-0219-...` and by the package guide; fixture scripts are validation tools, not end-user setup.
4. **Defer the resolver and document the gap with a decision log + a clean package-owned fallback entry that imports Clay facades internally.** — Selected. It satisfies the Task 4 acceptance criteria (gap explicitly documented with decision-log-backed rationale + temporary fallback that uses implemented generic primitives and not fixture-only copied manifests), preserves the one-line target, preserves the explicit `Ctrl+O` separation, and avoids a security-critical fold-in.

## Rationale and Evidence

- The one-line `loadPackage("@clay/markdown")` target and the explicit `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` separation are preserved unchanged in all docs, the primitive review, and the replanned Plan 023.
- The fallback entry point added to the Markdown package (`markdownLoadMode()` in `packages/markdown/dist/load.js`, re-exported from `./dist/index.js`) imports the `clay:packages`, `clay:modes`, `clay:commands`, and `clay:parse` facades directly and reuses the existing `loadMarkdownPackage` logic. It contains no inline manifest object, no copied fixture metadata, and no Markdown-specific Rust branch; it consumes only implemented generic primitives.
- `tests/package_loading_docs.rs::package_default_init_js_loading_documents_one_line_path_or_current_gap` continues to assert that no `loadPackage` export/op/registry row exists yet, and now also asserts the deferral is decision-log-backed and the package-owned fallback entry exists with clean internals.
- The security boundary is unchanged in Phase 18.5: package loading remains deny-by-default for arbitrary external specifiers, package-manager execution, registry fetching, and arbitrary specifier expansion; the runtime still cannot load modules from outside the configuration root except through the existing curated facades and the vendored `markdown-it` bundle.

## References

- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md` — the one-line package loading convention this deferral operates under.
- `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md` — Task 4 "Chosen Approach" and "Further Actions" authorize the deferral + decision log.
- `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md` — identifies `loadPackage("@clay/markdown")` as the only generic gap and names the candidate surfaces.
- `docs/reference/primitives/package-loading.md` — package-loading primitive contract and the documented gap.
- `docs/reference/packages/creating-packages.md` — package authoring guide and one-line loader status.
- `docs/wiki/modules/package-loading.md` — package loading implementation wiki and gap notes.
- `src/server/js_runtime.rs` — `ClayModuleLoader`, `clay_facade_source`, and the deny-by-default module boundary.
- `src/server/configuration.rs` — `canonical_local_file` config-root confinement.
- `src/server/ops/packages.rs` — existing package validation/load ops (no specifier resolver).
- `src/packages/service.rs` — `PackageService` validation/enable path the future resolver must reuse.
- `packages/markdown/dist/load.js` — package-owned load entry and the new `markdownLoadMode()` fallback.

## Consequences

- The end-user one-line `loadPackage("@clay/markdown")` remains a documented target, not a callable API, in Phase 18.5. Smoke and configuration fixtures continue to drive deterministic validation through `serverLoadPackage(packageJson)` plus package-owned helpers.
- A future phase must implement, as one focused unit: (a) a constrained first-party `@clay/*` specifier resolver op, (b) a `PackageService` path that resolves, validates, enables, and returns the declared `loadEntry`, (c) a `ClayModuleLoader` extension that loads only resolver-validated first-party package `loadEntry` modules from outside the config root, (d) a `loadPackage` facade export, (e) full docs/index/generated-registry/api-inventory/tests/Rust-visibility coverage, and (f) its own decision log recording the authority expansion.
- The package-owned `markdownLoadMode()` fallback entry is the shape that future resolver will invoke, so the Markdown package is ready for the bridge without further changes.
- No new filesystem, network, shell, AI, WASM, raw-op, native-widget, client-JS, package-enable/disable, or package-manager execution authority is granted by this decision.
