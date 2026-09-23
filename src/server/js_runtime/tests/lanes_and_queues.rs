use super::*;

/// Plan 127 task 6: a registration left behind by a revoked/disabled package is
/// refused identically at every lane. The host gate authenticates the command's
/// package identity before the command reaches a lane, so the refusal is typed
/// (`Revoked`) and costs no isolate work. Baseline with the gate disabled: both
/// lanes already failed closed, but only inside the isolate, through whichever
/// op the generated provider module happened to call
/// (`Runtime("Error: packages.package_not_enabled ...")`) — neither typed nor
/// lane-independent, and only reachable while that generated module keeps
/// checking provenance.
#[tokio::test]
async fn revoked_package_commands_refused_per_lane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let providers = lane_provider_pair(&service, "lanerevoke").await;
    let [general_provider, latency_provider] = providers.clone();
    let (request, window) = lane_completion_input("lanerevoke", 1, "lane");

    // Both lanes serve while their package is enabled.
    for (provider, expected) in [
        (general_provider.clone(), "inline"),
        (latency_provider.clone(), "module"),
    ] {
        let result = service
            .invoke_completion_provider(provider, request.clone(), window.clone())
            .await
            .expect("enabled packages serve both lanes");
        assert_eq!(result.items[0].label, expected);
    }
    let workers_before = service.workers_started();

    // Production revocation sequence (`clay package revoke <name>`): the
    // approval is revoked and the package disabled. The runtime keeps the
    // coordinator's registration, which must now refuse in every lane.
    {
        let op_state = service.test_op_state();
        let mut package_service = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        for package in ["@vendor/lanerevoke", "@vendor/lanerevokelat"] {
            package_service
                .revoke_package_approval(package)
                .expect("approval revoke");
            package_service.disable(package).expect("package disable");
        }
    }
    for (provider, expected_package, lane) in [
        (general_provider, "@vendor/lanerevoke", "general"),
        (latency_provider, "@vendor/lanerevokelat", "latency"),
    ] {
        let error = service
            .invoke_completion_provider(provider, request.clone(), window.clone())
            .await
            .expect_err("revoked package must not serve completions");
        assert!(
            matches!(
                &error,
                ClayRuntimeError::Revoked { package, version }
                    if package == expected_package && version == "0.1.0"
            ),
            "{lane} lane must refuse with the typed revocation error, got {error:?}"
        );
    }
    assert_eq!(
        service.workers_started(),
        workers_before,
        "a refusal is host-side: no lane may be replaced"
    );
}

/// Plan 127 task 6: a trusted reload rebuilds the trusted domain's lanes and
/// shares the third-party domain's lanes untouched — both the general-lane
/// (registry) and latency-lane (module) registrations keep serving from their
/// original isolates, without starting or replacing a worker.
#[tokio::test]
async fn reload_shares_third_party_lanes_untouched() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let providers = lane_provider_pair(&service, "lanereload").await;
    let third_party_generation_before =
        service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty);

    let reloaded = ClayJsRuntimeService::production_reload(&service);
    assert_eq!(
        reloaded.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty),
        third_party_generation_before,
        "trusted reload must not replace the third-party domain"
    );
    assert_eq!(
        reloaded.workers_started(),
        crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN as u64,
        "reload rebuilds exactly the trusted domain's lanes through one construction path"
    );

    // Reload commit re-registers the survived third-party payload; both lanes
    // must still answer from their untouched isolates.
    let snapshot = reloaded.third_party_registrations_snapshot();
    assert_eq!(snapshot.js_completion_providers.len(), providers.len());
    let oracle = crate::server::completion::CompletionCoordinator::new();
    reloaded
        .register_completion_providers(&oracle, 5, &snapshot)
        .expect("surviving third-party registrations re-register after reload");
    let (request, window) = lane_completion_input("lanereload", 2, "lane");
    for (provider, expected) in providers.into_iter().zip(["inline", "module"]) {
        let result = reloaded
            .invoke_completion_provider(provider, request.clone(), window.clone())
            .await
            .expect("surviving third-party lanes serve after a trusted reload");
        assert_eq!(result.items[0].label, expected);
    }
    assert_eq!(
        reloaded.workers_started(),
        crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN as u64,
        "serving both surviving lanes must not start a worker"
    );
}

/// Plan 127 task 6: the installed op inventory of a running lane is the
/// domain's op set — never a lane-specific one. Enumerated from inside each of
/// the four live isolates through the real dispatch path, so it is ground
/// truth for the running lanes rather than a claim about the wiring code.
#[tokio::test]
async fn lanes_share_their_domain_op_set() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    use crate::packages::bundled::RuntimeDomain;

    async fn lane_op_names(
        service: &ClayJsRuntimeService,
        domain: RuntimeDomain,
        lane: RuntimeLane,
    ) -> Vec<String> {
        let (response, receiver) = tokio::sync::oneshot::channel();
        let evaluation = service
            .dispatch_to_domain(
                domain,
                lane,
                super::super::worker::RuntimeCommand::Evaluate {
                    entry: RuntimeEntry::ControlledSource(
                        "Deno.core.ops.op_clay_runtime_record(Object.keys(Deno.core.ops).sort().join(','));"
                            .to_string(),
                    ),
                    workspace: None,
                    runtime_document_id: 1,
                    package_context: None,
                    metric: "runtime.lane_op_names",
                    response,
                },
                receiver,
            )
            .await
            .expect("every lane accepts a controlled op-inventory probe");
        evaluation
            .op_records
            .first()
            .expect("op inventory probe records its result")
            .split(',')
            .map(str::to_string)
            .collect()
    }

    let service = ClayJsRuntimeService::default();
    let trusted_general =
        lane_op_names(&service, RuntimeDomain::Trusted, RuntimeLane::General).await;
    let trusted_latency =
        lane_op_names(&service, RuntimeDomain::Trusted, RuntimeLane::Latency).await;
    let third_party_general =
        lane_op_names(&service, RuntimeDomain::ThirdParty, RuntimeLane::General).await;
    let third_party_latency =
        lane_op_names(&service, RuntimeDomain::ThirdParty, RuntimeLane::Latency).await;

    let trusted_only: Vec<&String> = trusted_general
        .iter()
        .filter(|op| !third_party_general.contains(op))
        .collect();
    println!(
        "PLAN127_LANES trusted_ops={} third_party_ops={} trusted_only={}",
        trusted_general.len(),
        third_party_general.len(),
        trusted_only.len()
    );
    assert!(!trusted_general.is_empty() && !third_party_general.is_empty());
    assert_eq!(
        trusted_general, trusted_latency,
        "both trusted lanes must install the trusted domain's op set"
    );
    assert_eq!(
        third_party_general, third_party_latency,
        "both third-party lanes must install the third-party domain's op set"
    );
    for op in [
        "op_clay_runtime_ping",
        "op_clay_configuration_get_state",
        "op_clay_documents_open_document",
        "op_clay_packages_load_package",
        "op_clay_packages_load_in_package_domain",
        "op_clay_language_server_authorize",
        "op_clay_modes_classify_document",
        "op_clay_theme_set_theme",
    ] {
        assert!(
            trusted_general.iter().any(|name| name == op),
            "trusted lane must install {op}"
        );
        assert!(
            !third_party_latency.iter().any(|name| name == op),
            "third-party latency lane must not install {op}"
        );
    }
    assert_eq!(
        service.workers_started(),
        2 * crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN as u64,
        "one construction path starts every lane of both domains"
    );
}

