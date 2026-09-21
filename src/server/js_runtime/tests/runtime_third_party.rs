use super::*;

#[tokio::test]
async fn js_runtime_evaluates_controlled_module() {
    let service = ClayJsRuntimeService::default();
    let result = service
        .evaluate_controlled_module(
            r#"
            const ping = Deno.core.ops.op_clay_runtime_ping();
            if (ping !== "clay-runtime-ready") {
                throw new Error(`unexpected ping: ${ping}`);
            }
            Deno.core.ops.op_clay_runtime_record("configured");
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["configured"]);
    assert_eq!(service.evaluation_count(), 1);
}

#[tokio::test]
async fn persistent_js_runtime_retains_global_state_between_evaluations() {
    let service = ClayJsRuntimeService::default();
    service
        .evaluate_controlled_module(r#"globalThis.__clayPersistentRuntime = 41;"#)
        .await
        .unwrap();
    let result = service
        .evaluate_controlled_module(
            r#"
            if (globalThis.__clayPersistentRuntime !== 41) {
                throw new Error("persistent runtime state missing");
            }
            Deno.core.ops.op_clay_runtime_record("persistent");
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["persistent"]);
    assert_eq!(service.evaluation_count(), 2);
}

// ── Plan 061 task 4: two-domain trust boundary tests ────────────────────

// ── Plan 061 task 5: package-scoped provenance adversarial tests ────────

#[tokio::test]
async fn package_provenance_ignores_caller_supplied_identity_fields() {
    let service = ClayJsRuntimeService::default();
    // Options still carry forged identity fields naming another package;
    // publication provenance must come from the executing-package context.
    let result = evaluate_as_package(
        &service,
        test_package_json(
            "@vendor/alpha",
            "alpha",
            &["render-decorations"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::RenderDecorations],
        r#"
        import { serverPublishDiagnostics } from "clay:diagnostics";
        serverPublishDiagnostics({
          packageName: "@vendor/beta",
          packageManifest: { name: "@vendor/beta", version: "9.9.9", clay: { apiPrefix: "beta" } },
          packagePrefix: "beta",
          permissions: ["render-decorations", "raw-ops"],
          documentId: 1,
          documentVersion: 1,
          viewport: { byteStart: 0, byteEnd: 8 },
          source: "s",
          spans: [{ byteStart: 0, byteEnd: 1, severity: "error", code: "x", message: "y" }],
        });
        "#,
    )
    .await
    .unwrap();
    let set = result.published_diagnostic_set.expect("diagnostic set");
    assert_eq!(set.provenance.package_name, "@vendor/alpha");
    assert_eq!(set.provenance.package_prefix, "alpha");
}

#[tokio::test]
async fn disabled_package_callback_publications_fail_closed() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@vendor/stale",
            "stale",
            &["parse-document", "render-decorations"],
            serde_json::json!({}),
        ),
        vec![
            crate::packages::permissions::PackagePermission::ParseDocument,
            crate::packages::permissions::PackagePermission::RenderDecorations,
        ],
        r#"
        import { serverPublishDiagnostics } from "clay:diagnostics";
        import { serverRegisterParseHandler } from "clay:parse";
        serverRegisterParseHandler({
          mode: "stale",
          module: { default: async (notification) => {
            serverPublishDiagnostics({
              documentId: notification.documentId,
              documentVersion: notification.documentVersion,
              viewport: notification.viewport,
              source: "stale",
              spans: [{ byteStart: 0, byteEnd: 1, severity: "error", code: "x", message: "y" }],
            });
            return { viewport: notification.viewport };
          } }
        });
        "#,
    )
    .await
    .unwrap();
    let registration = evaluation
        .js_parse_handlers
        .first()
        .expect("handler registered")
        .clone();

    // Disable the package host-side; the stale registration's callback
    // must fail closed at op ingress (enabled-set lookup), not publish.
    service
        .test_op_state()
        .package_service()
        .lock()
        .expect("package service mutex poisoned")
        .disable("@vendor/stale")
        .expect("package disables");
    let notification = ParseEditNotification {
        document_id: 1,
        document_version: 1,
        behavior_version: 1,
        package_prefix: "stale".to_string(),
        mode_id: "stale".to_string(),
        viewport: ParseByteRange::new(0, 4),
        invalidated_ranges: vec![ParseByteRange::new(0, 4)],
        accepted_edit: None,
        parse_windows: Vec::new(),
        memory_budget: None,
        trace_id: None,
        request_id: None,
    };
    // Two independent layers, asserted separately (Plan 127 task 6):
    // 1. A command that already reached the isolate must still fail closed at
    //    op ingress (enabled-set lookup). Dispatched straight to the lane's
    //    mailbox so this layer is exercised even if the host gate were gone.
    let (response, receiver) = tokio::sync::oneshot::channel();
    assert!(
        service
            .domain_lane_worker(
                crate::packages::bundled::RuntimeDomain::ThirdParty,
                RuntimeLane::General,
            )
            .sender
            .send(super::super::worker::RuntimeCommand::Parse {
                registration: registration.clone(),
                notification: notification.clone(),
                response,
            })
            .is_ok(),
        "lane accepts a pre-gate command"
    );
    let error = receiver
        .await
        .expect("worker replies")
        .expect_err("stale package callback must fail closed");
    assert!(
        error.to_string().contains("packages.package_not_enabled"),
        "in-isolate op ingress must refuse a disabled package, got {error}"
    );

    // 2. The host gate refuses the same request with a typed error before any
    //    lane work happens.
    let error = service
        .invoke_parse_handler(registration, notification)
        .await
        .unwrap_err();
    assert!(
        matches!(
            &error,
            ClayRuntimeError::Revoked { package, version }
                if package == "@vendor/stale" && version == "0.1.0"
        ),
        "host-side gate must refuse the stale registration, got {error:?}"
    );
}

