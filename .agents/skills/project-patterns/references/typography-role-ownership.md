# Typography Role Ownership

- Before reviewing or changing UI typography, run `npx ui-skills start`, inspect the relevant category, and load the smallest useful skill set (prefer 1, max 3); record selected slugs in the plan/review evidence.
- Clay user configuration owns concrete font-family fallback stacks and logical-pixel sizes for `monospace`, `proportional`, and `ui` profiles.
- Packages/modes declare semantic roles only; they must not select concrete families or absolute sizes. One role resolves both family and size.
- `core.code` defaults monospace; `core.text` and Markdown default proportional; Markdown code ranges use monospace; Clay/package component text defaults UI.
- Keep typography in a layout-affecting client registry and protocol snapshot separate from theme color/style state. Include typography/style revisions in layout invalidation.
- Resolve installed fonts on the client, retain generic fallbacks, and keep package JavaScript/IPC out of paint, input, and layout hot paths.
- Plans changing typography must cover editor shaping, mixed-role ranges, scrolling/viewport geometry, UI row/hit/accessibility geometry, package contracts, configuration/API docs, and deterministic fallback/invalidation tests.
- Decision source: `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`.