/// Plan 127 P1 acceptance: a slow general-lane command must not head-of-line
/// block latency-lane completions. The parse handler busy-loop holds the
/// domain's general lane; the module-backed completion provider is served by
/// the latency lane and answers while that handler is still running.
#[tokio::test]
async fn latency_lane_unblocked_by_busy_general_lane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "lanehol";
    let busy_ms = 500u64;
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(2_000));
    let (_, completion_registration) = module_backed_completion_provider(
        &service,
        "@vendor/lanehol",
        prefix,
        r#"
        export function provideCompletion(_request, window) {
          return {
            status: "ok",
            items: [{ label: "lane", insertText: "lane", detail: window.text }]
          };
        }
        "#,
    )
    .await;
    // Inline (token-backed) parse handler: it keeps the general lane busy and
    // stays on that lane by design.
    let parse_evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/lanehol-parser",
            "laneholparse",
            &["parse-document"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::ParseDocument],
        &format!(
            r#"
            import {{ serverRegisterParseHandler }} from "clay:parse";
            serverRegisterParseHandler({{
              mode: "laneholparse",
              runtimeBridge: true,
              timeoutMs: 2_000,
              module: {{ default: async (notification) => {{
                const start = Date.now();
                while (Date.now() - start < {busy_ms}) {{}}
                return {{ viewport: notification.viewport }};
              }} }}
            }});
            "#
        ),
    )
    .await
    .expect("inline busy parse handler registers");
    let parse_registration = parse_evaluation
        .js_parse_handlers
        .first()
        .cloned()
        .expect("busy parse handler registered");
    assert_eq!(
        service.registration_domain(&parse_registration.package),
        service.registration_domain(&completion_registration.package),
        "the baseline scenario keeps both commands in one trust domain"
    );
    let parse_notification = ParseEditNotification {
        document_id: 7,
        document_version: 3,
        behavior_version: 1,
        package_prefix: "laneholparse".to_string(),
        mode_id: "laneholparse".to_string(),
        viewport: ParseByteRange::new(0, 2),
        invalidated_ranges: vec![ParseByteRange::new(0, 2)],
        accepted_edit: None,
        parse_windows: Vec::new(),
        memory_budget: None,
        trace_id: None,
        request_id: None,
    };

    // Warm the latency lane (module import) and take an idle baseline.
    let mut idle_us = Vec::new();
    for request_id in 0..10u64 {
        let (request, window) = lane_completion_input(prefix, request_id, "fn");
        let start = Instant::now();
        let result = service
            .invoke_completion_provider(completion_registration.clone(), request, window)
            .await
            .expect("idle completion");
        idle_us.push(start.elapsed().as_micros());
        assert_eq!(result.items[0].label, "lane");
    }
    idle_us.sort_unstable();
    let idle_median_us = idle_us[idle_us.len() / 2];

    // Hold the general lane for `busy_ms`, then issue one completion 50 ms in:
    // at least 450 ms of the busy loop remain if the lanes shared a worker.
    let parse_task = tokio::spawn({
        let service = service.clone();
        let registration = parse_registration.clone();
        let notification = parse_notification.clone();
        async move {
            service
                .invoke_parse_handler(registration, notification)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let workers_before_completion = service.workers_started();
    let (request, window) = lane_completion_input(prefix, 50, "fn");
    let start = Instant::now();
    let completion = service
        .invoke_completion_provider(completion_registration.clone(), request, window)
        .await
        .expect("latency-lane completion");
    let busy_completion_us = start.elapsed().as_micros();
    assert_eq!(completion.items[0].label, "lane");
    assert_eq!(
        service.workers_started(),
        workers_before_completion,
        "a slow general-lane parse must not replace either lane"
    );
    assert!(
        busy_completion_us < 250_000,
        "completion must not queue behind the busy general lane: {busy_completion_us} us \
         (busy {busy_ms} ms, idle median {idle_median_us} us)"
    );
    let parse_result = parse_task.await.expect("parse task join");
    assert!(
        parse_result.is_ok(),
        "busy parse must complete under its timeout: {parse_result:?}"
    );
    eprintln!(
        "PLAN127_LANE busy_ms={busy_ms} idle_median_us={idle_median_us} \
         busy_completion_us={busy_completion_us} workers_started={}",
        service.workers_started()
    );
}

/// Plan 127 P1: a timeout poison on the latency lane replaces ONLY that
/// lane's worker. The general lane keeps serving, the domain generation
/// (registration ownership metadata) is untouched, and the replaced lane
/// rematerializes the module-backed handler by import — no evaluation replay.
#[tokio::test]
async fn lane_poison_replaces_only_that_lane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "lanepoison";
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/lanepoison",
        prefix,
        r#"
        export function provideCompletion(_request, window) {
          if (window.text === "loop") {
            while (true) {}
          }
          return { status: "ok", items: [{ label: "alive", insertText: "alive" }] };
        }
        "#,
    )
    .await;
    let domain = service.registration_domain(&registration.package);
    let baseline_workers = service.workers_started();
    let baseline_generation = service.domain_generation(domain);

    // Latency-lane timeout: replacement is lazy, so nothing starts yet.
    let (request, window) = lane_completion_input(prefix, 1, "loop");
    let timed_out = service
        .invoke_completion_provider(registration.clone(), request, window)
        .await;
    assert!(
        matches!(timed_out, Err(ClayRuntimeError::Timeout)),
        "runaway provider must time out, got {timed_out:?}"
    );
    assert_eq!(service.workers_started(), baseline_workers);

    // The general lane is untouched: it still serves and starts no worker.
    service
        .evaluate_third_party_module("Deno.core.ops.op_clay_runtime_record('general');")
        .await
        .expect("general lane serves after a latency-lane timeout");
    assert_eq!(
        service.workers_started(),
        baseline_workers,
        "general lane must not be replaced"
    );
    assert_eq!(
        service.domain_generation(domain),
        baseline_generation,
        "latency poison must not bump the domain generation"
    );

    // Next latency dispatch replaces only the latency lane; the module-backed
    // handler re-imports from the shared allowlist.
    let (request, window) = lane_completion_input(prefix, 2, "alive");
    let result = service
        .invoke_completion_provider(registration, request, window)
        .await
        .expect("latency lane serves after replacing only itself");
    assert_eq!(result.items[0].label, "alive");
    assert_eq!(service.workers_started(), baseline_workers + 1);
    assert_eq!(service.domain_generation(domain), baseline_generation);

    // General lane still alive without replacement.
    service
        .evaluate_third_party_module("Deno.core.ops.op_clay_runtime_record('general');")
        .await
        .expect("general lane still serves after the latency replacement");
    assert_eq!(service.workers_started(), baseline_workers + 1);
}