#[tokio::test]
async fn language_server_session_io_requires_executing_owner_package() {
    let service = ClayJsRuntimeService::default();
    // Package B knows A's package/contribution names and a session id, but
    // session IO is bound to the host-stamped executing package.
    let error = evaluate_as_package_with_ls_grant(
        &service,
        serde_json::json!({
            "name": "@vendor/b",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "beta",
                "entry": "./dist/index.js",
                "permissions": ["parse-document"],
                "capabilities": ["language-server"],
                "modes": [],
                "docs": "./docs/index.md",
                "contributions": {
                    "languageServers": [{
                        "id": "beta.server",
                        "executable": "/bin/true",
                        "args": []
                    }]
                }
            }
        }),
        vec![crate::packages::permissions::PackagePermission::ParseDocument],
        Some((
            "beta.server",
            std::fs::canonicalize("/bin/true").expect("canonical /bin/true"),
        )),
        r#"
        const identity = { sessionId: 1, package: "@vendor/a", contribution: "a.server" };
        try {
          await Deno.core.ops.op_clay_language_server_send_message(
            JSON.stringify({ ...identity, message: "x" }));
          throw new Error("cross-package session write must not succeed");
        } catch (error) {
          Deno.core.ops.op_clay_runtime_record(String(error));
        }
        "#,
    )
    .await
    .unwrap();
    assert!(
        evaluation_contains(&error, "language_server.session_owner_mismatch"),
        "cross-package session IO must fail, got {:?}",
        error.op_records
    );
}

#[tokio::test]
async fn third_party_provider_executes_in_third_party_runtime_only() {
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@vendor/domain-dynamic",
            "domaindyn",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "domaindyn.provider",
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 500, "maxItems": 8 }
                }]
            }),
        ),
        vec![crate::packages::permissions::PackagePermission::CompletionProvider],
        r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async (_request, window) => ({
                  status: "ok",
                  items: [{ label: "dynamic", insertText: "dynamic", detail: window.text }]
                })
              }
            });
            "#,
    )
    .await
    .unwrap();
    let coordinator = crate::server::completion::CompletionCoordinator::new();
    service
        .register_completion_providers(&coordinator, 4, &evaluation)
        .unwrap();
    let trusted_before =
        service.domain_evaluations(crate::packages::bundled::RuntimeDomain::Trusted);
    let third_party_before =
        service.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty);
    let reply_rx = coordinator
        .schedule_completion(
            "domaindyn.provider",
            crate::protocol::CompletionRequest {
                request_id: 92,
                client_id: 2,
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                cursor_byte_offset: 2,
                replacement_range: crate::protocol::CompletionReplacementRange {
                    byte_start: 2,
                    byte_end: 2,
                },
                trigger: crate::protocol::CompletionTrigger::Manual,
                provider_generation: 4,
                recent_completions: Box::new([]),
            },
            crate::server::completion::CompletionDocumentWindow {
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                package_prefix: "domaindyn".to_string(),
                byte_start: 0,
                byte_end: 2,
                text: "fn".to_string(),
            },
        )
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), reply_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.items[0].detail, "fn");
    assert!(
        service.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty)
            > third_party_before,
        "third-party provider must execute in the third-party runtime"
    );
    assert_eq!(
        service.domain_evaluations(crate::packages::bundled::RuntimeDomain::Trusted),
        trusted_before,
        "third-party provider invocation must not touch the trusted runtime"
    );
}

