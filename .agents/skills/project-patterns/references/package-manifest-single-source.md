# Package Manifest Single Source

- `clay.contributions` in `package.json` is the **only** package registration data path. Load entries (`load.js`) execute code only: import parse modules, wire bridge factories. No imperative registration calls in first-party package load ceremony (decision `2026-08-18-1758-single-manifest-package-loading.md`).
- Imperative registration APIs (`serverRegisterModePattern`, etc.) are reserved for user `init.js` configuration and runtime contributions.
- Tier 1 native grammars: the Rust `NativeGrammarDescriptor` owns grammar data, queries, and style maps. First-party package.json files do **not** declare `syntaxGrammars` — a package-side `styleMap` for a native grammar is dead data that will drift.
- Manifests may declare a `preset` (`code-mode`, `prose-mode`, `lsp-bridge`) that expands into the standard permissions, `apiDependencies`, extension points, and contribution families for the archetype; validation runs on the expanded set, and explicit deviating declarations win over preset defaults (decision `2026-08-18-1758-package-capability-presets.md`).
- Applies to every plan that adds or refactors a package. A new code-language package should be a manifest with `preset: "code-mode"` plus only deviating declarations, and a load entry that imports code — not a copy of the pre-2026-08 four-source boilerplate.
