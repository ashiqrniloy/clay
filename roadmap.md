# Clay Implementation Roadmap

## Window management with splits and tabs

## Command Centre

## Handling config, key binding, theme, font from UI with config file override

## File browser with dynamic root selection

## User Package and Config segregation with defined ~/.config/clay structure

## Agentic AI with Prism
- Prism upgrade with Web agent for search with Exa, Firecrawl, Brave search
- Agentic web action
- Web bridge

## JSON

## YAML

## TOML

## Terminal Emulator package

## Python

## Jupyter and IPYNB

## Latex

## Phase 22: AI-Safe Mutation and Region Locks

Support AI-generated edits without corrupting user state.

Focus areas:

- Make region locks first-class.
- Require AI edit sessions to carry explicit document versions, behavior versions, mode/package primitive versions, ranges, and permission scopes.
- Add preview/apply/reject flows.
- Add conflict explanations.
- Consider transaction logs and richer correction transactions.
- Separate extension/agent permissions from direct user input.
- Lock only the needed scope: range, document, behavior, mode, rendering primitive, or workspace.

Expected outcome:

- AI agents can propose or apply changes safely.
- User edits and agent edits have explicit conflict boundaries.
- AI-visible tools and mutation capabilities are documented and inspectable.

## Coding agent

## Markdown mode preview implementation with capabilities required for personal and work agent

## PDF mode with links to md files

## Personal Assistant Agent
- Extends markdown mode for personal knowledge management
- To do lists
- Schedule management
- Automation for daily tasks

## Work Agent
- Extends markdown mode for work management
- Office CLI with GUI

## Research Agent
- Reference management
- Show reference from source

## Finance Agent

## Clay agent
- Update wiki for AI agents and access in user device
- Extension writing methodology and knowledge system for AI agents

## UI update for managing agents

## Phase 21: Remote, Container, and Multi-Client Hardening

Make the server/client split useful beyond local IPC.

Focus areas:

- Remote server connection over secure transport.
- Container/toolbox/distrobox server startup and discovery.
- Live workspace-root discovery for UI/help surfaces, including a dedicated root-list protocol/server method if this is still needed before or beyond general Clay JS runtime wiring.
- SSL/TLS or SSH/tunnel strategy.
- Multiple clients connected to one server.
- Multiple documents open concurrently at scale.
- Read-only observer behavior for duplicate opens.
- Server concurrency and per-document actor scaling.
- CI coverage for `cargo fmt --check`, native `cargo test --all-targets`, Windows MSVC checks, generated registry freshness, package docs, and wiki navigation.
- Add `cargo bench --no-run` to CI to verify all Criterion benchmark targets compile on every push without running machine-variant timing loops.
- Promote Phase 14 advisory latency budget constants (`KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, `EDIT_ACK_P95_BUDGET_MS`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`, `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS`) and Phase 16 primitive/package budget constants (`DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `MODE_ACTIVATION_P95_BUDGET_MS`, completion/folding payload budgets, and package/mode validation payload ceilings) to hard CI thresholds only after verifying stability across at least one consistent CI runner and representative Phase 17/18 fixtures; document the promoted values and remove the advisory-only qualifier from `docs/development/performance.md` and primitive reference docs.
- If developer-only profiling hooks have been promoted to a stable user-facing feature by this phase, verify the `clay:diagnostics` Clay JS API exists with Markdown docs, inventory entry, generated registry entry, and lookup coverage; otherwise confirm the `no_public_configuration_needed_for_internal_perf_hooks` guard test remains active.

Expected outcome:

- A host client can connect to a server running in a target development environment.
- Clay can support local, container, and remote editing without changing the client authority model.



## Phase 23: Ecosystem and Repository Hardening

Prepare Clay packages and primitive APIs for a broader ecosystem after first-party package/mode proof points exist.

Focus areas:

- Package repository policy, package publishing workflow, trust, signatures or integrity checks beyond delegated package-manager integrity, offline/local packages, registry metadata, upgrades, removal, compatibility policy, package-manager environment diagnostics, and persistent shared package enable/disable state across CLI, in-app UI, and server runtime processes.
- Documentation coverage gates for Clay JS APIs, packages, generated registries, code wiki navigation, package-provided user-facing features, primitive contributions, and mode behavior.
- User/developer package UI for install, enable, disable, upgrade, remove, inspect permissions, inspect primitive contributions, and diagnose conflicts.
- Additional first-party package/mode examples beyond Markdown, using the primitive registry to expose missing capabilities iteratively.

Expected outcome:

- Clay has a sustainable package ecosystem path after proving package-controlled editing/rendering locally.
- The primitive registry grows through real modes while remaining inspectable and performance-safe.
