use super::*;

#[tokio::test]
async fn agent_facade_fails_closed_without_an_attached_host() {
    // A default service wires no agent host into its lanes (Plan 130 A1): the
    // facade must fail closed with a typed error, not hang or leak authority —
    // and no other test in the process can hand it one.
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            try {
              const { compact } = await import("clay:agent");
              await compact({ sessionId: "s1" });
              Deno.core.ops.op_clay_runtime_record("unexpected-success");
            } catch (error) {
              Deno.core.ops.op_clay_runtime_record(`denied:${String(error).slice(0, 80)}`);
            }
            "#,
        )
        .await
        .expect("agent facade denial remains a handled JS error");

    assert!(
        result
            .op_records
            .iter()
            .any(|record| record.contains("denied:"))
    );
}

#[tokio::test]
async fn lane_registration_queue_fails_closed_at_capacity() {
    // Plan 130 A1: the lane queue keeps the host queue's fail-closed ceiling,
    // so a runaway package load entry cannot queue unbounded while a runtime is
    // still hostless (the declaration is refused, never silently dropped).
    let cap = crate::server::agent::PENDING_REGISTRATION_CAP;
    let service = ClayJsRuntimeService::default();
    let state = service.test_op_state();
    for index in 0..cap {
        state
            .queue_agent_registration("agentProfile.register", serde_json::json!({ "i": index }))
            .expect("declarations below the ceiling queue");
    }
    assert!(
        state
            .queue_agent_registration("agentProfile.register", serde_json::json!({ "i": cap }))
            .is_err(),
        "the declaration over the ceiling fails closed"
    );
    assert_eq!(state.take_pending_agent_registrations().len(), cap);
}

#[tokio::test]
async fn registrations_queued_hostless_hand_over_to_a_late_attached_host() {
    use crate::server::agent::AgentHostHandle;

    // Plan 130 A1: a hostless runtime (embedded worker / unit harness) keeps
    // declarations on its own lane queue and hands them to the host's pending
    // queue when the server attaches one — the old process-global handoff, now
    // per-lane state. Applying them stays the host's job after initialize.
    let service = ClayJsRuntimeService::default();
    let state = service.test_op_state();
    state
        .queue_agent_registration("agentProfile.register", serde_json::json!({ "name": "p" }))
        .expect("hostless lane queues the declaration");
    assert_eq!(
        state.take_pending_agent_registrations().len(),
        1,
        "the declaration waits on the lane queue"
    );
    state
        .queue_agent_registration("skill.register", serde_json::json!({ "name": "s" }))
        .expect("hostless lane queues the declaration");

    let handle = AgentHostHandle::inert();
    let host = handle.host();
    state.set_agent_host(handle.clone());

    assert!(
        state.take_pending_agent_registrations().is_empty(),
        "handoff drains the lane queue"
    );
    assert_eq!(
        host.pending_registration_len().await,
        1,
        "the remaining declaration moved to the host's own queue, in order"
    );
    state.set_agent_host(handle);
    assert_eq!(
        host.pending_registration_len().await,
        1,
        "re-wiring the same lane does not duplicate the handoff"
    );
}

#[tokio::test]
async fn agent_host_wiring_reaches_every_lane_and_stays_per_service() {
    use crate::packages::bundled::RuntimeDomain;
    use crate::server::js_runtime::RuntimeLane;

    // Plan 130 A1: the server-owned handle reaches every lane of the service it
    // was installed on, and no other service in the process can see it.
    let wired = ClayJsRuntimeService::default();
    let host = crate::server::agent::AgentHostHandle::inert();
    wired.set_agent_host(host.clone());
    for domain in [RuntimeDomain::Trusted, RuntimeDomain::ThirdParty] {
        for lane in RuntimeLane::ALL {
            let lane_host = wired
                .test_lane_op_state(domain, lane)
                .agent_host()
                .expect("every lane carries the server's host");
            assert!(
                lane_host.same_host(&host),
                "lane {domain:?}/{lane:?} must carry the installed host"
            );
        }
    }

    let hostless = ClayJsRuntimeService::default();
    assert!(
        hostless.test_op_state().agent_host().is_err(),
        "a hostless service fails closed instead of reaching another service's host"
    );
}

