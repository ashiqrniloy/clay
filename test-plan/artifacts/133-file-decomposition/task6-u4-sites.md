plan 133 task 6 (U4) — match_same_arms site ledger

BEFORE (baseline task-1 artifact; paths re-mapped after tasks 2-5):
  src/behavior/manifest.rs:98
  src/client/mod.rs:1642
  src/client/tests/client_events.rs:404, 446
  src/client/tests/real_server_tabs.rs:129, 344
  src/packages/record/documentation.rs:155
  src/packages/service.rs:198
  src/packages/verbs.rs:123
  src/protocol/agent.rs:407
  src/protocol/caret.rs:71
  src/server/agent.rs:1215, 1457
  src/server/connection/delivery.rs:146
  src/server/connection/runtime.rs:306
  src/server/document.rs:799
  src/server/language_server.rs:735
  src/server/menu_sessions.rs:509
  src/server/ops/modes.rs:300, 321
  src/server/ops/packages.rs:793
  src/server/runtime_generation_tests.rs:2971 (was src/server/mod.rs:5890)
  src/shell/components.rs:240
  src/shell/package_ui.rs:604, 806
  src/shell/theme.rs:389, 397, 483, 487, 571, 583, 587, 694, 698, 710, 714, 1297
  tests/editor_performance.rs:576
  TOTAL 38 = 32 production + 6 test-side

AFTER FINAL PEDANTIC RUN (cargo clippy --all-targets -- -W clippy::pedantic):
  match_same_arms warnings: 0

Sites resolved by clippy-suggested merges (restored files, applied byte-safely — 30):
  src/behavior/manifest.rs: [98]
  src/packages/record/documentation.rs: [155]
  src/packages/service.rs: [198]
  src/packages/verbs.rs: [123]
  src/server/agent.rs: [1215, 1457]
  src/server/connection/delivery.rs: [146]
  src/server/connection/runtime.rs: [306]
  src/server/document.rs: [799]
  src/server/language_server.rs: [735]
  src/server/menu_sessions.rs: [509]
  src/server/ops/modes.rs: [300, 321]
  src/server/ops/packages.rs: [793]
  src/shell/components.rs: [240]
  src/shell/package_ui.rs: [604, 806]
  src/shell/theme.rs: [389, 397, 483, 487, 571, 583, 587, 694, 698, 710, 714, 1297]
  tests/editor_performance.rs: [576]

Sites handled by hand (task-touched or new files — 8):
  src/client/mod.rs:1642 (merged arm clean)
  src/client/tests/client_events.rs:404, 446 (clean)
  src/client/tests/real_server_tabs.rs:129 (clean), 344 (repaired)
  src/protocol/agent.rs:407 (repaired; the corrupted splice had eaten the match tail)
  src/protocol/caret.rs:71 (clean)
  src/server/runtime_generation_tests.rs:2971 (repaired)

Follow-up gate findings after the merges (default clippy, fixed by hand):
  clippy::single_match at tests/editor_performance.rs:562,
  src/client/tests/client_events.rs:398, :439, src/server/runtime_generation_tests.rs:2968
  -> rewritten as `if let` with the same body.