/// Plan 127 P1 / Plan 061 parity: a THIRD-PARTY latency lane gets the same
/// third-party extension set as its general lane — trusted-only bridge ops are
/// absent — and it is a separate isolate (general-lane globals invisible).
#[tokio::test]
async fn third_party_lane_denies_trusted_ops() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "lanedenial";
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/lanedenial",
        prefix,
        r#"
        export function provideCompletion(_request, _window) {
          const ops = globalThis.Deno?.core?.ops ?? {};
          return {
            status: "ok",
            items: [{
              label: `trustedBridge=${typeof ops.op_clay_packages_load_in_package_domain !== "undefined"};generalGlobal=${globalThis.__laneProbe === "general"}`,
              insertText: "lane"
            }]
          };
        }
        "#,
    )
    .await;
    let domain = service.registration_domain(&registration.package);
    assert_eq!(domain, crate::packages::bundled::RuntimeDomain::ThirdParty);
    // Mark the general-lane isolate; the latency lane must not see this global.
    service
        .evaluate_third_party_module("globalThis.__laneProbe = 'general';")
        .await
        .expect("general lane accepts the probe global");
    let (request, window) = lane_completion_input(prefix, 1, "fn");
    let result = service
        .invoke_completion_provider(registration, request, window)
        .await
        .expect("third-party latency-lane completion");
    assert_eq!(
        result.items[0].label, "trustedBridge=false;generalGlobal=false",
        "third-party latency lane must keep the third-party op set and stay isolate-separate"
    );
}

/// Plan 127 P1: lane count and per-lane heap ceilings are server-owned
/// budgets, and the latency lane never exceeds the configured service ceiling
/// (synthetic small-heap services stay bounded on every lane).
#[test]
fn lane_heap_limits_stay_within_the_configured_budget() {
    use crate::perf::budgets::{
        JS_RUNTIME_HEAP_LIMIT_BYTES, JS_RUNTIME_LANES_PER_DOMAIN,
        JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES,
    };

    assert_eq!(
        super::super::RuntimeLane::ALL.len(),
        JS_RUNTIME_LANES_PER_DOMAIN
    );
    assert_eq!(
        super::super::lane_heap_limit_bytes(JS_RUNTIME_HEAP_LIMIT_BYTES, RuntimeLane::General),
        JS_RUNTIME_HEAP_LIMIT_BYTES
    );
    assert_eq!(
        super::super::lane_heap_limit_bytes(JS_RUNTIME_HEAP_LIMIT_BYTES, RuntimeLane::Latency),
        JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES
    );
    const { assert!(JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES < JS_RUNTIME_HEAP_LIMIT_BYTES) };
    assert_eq!(
        super::super::lane_heap_limit_bytes(4 * 1024, RuntimeLane::Latency),
        4 * 1024,
        "a configured ceiling below the latency budget must win"
    );
}