#[tokio::test]
async fn slow_third_party_provider_poisons_only_third_party_domain() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@vendor/slow-dynamic",
            "slowdyn",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "slowdyn.provider",
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 50, "maxItems": 8 }
                }]
            }),
        ),
        vec![crate::packages::permissions::PackagePermission::CompletionProvider],
        r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async () => { for (;;) {} }
              }
            });
            "#,
    )
    .await
    .unwrap();
    let coordinator = crate::server::completion::CompletionCoordinator::new();
    service
        .register_completion_providers(&coordinator, 4, &evaluation)
        .unwrap();
    let reply_rx = coordinator
        .schedule_completion(
            "slowdyn.provider",
            crate::protocol::CompletionRequest {
                request_id: 93,
                client_id: 2,
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                cursor_byte_offset: 2,
                replacement_range: crate::protocol::CompletionReplacementRange {
                    byte_start: 2,
                    byte_end: 2,
                },
                trigger: crate::protocol::CompletionTrigger::Manual,
                provider_generation: 4,
                recent_completions: Box::new([]),
            },
            crate::server::completion::CompletionDocumentWindow {
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                package_prefix: "slowdyn".to_string(),
                byte_start: 0,
                byte_end: 2,
                text: "fn".to_string(),
            },
        )
        .unwrap();
    // The busy-loop provider times out; only the third-party domain is
    // poisoned. The trusted runtime keeps answering immediately.
    // The busy-loop provider times out inside the coordinator; the
    // request-scoped reply is dropped and the receiver observes
    // cancellation instead of a result.
    let outcome = tokio::time::timeout(Duration::from_secs(1), reply_rx).await;
    assert!(matches!(outcome, Ok(Err(_))));
    let trusted = service
        .evaluate_controlled_module(
            r#"Deno.core.ops.op_clay_runtime_record(Deno.core.ops.op_clay_runtime_ping());"#,
        )
        .await
        .expect("trusted runtime survives third-party provider timeout");
    assert_eq!(trusted.op_records, vec!["clay-runtime-ready"]);
}

/// Plan 061 task 12: a trusted-generation reload shares the third-party
/// domain — providers keep answering in the SAME worker (generation and
/// evaluation counters unchanged), and the reload snapshot re-registers
/// them under a new generation.
#[tokio::test]
async fn trusted_reload_preserves_third_party_providers() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@vendor/reload-survivor",
            "survivor",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "survivor.provider",
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 500, "maxItems": 8 }
                }]
            }),
        ),
        vec![crate::packages::permissions::PackagePermission::CompletionProvider],
        r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async () => ({
                  status: "ok",
                  items: [{ label: "survivor", insertText: "survivor" }]
                })
              }
            });
            "#,
    )
    .await
    .unwrap();

    let third_party_generation_before =
        service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty);
    let reloaded = ClayJsRuntimeService::production_reload(&service);
    assert_eq!(
        reloaded.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty),
        third_party_generation_before,
        "trusted reload must not replace the third-party worker"
    );

    // Re-register the surviving third-party registrations under the new
    // generation exactly like the reload commit path does.
    let snapshot = reloaded.third_party_registrations_snapshot();
    assert_eq!(
        snapshot.js_completion_providers.len(),
        evaluation.js_completion_providers.len(),
        "surviving worker must expose its registration payload"
    );
    let coordinator = crate::server::completion::CompletionCoordinator::new();
    reloaded
        .register_completion_providers(&coordinator, 5, &snapshot)
        .unwrap();
    let third_party_evals_before =
        reloaded.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty);
    let reply_rx = coordinator
        .schedule_completion(
            "survivor.provider",
            crate::protocol::CompletionRequest {
                request_id: 95,
                client_id: 2,
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                cursor_byte_offset: 2,
                replacement_range: crate::protocol::CompletionReplacementRange {
                    byte_start: 2,
                    byte_end: 2,
                },
                trigger: crate::protocol::CompletionTrigger::Manual,
                provider_generation: 5,
                recent_completions: Box::new([]),
            },
            crate::server::completion::CompletionDocumentWindow {
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                package_prefix: "survivor".to_string(),
                byte_start: 0,
                byte_end: 2,
                text: "su".to_string(),
            },
        )
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), reply_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.items[0].label, "survivor");
    assert!(
        reloaded.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty)
            > third_party_evals_before,
        "provider must answer in the shared third-party worker after reload"
    );
}

