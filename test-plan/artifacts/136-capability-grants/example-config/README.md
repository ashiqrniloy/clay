# Plan 136 task 11 — canonical example configuration

Evidence for the capability-grant section in the shipped example tree.

## What changed

| File | Change |
| --- | --- |
| `examples/config/packages/third-party.js` | New commented `Capability grants` section (CLI form, `authorize({...})` form, inspect/revoke, replacement semantics, rejected hidden keys, self-grant refusal) and the adoption step list now includes the grant step |
| `examples/config/init.js` | Section 11 points at the grant section and names the `clay package authorize` verb |
| `examples/config/README.md` | Third-party module row describes the grant surface |

The section lives in `packages/third-party.js` because that is the module the
canonical tree loads for third-party configuration (init.js section 11); init.js
keeps the pointer. No third-party package ships with Clay, so the section is
commented documentation, not an active grant.

## Option surface annotated from the validated parser

- `clay package authorize <name> --capability <cap>... [--runtime-profile <p>] [--approved-by <who>]`
- `--capability` repeatable + required; the manifest must declare it
  (`clay.permissions` or the legacy `clay.capabilities` alias); a later grant
  **replaces** the whole set
- `--runtime-profile` `native-trust` (default) | `sandboxed` | `restricted`
- `--approved-by` `cli` (default) | `user` | `config`
- `authorize({ package, capabilities, runtimeProfile, approvedBy })` with the
  same defaults, attributed `config`
- inspect output labels as emitted by `verbs::format_grant_lines`:
  `Grants: <names> (<profile>)`, `Granted by:`, `Approved by:`, `Ungranted:`
- `clay package revoke <name>` withdraws approval **and** grant

## Checks

- `node-check-and-active-lines.txt` — `node --check` passes for `init.js` and
  `packages/third-party.js`; the only uncommented line in the third-party
  template is still the pre-existing `import { loadPackage } from "clay:packages";`,
  so the active config is unchanged and copy-safe (also covered by
  `tests/clay_js_doc_registry.rs::canonical_example_active_configuration_is_copy_safe`).
- `tests/clay_js_api_inventory.rs::plan136_configuration_documents_the_capability_grant_surface`
  now also pins 13 template markers plus the init.js pointer, so the section
  cannot silently rot.
- `gate-check-full-task11.log` — `scripts/check.sh full` (the example-config boot
  test evaluates the real tree in the real runtime, which is stronger than
  `node --check`).
