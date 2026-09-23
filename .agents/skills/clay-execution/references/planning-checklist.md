# Clay Planning Checklist

Use before writing or updating any Clay plan.

## Required Checks

- **UI skill routing:** If work touches UI, theme, typography, tokens, components, layout, SDUI, or accessibility, read `references/ui.md` plus the component/token catalogs before reviewing or editing implementation, and list them under each UI task's `Documentation Reviewed`; plan-level evidence does not substitute. Substantial new-surface design tasks additionally load `impeccable`, `full-output-enforcement`, `high-end-visual-design`, and `design-taste-frontend` per `ui.md`.
- **Decision alignment:** Identify relevant decision logs and roadmap sections.
- **Authority boundary:** State which component owns state, behavior, execution, persistence, and validation. The separate Rust server remains authoritative; Tauri is a narrow OS/transport bridge; React owns presentation; CodeMirror owns local editor hot-path state (see `packages.md` → Authority Boundaries). Agent work uses the agent-host rules in `packages.md` (Prism in `clay-agent`, ACP out, AG-UI limited to the React-facing Tauri transport). Product landings and agent profiles are packages; core owns host primitives, not greeting copy.
- **Client hot path:** Confirm ordinary CodeMirror typing/rendering applies locally and does not block on React rerenders, Tauri/server IPC, server work, package JavaScript, AI, file IO, or full-document serialization.
- **Server authority:** Preserve server ownership of canonical documents, versions, transactions, file/workspace authority, extension execution, leases, locks.
- **Behavior manifest:** If a feature changes hot-path editor behavior, decide whether it belongs in a server-issued behavior manifest, a server-first command, or a later phase (`protocol-perf.md`).
- **Documentation as code:** Public programmatic behavior is exposed and documented through Clay JS APIs; server-side Rust public functions must have Clay JS APIs or be private/`pub(crate)`; Clay JS APIs include user-facing names, key binding metadata, custom properties for behavior-changing settings, Markdown docs, generated registry coverage, lookup access. Internal implementation details belong in the wiki, not the public registry.
- **Clay JS API naming:** Apply `js-api.md` when designing/documented APIs: concise behavior-oriented callable exports, distinct from stable registry IDs and `user_facing_name`, server/client authority markers for editor-core APIs, package API provenance prefixes.
- **Configuration:** User configuration starts at `~/.clay/init.js`; each configuration option is a documented Clay JS API, not an undocumented config key.
- **Security:** Say what authority is not introduced: file IO, network, script execution, WASM, AI mutation, remote listener, shell, etc.
- **Performance:** Prefer deltas, bounded queues, per-document ordering, cancellable background work, viewport-bounded rendering, code-split heavy web renderers, explicit React render-count/bundle budgets.
- **Phase boundary:** If enforcement is deferred, describe it as a scoped limitation of the approved architecture, not a competing model.
- **Decision-log feedback:** After logging a decision, update the smallest relevant `clay-execution` reference file if the decision creates reusable planning guidance (see `SKILL.md` → Plan Creation and Decision-Log Integration).

## UI Prototype and Approval Duty

Source: user instruction 2026-09-11 (see `create-plan/references/clay.md` → UI Prototype and Explicit User Approval Task).

- Any plan touching app UI lists, in order: a prototype task (`design-artifacts/prototypes/<slug>/`), a freeze/approval task (`design-artifacts/approved/<slug>/`, explicit user approval required), then implementation tasks that cite the approved artifact path.
- If an approved artifact already covers the exact surface and state set, cite it and skip the prototype; a missing state stops and requests one.
- Approved artifacts are append-only and tracked in git; implementation deviations are fixed or re-approved, never silent.
- `design-artifacts/README.md` states the contract (prototype = no authority, approved = binding, `DESIGN.md` = normative language).

## Visual and Accessibility Review Duty

Source: `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.

- Every UI-changing plan includes one post-implementation visual and accessibility review task before final docs/wiki work.
- That review also compares the running UI against the plan's approved artifacts, surface by surface, and records each deviation with its disposition (fixed to match, or explicitly re-approved).
- Launch representative states, capture and inspect screenshots, retain their paths and findings in completion evidence.
- When `computer-use-linux` is available, call `get_app_state` first and verify accessibility tree semantics, keyboard flow, visible focus, modal containment, and announcements for changed controls.
- If live review tooling is unavailable, record the exact blocker and leave manual acceptance unresolved; structural tests are not visual proof.