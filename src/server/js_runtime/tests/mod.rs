//! Stay-in-crate unit tests for the Clay JS runtime service: worker lifecycle,
//! lane isolation, configuration/init-js loading, package + module-loader
//! confinement, language/completion/parse facades, markdown adapters, themes,
//! icon packs, and agent host wiring.
use std::{
    fs,
    path::Path,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tokio::sync::Mutex;

use deno_core::{
    ModuleLoadOptions, ModuleLoadResponse, ModuleLoader, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind,
};

use super::evaluation::evaluate_loaded_module;
use super::source::{CONTROLLED_MAIN_SPECIFIER, ClayModuleLoader};
use super::worker::{
    create_js_runtime, prepare_runtime_entry, run_bounded_evaluation, start_runtime_worker,
};
use super::{
    ClayJsRuntimeService, ClayRuntimeError, ClayRuntimeEvaluation, PackageLoadEntryAllowlist,
    RuntimeEntry, RuntimeLane,
};
use crate::perf::budgets::{
    BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, JS_RUNTIME_EVALUATION_TIMEOUT_MS,
    JS_RUNTIME_HEAP_LIMIT_BYTES,
};
use crate::protocol::{
    BehaviorVersion, DiagnosticSeverity, EnterRule, ParseByteRange, ParseEditNotification,
    ParsePolicy, ParseWindowSnapshot,
};

/// Raw-runtime tests that exercise `loadPackage` need a third-party
/// worker sharing the test op state's package authority and load-entry
/// allowlist for the cross-domain bridge (Plan 061 task 12). The returned
/// worker must outlive the evaluations it serves.
fn wire_test_third_party_bridge(
    op_state: &Arc<crate::server::ops::ClayOpState>,
) -> Arc<super::RuntimeWorker> {
    let worker = start_runtime_worker(
        Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        JS_RUNTIME_HEAP_LIMIT_BYTES,
        crate::packages::bundled::RuntimeDomain::ThirdParty,
        op_state.package_service_arc(),
        op_state.load_entry_allowlist(),
        RuntimeLane::General,
    );
    op_state.set_third_party_commands(worker.sender.clone());
    worker
}
use crate::server::configuration::ConfigurationRuntime;
use crate::server::parse_coordinator::{ParseCoordinator, ParseScheduleRequest};
use crate::server::workspace::WorkspaceState;

fn init_git_repo(root: &Path) {
    git(root, ["init", "-b", "main"]);
    git(root, ["config", "user.email", "clay@example.invalid"]);
    git(root, ["config", "user.name", "Clay Test"]);
}

fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS", "")
        .status()
        .unwrap();
    assert!(status.success(), "git command failed: {args:?}");
}

fn config_fixture(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("clay-{name}-{suffix}"));
    fs::create_dir_all(&root).expect("create configuration fixture root");
    root
}

/// Build a synthetic package manifest for provenance tests.
fn test_package_json(
    name: &str,
    api_prefix: &str,
    permissions: &[&str],
    contributions: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "permissions": permissions,
            "modes": [api_prefix],
            "docs": "./docs/index.md",
            "contributions": contributions,
        }
    })
}

/// Host-side install + authorize + enable of a synthetic package, then
/// evaluate `source` with that package's host-stamped provenance. This is
/// the same flow production package adoption uses: authority comes from
/// the enabled set and authorization record, never caller manifests.
async fn evaluate_as_package(
    service: &ClayJsRuntimeService,
    package_json: serde_json::Value,
    approved: Vec<crate::packages::permissions::PackagePermission>,
    source: &str,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    evaluate_as_package_with_ls_grant(service, package_json, approved, None, source).await
}

