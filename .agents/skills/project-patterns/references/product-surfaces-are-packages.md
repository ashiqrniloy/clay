# Product Surfaces Are Packages

- Default landing, Chat, Coding Agent, and later Work/PA/Research are
  first-party JS packages on Clay primitives — not compiled irreplaceable
  chrome. Canonical enablement is one `loadPackage` line.
- Third-party packages may `replaces` those packages or `extends` declared
  extension points with exact user approval. Replacement stays in the
  third-party runtime. Core/bootstrap and `clay-agent` are not packages.
- Empty/new-tab `main` hosts at most one pane-content contribution. No
  contribution → core fallback (Open File / Open Folder only). Do not add
  product-named pane kinds (`Agent`) for replaceable surfaces.
- Clay core still owns pane topology, React component registry, Command Centre,
  tab bar, Tauri OS dialogs, Prism daemon, vault, and `agent.*` authority.
  Packages declare inert SDUI and call documented APIs; they never spawn the
  daemon or receive direct Tauri IPC.
- Plans that add a product landing or agent must ship it as a bundled
  first-party package with extension points, one-line load, unload→fallback
  tests, and replace/rollback tests. Do not plan a core disabled stub for a
  future agent.
- Decisions: `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`, `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`.
