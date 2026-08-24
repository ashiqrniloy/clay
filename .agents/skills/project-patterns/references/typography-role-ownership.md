# Typography Role Ownership

- Before reviewing or changing UI typography, load `clay-ui` plus the mandatory `impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend` skills; record all of them in the task's plan/review evidence.
- Clay user configuration owns concrete font-family fallback stacks and logical-pixel sizes for `monospace`, `proportional`, and `ui` profiles.
- Packages/modes declare semantic roles only; they must not select concrete families or absolute sizes. One role resolves both family and size.
- `core.code` defaults monospace; `core.text` and Markdown default proportional; Markdown code ranges use monospace; Clay/package component text defaults UI.
- Keep typography in a layout-affecting client registry and protocol snapshot separate from theme color/style state. Include typography/style revisions in layout invalidation.
- Resolve installed fonts on the client, retain generic fallbacks, and keep package JavaScript/IPC out of paint, input, and layout hot paths.
- Plans changing typography must cover editor shaping, mixed-role ranges, scrolling/viewport geometry, UI row/hit/accessibility geometry, package contracts, configuration/API docs, and deterministic fallback/invalidation tests.
- Decision source: `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`.