/// Same as [`evaluate_as_package`], additionally recording a
/// language-server grant (which approves the `language-server`
/// capability) for analyzer/session provenance tests.
/// Host-side adoption flow for synthetic test packages: install, approve
/// the exact capability set, optionally grant a language-server
/// contribution, enable. Idempotent per package name/version.
fn ensure_synthetic_package_enabled(
    service: &ClayJsRuntimeService,
    package_json: serde_json::Value,
    approved: Vec<crate::packages::permissions::PackagePermission>,
    language_server_grant: Option<(&str, std::path::PathBuf)>,
) -> crate::packages::record::PackageRecord {
    let record = crate::packages::record::assemble_package_record(&package_json)
        .expect("synthetic package record must assemble");
    let root = config_fixture("package-provenance");
    let op_state = service.test_op_state();
    let mut locked = op_state
        .package_service()
        .lock()
        .expect("package service mutex poisoned");
    if locked
        .enabled_record(&record.manifest.name, &record.manifest.version)
        .is_none()
    {
        locked
            .install_from_value_at_root_with_spec(package_json, root, "local:provenance-test")
            .expect("synthetic package installs");
        locked
            .authorize_package(
                &record.manifest.name,
                approved.clone(),
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .expect("synthetic package authorizes");
        // LS capability must be granted before enable: declared
        // capabilities require a current grant at enable time.
        if let Some((contribution, executable)) = &language_server_grant {
            locked
                .authorize_language_server(
                    &record.manifest.name,
                    contribution,
                    executable.clone(),
                    vec![1],
                    "test",
                )
                .expect("language-server grant authorizes");
        }
        // Pre-execution adoption gate: synthetic third-party packages
        // need an exact durable approval before enable.
        locked
            .approve_package(&record.manifest.name, "test")
            .expect("synthetic package approves");
        locked
            .enable(&record.manifest.name)
            .expect("synthetic package enables");
    }
    record
}

async fn evaluate_as_package_with_ls_grant(
    service: &ClayJsRuntimeService,
    package_json: serde_json::Value,
    approved: Vec<crate::packages::permissions::PackagePermission>,
    language_server_grant: Option<(&str, std::path::PathBuf)>,
    source: &str,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    let record =
        ensure_synthetic_package_enabled(service, package_json, approved, language_server_grant);
    // Evaluate in the runtime domain that owns the package (Plan 061 task
    // 7): third-party packages run their callbacks in the third-party
    // worker, which is where provider dispatch sends their commands.
    let domain = service
        .test_op_state()
        .package_service()
        .lock()
        .expect("package service mutex poisoned")
        .enabled_record(&record.manifest.name, &record.manifest.version)
        .map(|enabled| enabled.runtime_domain)
        .unwrap_or(crate::packages::bundled::RuntimeDomain::ThirdParty);
    service
        .evaluate_entry_as_package(
            domain,
            &record,
            RuntimeEntry::ControlledSource(source.to_string()),
            "runtime.evaluate_as_package",
        )
        .await
}

/// Same as [`evaluate_as_package`] but stamps the synthetic package into
/// the trusted domain: for tests exercising trusted-only ops (mode
/// activation, package loading) that third-party runtimes cannot reach.
async fn evaluate_as_trusted_package(
    service: &ClayJsRuntimeService,
    package_json: serde_json::Value,
    approved: Vec<crate::packages::permissions::PackagePermission>,
    source: &str,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    let record = ensure_synthetic_package_enabled(service, package_json, approved, None);
    service
        .test_op_state()
        .package_service()
        .lock()
        .expect("package service mutex poisoned")
        .force_enabled_runtime_domain_for_test(
            &record.manifest.name,
            &record.manifest.version,
            crate::packages::bundled::RuntimeDomain::Trusted,
        );
    service
        .evaluate_entry_as_package(
            crate::packages::bundled::RuntimeDomain::Trusted,
            &record,
            RuntimeEntry::ControlledSource(source.to_string()),
            "runtime.evaluate_as_trusted_package",
        )
        .await
}

// Test suites (see each file for its scope).
mod agent_lane_host_wiring;
mod coding_agent_and_launcher;
mod config_fixture_workflows;
mod configuration_keybindings;
mod configuration_runtime;
mod decoration_and_diagnostics_facades;
mod document_and_git_facades;
mod editor_control;
mod editor_layout_and_design_system;
mod facades_and_modes;
mod lanes_and_queues;
mod language_facades;
mod language_packages;
mod language_registration_and_primitives;
mod load_package_and_module_loader;
mod load_package_markdown_defaults;
mod markdown_windowed_adapter;
mod package_adoption;
mod plan112_icon_packs;
mod runtime_errors_and_authorization;
mod runtime_limits;
mod runtime_themes_and_typography;
mod runtime_third_party;
mod themes_and_appearance;
fn evaluation_contains(evaluation: &ClayRuntimeEvaluation, needle: &str) -> bool {
    evaluation
        .op_records
        .iter()
        .any(|record| record.contains(needle))
}

#[cfg(target_os = "linux")]
fn process_rss_kib_and_threads() -> (u64, usize) {
    let status = fs::read_to_string("/proc/self/status").expect("read process status");
    let rss_kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse().ok())
        .expect("parse VmRSS from process status");
    let threads = fs::read_dir("/proc/self/task")
        .expect("read process task directory")
        .count();
    (rss_kib, threads)
}

