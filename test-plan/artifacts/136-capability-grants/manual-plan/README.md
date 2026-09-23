# Plan 136 task 13 — manual test plan execution

Executes the module 09 / module 11 steps this plan adds, on a real Linux build
(`cargo build --bin clay -p clay`, `cargo build -p clay-desktop`), in isolated
mode-700 scratch roots with a private socket per run. Teardown is PID-based
(`run-live.sh stop`); no pattern kills.

## Roots

| Root | Mode | What it runs |
| --- | --- | --- |
| `/tmp/clay-plan136-manual-granted` | `granted-lane` | CLI grant verb → enable → load (P56 positive, P57) |
| `/tmp/clay-plan136-manual-granted2` | `granted-lane` | same, plus the driver typing 6 characters and the perf summary (Q44 live leg) |
| `/tmp/clay-plan136-manual-ungranted` | `ungranted-lane` | adopt, never grant → `MissingCapabilityGrant` (P56 negative) |
| `/tmp/clay-plan136-manual-config` | `config-granted-lane` | the config `authorize` call, two launches on one store, then revoke/replacement/undeclared legs (P58, P57) |

## Evidence index

| File | What it shows |
| --- | --- |
| `granted/authorize.log`, `granted/inspect.log`, `granted/enable.log` | `Authorized @fixture/lane: completion-provider, mode-registration, parse-document (native-trust)` / `granted by: cli`; the `Grants:` + `Granted by:` lines in inspect; `Enabled @fixture/lane` |
| `granted/server.log` | zero `configuration failed`/`packages.load_failed` lines for the granted load |
| `ungranted/enable.log`, `ungranted/server.log` | `Error: MissingCapabilityGrant { package_name: "@fixture/lane", capability: CompletionProvider }` and the sanitized `configuration failed [packages.load_failed]: JavaScript runtime evaluation failed.` |
| `config/approvals-run1.json`, `approvals-run2.json` | the durable `grant {capabilities, runtime_profile: native-trust, granted_by: config, granted_at}` written by `init.js` |
| `config/inspect-fresh-cli-run1.log`, `enable-fresh-cli-run1.log` | a separate `clay` process reads `Grants: … (native-trust)` / `Granted by: config` and enables successfully |
| `config/server-run1.log`, `server-run2.log`, `config/init.js` | the config evaluation ran (agent registration lines) and the second launch loaded the package with no failure lines |
| `config/revoke.log`, `inspect-after-revoke.log`, `enable-after-revoke.log`, `approvals-after-revoke.json` | `Adoption: approval revoked` + `Ungranted: … (declared, not granted)`, `AdoptionRequired { code: "package_approval.revoked" }`, `grant: null` + `revoked: true` |
| `config/authorize-replacement.log`, `inspect-after-replacement.log`, `enable-after-replacement.log` | re-granting only `mode-registration` drops the other two (`Ungranted: completion-provider, parse-document`) and `enable` fails closed with `MissingCapabilityGrant` again |
| `config/authorize-undeclared.log`, `authorize-replacement.log` (revoked-record leg) | `does not declare capability package-control in its manifest`; `authorize` refuses to manufacture an approval on a revoked record (`run clay package adopt … first`) |
| `config/authorize-full-again.log`, `enable-full-again.log`, `approvals-final.json` | full re-grant with `--approved-by user` (`granted_by: user`) restores `Enabled @fixture/lane` |
| `granted-typed/editor-before.txt`, `editor-after.txt`, `driver.log`, `window-*.png` | focus guard held (clay-desktop focused, 952×1150+963+45), typing landed 29 → 35 chars, popup probe reports `no completion surface or status text` (recorded ceiling) |
| `granted-typed/perf-summary.json` | `server.edit_ack` p50 165.0 µs / p95 191.3 µs over 6 edits, `server.document.apply_edit` p50 42.6 µs, all 20 `js_runtime.lane.<domain>.<lane>.*` keys, 175 retained / 0 dropped |
| `automated-lane.txt` | `PLAN136_GRANTED_LANE busy_ms=500 idle_median_us=1619 busy_completion_us=2937 workers_started=4`, `PLAN127_LANE busy_ms=500 idle_median_us=2278 busy_completion_us=2852`, 17 lane tests green |

## Ceilings recorded

- The live completion popup cannot be driven: package-owned modes do not
  activate for open documents and a package mode rejects the built-in
  `completion.trigger` command, so the latency numbers come from the automated
  lane harness while the live leg contributes the edit-ack envelope and the lane
  occupancy counters (module 11 Q44 records this).
- The ungranted run's lane counters come from the bundled packages the fixture
  config loads, not from the ungranted fixture; that fixture's fail-closed
  evidence is the `MissingCapabilityGrant` + `packages.load_failed` pair.
- `granted/driver.log` shows one earlier attempt where the compositor had no
  `clay-desktop` window yet; the drill now waits before driving input.

## Harness changes from this task

- `run-live.sh` gained the `config-granted-lane` mode and lets
  `CLAY_LIVE_KEEP_ROOT=1` keep the store across launches for that mode (so a
  config-written grant can be checked across a restart).
- `test-plan/artifacts/136-capability-grants/init-config-granted-lane.js` is the
  fixture config that calls `authorize` and then `loadPackage`.
