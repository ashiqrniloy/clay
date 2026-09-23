---
date: 2026-09-23 19:46
status: approved
decision_about: "Dev toolchain management: mise becomes the single dev-environment setup and version-pinning mechanism (bun, transitional node), local and CI"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: mise-Based Dev Toolchain

## Decision

Clay's dev environment setup moves to **mise** (`jdx/mise`): one
`mise.toml` at the repo root pins every dev-tool version (Bun, and Node
during the Bun transition), `mise install` is the one-command setup for a
fresh clone, and CI installs the same pinned tools via the mise GitHub
action so local and CI toolchains cannot drift. Rust stays on
rustup/`rust-toolchain.toml` (already canonical; see plan 148's toolchain
policy task for the pin decision — mise does not take rust over).

## Context

- Today's setup is scattered: `setup-node` pins Node 24 in CI only,
  `docs/development/build-and-test.md` describes manual prerequisites,
  and Bun arrives via the runtime migration (decision
  2026-09-23-1946-bun-runtime-nodejs-surfaces) with no pin at all.
- User objective: simplify dev setup and make it portable across
  machines.

## Approval

- Proposed by: user.
- Approved by user: Yes
- Approval evidence: "I want to shift the dev environment setup to a mise
  based setup to simplify the dev setup and portability in terms of dev
  setup." (2026-09-23)

## Alternatives Considered

- Keep per-tool setup (setup-node + manual bun install) — rejected:
  drift between machines and CI, no single setup command.
- asdf — rejected: mise is its maintained superset (toml config, faster,
  same plugin ecosystem).
- Nix/devenv — rejected: heavier machinery than version pinning needs for
  this repo.
- mise also managing Rust — rejected for now: rustup +
  `rust-toolchain.toml` already covers it and plan 148 (toolchain
  pinning) owns that policy; two rust sources would fork the toolchain.

## Consequences

- Plan 137 implements: root `mise.toml`, CI switch to the mise action,
  docs update. Plan 139 removes the transitional Node pin when the daemon
  runs on Bun.
- Contributors install one tool (mise); `mise install` + rustup covers
  everything.
- Windows dev: mise on Windows is experimental; Windows remains a
  long-term target (platform-validation rule), Linux CI stays the
  blocking gate; the mise gap is documented, not blocking.