// ---- Phase 2 @clay/coding-agent: default init.js loading experience ----

/// Clean-init drill (plan 108 task 6): a fresh `init.js` whose only statement
/// is the one-line package load activates the Coding Agent's working defaults
/// — manifest command + chrome extension point via the real loadPackage path,
/// profile registration declaration (queued hostless; applied when a host
/// installs). Declarations are content-asserted: concurrent document-flow
/// tests may queue identical entries at any time — cross-entry ordering is
/// not observable, the load-entry source test owns per-load order.
#[tokio::test]
async fn coding_agent_clean_init_one_line_activates_working_defaults() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    // Baseline: this lane starts with an empty queue (hostless service state).
    let _ = drain_all_pending_registrations(&service);

    let root = config_fixture("coding-agent-clean-init");
    fs::write(
        root.join("init.js"),
        r#"
import { loadPackage } from "clay:packages";
await loadPackage("@clay/coding-agent");
"#,
    )
    .unwrap();

    let result = service
        .load_configuration_from_root(root)
        .await
        .expect("one-line init.js must load the Coding Agent");
    assert!(
        result.ui_contributions.validate().is_ok(),
        "contributions from the one-line load must form a valid UI registry"
    );

    // Manifest command registers with server-first routing (no user plumbing).
    let commands = service.test_op_state().command_registry_snapshot();
    let command = commands
        .iter()
        .find(|command| command.command_id == "coding-agent.profile")
        .expect("coding-agent.profile command registers from the manifest");
    assert_eq!(command.package_name, "@clay/coding-agent");
    assert_eq!(
        command.routing_policy,
        crate::protocol::RoutingPolicy::ServerFirst
    );

    // Registration declarations queued in contract order: no skill
    // declarations (skills come from disk discovery, not the package), the
    // coding profile without a skills field, ten profile tools.
    let drained = drain_all_pending_registrations(&service);
    assert!(
        !drained.iter().any(|entry| entry.method == "skill.register"),
        "no hardcoded skill declaration queues; disk discovery owns skills"
    );
    assert!(
        drained.iter().any(|entry| {
            entry.method == "agentProfile.register"
                && entry.params["name"] == "coding"
                && entry.params.get("skills").is_none()
                && entry.params["tools"].as_array().map(Vec::len) == Some(10)
        }),
        "coding profile declaration queues without a skills field"
    );
    // The slash surface queues as inert command declarations after the
    // profile: nine distinct commands (concurrent document-flow tests may
    // queue identical duplicates, so count distinct names, not entries).
    let command_names: std::collections::BTreeSet<String> = drained
        .iter()
        .filter(|entry| entry.method == "command.register")
        .filter_map(|entry| entry.params["name"].as_str().map(str::to_string))
        .collect();
    let expected: std::collections::BTreeSet<String> = [
        "/compact",
        "/new",
        "/n",
        "/branch",
        "/tree",
        "/fork",
        "/clone",
        "/open-session",
        "/open-session-as-fork",
        "/discard",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(command_names, expected, "ten slash commands queue");
    let first_command = drained
        .iter()
        .find(|entry| entry.method == "command.register")
        .expect("commands queue");
    assert_eq!(first_command.params["handler"], "compact");
    // The named pane surface (activation "pane") registers in the UI
    // registry immediately (a process-local op, no daemon): the wire snapshot
    // carries it as a named pane surface, never as the empty-tab landing
    // (the launcher package owns that in a later plan 118 task).
    let wire = service
        .test_op_state()
        .ui_contributions()
        .wire_snapshot(1, |_| crate::protocol::PackageUiTrustDomain::Trusted)
        .unwrap();
    assert_eq!(
        wire.surfaces.len(),
        1,
        "surface wires as a named pane surface"
    );
    assert_eq!(wire.surfaces[0].id, "coding-agent.surface");
    assert_eq!(wire.surfaces[0].package_name, "@clay/coding-agent");
    assert!(
        wire.allows_action(1, "coding-agent.profile"),
        "surface action targets validate"
    );
    assert!(
        wire.empty_tab.is_none(),
        "the agent package never claims the empty-tab landing"
    );
}

