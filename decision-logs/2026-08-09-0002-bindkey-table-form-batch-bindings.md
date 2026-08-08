---
date: 2026-08-09 00:02
status: approved
decision_about: "bindKey/unbindKey batch table form"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: bindKey/unbindKey gain a batch table form

## Decision

`clay:keybindings` `bindKey` and `unbindKey` gain overloaded table forms:
`bindKey({ scope, bindings: { chord: command, ... } })` (one call, one scope,
a chord→command map) and `unbindKey({ scope, keys: [chord, ...] })`. The
single-argument forms stay unchanged and fully supported. Table-form calls
are all-or-nothing: every entry is validated before any is applied, a bad
entry rejects the whole table naming its 1-based index, and duplicate chords
inside a table collapse to the last value (JSON semantics) preserving the
per-chord "last binding wins" rule. No per-entry scope override was added.

## Context

The user found the per-call form unergonomic: every binding repeats
`bindKey(...)`, the full `clay.*` command ID, and `{ scope: ... }`. A
proposal presented three shapes (chord→command map with hoisted scope,
pairs array, scope-keyed map) plus a chained builder; the user approved the
map form and asked that batch `unbindKey` ship in the same round.

## Approval

- Proposed by: user (pushed back on ergonomics) + agent (shapes/semantics).
- Approved by user: Yes
- Approval evidence: "Let's go with your recommendations for the open
  questions. Implement unbindkey already as well." (accepted map table form,
  batch unbindKey in scope, no per-entry scope override).

## Alternatives Considered

1. **Pairs array** (`bindKey({ scope }, [["Ctrl+O", cmd], ...])`) — more
   verbose than the map (brackets per pair); not selected.
2. **Scope-keyed map** (`bindKey({ editor: {...}, global: {...} })`) — most
   compact for mixed scopes but scope names collide with future option keys
   (e.g. `when`); not selected.
3. **Chained builder** (`bindKey({ scope }).set(...).set(...)`) — fluent but
   needs an intermediate object and per-call ops; more machinery for no
   gain; not selected.
4. **Per-entry scope override** (`{ chord: { command, scope } }`) — added
   flexibility, cost ergonomics; rejected for now (YAGNI).
5. **Separate `bindKeys()` function** — one name per form is cleaner to
   dispatch but the user explicitly wanted "call bindKey once"; overloaded
   `bindKey` keeps a single name; not selected.

## Implementation Notes

- `src/server/ops/keybindings.rs`: new ops `op_clay_keybindings_bind_keys` /
  `op_clay_keybindings_unbind_keys`; shared pure helper `build_rule`;
  two-pass validate-then-apply; entry-indexed diagnostics reusing existing
  `invalid_bind`/`invalid_unbind` codes; batch bind returns the bound
  records in table order, batch unbind returns the final binding list.
- `runtime/js/keybindings.js` + `keybindings.d.ts`: object-first-argument
  overloads; `KeyBindingTable` / `KeyUnbindTable` types.
- No new public API surface beyond overloads (no new facade exports, no new
  config keys); ops are trusted-domain only; op-inventory pins updated
  (79 → 81 in `src/server/ops/mod.rs` domain test and
  `tests/primitives_docs.rs` plan 061 rebaseline, plan 061 inventory table
  annotated).
- `examples/init.js` section 7 migrated to the table form (active bindings
  and the commented default-keybinding reference); single form kept as
  documented alternative.
- Tests: 4 parser unit tests + 3 runtime tests (batch bind, all-or-nothing,
  batch unbind) + init.js loads-cleanly regression; full gate green.