/// Plan 127 P1: a language-intelligence provider that declares its
/// `moduleSpecifier` runs on the domain's latency lane (module import), same
/// as completion. Covers the LI op's specifier validation and the LI
/// evaluation branch; inline `module: {...}` registrations keep the existing
/// general-lane path.
#[tokio::test]
async fn language_intelligence_module_specifier_serves_from_latency_lane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "laneli";
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("lane-li-provider").join(prefix);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
        import * as providerModule from "./intel.js";
        export default function load() {
          serverRegisterLanguageIntelligenceProvider({
            provider: {
              id: "laneli.intelligence",
              features: ["hover"],
              timeoutMs: 500
            },
            module: providerModule,
            moduleSpecifier: import.meta.resolve("./intel.js")
          });
        }
        "#,
    );
    fs::write(
        root.join("dist/intel.js"),
        r#"
        export function provideLanguageIntelligence(request, window) {
          return {
            status: "ok",
            markdown: `lane:${request.feature}:${window.text}`,
            range: { byteStart: 0, byteEnd: window.text.length }
          };
        }
        "#,
    )
    .expect("write module-backed language provider");
    let package_json = test_package_json(
        "@vendor/laneli",
        prefix,
        &["parse-document"],
        serde_json::json!({}),
    );
    let approved = vec![crate::packages::permissions::PackagePermission::ParseDocument];
    ensure_synthetic_package_enabled(&service, package_json.clone(), approved.clone(), None);
    let load_specifier = "clay://packages/@vendor/laneli/dist/load.js";
    service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            load_specifier,
            root.join("dist/load.js"),
            root.clone(),
            Some("@vendor/laneli"),
        );
    let evaluation = evaluate_as_package(
        &service,
        package_json,
        approved,
        &format!("const m = await import({load_specifier:?}); await m.default();"),
    )
    .await
    .expect("module-backed language provider fixture load");
    let registration = evaluation
        .js_language_intelligence_providers
        .first()
        .cloned()
        .expect("fixture must register a JS language-intelligence provider");
    assert!(
        registration.module_specifier.is_some(),
        "fixture registration must carry the resolved module specifier"
    );
    assert_eq!(
        RuntimeLane::for_provider(registration.module_specifier.as_deref()),
        RuntimeLane::Latency
    );

    let request = crate::protocol::LanguageIntelligenceRequest {
        request_id: 7,
        client_id: 1,
        document_id: 1,
        document_version: 1,
        behavior_version: 1,
        cursor_byte_offset: 0,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 1,
    };
    let window = crate::server::language_intelligence::LanguageIntelligenceDocumentWindow {
        document_id: 1,
        document_version: 1,
        behavior_version: 1,
        byte_start: 0,
        byte_end: 4,
        text: "fn()".to_string(),
        active_mode: prefix.to_string(),
    };
    let result = service
        .invoke_language_intelligence_provider(registration, request, window)
        .await
        .expect("latency-lane language-intelligence provider");
    crate::server::language_intelligence::validate_result(&result).unwrap();
    assert_eq!(
        result.status,
        crate::protocol::LanguageIntelligenceStatus::Ok
    );
    match result.payload {
        crate::protocol::LanguageIntelligencePayload::Hover(hover) => {
            assert_eq!(hover.markdown, "lane:hover:fn()");
            assert_eq!(hover.range, Some(crate::protocol::TextByteRange::new(0, 4)));
        }
        other => panic!("expected hover payload, got {other:?}"),
    }
}

/// Plan 127 P1 security gate: `moduleSpecifier` must resolve to a loaded
/// module owned by the registering package (same rule as document analyzers);
/// a foreign or unloaded specifier is rejected before any latency lane sees it.
#[tokio::test]
async fn completion_and_language_providers_reject_unowned_module_specifier() {
    let service = ClayJsRuntimeService::default();
    for (api_prefix, source) in [
        (
            "unownedc",
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: { provideCompletion: async () => ({ status: "ok", items: [] }) },
              moduleSpecifier: "clay://packages/other/worker.js"
            });
            "#,
        ),
        (
            "unownedl",
            r#"
            import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
            serverRegisterLanguageIntelligenceProvider({
              provider: { id: "unownedl.intelligence", features: ["hover"] },
              module: { provideLanguageIntelligence: async () => ({ status: "ok" }) },
              moduleSpecifier: "clay://packages/other/worker.js"
            });
            "#,
        ),
    ] {
        let package_json = test_package_json(
            &format!("@vendor/{api_prefix}"),
            api_prefix,
            &["completion-provider", "parse-document"],
            serde_json::json!({
                "completionProviders": [{
                    "id": format!("{api_prefix}.provider"),
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 50, "maxItems": 8 }
                }]
            }),
        );
        let approved = vec![
            crate::packages::permissions::PackagePermission::CompletionProvider,
            crate::packages::permissions::PackagePermission::ParseDocument,
        ];
        let error = evaluate_as_package(&service, package_json, approved, source)
            .await
            .expect_err("unowned moduleSpecifier must be rejected");
        assert!(
            error
                .to_string()
                .contains("moduleSpecifier must resolve to a loaded module owned by the package"),
            "unexpected moduleSpecifier rejection: {error}"
        );
    }
}

/// Plan 127 P2 acceptance: a typing burst on one document must not grow the
/// lane backlog. Every newer request replaces the older undelivered one for the
/// same work key, older callers learn `Superseded` (never a stale result), and
/// the newest request is served.
#[tokio::test]
async fn queue_bounded_under_flood() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "queueflood";
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/queueflood",
        prefix,
        PLAN127_HOLDING_PROVIDER,
    )
    .await;
    let flood = 40_u64;
    let documents = vec![7_u64; flood as usize];
    let outcomes = plan127_completion_burst(&service, &registration, prefix, &documents).await;
    // Synthetic `@vendor/...` packages are adopted third-party packages, so
    // their module-backed provider runs on the third-party latency lane.
    let stats = service.lane_queue_stats(
        crate::packages::bundled::RuntimeDomain::ThirdParty,
        RuntimeLane::Latency,
    );
    assert!(stats.capacity > 4, "budget too small to flood: {stats:?}");
    assert!(
        stats.peak_pending <= 2,
        "one-document flood must collapse onto a single queue slot: {stats:?}"
    );
    assert_eq!(
        stats.evicted, 0,
        "same-key supersede, not capacity eviction: {stats:?}"
    );
    assert!(
        stats.superseded >= flood - 2,
        "newest wins and older undelivered work is dropped: {stats:?}"
    );
    assert_eq!(
        plan127_superseded_count(&outcomes),
        stats.superseded,
        "every dropped command answers Superseded to its caller"
    );
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.is_ok() || matches!(outcome, Err(ClayRuntimeError::Superseded))),
        "dropped work must not surface as another error: {outcomes:?}"
    );
    let labels = plan127_ok_labels(&outcomes);
    assert!(
        labels.contains(&format!("q{flood}")),
        "newest request must be served: {labels:?}"
    );
    assert_eq!(
        labels.len() as u64,
        flood - stats.superseded,
        "served + superseded must account for every request"
    );
    println!(
        "PLAN127_QUEUE_FLOOD requests={flood} peak_pending={} capacity={} superseded={} evicted={} served={}",
        stats.peak_pending,
        stats.capacity,
        stats.superseded,
        stats.evicted,
        labels.len()
    );
    let _ = fs::remove_dir_all(config_fixture("lane-module-provider"));
}