/// Helper: call the raw resolver op from a controlled module. The public
/// `loadPackage` facade is wired in Phase 18.6 task 5; these op-level
/// tests exercise the resolver directly so the security boundary is
/// covered before the facade lands.
async fn resolve_by_specifier(specifier: &str) -> Result<String, String> {
    let source = format!(
        r#"
        const result = Deno.core.ops.op_clay_packages_load_package_by_specifier(
          JSON.stringify({{ specifier: {specifier:?} }})
        );
        globalThis.__clay_result = result;
        "#
    );
    match ClayJsRuntimeService::default()
        .evaluate_controlled_module(source)
        .await
    {
        Ok(_) => Ok("ok".to_string()),
        Err(error) => Err(error.to_string()),
    }
}

fn loadable_package_fixture(name: &str, api_prefix: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "capabilities": [],
            "modes": [],
            "docs": "./docs/index.md",
            "apiDependencies": [],
            "performance": {
                "estimatedManifestBytes": 256,
                "hotPathPolicy": "no hot-path JS on keypress/paint"
            },
            "contributions": {}
        }
    })
}

fn write_loadable_package(root: &Path, load_source: &str) {
    fs::create_dir_all(root.join("dist")).expect("create package dist directory");
    fs::create_dir_all(root.join("docs")).expect("create package docs directory");
    fs::write(root.join("dist/index.js"), "export {};\n").expect("write package entry");
    fs::write(root.join("dist/load.js"), load_source).expect("write package loadEntry");
    fs::write(
        root.join("dist/helper.js"),
        "Deno.core.ops.op_clay_runtime_record(\"helper loaded\"); export {};\n",
    )
    .expect("write package helper");
    fs::write(root.join("docs/index.md"), "# Fixture\n").expect("write package docs");
}

async fn evaluate_with_seeded_package(
    specifier: &str,
    package_name: &str,
    api_prefix: &str,
    package_root: PathBuf,
    load_source: &str,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    evaluate_with_seeded_package_adoption(
        specifier,
        package_name,
        api_prefix,
        package_root,
        load_source,
        true,
    )
    .await
}

async fn evaluate_with_seeded_package_adoption(
    specifier: &str,
    package_name: &str,
    api_prefix: &str,
    package_root: PathBuf,
    load_source: &str,
    adopt: bool,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    write_loadable_package(&package_root, load_source);
    let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
        Arc::new(Mutex::new(WorkspaceState::new())),
        1,
    ));
    let package_json = loadable_package_fixture(package_name, api_prefix);
    {
        let mut service = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        service
            .install_from_value_at_root_with_spec(package_json, package_root, specifier)
            .expect("seed package install succeeds");
        service
            .authorize_package(
                package_name,
                Vec::new(),
                crate::packages::authorization::RuntimeProfile::NativeTrust,
                "test-user",
            )
            .expect("seed package authorization succeeds");
        if adopt {
            service
                .approve_package(package_name, "test")
                .expect("seed package adoption approval succeeds");
        }
    }
    // Third-party packages load through the cross-domain bridge; the
    // worker must outlive the evaluations below.
    let _third_party_worker = wire_test_third_party_bridge(&op_state);
    let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER).unwrap();
    let loader = Rc::new(ClayModuleLoader::new(
        main_specifier,
        None,
        None,
        op_state.load_entry_allowlist(),
        crate::packages::bundled::RuntimeDomain::Trusted,
    ));
    let (mut runtime, heap_limit_hit) = create_js_runtime(
        Arc::clone(&op_state),
        Rc::clone(&loader),
        JS_RUNTIME_HEAP_LIMIT_BYTES,
        crate::packages::bundled::RuntimeDomain::Trusted,
    );
    let source = format!(
        r#"
        import {{ loadPackage }} from "clay:packages";
        await loadPackage({specifier:?});
        "#
    );
    let loaded = prepare_runtime_entry(RuntimeEntry::ControlledSource(source), 1).unwrap();
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(
        &mut runtime,
        &op_state,
        loaded,
        Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        true,
        &heap_limit_hit,
    )
    .await
}

