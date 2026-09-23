# Plan 136 task 14 — code wiki update

The wiki was updated once, after the implementation and manual-plan tasks
passed, per the plan's chosen approach (`docs-as-code.md`: update the wiki once
after the change passes tests).

## Pages

| Page | What it gained |
| --- | --- |
| `docs/wiki/modules/third-party-runtime-authority.md` | A `## Capability Grants` section: the four read surfaces behind the single `PackageService::capability_granted` path, provenance matching, the three grant sources (Clay JS `authorize`, `clay package authorize`, runtime profile), what a grant can never do (no self-grant — `Facade::trusted` plus `packages.grant_during_activation`; no undeclared capability; no widening by repetition; no approval), revoke clearing the grant (`grant: null`) and returning to `AdoptionRequired { code: "package_approval.revoked" }`, and a pointer to the lane counters. Source list gained `authorization.rs`, the authorize op, `facades.rs`, `packages.js`, and the CLI verb; invariants gained the grant rules; tests gained the durable-grant, CLI-verb and self-grant commands; Related gained `authorize.md`, the configuration guide and `performance.md`. |
| `docs/wiki/modules/persistent-runtime-hardening.md` | A `### Lane occupancy counters` subsection: the five counters and where each is bumped, the 20 static names from `JS_RUNTIME_LANE_METRICS`, the atomic/lock-free dispatch path with report-time recording (why: `&'static str` names, capped snapshot buffer, `MetricSummary.total`), a runnable `CLAY_PERF_PROFILE`/`CLAY_PERF_REPORT_DIR` recipe to read them, the measured granted-fixture numbers, the client-free comparison finding, the burst-test peak/supersede/evict numbers, and the keep-2-lanes tuning decision with its revisit triggers. Tests gained the two lane-counter tests and the plan-136 evidence pointer. |
| `docs/wiki/index.md` | Both module-map descriptions now name the plan-136 work (capability-grant surface; occupancy counters + tuning decision). |

## Contract test

`tests/documentation_coverage.rs::plan136_wiki_pages_describe_capability_grants_and_lane_counters`
(17 markers across the two pages and the index) plus a stale-claim scan over
`docs/wiki/modules` and `docs/wiki/flows` for the pre-plan-136 claims ("no
user-facing entry point for grants", "no capability grant surface", …), so the
pages cannot silently lose the surface or regress to the old state.

Run:

```bash
cargo test --test protocol documentation_coverage::plan136
```

Mutation checks in `mutation-checks.txt` (renaming the grant heading and
replacing the counter identifiers each fail the test with the marker named).