/// Plan 127 P2 acceptance: queue space that exists is never taken from a
/// distinct document — only same-key work is superseded.
#[tokio::test]
async fn distinct_documents_never_superseded() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "queuedocs";
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/queuedocs",
        prefix,
        PLAN127_HOLDING_PROVIDER,
    )
    .await;
    let documents: Vec<u64> = (0..8).map(|index| 100 + index).collect();
    let outcomes = plan127_completion_burst(&service, &registration, prefix, &documents).await;
    // Synthetic `@vendor/...` packages are adopted third-party packages, so
    // their module-backed provider runs on the third-party latency lane.
    let stats = service.lane_queue_stats(
        crate::packages::bundled::RuntimeDomain::ThirdParty,
        RuntimeLane::Latency,
    );
    assert_eq!(
        stats.superseded, 0,
        "distinct documents must never supersede each other: {stats:?}"
    );
    assert_eq!(stats.evicted, 0, "queue space existed: {stats:?}");
    assert!(
        stats.peak_pending >= documents.len() - 2,
        "distinct documents must actually queue behind the busy lane: {stats:?}"
    );
    assert_eq!(
        plan127_ok_labels(&outcomes).len(),
        documents.len(),
        "every distinct document must be served: {outcomes:?}"
    );
    println!(
        "PLAN127_QUEUE_DISTINCT documents={} peak_pending={} capacity={} superseded={} evicted={}",
        documents.len(),
        stats.peak_pending,
        stats.capacity,
        stats.superseded,
        stats.evicted
    );
    let _ = fs::remove_dir_all(config_fixture("lane-module-provider"));
}

/// Plan 127 P2 acceptance: past the supersedable capacity the mailbox stays
/// bounded by evicting the oldest undelivered supersedable work (stale-first),
/// never by queueing without limit and never by rejecting the newest request.
#[tokio::test]
async fn queue_evicts_oldest_at_capacity() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "queuefull";
    let capacity = crate::perf::budgets::JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY;
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/queuefull",
        prefix,
        PLAN127_HOLDING_PROVIDER,
    )
    .await;
    let documents: Vec<u64> = (0..capacity as u64 + 8).map(|index| 200 + index).collect();
    let outcomes = plan127_completion_burst(&service, &registration, prefix, &documents).await;
    // Synthetic `@vendor/...` packages are adopted third-party packages, so
    // their module-backed provider runs on the third-party latency lane.
    let stats = service.lane_queue_stats(
        crate::packages::bundled::RuntimeDomain::ThirdParty,
        RuntimeLane::Latency,
    );
    assert!(
        stats.peak_pending <= capacity,
        "mailbox must stay bounded by its budget: {stats:?}"
    );
    assert_eq!(
        stats.superseded, 0,
        "distinct documents do not supersede: {stats:?}"
    );
    assert!(
        stats.evicted >= documents.len() as u64 - capacity as u64 - 3,
        "overflow must drop oldest supersedable work: {stats:?}"
    );
    assert_eq!(
        plan127_superseded_count(&outcomes),
        stats.evicted,
        "evicted callers answer Superseded"
    );
    assert_eq!(
        plan127_ok_labels(&outcomes).len() as u64,
        documents.len() as u64 - stats.evicted,
        "every request is served or evicted"
    );
    let labels = plan127_ok_labels(&outcomes);
    assert!(
        labels.contains(&format!("q{}", documents.len())),
        "the newest request must survive capacity pressure: {labels:?}"
    );
    println!(
        "PLAN127_QUEUE_CAPACITY requests={} peak_pending={} capacity={} superseded={} evicted={} served={}",
        documents.len(),
        stats.peak_pending,
        stats.capacity,
        stats.superseded,
        stats.evicted,
        labels.len()
    );
    let _ = fs::remove_dir_all(config_fixture("lane-module-provider"));
}

/// Plan 132: lane channel capacities are transport contract, not an
/// implementation detail — a lane that silently grows its buffer changes
/// which overflow a slow connection sees. 16 for the advisory editor-command
/// lane, 4 for each state lane.
#[tokio::test]
async fn lane_channel_capacities_are_preserved() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();

    let mut commands = service.subscribe_editor_commands();
    for index in 0..17 {
        service
            .editor_commands
            .publish(crate::protocol::EditorCommandRequest {
                command_id: format!("editor.fixture{index}"),
                package_prefix: "fixture".to_string(),
                mode_id: "clay.default".to_string(),
            });
    }
    assert!(
        matches!(
            commands.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
        ),
        "editor-command lane buffers exactly 16"
    );

    let mut carets = service.subscribe_caret_styles();
    let mut layouts = service.subscribe_editor_layout();
    let mut preferences = service.subscribe_shell_preferences();
    for _ in 0..5 {
        service.caret_styles.publish(None);
        service.editor_layouts.publish(None);
        service
            .shell_preferences
            .publish(crate::protocol::ShellPreferences {
                pane_focus_policy: "click".to_string(),
            });
    }
    // Five publishes into a capacity-4 lane drop exactly one message; a lane
    // that grew its buffer would not report `Lagged` at all.
    assert!(matches!(
        carets.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
    ));
    assert!(matches!(
        layouts.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
    ));
    assert!(matches!(
        preferences.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
    ));
}

// ── Plan 136 task 6: the granted third-party fixture ─────────────────────────

/// Plan 136 task 6 fixture: the plan-127 fixture shape reached through the
/// plan-136 grant surface. ONE third-party package declares `parse-document` +
/// `completion-provider`, registers a deliberately slow parse handler (inline
/// module object → general lane) and a `moduleSpecifier` completion provider
/// (→ latency lane).
struct GrantedLaneFixture {
    name: String,
    root: PathBuf,
    load_specifier: String,
    package_json: serde_json::Value,
}

