// Auto-extracted from js_runtime.rs (Plan 090 task 3). Private submodule: eval family.
use std::time::Duration;
use std::{rc::Rc, sync::Arc};

use deno_core::{JsRuntime, ModuleSpecifier};

use crate::protocol::{IncrementalParseUpdate, ParseEditNotification, RuntimeDiagnostic};
use crate::server::completion::CompletionProviderError;
use crate::server::configuration::ConfigurationRuntime;
use crate::server::ops::ClayOpState;

use super::error::{ClayRuntimeError, ClayRuntimeEvaluation, DocumentAnalysisInvocation};
use super::source::ClayModuleLoader;
use super::validation::{
    completion_request_json, completion_result_from_json, completion_window_json,
    document_analysis_event_json, language_intelligence_request_json,
    language_intelligence_result_from_json, language_intelligence_window_json,
    parse_notification_json, parse_update_json,
};
use super::worker::{LoadedRuntimeEntry, harvest_op_state_evaluation};

pub(super) async fn evaluate_loaded_module(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loaded_configuration: LoadedRuntimeEntry,
    timeout: Duration,
    use_main_module: bool,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    if let Some(configuration) = &loaded_configuration.configuration {
        runtime
            .op_state()
            .borrow_mut()
            .put(Arc::clone(configuration));
    }
    let terminate_handle = runtime.v8_isolate().thread_safe_handle();
    let timer = TerminationTimer::start(timeout, terminate_handle);

    let evaluation_result: Result<ClayRuntimeEvaluation, ClayRuntimeError> = async {
        let module_id = if use_main_module {
            if let Some(source) = loaded_configuration.main_source {
                runtime
                    .load_main_es_module_from_code(&loaded_configuration.main_specifier, source)
                    .await
            } else {
                runtime
                    .load_main_es_module(&loaded_configuration.main_specifier)
                    .await
            }
        } else if let Some(source) = loaded_configuration.main_source {
            runtime
                .load_side_es_module_from_code(&loaded_configuration.main_specifier, source)
                .await
        } else {
            runtime
                .load_side_es_module(&loaded_configuration.main_specifier)
                .await
        }
        .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
        let result = runtime.mod_evaluate(module_id);
        runtime
            .run_event_loop(Default::default())
            .await
            .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
        result
            .await
            .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
        // Phase 20.6: apply persisted user preferences (`preferences.json`,
        // source `ui-session`) AFTER init.js so the documented precedence
        // holds — package/canonical defaults < init.js < UI session. Validation
        // reuses the same Rust functions as the `setTheme`/`setAppearance`/
        // `setTypography` ops so no bounds/authority logic is duplicated. A
        // corrupted preference field is dropped with a diagnostic record so a
        // bad file never breaks startup.
        if let Some(configuration) = &loaded_configuration.configuration {
            apply_persisted_preferences(op_state, configuration);
        }
        let configuration_diagnostics = loaded_configuration
            .configuration
            .as_ref()
            .map(|configuration| {
                configuration
                    .take_module_errors()
                    .into_iter()
                    .map(|error| {
                        RuntimeDiagnostic::warning(
                            "configuration.module_failed",
                            format!(
                                "Optional configuration module {} failed: {}",
                                error.path, error.message
                            ),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut evaluation = harvest_op_state_evaluation(op_state);
        evaluation.configuration_diagnostics = configuration_diagnostics;
        Ok(evaluation)
    }
    .await;

    if timer.did_fire() {
        let _ = runtime.v8_isolate().cancel_terminate_execution();
        return Err(ClayRuntimeError::Timeout);
    }
    if heap_limit_hit.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = runtime.v8_isolate().cancel_terminate_execution();
        return Err(ClayRuntimeError::HeapLimit);
    }
    evaluation_result
}

/// Apply persisted UI-session preferences to `op_state` after init.js has run.
/// Theme is applied first (marks an explicit theme active), then appearance
/// (no canonical re-resolve over the explicit theme), then typography. Absent
/// fields leave init.js / canonical defaults in place. Failures (corrupted
/// field, unresolvable theme package) are recorded as diagnostics and skipped.
pub(super) fn apply_persisted_preferences(
    op_state: &Arc<ClayOpState>,
    configuration: &ConfigurationRuntime,
) {
    let prefs = configuration.load_preferences();
    for diagnostic in &prefs.diagnostics {
        op_state.record(format!("preferences: {diagnostic}"));
    }
    if let Some(specifier) = &prefs.theme
        && let Err(error) = crate::server::ops::theme::apply_theme(op_state, specifier)
    {
        op_state.record(format!(
            "preferences: theme `{specifier}` rejected: {error}"
        ));
    }
    if let Some(appearance) = prefs.appearance {
        crate::server::ops::theme::apply_appearance(op_state, appearance, true);
    }
    if let Some(typography_value) = &prefs.typography {
        match serde_json::to_string(typography_value) {
            Ok(json) => {
                if let Err(error) =
                    crate::server::ops::typography::apply_typography(op_state, &json)
                {
                    op_state.record(format!("preferences: typography rejected: {error}"));
                }
            }
            Err(error) => {
                op_state.record(format!(
                    "preferences: typography serialization failed: {error}"
                ));
            }
        }
    }
}

pub(super) async fn evaluate_js_parse_handler(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
    notification: ParseEditNotification,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
    let source = format!(
        r#"
const registry = globalThis.__clayParseHandlers ?? Object.create(null);
const handler = registry[{token:?}];
if (typeof handler !== "function") {{
  throw new Error("parse.handler_missing: registered parse handler is unavailable");
}}
const notification = {notification};
const update = await handler(notification);
Deno.core.ops.op_clay_parse_store_update(JSON.stringify(update ?? null));
"#,
        token = registration.token,
        notification = parse_notification_json(&notification),
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/parse-{}.js",
            registration.token.replace(':', "-")
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let update_json = op_state.take_parse_update_json().ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: handler produced no update".to_string())
    })?;
    let mut update = parse_update_json(&update_json, registration, notification)?;
    if update.folding_update.is_none() {
        update.folding_update = op_state.published_folding_set();
    }
    Ok(update)
}

#[expect(
    clippy::too_many_arguments,
    reason = "document analyzer invocation keeps runtime, registration, event, timeout, and heap containment explicit"
)]
pub(super) async fn evaluate_js_document_analyzer(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::document_analysis::JsDocumentAnalyzerRegistration,
    event: crate::server::document_analysis::DocumentAnalysisEvent,
    invocation_id: u64,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<DocumentAnalysisInvocation, ClayRuntimeError> {
    let module_specifier = serde_json::to_string(&registration.module_specifier)
        .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
    let export_name = serde_json::to_string(&registration.export_name)
        .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
    let event_json = document_analysis_event_json(registration, &event);
    let source = format!(
        r#"
import * as analyzerModule from {module_specifier};
const handler = analyzerModule[{export_name}];
if (typeof handler !== "function") {{
  throw new Error("analysis.handler_missing: registered analyzer export is unavailable");
}}
const result = await handler({event_json});
Deno.core.ops.op_clay_runtime_record(JSON.stringify(result ?? null));
"#,
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/document-analysis-{}-{invocation_id}.js",
            registration.id.replace([':', '/'], "-")
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    let evaluation =
        evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let response_json = evaluation
        .op_records
        .last()
        .map(String::as_str)
        .unwrap_or("null");
    let response = match &event {
        crate::server::document_analysis::DocumentAnalysisEvent::Completion { request, .. } => {
            crate::server::document_analysis::DocumentAnalysisResponse::Completion(
                completion_result_from_json(response_json, &registration.package, request)
                    .map_err(|error| CompletionProviderError::ProviderFailed(error.to_string())),
            )
        }
        crate::server::document_analysis::DocumentAnalysisEvent::LanguageIntelligence {
            request,
            ..
        } => crate::server::document_analysis::DocumentAnalysisResponse::LanguageIntelligence(
            language_intelligence_result_from_json(
                response_json,
                &registration.package,
                request,
            )
            .map_err(|error| {
                crate::server::language_intelligence::LanguageIntelligenceProviderError::ProviderFailed(
                    error.to_string(),
                )
            }),
        ),
        _ => crate::server::document_analysis::DocumentAnalysisResponse::None,
    };
    Ok(DocumentAnalysisInvocation {
        decorations: evaluation.published_decoration_set,
        diagnostics: evaluation.published_diagnostic_set,
        response,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "completion JS bridge needs runtime, registration, request, window, timeout, and heap state together"
)]
pub(super) async fn evaluate_js_completion_provider(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::completion::JsCompletionProviderRegistration,
    request: crate::protocol::CompletionRequest,
    window: crate::server::completion::CompletionDocumentWindow,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<crate::protocol::CompletionResultSet, ClayRuntimeError> {
    let source = format!(
        r#"
const registry = globalThis.__clayCompletionHandlers ?? Object.create(null);
const handler = registry[{token:?}];
if (typeof handler !== "function") {{
  throw new Error("completion.handler_missing: registered completion handler is unavailable");
}}
const result = await handler({request}, {window});
Deno.core.ops.op_clay_completion_store_result(JSON.stringify(result ?? null));
"#,
        token = registration.token,
        request = completion_request_json(&request),
        window = completion_window_json(&window),
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/completion-{}-{}.js",
            registration.token.replace(':', "-"),
            request.request_id,
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let result_json = op_state.take_completion_result_json().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "completion.invalid_result: handler produced no result".to_string(),
        )
    })?;
    completion_result_from_json(&result_json, &registration.package, &request)
}

#[allow(
    clippy::too_many_arguments,
    reason = "language-intelligence JS bridge mirrors the parse-handler worker path and needs request+window inputs together"
)]
pub(super) async fn evaluate_js_language_intelligence_provider(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration,
    request: crate::protocol::LanguageIntelligenceRequest,
    window: crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<crate::protocol::LanguageIntelligenceResult, ClayRuntimeError> {
    let source = format!(
        r#"
const registry = globalThis.__clayLanguageIntelligenceHandlers ?? Object.create(null);
const handler = registry[{token:?}];
if (typeof handler !== "function") {{
  throw new Error("language.handler_missing: registered language-intelligence handler is unavailable");
}}
const request = {request};
const window = {window};
const result = await handler(request, window);
Deno.core.ops.op_clay_language_store_intelligence_result(JSON.stringify(result ?? null));
"#,
        token = registration.token,
        request = language_intelligence_request_json(&request),
        window = language_intelligence_window_json(&window),
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/language-intelligence-{}-{}.js",
            registration.token.replace(':', "-"),
            request.request_id,
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let result_json = op_state
        .take_language_intelligence_result_json()
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "language.invalid_result: handler produced no result".to_string(),
            )
        })?;
    language_intelligence_result_from_json(&result_json, &registration.package, &request)
}