/// Plan 061 task 12: a poisoned third-party domain is replaced once and
/// replays ONLY the current approved graph; deterministic registration
/// tokens make the pre-poison coordinator registrations valid again.
#[tokio::test]
async fn third_party_poison_replays_approved_graph_and_restores_providers() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(100));
    let root = config_fixture("third-party-replay").join("replayd");
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        serverRegisterCompletionProvider({
          module: {
            provideCompletion: async () => ({
              status: "ok",
              items: [{ label: "replayed", insertText: "replayed" }]
            })
          }
        });
        export default function load() {}
        "#,
    );
    let package_json = test_package_json(
        "@vendor/replayd",
        "replayd",
        &["completion-provider"],
        serde_json::json!({
            "completionProviders": [{
                "id": "replayd.provider",
                "triggerCharacters": ["."],
                "budgets": { "timeoutMs": 50, "maxItems": 8 }
            }]
        }),
    );
    let approved = vec![crate::packages::permissions::PackagePermission::CompletionProvider];
    ensure_synthetic_package_enabled(&service, package_json.clone(), approved.clone(), None);
    // Record the load entry so the poison-recovery replay can re-import it.
    service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            "clay://packages/@vendor/replayd/dist/load.js",
            root.join("dist/load.js"),
            root.clone(),
            Some("@vendor/replayd"),
        );
    let evaluation = evaluate_as_package(
        &service,
        package_json.clone(),
        approved.clone(),
        r#"const m = await import("clay://packages/@vendor/replayd/dist/load.js"); await m.default();"#,
    )
    .await
    .unwrap();
    let coordinator = crate::server::completion::CompletionCoordinator::new();
    service
        .register_completion_providers(&coordinator, 4, &evaluation)
        .unwrap();

    // Poison the third-party domain with a busy-loop evaluation.
    let _ = evaluate_as_package(&service, package_json, approved, "for (;;) {}").await;
    let generation_after_poison =
        service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty);

    // The next third-party dispatch replaces the worker and replays the
    // approved graph: the provider answers again under the same token.
    let reply_rx = coordinator
        .schedule_completion(
            "replayd.provider",
            crate::protocol::CompletionRequest {
                request_id: 96,
                client_id: 2,
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                cursor_byte_offset: 2,
                replacement_range: crate::protocol::CompletionReplacementRange {
                    byte_start: 2,
                    byte_end: 2,
                },
                trigger: crate::protocol::CompletionTrigger::Manual,
                provider_generation: 4,
                recent_completions: Box::new([]),
            },
            crate::server::completion::CompletionDocumentWindow {
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                package_prefix: "replayd".to_string(),
                byte_start: 0,
                byte_end: 2,
                text: "re".to_string(),
            },
        )
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), reply_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.items[0].label, "replayed");
    assert!(
        service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty)
            > generation_after_poison,
        "poison recovery must bump the third-party domain generation"
    );
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// Plan 061 task 13 adversarial: the cross-domain load bridge rejects a
/// TRUSTED record — config must never route bundled packages through the
/// third-party runtime via the bridge op.
#[tokio::test]
async fn cross_domain_load_bridge_rejects_trusted_records() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let error = service
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            const result = JSON.parse(
                await Deno.core.ops.op_clay_packages_load_package_by_specifier(
                    JSON.stringify({ specifier: "@clay/markdown" })
                )
            );
            await Deno.core.ops.op_clay_packages_load_in_package_domain(
                JSON.stringify(result)
            );
            "#,
        )
        .await
        .expect_err("bridge must reject a trusted-domain record");
    let message = error.to_string();
    assert!(
        message.contains("is not a third-party package"),
        "unexpected bridge denial message: {message}"
    );
    assert_eq!(
        service.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty),
        0,
        "denied bridge call must not touch the third-party runtime"
    );
}