fn granted_lane_fixture(package_name: &str, api_prefix: &str, busy_ms: u64) -> GrantedLaneFixture {
    let root = config_fixture("plan136-granted-lane").join(api_prefix);
    write_loadable_package(
        &root,
        &format!(
            r#"
            import {{ serverRegisterCompletionProvider }} from "clay:completion";
            import {{ serverRegisterParseHandler }} from "clay:parse";
            import * as parser from "./parser.js";
            import * as provider from "./provider.js";
            export default function load() {{
              serverRegisterParseHandler({{
                mode: "{api_prefix}",
                module: parser,
                exportName: "parseGrantedDocument",
                parseUnit: "line-group",
                viewportPriority: true,
                timeoutMs: 2_000
              }});
              // `module` binds the handler in this isolate and marks the
              // registration as runtime-bridged; `moduleSpecifier` is what
              // lets the latency lane materialize the same handler by import.
              serverRegisterCompletionProvider({{
                module: provider,
                moduleSpecifier: import.meta.resolve("./provider.js"),
                exportName: "provideCompletion"
              }});
            }}
            "#
        ),
    );
    fs::write(
        root.join("dist/parser.js"),
        format!(
            r#"
            const BUSY_MS = {busy_ms};
            export async function parseGrantedDocument(notification) {{
              const deadline = Date.now() + BUSY_MS;
              while (Date.now() < deadline) {{}}
              return {{ viewport: notification?.viewport ?? null }};
            }}
            "#
        ),
    )
    .expect("write slow package parse handler");
    fs::write(
        root.join("dist/provider.js"),
        r#"
        export async function provideCompletion(_request, _window) {
          return {
            status: "ok",
            items: [{
              label: "granted-lane",
              insertText: "granted-lane",
              detail: "granted module-backed provider (latency lane)"
            }]
          };
        }
        "#,
    )
    .expect("write module-backed provider module");
    let package_json = test_package_json(
        package_name,
        api_prefix,
        &["mode-registration", "parse-document", "completion-provider"],
        serde_json::json!({
            "modePatterns": [{
                "mode": api_prefix,
                "displayName": "Granted Lane",
                "extensions": ["grantedlane"]
            }],
            "completionProviders": [{
                "id": format!("{api_prefix}.provider"),
                "priority": 0,
                "budgets": { "timeoutMs": 2_000, "maxItems": 8 }
            }]
        }),
    );
    GrantedLaneFixture {
        name: package_name.to_string(),
        load_specifier: format!("clay://packages/{package_name}/dist/load.js"),
        root,
        package_json,
    }
}

/// Install, adopt, grant, and enable the fixture through the real service path
/// (the same order the host CLI now exposes), then run its load entry with the
/// package's host-stamped provenance. Returns the parse-handler and
/// completion-provider registrations the load registered.
async fn load_granted_lane_fixture(
    service: &ClayJsRuntimeService,
    fixture: &GrantedLaneFixture,
) -> (
    crate::server::parse_coordinator::JsParseHandlerRegistration,
    crate::server::completion::JsCompletionProviderRegistration,
) {
    use crate::packages::permissions::PackagePermission;
    let approved = vec![
        PackagePermission::ModeRegistration,
        PackagePermission::ParseDocument,
        PackagePermission::CompletionProvider,
    ];
    ensure_synthetic_package_enabled(
        service,
        fixture.package_json.clone(),
        approved.clone(),
        None,
    );
    service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            &fixture.load_specifier,
            fixture.root.join("dist/load.js"),
            fixture.root.clone(),
            Some(&fixture.name),
        );
    let evaluation = evaluate_as_package(
        service,
        fixture.package_json.clone(),
        approved,
        &format!(
            "const m = await import({:?}); await m.default();",
            fixture.load_specifier
        ),
    )
    .await
    .expect("granted lane fixture load");
    let parse_registration = evaluation
        .js_parse_handlers
        .iter()
        .find(|registration| registration.package.manifest.name == fixture.name)
        .cloned()
        .expect("granted fixture must register a parse handler");
    let completion_registration = evaluation
        .js_completion_providers
        .iter()
        .find(|registration| registration.package.manifest.name == fixture.name)
        .cloned()
        .expect("granted fixture must register a completion provider");
    (parse_registration, completion_registration)
}

fn granted_lane_parse_notification(api_prefix: &str) -> ParseEditNotification {
    ParseEditNotification {
        document_id: 7,
        document_version: 3,
        behavior_version: 1,
        package_prefix: api_prefix.to_string(),
        mode_id: api_prefix.to_string(),
        viewport: ParseByteRange::new(0, 2),
        invalidated_ranges: vec![ParseByteRange::new(0, 2)],
        accepted_edit: None,
        parse_windows: Vec::new(),
        memory_budget: None,
        trace_id: None,
        request_id: None,
    }
}