/// Watchdog that terminates a V8 isolate when an evaluation exceeds a budget.
///
/// Spawns a lightweight OS thread that sleeps in 10 ms ticks until either the
/// timeout elapses (then calls `terminate_execution`) or [`did_fire`] cancels
/// it. `did_fire` is called on the happy path after evaluation completes and
/// atomically reports whether the watchdog already fired.
///
/// ponytail: one thread per evaluation. Ceiling: evaluations are infrequent
/// (startup config load, per-document loadEntry) so a polling thread per
/// evaluation is cheap; if evaluation frequency rises, switch to a shared
/// timer wheel or `tokio::time::timeout` on a `LocalSet`-spawned task.
pub(super) struct TerminationTimer {
    fired: Arc<std::sync::atomic::AtomicBool>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl TerminationTimer {
    fn start(timeout: Duration, handle: deno_core::v8::IsolateHandle) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};

        let fired = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let (fired_clone, cancel_clone) = (Arc::clone(&fired), Arc::clone(&cancel));
        let join = std::thread::Builder::new()
            .name("clay-js-runtime-timeout".to_string())
            .spawn(move || {
                let start = std::time::Instant::now();
                while start.elapsed() < timeout {
                    if cancel_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                // Timeout elapsed before cancellation: terminate the isolate so
                // the blocked evaluation returns control.
                fired_clone.store(true, Ordering::Relaxed);
                handle.terminate_execution();
            })
            .expect("failed to spawn JS runtime timeout watchdog thread");
        Self {
            fired,
            cancel,
            join: Some(join),
        }
    }

    /// Cancels the watchdog and returns whether it had already fired.
    fn did_fire(mut self) -> bool {
        use std::sync::atomic::Ordering;

        self.cancel.store(true, Ordering::Relaxed);
        let fired = self.fired.load(Ordering::Relaxed);
        // Detach rather than join: the thread observes `cancel` and exits within
        // a 10 ms tick. Joining is safe (terminate is non-blocking) but detaching
        // keeps the happy path off any thread-synchronization latency.
        self.join.take();
        fired
    }
}