/// Plan 035 task 8: prove the one-line `init.js` default loads an installed,
/// authorized, *user-installed* (non-`@clay/*`) package the same way it
/// loads `@clay/markdown`. Mirrors [`evaluate_with_seeded_package`] but
/// evaluates a real `~/.clay/init.js`-shaped config root instead of
/// a controlled module source, so the loadEntry import + default-export
/// invocation is exercised through the configuration runtime path.
async fn evaluate_init_js_with_seeded_package(
    config_root: PathBuf,
    specifier: &str,
    package_name: &str,
    api_prefix: &str,
    package_root: PathBuf,
    load_source: &str,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    write_loadable_package(&package_root, load_source);
    let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
        Arc::new(Mutex::new(WorkspaceState::new())),
        1,
    ));
    let _third_party_worker = wire_test_third_party_bridge(&op_state);
    let package_json = loadable_package_fixture(package_name, api_prefix);
    {
        let mut service = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        service
            .install_from_value_at_root_with_spec(package_json, package_root, specifier)
            .expect("seed package install succeeds");
        service
            .authorize_package(
                package_name,
                Vec::new(),
                crate::packages::authorization::RuntimeProfile::NativeTrust,
                "test-user",
            )
            .expect("seed package authorization succeeds");
        service
            .approve_package(package_name, "test")
            .expect("seed package adoption approval succeeds");
    }
    let loaded = prepare_runtime_entry(RuntimeEntry::ConfigurationRoot(config_root), 1).unwrap();
    let loader = Rc::new(ClayModuleLoader::new(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
        op_state.load_entry_allowlist(),
        crate::packages::bundled::RuntimeDomain::Trusted,
    ));
    let (mut runtime, heap_limit_hit) = create_js_runtime(
        Arc::clone(&op_state),
        Rc::clone(&loader),
        JS_RUNTIME_HEAP_LIMIT_BYTES,
        crate::packages::bundled::RuntimeDomain::Trusted,
    );
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(
        &mut runtime,
        &op_state,
        loaded,
        Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        true,
        &heap_limit_hit,
    )
    .await
}

/// Build an isolated `ClayModuleLoader` with a manually-populated allowlist
/// (no resolver op, no real runtime) so the resolve/load gate is tested in
/// isolation. `configuration` mirrors the runtime's config-root branch.
fn loader_with_allowlist(
    entries: &[(&str, PathBuf, PathBuf)],
    configuration: Option<Arc<ConfigurationRuntime>>,
) -> ClayModuleLoader {
    let allowlist = Arc::new(PackageLoadEntryAllowlist::default());
    for (specifier, path, package_root) in entries {
        allowlist.record(specifier, path.clone(), package_root.clone());
    }
    let main_specifier = ModuleSpecifier::parse("clay://runtime/main.js").unwrap();
    ClayModuleLoader::new(
        main_specifier,
        None,
        configuration,
        allowlist,
        crate::packages::bundled::RuntimeDomain::Trusted,
    )
}

fn default_load_options() -> ModuleLoadOptions {
    ModuleLoadOptions {
        is_dynamic_import: false,
        is_synchronous: false,
        requested_module_type: RequestedModuleType::None,
    }
}

fn collect_kinds<'a>(component: &'a crate::shell::PackageUiComponentTree, out: &mut Vec<&'a str>) {
    out.push(component.kind.as_str());
    for item in &component.items {
        let _ = item;
    }
    for child in &component.children {
        collect_kinds(child, out);
    }
}