/// Plan 061 task 13 adversarial: poison recovery replays ONLY the current
/// approved graph — a package disabled after poisoning is not replayed,
/// and its pre-poison coordinator token fails closed.
#[tokio::test]
async fn third_party_poison_replay_skips_disabled_packages() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(100));
    let root = config_fixture("third-party-replay-skip").join("skipd");
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        serverRegisterCompletionProvider({
          module: {
            provideCompletion: async () => ({
              status: "ok",
              items: [{ label: "skipped", insertText: "skipped" }]
            })
          }
        });
        export default function load() {}
        "#,
    );
    let package_json = test_package_json(
        "@vendor/skipd",
        "skipd",
        &["completion-provider"],
        serde_json::json!({
            "completionProviders": [{
                "id": "skipd.provider",
                "triggerCharacters": ["."],
                "budgets": { "timeoutMs": 50, "maxItems": 8 }
            }]
        }),
    );
    let approved = vec![crate::packages::permissions::PackagePermission::CompletionProvider];
    ensure_synthetic_package_enabled(&service, package_json.clone(), approved.clone(), None);
    service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            "clay://packages/@vendor/skipd/dist/load.js",
            root.join("dist/load.js"),
            root.clone(),
            Some("@vendor/skipd"),
        );
    let evaluation = evaluate_as_package(
        &service,
        package_json.clone(),
        approved.clone(),
        r#"const m = await import("clay://packages/@vendor/skipd/dist/load.js"); await m.default();"#,
    )
    .await
    .unwrap();
    let coordinator = crate::server::completion::CompletionCoordinator::new();
    service
        .register_completion_providers(&coordinator, 4, &evaluation)
        .unwrap();
    // Poison, then disable the package before any dispatch replays it.
    let _ = evaluate_as_package(&service, package_json, approved, "for (;;) {}").await;
    service
        .test_op_state()
        .package_service()
        .lock()
        .unwrap()
        .disable("@vendor/skipd")
        .unwrap();
    let reply_rx = coordinator
        .schedule_completion(
            "skipd.provider",
            crate::protocol::CompletionRequest {
                request_id: 97,
                client_id: 2,
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                cursor_byte_offset: 2,
                replacement_range: crate::protocol::CompletionReplacementRange {
                    byte_start: 2,
                    byte_end: 2,
                },
                trigger: crate::protocol::CompletionTrigger::Manual,
                provider_generation: 4,
                recent_completions: Box::new([]),
            },
            crate::server::completion::CompletionDocumentWindow {
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                package_prefix: "skipd".to_string(),
                byte_start: 0,
                byte_end: 2,
                text: "sk".to_string(),
            },
        )
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(1), reply_rx).await;
    // A dropped request-scoped reply or silent drop are both fail-closed:
    // no completion is ever produced from the disabled package.
    if let Ok(Ok(result)) = outcome {
        assert_ne!(
            result.status,
            crate::protocol::CompletionStatus::Ok,
            "disabled package must not be replayed; provider must fail closed"
        );
    }
    assert!(
        service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty) > 1,
        "poison recovery must have replaced the third-party worker"
    );
    service
        .evaluate_third_party_module("Deno.core.ops.op_clay_runtime_record('alive');")
        .await
        .expect("third-party domain stays alive after replay skip");
    assert!(
        !service
            .test_op_state()
            .package_service()
            .lock()
            .unwrap()
            .inspect("@vendor/skipd")
            .unwrap()
            .is_enabled,
        "replay must not re-enable the disabled package"
    );
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// Plan 061 task 13 adversarial: a third-party package cannot call
/// loadPackage — the loader op is trusted-only, so the public
/// clay:packages facade fails closed by op absence in the third-party
/// runtime.
#[tokio::test]
async fn third_party_package_cannot_load_other_packages() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let error = evaluate_as_package(
        &service,
        test_package_json("@vendor/loader", "loaderd", &[], serde_json::json!({})),
        Vec::new(),
        r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            "#,
    )
    .await
    .expect_err("third-party loadPackage must fail closed");
    let message = error.to_string();
    // clay:packages is not in the third-party facade allowlist: denial at
    // the import boundary, before any op is reachable.
    assert!(
        message.contains("not allowed in the server runtime boundary"),
        "expected import-boundary denial, got: {message}"
    );
    assert!(
        service
            .test_op_state()
            .package_service()
            .lock()
            .unwrap()
            .enabled_records()
            .all(|record| record.manifest.name != "@clay/markdown"),
        "denied loadPackage must not enable the target package"
    );
}

