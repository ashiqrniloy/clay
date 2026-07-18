---
date: 2026-07-18 03:52
status: approved
decision_about: "Phase 20 revisit of Masonry pixel-buffer/GPU snapshot coverage"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Re-defer pixel-buffer / GPU snapshots after Masonry 0.4 TestHarness revisit

## Decision

Phase 20 **re-defers** pixel-buffer / GPU snapshot testing. Structural headless coverage via `SduiObservableSnapshot` / `SduiStatusObservation` remains the hard automated regression layer.

Masonry 0.4's `masonry_testing::TestHarness` / `assert_render_snapshot` is available and was investigated, but it is **not** adopted in Clay for Phase 20 because it does not exercise Clay's production GPU render path and still fails the CI-determinism prerequisites recorded in `docs/development/ui-observability.md`.

Clay does **not** add a `masonry_testing` dependency, `tests/ui_snapshots.rs`, or golden PNG screenshots in this phase.

## Evidence from the revisit

1. **Headless capture exists, but is CPU-forced.** `masonry_testing` 0.4.0 `TestHarness` constructs Vello with `use_cpu: true` hardcoded. Snapshot goldens therefore exercise a CPU rasterizer, not the live GPU path Clay ships.
2. **Production path is already GPU.** `masonry_winit` 0.4.0 creates Vello with default `use_cpu: false`, `AaSupport::area_only()`, renders to a wgpu texture, and blits to the window surface. Clay paints into that `Scene` (fills, clips, `render_text`).
3. **Parley remains CPU-side shaping/layout.** Font shaping and layout stay on CPU; only scene rasterization is GPU-accelerated. That split is normal for this stack and is not a Phase 20 defect.
4. **Renderer upgrades are blocked on Masonry.** Clay/Masonry pin Vello 0.6.0, Parley 0.6.0, and wgpu 26.0.1. Newer crates.io releases (Vello 0.9 / Parley 0.11 / wgpu 30) cannot be adopted until a newer Masonry release bumps transitive deps. Masonry / `masonry_winit` 0.4.0 remain the latest published versions.
5. **CI brittleness remains.** Even CPU harness goldens depend on pinned fonts, DPI/scale factor, antialiasing, and theme inputs. Clay's custom `EditorWidget` / SDUI / shell compositions still lack a production-GPU-faithful, CI-safe offscreen target. Structural observability already covers tree shape, status, accessibility roles/labels, and payload budgets without those failure modes.
6. **Custom GPU harness rejected for Phase 20.** Building a Clay-owned GPU offscreen capture path would be high effort, CI-fragile (requires GPU in CI), and unsupported by Masonry; it is out of scope for daily-editing product hardening.

## Exact Semantics Going Forward

1. Keep `SduiObservableSnapshot`, `SduiStatusObservation`, accessibility-tree assertions, and documented GUI smoke as the regression strategy.
2. Do not land `assert_render_snapshot!` goldens, screenshot directories, or `masonry_testing` as a Clay dependency in Phase 20.
3. Update docs so Phase 15/20 language no longer claims Masonry 0.4 has "no headless render surface." The accurate statement is: headless CPU capture exists via `TestHarness`, but GPU-faithful deterministic CI snapshots are still unavailable.
4. Promote pixel / GPU snapshots only when **all** of the following exist:
   - a CI-friendly offscreen target that can run the **production** GPU path (`use_cpu: false`) or an explicitly accepted GPU-faithful alternative;
   - fixed font, DPI, scale-factor, theme, and window-size inputs;
   - golden review/update workflow with tolerance rules;
   - security review confirming the harness does not open remote listeners, read user documents, expose secrets, or grant filesystem/shell authority.
5. Renderer/GPU performance gains that require Vello/Parley/wgpu upgrades wait on a newer Masonry release; Phase 20 does not vendor a forked Masonry stack solely for snapshots.

## Context

Plan 055 Task 9 required an explicit revisit of Phase 15's deferred pixel-buffer/GPU snapshot path now that Masonry 0.4 exposes `TestHarness` / `assert_render_snapshot`. The roadmap Phase 20 focus area asked to add pixel-accurate snapshots **if** Masonry/winit now supports deterministic offscreen rendering, otherwise keep structural observability.

After the GPU/stack survey, the agent recommended option 2 (re-defer with evidence). The user approved that option.

## Approval

- Proposed by: agent
- Approved by user: Yes
- Approval evidence: After the Task 9 GPU/stack analysis and three options (adopt CPU TestHarness; re-defer with evidence; custom GPU harness), the user replied `2`.

## Alternatives Considered

1. **Adopt CPU `TestHarness` for shell/SDUI chrome goldens.** — Rejected for Phase 20. Would add pixel coverage, but goldens would not match production GPU rasterization and would still risk font/AA churn without improving GPU fidelity.
2. **Re-defer with written evidence; keep structural observability as the hard gate.** — **Chosen.** Satisfies the required revisit, updates outdated "no headless surface" language, and avoids non-GPU-faithful golden debt.
3. **Custom Clay GPU offscreen snapshot harness.** — Rejected for Phase 20. High effort, CI-fragile, unsupported by Masonry; not justified for daily-editing hardening.

## Consequences

- Phase 20 Task 9 completes as an evidence-backed re-deferral, not as a pixel-suite landing.
- `docs/development/ui-observability.md`, roadmap Phase 15/20 notes, and the Phase 20 primitive review must cite this decision and the `use_cpu: true` finding.
- Future phases may adopt `masonry_testing` only if prerequisites above are met, or after Masonry provides a GPU-capable deterministic harness option.
- GPU maximization work remains: keep production on GPU (already true), profile paint/layout with existing perf hooks, and wait for Masonry to unlock newer Vello/wgpu — not invent snapshot goldens that skip the GPU path.
