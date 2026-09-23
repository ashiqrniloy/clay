---
date: 2026-09-08 21:41
status: approved
decision_about: "Binary provisioning: table-driven presence check plus permissioned install"
proposed_by: agent
explicitly_approved_by_user: true
---

# Decision: Binary provisioning is deny-by-default with per-invocation explicit approval

## Decision

Feature-referenced external binaries (obscura, graft, qmd, ripgrep, and
future entries) are provisioned through a single data table
(`src/packages/binaries.rs`), never per-binary code branches. The model:

- **Presence check is user-triggered only** (`clay install` with no spec):
  report present/absent per binary using each feature's existing
  fail-closed resolution rules (explicit configured path → PATH, plus the
  entry's documented absolute fallback). No startup checks, no editor
  hot-path checks, no new silent resolution.
- **Install requires explicit per-invocation approval**: the report names
  the missing binary, the feature that needs it, and the exact command
  Clay would run. The install only executes when the same invocation
  carries `--bin <name> --yes`; without `--yes` Clay prints the command
  and changes nothing (non-interactive contexts report, never prompt).
- **Lifecycle scripts stay off unless `--allow-scripts` is passed on the
  same invocation** (binary-distribution packages commonly need them, so
  the approval is per-invocation and recorded in the command output).
- **Installs route through the shared npm-compatible manager backend**
  into the Clay-owned package store (binaries land in
  `<store>/node_modules/.bin`); provisioning never touches package
  records, grants, ledger entries, or feature availability.
- **Features never gain ambient authority**: each feature's existing
  resolution (e.g. `CLAY_OBSCURA_BIN` → PATH → `/usr/local/bin/obscura`,
  graft `cliPath` → peer → host packageRoot, `qmdPath`) is unchanged;
  provisioning only places files. Presence-only entries (no verified npm
  binary package: obscura, qmd, ripgrep) print the manual install command
  and can never be auto-installed by Clay.
- **Removal is documented per entry** at install time (store-relative
  `npm remove --prefix <store> <pkg>` or the manual step); no
  binary-specific removal code.

`accept_update` in `src-tauri/src/release.rs` remains the only
payload-apply gate; provisioning downloads nothing itself — the manager
does, under `--ignore-scripts` default.

## Context

The roadmap Phase 3 bullet says: check if the host has the binaries
installed; if not, install with permission. A silent `npm i -g` at
feature use would violate the external-process authority pattern
(deny-by-default, explicit approval, truthful containment language) and
the lifecycle-script default. A data table keeps "add a binary" a
one-line change.

## Alternatives considered

- Bundle all binaries in the `@arnilo/clay` npm package — rejected:
  platform/size cost for users who never touch the features.
- Silent global install at feature use — rejected: deny-by-default
  violation; no per-invocation record.
- Presence-only reporting with no install path — rejected: the roadmap
  explicitly requires the permissioned install path.

## Approval

- Proposed by: agent (plan 115 task 7 "Chosen Approach")
- Approved by user: Yes
- Approval evidence: user approved plan 115 (including this task's
  approach: "a decision log entry is written and approved before any
  script-enabled binary install path is implemented") and instructed
  "Complete next task Provision used binaries: presence check plus
  permissioned install" (2026-09-08).