#[tokio::test]
async fn third_party_runtime_cannot_see_trusted_ops_or_admin_modules() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();

    // Trusted-only/admin op names are not even enumerable in the
    // third-party isolate; public contribution ops exist in both. The
    // seven editor ops are shared (follow-up round `editor-control`) but
    // gated per call by permission + declared mode; visibility alone
    // grants nothing.
    let probe = service
        .evaluate_third_party_module(
            r#"
            const ops = Deno.core.ops;
            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                ping: typeof ops.op_clay_runtime_ping,
                configGet: typeof ops.op_clay_configuration_get_state,
                openDocument: typeof ops.op_clay_documents_open_document,
                loadPackage: typeof ops.op_clay_packages_load_package,
                authorizeLs: typeof ops.op_clay_language_server_authorize,
                classify: typeof ops.op_clay_modes_classify_document,
                setTheme: typeof ops.op_clay_theme_set_theme,
                editorMove: typeof ops.op_clay_editor_move_cursor,
                editorSelect: typeof ops.op_clay_editor_set_selection,
                editorCaret: typeof ops.op_clay_editor_set_cursor_style,
                editorAddCursor: typeof ops.op_clay_editor_add_cursor,
                editorColumnSelect: typeof ops.op_clay_editor_column_select,
                editorTextobject: typeof ops.op_clay_editor_select_textobject,
                editorSmartSelect: typeof ops.op_clay_editor_smart_select,
                publicRegister: typeof ops.op_clay_commands_register_command,
                publicPublish: typeof ops.op_clay_decorations_publish_decorations,
            }));
            "#,
        )
        .await
        .expect("third-party op probe evaluation");
    assert_eq!(
        probe.op_records,
        vec![
            r#"{"ping":"undefined","configGet":"undefined","openDocument":"undefined","loadPackage":"undefined","authorizeLs":"undefined","classify":"undefined","setTheme":"undefined","editorMove":"function","editorSelect":"function","editorCaret":"function","editorAddCursor":"function","editorColumnSelect":"function","editorTextobject":"function","editorSmartSelect":"function","publicRegister":"function","publicPublish":"function"}"#
        ]
    );

    // The same names resolve to real functions in the trusted isolate.
    let trusted_probe = service
        .evaluate_controlled_module(
            r#"
            const ops = Deno.core.ops;
            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                ping: typeof ops.op_clay_runtime_ping,
                configGet: typeof ops.op_clay_configuration_get_state,
                loadPackage: typeof ops.op_clay_packages_load_package,
                editorMove: typeof ops.op_clay_editor_move_cursor,
                editorTextobject: typeof ops.op_clay_editor_select_textobject,
            }));
            "#,
        )
        .await
        .expect("trusted op probe evaluation");
    assert_eq!(
        trusted_probe.op_records,
        vec![
            r#"{"ping":"function","configGet":"function","loadPackage":"function","editorMove":"function","editorTextobject":"function"}"#
        ]
    );

    // Admin/internal facade modules do not resolve in the third-party
    // domain; public facades do.
    for specifier in [
        "clay:configuration",
        "clay:documents",
        "clay:workspace",
        "clay:keybindings",
        "clay:packages",
        "clay:theme",
        "clay:application",
        "clay:editor",
        "clay:shell",
    ] {
        let result = service
            .evaluate_third_party_module(format!(r#"import "{specifier}";"#))
            .await;
        assert!(
            result.is_err(),
            "third-party domain must reject admin facade {specifier}"
        );
    }
    for specifier in ["clay:commands", "clay:decorations", "clay:sdui"] {
        service
            .evaluate_third_party_module(format!(r#"import "{specifier}";"#))
            .await
            .unwrap_or_else(|error| panic!("third-party public facade {specifier}: {error}"));
    }
}

#[tokio::test]
async fn domain_globals_and_module_state_do_not_cross() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    service
        .evaluate_controlled_module("globalThis.__clayDomainProbe = 'trusted';")
        .await
        .expect("trusted global write");
    let third_party = service
        .evaluate_third_party_module(
            "Deno.core.ops.op_clay_runtime_record(String(globalThis.__clayDomainProbe));",
        )
        .await
        .expect("third-party global read");
    assert_eq!(third_party.op_records, vec!["undefined"]);
    service
        .evaluate_third_party_module("globalThis.__clayDomainProbe = 'third-party';")
        .await
        .expect("third-party global write");
    let trusted = service
        .evaluate_controlled_module(
            "Deno.core.ops.op_clay_runtime_record(String(globalThis.__clayDomainProbe));",
        )
        .await
        .expect("trusted global read");
    assert_eq!(trusted.op_records, vec!["trusted"]);
}

#[tokio::test]
async fn third_party_termination_replaces_only_third_party_generation() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
    // Runaway third-party evaluation terminates only its own domain.
    let timed_out = service.evaluate_third_party_module("while (true) {}").await;
    assert!(
        matches!(timed_out, Err(ClayRuntimeError::Timeout)),
        "third-party runaway must time out, got {timed_out:?}"
    );
    // Trusted domain stays responsive without any worker replacement. Two
    // trust domains × their lanes are the baseline.
    let baseline_workers = 2 * crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN as u64;
    let workers_after_timeout = service.workers_started();
    service
        .evaluate_controlled_module("Deno.core.ops.op_clay_runtime_ping();")
        .await
        .expect("trusted runtime remains responsive after third-party termination");
    assert_eq!(workers_after_timeout, baseline_workers);
    assert_eq!(service.workers_started(), baseline_workers);
    // Third-party domain recovers by replacing only its own general-lane worker.
    service
        .evaluate_third_party_module("Deno.core.ops.op_clay_runtime_record('recovered');")
        .await
        .expect("third-party domain restarts after termination");
    assert_eq!(service.workers_started(), baseline_workers + 1);
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "manual Plan 061 before/after resource baseline"]
async fn runtime_resource_baseline_probe() {
    let (rss_before_kib, threads_before) = process_rss_kib_and_threads();
    let startup_started = Instant::now();
    let service = ClayJsRuntimeService::default();
    service
        .evaluate_controlled_module("Deno.core.ops.op_clay_runtime_ping();")
        .await
        .expect("first runtime evaluation");
    let startup_us = startup_started.elapsed().as_micros();
    tokio::time::sleep(Duration::from_millis(25)).await;
    let (rss_after_start_kib, threads_after_start) = process_rss_kib_and_threads();

    let mut warm_evaluation_us = Vec::with_capacity(20);
    for _ in 0..20 {
        let started = Instant::now();
        service
            .evaluate_controlled_module("Deno.core.ops.op_clay_runtime_ping();")
            .await
            .expect("warm runtime evaluation");
        warm_evaluation_us.push(started.elapsed().as_micros());
    }
    warm_evaluation_us.sort_unstable();
    let warm_evaluation_median_us = warm_evaluation_us[warm_evaluation_us.len() / 2];

    let mut package_load_us = Vec::new();
    for specifier in [
        "@clay/rust",
        "@clay/markdown",
        "@clay/git",
        "@clay/theme-gruvbox-material-dark",
    ] {
        let started = Instant::now();
        service
            .evaluate_controlled_module(format!(
                "import {{ loadPackage }} from 'clay:packages'; await loadPackage({specifier:?});"
            ))
            .await
            .unwrap_or_else(|error| panic!("load {specifier}: {error}"));
        package_load_us.push((specifier, started.elapsed().as_micros()));
    }
    let enabled_packages = service
        .test_op_state()
        .package_service()
        .lock()
        .expect("package service mutex poisoned")
        .enabled_records()
        .count();
    tokio::time::sleep(Duration::from_millis(25)).await;
    let (rss_after_packages_kib, threads_after_packages) = process_rss_kib_and_threads();

    let reload_started = Instant::now();
    let candidate = ClayJsRuntimeService::default();
    candidate
        .evaluate_controlled_module("Deno.core.ops.op_clay_runtime_ping();")
        .await
        .expect("candidate runtime evaluation");
    let candidate_reload_us = reload_started.elapsed().as_micros();
    tokio::time::sleep(Duration::from_millis(25)).await;
    let (rss_with_candidate_kib, threads_with_candidate) = process_rss_kib_and_threads();
    drop(candidate);

    // Plan 061 task 4 / Plan 127 P1: analysis invokes route through the two
    // domain runtimes and their lanes; no per-analyzer persistent runtimes
    // exist anymore.
    let (rss_with_max_analysis_kib, threads_with_max_analysis) =
        (rss_with_candidate_kib, threads_with_candidate);

    assert_eq!(enabled_packages, 4);
    assert_eq!(
        service.workers_started(),
        2 * crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN as u64
    );
    eprintln!(
        "PLAN061_RUNTIME_BASELINE rss_before_kib={rss_before_kib} rss_after_start_kib={rss_after_start_kib} rss_after_packages_kib={rss_after_packages_kib} rss_with_candidate_kib={rss_with_candidate_kib} rss_with_max_analysis_kib={rss_with_max_analysis_kib} threads_before={threads_before} threads_after_start={threads_after_start} threads_after_packages={threads_after_packages} threads_with_candidate={threads_with_candidate} threads_with_max_analysis={threads_with_max_analysis} startup_us={startup_us} warm_evaluation_median_us={warm_evaluation_median_us} candidate_reload_us={candidate_reload_us} enabled_packages={enabled_packages} package_load_us={package_load_us:?} main_heap_limit_bytes={JS_RUNTIME_HEAP_LIMIT_BYTES} persistent_workers_started={}",
        service.workers_started()
    );

    // Plan 061 task 13: third-party provider latency + bridge
    // saturation (serial cross-domain dispatches through the completion
    // coordinator).
    let provider_evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@vendor/probe-provider",
            "probeprov",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "probeprov.provider",
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 500, "maxItems": 8 }
                }]
            }),
        ),
        vec![crate::packages::permissions::PackagePermission::CompletionProvider],
        r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async (_request, window) => ({
                  status: "ok",
                  items: [{ label: "probe", insertText: "probe", detail: window.text }]
                })
              }
            });
            "#,
    )
    .await
    .unwrap();
    let coordinator = crate::server::completion::CompletionCoordinator::new();
    service
        .register_completion_providers(&coordinator, 4, &provider_evaluation)
        .unwrap();
    let completion_window = crate::server::completion::CompletionDocumentWindow {
        document_id: 7,
        document_version: 3,
        behavior_version: 5,
        package_prefix: "probeprov".to_string(),
        byte_start: 0,
        byte_end: 2,
        text: "pr".to_string(),
    };
    let mut provider_invoke_us = Vec::with_capacity(20);
    let saturation_started = Instant::now();
    for index in 0..20u32 {
        let started = Instant::now();
        let reply_rx = coordinator
            .schedule_completion(
                "probeprov.provider",
                crate::protocol::CompletionRequest {
                    request_id: u64::from(100 + index),
                    client_id: 2,
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    cursor_byte_offset: 2,
                    replacement_range: crate::protocol::CompletionReplacementRange {
                        byte_start: 2,
                        byte_end: 2,
                    },
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 4,
                    recent_completions: Box::new([]),
                },
                completion_window.clone(),
            )
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), reply_rx)
            .await
            .unwrap()
            .unwrap();
        provider_invoke_us.push(started.elapsed().as_micros());
    }
    provider_invoke_us.sort_unstable();
    let provider_invoke_median_us = provider_invoke_us[provider_invoke_us.len() / 2];
    let bridge_saturation_20_serial_us = saturation_started.elapsed().as_micros();

    // Third-party recovery: poison the domain with a busy-loop, then time
    // the first successful provider answer (replace + replay + invoke).
    let recovery_service = ClayJsRuntimeService::with_timeout(Duration::from_millis(50));
    let recovery_root = config_fixture("third-party-recovery-probe").join("recoverd");
    write_loadable_package(
        &recovery_root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        serverRegisterCompletionProvider({
          module: {
            provideCompletion: async () => ({
              status: "ok",
              items: [{ label: "recovered", insertText: "recovered" }]
            })
          }
        });
        export default function load() {}
        "#,
    );
    let recovery_json = test_package_json(
        "@vendor/recoverd",
        "recoverd",
        &["completion-provider"],
        serde_json::json!({
            "completionProviders": [{
                "id": "recoverd.provider",
                "triggerCharacters": ["."],
                "budgets": { "timeoutMs": 50, "maxItems": 8 }
            }]
        }),
    );
    let recovery_permissions =
        vec![crate::packages::permissions::PackagePermission::CompletionProvider];
    ensure_synthetic_package_enabled(
        &recovery_service,
        recovery_json.clone(),
        recovery_permissions.clone(),
        None,
    );
    recovery_service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            "clay://packages/@vendor/recoverd/dist/load.js",
            recovery_root.join("dist/load.js"),
            recovery_root.clone(),
            Some("@vendor/recoverd"),
        );
    let recovery_evaluation = evaluate_as_package(
        &recovery_service,
        recovery_json.clone(),
        recovery_permissions.clone(),
        r#"const m = await import("clay://packages/@vendor/recoverd/dist/load.js"); await m.default();"#,
    )
    .await
    .unwrap();
    let recovery_coordinator = crate::server::completion::CompletionCoordinator::new();
    recovery_service
        .register_completion_providers(&recovery_coordinator, 4, &recovery_evaluation)
        .unwrap();
    let _ = evaluate_as_package(
        &recovery_service,
        recovery_json,
        recovery_permissions,
        "for (;;) {}",
    )
    .await;
    let recovery_started = Instant::now();
    let reply_rx = recovery_coordinator
        .schedule_completion(
            "recoverd.provider",
            crate::protocol::CompletionRequest {
                request_id: 200,
                client_id: 2,
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                cursor_byte_offset: 2,
                replacement_range: crate::protocol::CompletionReplacementRange {
                    byte_start: 2,
                    byte_end: 2,
                },
                trigger: crate::protocol::CompletionTrigger::Manual,
                provider_generation: 4,
                recent_completions: Box::new([]),
            },
            crate::server::completion::CompletionDocumentWindow {
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                package_prefix: "recoverd".to_string(),
                byte_start: 0,
                byte_end: 2,
                text: "re".to_string(),
            },
        )
        .unwrap();
    let recovered = tokio::time::timeout(Duration::from_secs(3), reply_rx)
        .await
        .unwrap()
        .unwrap();
    let third_party_recovery_us = recovery_started.elapsed().as_micros();
    assert_eq!(recovered.items[0].label, "recovered");
    let _ = fs::remove_dir_all(recovery_root.parent().unwrap());

    eprintln!(
        "PLAN061_CROSS_DOMAIN provider_invoke_median_us={provider_invoke_median_us} bridge_saturation_20_serial_us={bridge_saturation_20_serial_us} third_party_recovery_us={third_party_recovery_us}"
    );
}