/// Plan 136 task 6 acceptance: an adopted third-party package with
/// `parse-document` + `completion-provider` grants answers a completion from
/// the latency lane while its own 500 ms parse handler holds the general lane,
/// and the answer carries the package's provenance.
#[tokio::test]
async fn granted_third_party_provider_serves_from_latency_lane_when_general_lane_busy() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    use crate::packages::bundled::RuntimeDomain;

    let busy_ms = 500u64;
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(2_000));
    let fixture = granted_lane_fixture("@vendor/grantedlane", "grantedlane", busy_ms);
    let (parse_registration, completion_registration) =
        load_granted_lane_fixture(&service, &fixture).await;

    // Lane identity: the provider registration carries the package-owned
    // module specifier (latency lane) while the parse handler stays inline
    // (general lane), and the grant did not move the package out of the
    // third-party domain.
    let provider_module = completion_registration
        .module_specifier
        .clone()
        .expect("module-backed provider must carry its resolved module specifier");
    let allowlist = service.test_op_state().load_entry_allowlist();
    assert!(
        allowlist.is_package_module(&provider_module, &fixture.name),
        "the latency lane must materialize the owning package's module"
    );
    assert!(
        !allowlist.is_package_module(&provider_module, "@vendor/someone-else"),
        "the latency lane must never materialize another package's module"
    );
    assert_eq!(
        service.registration_domain(&completion_registration.package),
        RuntimeDomain::ThirdParty,
        "a capability grant never promotes a package into the trusted domain"
    );
    assert_eq!(
        service.registration_domain(&parse_registration.package),
        service.registration_domain(&completion_registration.package),
        "the A/B pair stays in one trust domain, so only the lane differs"
    );

    let (request, window) = lane_completion_input("grantedlane", 1, "gr");
    let mut idle_us = Vec::new();
    for request_id in 0..10u64 {
        let (request, window) = lane_completion_input("grantedlane", request_id, "gr");
        let start = Instant::now();
        let result = service
            .invoke_completion_provider(completion_registration.clone(), request, window)
            .await
            .expect("idle completion");
        idle_us.push(start.elapsed().as_micros());
        assert_eq!(result.items[0].label, "granted-lane");
        assert_eq!(
            result.provenance.package_name, fixture.name,
            "a granted provider's answer carries its package provenance"
        );
        assert_eq!(result.provenance.package_prefix, "grantedlane");
    }
    idle_us.sort_unstable();
    let idle_median_us = idle_us[idle_us.len() / 2];

    // Hold the general lane for `busy_ms` with the package's own parse handler,
    // then issue one completion 50 ms in: at least 450 ms of the busy loop
    // remain if the lanes shared a worker.
    let parse_task = tokio::spawn({
        let service = service.clone();
        let registration = parse_registration.clone();
        let notification = granted_lane_parse_notification("grantedlane");
        async move {
            service
                .invoke_parse_handler(registration, notification)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let workers_before_completion = service.workers_started();
    let start = Instant::now();
    let completion = service
        .invoke_completion_provider(completion_registration.clone(), request, window)
        .await
        .expect("granted latency-lane completion");
    let busy_completion_us = start.elapsed().as_micros();
    assert_eq!(completion.items[0].label, "granted-lane");
    assert_eq!(completion.provenance.package_name, fixture.name);
    assert!(
        !parse_task.is_finished(),
        "the general lane must still be held when the latency lane answers"
    );
    assert_eq!(
        service.workers_started(),
        workers_before_completion,
        "a busy general lane must not replace either lane"
    );
    assert!(
        busy_completion_us < 250_000,
        "completion must not queue behind the busy general lane: {busy_completion_us} us \
         (busy {busy_ms} ms, idle median {idle_median_us} us)"
    );
    let parse_result = parse_task.await.expect("parse task join");
    assert!(
        parse_result.is_ok(),
        "busy parse must complete under its timeout: {parse_result:?}"
    );
    eprintln!(
        "PLAN136_GRANTED_LANE busy_ms={busy_ms} idle_median_us={idle_median_us} \
         busy_completion_us={busy_completion_us} workers_started={}",
        service.workers_started()
    );
}

/// Plan 136 task 6 security acceptance: the same fixture without its grants
/// (and with a partial grant) still fails closed with
/// `MissingCapabilityGrant`, registers no provider, and starts no lane.
#[tokio::test]
async fn ungranted_third_party_provider_still_fails_closed() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    use crate::packages::authorization::RuntimeProfile;
    use crate::packages::permissions::PackagePermission;
    use crate::packages::service::PackageServiceError;

    let service = ClayJsRuntimeService::default();
    let workers_before = service.workers_started();
    let fixture = granted_lane_fixture("@vendor/ungrantedlane", "ungrantedlane", 500);
    let op_state = service.test_op_state();
    let mut package_service = op_state
        .package_service()
        .lock()
        .expect("package service mutex poisoned");
    package_service
        .install_from_value_at_root_with_spec(
            fixture.package_json.clone(),
            fixture.root.clone(),
            "local:plan136-grant-fixture",
        )
        .expect("fixture installs");
    package_service
        .approve_package(&fixture.name, "test")
        .expect("adoption persists");

    // Adoption alone is never authority.
    let error = package_service
        .enable(&fixture.name)
        .expect_err("an adopted package with no grant must fail closed");
    assert!(
        matches!(error, PackageServiceError::MissingCapabilityGrant { .. }),
        "expected MissingCapabilityGrant, got {error}"
    );

    // A partial grant still fails closed: every declared capability needs one.
    package_service
        .authorize_package(
            &fixture.name,
            vec![PackagePermission::CompletionProvider],
            RuntimeProfile::Restricted,
            "test",
        )
        .expect("partial grant records");
    let error = package_service
        .enable(&fixture.name)
        .expect_err("a partially granted package must still fail closed");
    assert!(
        matches!(error, PackageServiceError::MissingCapabilityGrant { .. }),
        "expected MissingCapabilityGrant for the ungranted capabilities, got {error}"
    );
    drop(package_service);

    assert!(
        service
            .third_party_registrations_snapshot()
            .js_completion_providers
            .iter()
            .all(|registration| registration.package.manifest.name != fixture.name),
        "an ungranted package must contribute no completion provider"
    );
    assert_eq!(
        service.workers_started(),
        workers_before,
        "a host-side refusal must not start a lane"
    );
}

/// Plan 136 task 7 acceptance: lane occupancy is observable per (domain, lane),
/// and the counters keep lanes apart. The granted fixture holds the general lane
/// with its parse handler while its module-backed provider answers completions
/// on the latency lane; the two lanes' dispatch counts move independently, the
/// trusted domain's lanes absorb nothing, and the report-time recording maps
/// each lane's mailbox state onto that lane's own metric names (the values a
/// `CLAY_PERF_REPORT_DIR` run writes into its summary).
#[tokio::test]
async fn lane_command_counters_track_general_and_latency_separately() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    use crate::packages::bundled::RuntimeDomain;

    let busy_ms = 500u64;
    let completions = 12u64;
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(2_000));
    let fixture = granted_lane_fixture("@vendor/lanecounters", "lanecounters", busy_ms);
    let (parse_registration, completion_registration) =
        load_granted_lane_fixture(&service, &fixture).await;

    let trusted_general_before = service
        .lane_queue_stats(RuntimeDomain::Trusted, RuntimeLane::General)
        .dispatched;
    let trusted_latency_before = service
        .lane_queue_stats(RuntimeDomain::Trusted, RuntimeLane::Latency)
        .dispatched;

    // Mixed workload: the package's own 500 ms parse handler holds the general
    // lane while its provider answers completions on the latency lane.
    let parse_task = tokio::spawn({
        let service = service.clone();
        let registration = parse_registration.clone();
        let notification = granted_lane_parse_notification("lanecounters");
        async move {
            service
                .invoke_parse_handler(registration, notification)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut busy_us = Vec::new();
    for request_id in 0..completions {
        let (request, window) = lane_completion_input("lanecounters", request_id, "lane");
        let start = Instant::now();
        let result = service
            .invoke_completion_provider(completion_registration.clone(), request, window)
            .await
            .expect("latency-lane completion while the general lane is held");
        busy_us.push(start.elapsed().as_micros());
        assert_eq!(result.items[0].label, "granted-lane");
    }
    assert!(
        !parse_task.is_finished(),
        "the general lane must still be held while the latency lane answers"
    );
    parse_task
        .await
        .expect("parse task join")
        .expect("busy parse completes under its timeout");

    let general = service.lane_queue_stats(RuntimeDomain::ThirdParty, RuntimeLane::General);
    let latency = service.lane_queue_stats(RuntimeDomain::ThirdParty, RuntimeLane::Latency);
    let trusted_general = service.lane_queue_stats(RuntimeDomain::Trusted, RuntimeLane::General);
    let trusted_latency = service.lane_queue_stats(RuntimeDomain::Trusted, RuntimeLane::Latency);

    // Separation: the fixture load plus its parse handler are general-lane
    // dispatches, the completions are latency-lane dispatches, and neither
    // domain's counters pick up the other's work.
    assert!(
        general.dispatched >= 1,
        "the fixture load and parse handler must be counted on the general lane: {general:?}"
    );
    assert!(
        latency.dispatched >= completions,
        "every completion must be counted on the latency lane: {latency:?}"
    );
    assert_eq!(
        trusted_general.dispatched, trusted_general_before,
        "third-party work must not be counted on the trusted domain's general lane"
    );
    assert_eq!(
        trusted_latency.dispatched, trusted_latency_before,
        "third-party work must not be counted on the trusted domain's latency lane"
    );

    // Report-time recording: one snapshot per (domain, lane) metric, carrying
    // the mailbox's own count, and no document/provider content.
    let recorder = crate::perf::metrics::PerfRecorder::for_test(true);
    service.record_lane_metrics(&recorder);
    let summary = recorder.summary();
    let names = crate::perf::metrics::JS_RUNTIME_LANE_METRICS;
    let third_party = 1;
    assert_eq!(
        summary.metrics[names[third_party][0].dispatched].total, general.dispatched,
        "recorded general-lane dispatch count must match the mailbox"
    );
    assert_eq!(
        summary.metrics[names[third_party][1].dispatched].total, latency.dispatched,
        "recorded latency-lane dispatch count must match the mailbox"
    );
    assert_eq!(
        summary.metrics[names[third_party][1].peak_pending].total, latency.peak_pending as u64,
        "recorded latency-lane occupancy must match the mailbox"
    );
    assert_eq!(
        summary.metrics[names[third_party][1].superseded].total, latency.superseded,
        "recorded latency-lane supersede count must match the mailbox"
    );
    assert_eq!(
        summary.metrics[names[third_party][1].evicted].total, latency.evicted,
        "recorded latency-lane eviction count must match the mailbox"
    );
    assert_eq!(
        summary.metrics[names[0][0].dispatched].total, trusted_general.dispatched,
        "an idle lane is reported as zero, not omitted"
    );
    assert_eq!(
        summary.metrics[names[0][1].dispatched].total, trusted_latency.dispatched,
        "an idle lane is reported as zero, not omitted"
    );

    busy_us.sort_unstable();
    let p50_us = busy_us[busy_us.len() / 2];
    let p95_us = busy_us[(busy_us.len() * 95).div_ceil(100).saturating_sub(1)];
    let max_us = *busy_us.last().expect("completions were measured");
    for (domain, lane, stats) in [
        ("trusted", "general", trusted_general),
        ("trusted", "latency", trusted_latency),
        ("third_party", "general", general),
        ("third_party", "latency", latency),
    ] {
        eprintln!(
            "PLAN136_LANE_OCCUPANCY domain={domain} lane={lane} dispatched={} pending={} \
             peak_pending={} capacity={} superseded={} evicted={}",
            stats.dispatched,
            stats.pending_supersedable,
            stats.peak_pending,
            stats.capacity,
            stats.superseded,
            stats.evicted
        );
    }
    eprintln!(
        "PLAN136_LANE_LATENCY busy_ms={busy_ms} completions={completions} p50_us={p50_us} \
         p95_us={p95_us} max_us={max_us}"
    );
    assert!(
        p95_us < 250_000,
        "the latency lane must stay responsive under a busy general lane: p95 {p95_us} us"
    );
    let _ = fs::remove_dir_all(config_fixture("plan136-granted-lane"));
}

/// Plan 136 task 7: counting a dispatched command takes no lock beyond the queue
/// hand-off the mailbox already performs. The counter is a lock-free atomic —
/// the compile-time check lives next to the field
/// (`worker.rs`: `const _: fn(&CommandMailbox) -> &AtomicU64`) — and the count
/// itself is maintained by the dispatch path, so it is exact and lane-local
/// (the plan-127 lane isolation suite stays green in this same run).
#[tokio::test]
async fn lane_counters_do_not_require_locking_on_the_dispatch_path() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    use crate::packages::bundled::RuntimeDomain;

    const INSTANT_PROVIDER: &str = r#"
    export async function provideCompletion(_request, window) {
      return { status: "ok", items: [{ label: window.text, insertText: "q" }] };
    }
    "#;
    let completions = 3u64;
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/lanecounters2",
        "lanecounters2",
        INSTANT_PROVIDER,
    )
    .await;
    let general_before = service
        .lane_queue_stats(RuntimeDomain::ThirdParty, RuntimeLane::General)
        .dispatched;
    let latency_before = service
        .lane_queue_stats(RuntimeDomain::ThirdParty, RuntimeLane::Latency)
        .dispatched;

    for request_id in 0..completions {
        let (request, window) = lane_completion_input("lanecounters2", request_id, "lane");
        let result = service
            .invoke_completion_provider(registration.clone(), request, window)
            .await
            .expect("module-backed completion");
        assert_eq!(result.items[0].label, "lane");
    }

    let general = service.lane_queue_stats(RuntimeDomain::ThirdParty, RuntimeLane::General);
    let latency = service.lane_queue_stats(RuntimeDomain::ThirdParty, RuntimeLane::Latency);
    assert_eq!(
        latency.dispatched - latency_before,
        completions,
        "every dispatched completion is counted exactly once: {latency:?}"
    );
    assert_eq!(
        general.dispatched, general_before,
        "a latency-lane dispatch must not touch the general lane's counter: {general:?}"
    );
    assert_eq!(
        latency.superseded, 0,
        "sequential completions must not supersede each other: {latency:?}"
    );
    let _ = fs::remove_dir_all(config_fixture("lane-module-provider"));
}
