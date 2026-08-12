# VENT

Feedback log. Repeated/systemic workflow friction that should become future automation, docs, or workflow fixes.

## 26-08-12 01:15 — conformance-test-target-discovery

UI conformance checks initially used direct cargo test targets, but Cargo.toml sets autotests = false and registers only suite wrappers; both direct targets failed before rerunning through cargo test --test editor filters. Add a project test-command map or expose stable wrapper aliases so plan tasks can invoke conformance suites directly.
## 26-08-12 17:16 — git-checkout-destroyed-uncommitted-phase-work

During the real_server_end_to_end harness fix, I ran `git checkout -- src/server/connection.rs src/client/mod.rs src/protocol/codec.rs` to strip temporary debug instrumentation and accidentally destroyed ~1100 lines of UNCOMMITTED Phase 24.1/24.2 working-tree changes in connection.rs and client/mod.rs (the Plan 081/082 preserved baseline). Recovery required: finding a mid-24.1 stash commit via `git fsck --lost-found` (4a3eb152, only partial state), then manually reconstructing the task-4/6/8/11 wiring and 7 tests from session observations — hours of rework with real risk of subtle divergence. Prevention: never use `git checkout -- <file>` to revert debug edits when the tree contains long-running uncommitted work; instead revert debug lines with targeted edits (or `git stash push -- <paths>` first), and commit/stash the working tree before starting a new task. A project rule like "phase working trees must be committed at task boundaries" would have made this a non-event.