/// Registration declarations queued by a hostless runtime (unit harness) live
/// on that service's own lane op state (Plan 130 A1), so a test sees exactly
/// its own declarations — never another test's.
fn drain_all_pending_registrations(
    service: &ClayJsRuntimeService,
) -> Vec<crate::server::agent::PackageRegistration> {
    service.test_op_state().take_pending_agent_registrations()
}

fn plan112_icon_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/configuration/plan112-icons/{name}"))
}

/// Plan 127 P1 lane tests: one completion request/window pair for a provider
/// registered under `package_prefix`.
fn lane_completion_input(
    package_prefix: &str,
    request_id: u64,
    text: &str,
) -> (
    crate::protocol::CompletionRequest,
    crate::server::completion::CompletionDocumentWindow,
) {
    (
        crate::protocol::CompletionRequest {
            request_id,
            client_id: 2,
            document_id: 7,
            document_version: 3,
            behavior_version: 1,
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
            behavior_version: 1,
            package_prefix: package_prefix.to_string(),
            byte_start: 0,
            byte_end: 2,
            text: text.to_string(),
        },
    )
}

/// Plan 127 P1: seed an on-disk package whose load entry registers a
/// MODULE-BASED completion provider — `moduleSpecifier` resolved by
/// `import.meta.resolve` and validated against the shared allowlist — so the
/// provider can be served from the owning domain's latency lane and
/// rematerialized there by module import (no evaluation replay). Returns the
/// load evaluation and the registration.
async fn module_backed_completion_provider(
    service: &ClayJsRuntimeService,
    package_name: &str,
    api_prefix: &str,
    provider_module_source: &str,
) -> (
    ClayRuntimeEvaluation,
    crate::server::completion::JsCompletionProviderRegistration,
) {
    let root = config_fixture("lane-module-provider").join(api_prefix);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        import * as completionModule from "./completion.js";
        export default function load() {
          serverRegisterCompletionProvider({
            module: completionModule,
            moduleSpecifier: import.meta.resolve("./completion.js"),
            exportName: "provideCompletion"
          });
        }
        "#,
    );
    fs::write(root.join("dist/completion.js"), provider_module_source)
        .expect("write module-backed provider module");
    let package_json = test_package_json(
        package_name,
        api_prefix,
        &["completion-provider"],
        serde_json::json!({
            "completionProviders": [{
                "id": format!("{api_prefix}.provider"),
                "triggerCharacters": ["."],
                "budgets": { "timeoutMs": 2_000, "maxItems": 8 }
            }]
        }),
    );
    let approved = vec![crate::packages::permissions::PackagePermission::CompletionProvider];
    ensure_synthetic_package_enabled(service, package_json.clone(), approved.clone(), None);
    let load_specifier = format!("clay://packages/{package_name}/dist/load.js");
    service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            &load_specifier,
            root.join("dist/load.js"),
            root.clone(),
            Some(package_name),
        );
    let evaluation = evaluate_as_package(
        service,
        package_json,
        approved,
        &format!("const m = await import({load_specifier:?}); await m.default();"),
    )
    .await
    .expect("module-backed provider fixture load");
    // The op state's harvest is cumulative across the service, so select the
    // registration this fixture's package owns rather than the first entry.
    let registration = evaluation
        .js_completion_providers
        .iter()
        .find(|registration| registration.package.manifest.name == package_name)
        .cloned()
        .expect("fixture must register a JS completion provider");
    assert!(
        registration.module_specifier.is_some(),
        "fixture registration must carry the resolved module specifier"
    );
    (evaluation, registration)
}

