Plan 136 task 6 live negative check (2026-09-23)

Harness: CLAY_LIVE_ROOT=/tmp/clay-plan136-ungranted \
  test-plan/artifacts/127-lane-scheduling/run-live.sh start ungranted-lane
Fixture: test-plan/artifacts/127-lane-scheduling/fixture-package (@fixture/lane 0.1.0),
installed with pnpm and adopted through `clay package adopt`, never granted.

enable.log        `clay package enable @fixture/lane` -> MissingCapabilityGrant { capability: CompletionProvider }
inspect.log       `clay package inspect` -> Adoption: approved, no Grants line,
                  Ungranted: completion-provider, mode-registration, parse-document
server-load-failed.txt  server-side `packages.load_failed` diagnostic for the init.js load
                  (the plan 127 grant-gate check, preserved)
