# Language Capability Sequencing

- After Phase 18, generic editor capabilities must land before Phase 19 hot reload: transient menu/command execution, generic text/code fallback modes, package-provided syntax grammars, completion, workspace/file navigation, Git package foundations, then first-party language package expansion.
- Syntax highlighting must prove package-provided grammar contributions first. Do not start by bundling Rust/TypeScript/JavaScript grammar ownership into Clay core.
- `@clay/rust`, `@clay/typescript`, and `@clay/javascript` begin as grammar-only first-party packages: grammar/query assets, syntax metadata, style-token mapping, and provenance. They do not become full modes until later phases expand them through generic primitives.
- Active major mode and active syntax grammar are related but separate. Grammar packages may attach syntax to `core.code`/`core.text` fallback documents without declaring a full language mode.
- Clay core owns generic grammar contribution contracts, validation, parse scheduling, decoration transport, and hot-path safety. Language packages own grammar/query assets and package-visible metadata.
- Arbitrary third-party grammar/native artifact loading stays out of scope until a dedicated security/trust decision defines integrity, sandboxing, and user authorization.
- Language package plans must include primitive review, package-provided grammar resolution tests, disabled/invalid package fallback tests, docs/registry/wiki coverage, and no language-specific Rust branches.
- Decision log source: `decision-logs/2026-06-29-2006-package-provided-grammar-and-capability-phases.md`.