/// Plan 127 task 6 fixture: two third-party packages — one registering an
/// inline completion provider (served by the domain's general lane from the
/// global handler registry) and one registering a module-backed provider
/// (served by the latency lane through module materialization) — so one test
/// can aim commands at both lanes with real registrations. One package cannot
/// own both shapes: a registration call claims every provider the manifest
/// declares, and the lane follows the call's `moduleSpecifier`.
async fn lane_provider_pair(
    service: &ClayJsRuntimeService,
    base: &str,
) -> [crate::server::completion::JsCompletionProviderRegistration; 2] {
    let approved = vec![crate::packages::permissions::PackagePermission::CompletionProvider];

    // General lane: inline handler, registered into the isolate's global
    // handler registry (no module specifier).
    let general_name = format!("@vendor/{base}");
    let root = config_fixture("lane-inline-provider").join(base);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        export default function load() {
          serverRegisterCompletionProvider({
            module: {
              provideCompletion: async () => ({
                status: "ok",
                items: [{ label: "inline", insertText: "inline" }]
              })
            }
          });
        }
        "#,
    );
    let package_json = test_package_json(
        &general_name,
        base,
        &["completion-provider"],
        serde_json::json!({
            "completionProviders": [{
                "id": format!("{base}.provider"),
                "triggerCharacters": ["."],
                "budgets": { "timeoutMs": 2_000, "maxItems": 8 }
            }]
        }),
    );
    ensure_synthetic_package_enabled(service, package_json.clone(), approved.clone(), None);
    let load_specifier = format!("clay://packages/{general_name}/dist/load.js");
    service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            &load_specifier,
            root.join("dist/load.js"),
            root.clone(),
            Some(&general_name),
        );
    let evaluation = evaluate_as_package(
        service,
        package_json,
        approved.clone(),
        &format!("const m = await import({load_specifier:?}); await m.default();"),
    )
    .await
    .expect("inline provider fixture load");
    let inline = evaluation
        .js_completion_providers
        .iter()
        .find(|registration| registration.package.manifest.name == general_name)
        .cloned()
        .expect("inline fixture must register a provider");

    // Latency lane: the module-backed registration shape the analyzer
    // precedent and Plan 127 P1 introduced.
    let latency_prefix = format!("{base}lat");
    let (_, module_backed) = module_backed_completion_provider(
        service,
        &format!("@vendor/{latency_prefix}"),
        &latency_prefix,
        r#"
        export function provideCompletion(_request, _window) {
          return { status: "ok", items: [{ label: "module", insertText: "module" }] };
        }
        "#,
    )
    .await;

    assert!(inline.module_specifier.is_none());
    assert!(module_backed.module_specifier.is_some());
    [inline, module_backed]
}

/// Plan 127 P2: provider module whose first invocation holds the serving lane
/// for 400 ms. Later invocations return immediately and echo the window text,
/// so a test can identify which request produced which result.
const PLAN127_HOLDING_PROVIDER: &str = r#"
export async function provideCompletion(_request, window) {
  if (!globalThis.__plan127Hold) {
    globalThis.__plan127Hold = true;
    const until = Date.now() + 400;
    while (Date.now() < until) {}
  }
  return {
    status: "ok",
    items: [{ label: window.text, insertText: "q" }]
  };
}
"#;

/// Spawn one completion invocation per document id, all issued before any of
/// them can finish: the first invocation holds the lane, the rest pile up
/// behind it. Results come back in spawn order.
async fn plan127_completion_burst(
    service: &ClayJsRuntimeService,
    registration: &crate::server::completion::JsCompletionProviderRegistration,
    prefix: &str,
    document_ids: &[u64],
) -> Vec<Result<crate::protocol::CompletionResultSet, ClayRuntimeError>> {
    let mut tasks = Vec::with_capacity(document_ids.len());
    for (index, document_id) in document_ids.iter().enumerate() {
        let service = service.clone();
        let registration = registration.clone();
        let label = format!("q{}", index + 1);
        let (mut request, mut window) = lane_completion_input(prefix, index as u64 + 1, &label);
        request.document_id = *document_id;
        window.document_id = *document_id;
        tasks.push(tokio::spawn(async move {
            service
                .invoke_completion_provider(registration, request, window)
                .await
        }));
    }
    let mut outcomes = Vec::with_capacity(tasks.len());
    for task in tasks {
        outcomes.push(task.await.expect("completion burst task must join"));
    }
    outcomes
}

fn plan127_ok_labels(
    outcomes: &[Result<crate::protocol::CompletionResultSet, ClayRuntimeError>],
) -> Vec<String> {
    outcomes
        .iter()
        .filter_map(|outcome| outcome.as_ref().ok())
        .map(|result| result.items[0].label.clone())
        .collect()
}

fn plan127_superseded_count(
    outcomes: &[Result<crate::protocol::CompletionResultSet, ClayRuntimeError>],
) -> u64 {
    outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Err(ClayRuntimeError::Superseded)))
        .count() as u64
}
