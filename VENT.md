# VENT

Feedback log. Repeated/systemic workflow friction that should become future automation, docs, or workflow fixes.

## 26-08-12 01:15 — conformance-test-target-discovery

UI conformance checks initially used direct cargo test targets, but Cargo.toml sets autotests = false and registers only suite wrappers; both direct targets failed before rerunning through cargo test --test editor filters. Add a project test-command map or expose stable wrapper aliases so plan tasks can invoke conformance suites directly.
## 26-08-12 17:16 — git-checkout-destroyed-uncommitted-phase-work

During the real_server_end_to_end harness fix, I ran `git checkout -- src/server/connection.rs src/client/mod.rs src/protocol/codec.rs` to strip temporary debug instrumentation and accidentally destroyed ~1100 lines of UNCOMMITTED Phase 24.1/24.2 working-tree changes in connection.rs and client/mod.rs (the Plan 081/082 preserved baseline). Recovery required: finding a mid-24.1 stash commit via `git fsck --lost-found` (4a3eb152, only partial state), then manually reconstructing the task-4/6/8/11 wiring and 7 tests from session observations — hours of rework with real risk of subtle divergence. Prevention: never use `git checkout -- <file>` to revert debug edits when the tree contains long-running uncommitted work; instead revert debug lines with targeted edits (or `git stash push -- <paths>` first), and commit/stash the working tree before starting a new task. A project rule like "phase working trees must be committed at task boundaries" would have made this a non-event.
## 26-08-13 03:48 — ctx7-masonry-lookup

Context7 library lookup could not resolve Linebender Masonry after two targeted queries (returned unrelated GTK/layer libraries); workaround used the already-known `/linebender/xilem` docs ID plus version-exact local Masonry 0.4.0 source. Prevent by indexing Masonry as a first-class Context7 library or documenting the canonical ID.
## 26-08-13 07:28 — plan-doc-sync

Plan 084 task execution needed repeated manual doc/catalog synchronization and task checkbox/record edits across plan, wiki, reference docs, and test-plan files; one manual-step number collision was introduced and fixed, and static guard tests initially overfit implementation strings then were simplified. Add a plan-completion helper/checker that validates task records, numbered manual tables, and required catalog/wiki paths before final gates.
