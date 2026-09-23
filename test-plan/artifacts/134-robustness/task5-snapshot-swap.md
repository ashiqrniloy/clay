# Plan 134 task 5 — R3 snapshot swap only on change (2026-09-22)

Baseline: the clone site was `src/server/js_runtime/mod.rs` (post-plan-133
line ~801) inside `evaluate_entry_for_domain`'s success branch:

```rust
*self.completion_providers.lock_or_recover() = evaluation.completion_providers.clone();
```

Every successful evaluation — including theme-only reloads and no-op
re-evaluations — replaced the stored `Vec` with a fresh clone, even when the
provider list was identical.

## Change

One helper in `src/server/js_runtime/mod.rs`:

```rust
fn replace_snapshot_only_on_change<T: Clone + PartialEq>(
    slot: &std::sync::Mutex<Vec<T>>,
    providers: &[T],
) -> bool { ... }
```

- Gate: structural `PartialEq` on the slice (`stored.as_slice() == providers`).
  `CompletionProviderMeta` derives `Debug, Clone, PartialEq, Eq`; the list is
  bounded (provider cap) and contains scalars/short strings, so the comparison
  allocates nothing — no stringifying, no key hashing.
- On a difference only, `providers.to_vec()` clones and replaces; returns
  `true` when it swapped.
- Chosen over a generation-stamp key: a stamp alone would miss same-generation
  list edits; structural equality is both cheap and exact.

Call site: the evaluation success branch calls the helper instead of the
unconditional clone; the counter bumps (`self.evaluations`,
`domain_runtime.evaluations`) are unchanged.

## Test

`provider_snapshot_not_replaced_when_unchanged`
(`src/server/js_runtime/tests/facades_and_modes.rs`):

- stores `[core.a, core.b]`, then stores the identical list again — asserts the
  helper reports **no replacement** and the stored values are unchanged;
- stores a changed list `[core.a, core.c]` — asserts a replacement happens and
  readers see the new values.

The helper's boolean is the deterministic swap signal, so no allocation counter
or global hook is needed. The existing end-to-end fixture test
(`config_fixture_workflows::language_packages_config_fixture_loads_and_registers_all_contributions`)
still proves downstream readers see the same values as the evaluation result.