/// Double load in one init.js is idempotent: the generation cache answers the
/// second call, so the package enables once and the command registers once.
#[tokio::test]
async fn coding_agent_double_load_is_idempotent_within_one_generation() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let _ = drain_all_pending_registrations(&service);

    let root = config_fixture("coding-agent-double-load");
    fs::write(
        root.join("init.js"),
        r#"
import { loadPackage } from "clay:packages";
await loadPackage("@clay/coding-agent");
await loadPackage("@clay/coding-agent");
"#,
    )
    .unwrap();

    service
        .load_configuration_from_root(root)
        .await
        .expect("double load stays idempotent");

    let commands = service
        .test_op_state()
        .command_registry_snapshot()
        .into_iter()
        .filter(|command| command.package_name == "@clay/coding-agent")
        .count();
    // Fourteen manifest commands (profile + close + cycle-effort client
    // command + resume + ten slash surface) register once. The two
    // agent-settings client commands were retired when the page moved into
    // the coding agent's own Settings tab (plan 117 follow-up).
    assert_eq!(
        commands, 14,
        "double load must not duplicate the package commands"
    );
}

/// Without the load line the package contributes nothing: no command, no
/// residue (another package's contributions stay untouched).
#[tokio::test]
async fn coding_agent_absent_load_line_leaves_no_residue() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;

    let root = config_fixture("coding-agent-absent");
    fs::write(
        root.join("init.js"),
        r#"
import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
"#,
    )
    .unwrap();

    let service = ClayJsRuntimeService::default();
    service
        .load_configuration_from_root(root)
        .await
        .expect("markdown-only init.js must load");

    let commands = service.test_op_state().command_registry_snapshot();
    assert!(
        !commands
            .iter()
            .any(|command| command.package_name == "@clay/coding-agent"),
        "no coding-agent command without the load line"
    );
}

/// Malformed declarations fail closed with a typed error before entering any
/// queue, and a valid declaration through the same facade still resolves.
#[tokio::test]
async fn coding_agent_malformed_registration_fails_closed_without_queue_corruption() {
    let service = ClayJsRuntimeService::default();
    let result = service
        .evaluate_controlled_module(
            r#"
            const { profileRegister, skillRegister } = await import("clay:agent");
            try {
              await profileRegister({ description: "no name" });
              Deno.core.ops.op_clay_runtime_record("unexpected-success");
            } catch (error) {
              Deno.core.ops.op_clay_runtime_record(`denied:${String(error).slice(0, 120)}`);
            }
            try {
              await skillRegister({ name: "x", toolNames: [42] });
              Deno.core.ops.op_clay_runtime_record("unexpected-success");
            } catch (error) {
              Deno.core.ops.op_clay_runtime_record(`denied:${String(error).slice(0, 120)}`);
            }
            await profileRegister({ name: "coding", tools: ["read"] });
            Deno.core.ops.op_clay_runtime_record("valid-ok");
            "#,
        )
        .await
        .expect("malformed registrations stay handled JS errors");

    let denied = result
        .op_records
        .iter()
        .filter(|record| record.starts_with("denied:") && record.contains("agent.invalid_params"))
        .count();
    assert_eq!(denied, 2, "both malformed declarations fail closed");
    assert!(
        !result
            .op_records
            .iter()
            .any(|record| record.contains("unexpected-success")),
        "no malformed declaration may report success"
    );
    assert!(
        result.op_records.iter().any(|record| record == "valid-ok"),
        "a well-shaped declaration still resolves after rejections"
    );
}

// ---------------------------------------------------------------------------
// Plan 112 task 13: icon-pack configuration contract (load+select path,
// order-independent selection, modular imports, restart reproduction,
// default/deny behavior). Fixtures: tests/fixtures/configuration/plan112-icons/.
// ---------------------------------------------------------------------------
