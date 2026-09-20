use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::sync::oneshot;

use crate::perf::budgets::{JS_RUNTIME_EVALUATION_TIMEOUT_MS, JS_RUNTIME_HEAP_LIMIT_BYTES};
use crate::protocol::{IncrementalParseUpdate, ParseEditNotification};

use super::{
    configuration::ConfigurationRuntime, ops::PackageLoadEntryAllowlist, workspace::WorkspaceState,
};

mod error;
mod evaluation;
mod source;
mod validation;
mod worker;

pub(crate) use self::error::{ClayRuntimeError, ClayRuntimeEvaluation, DocumentAnalysisInvocation};
pub(crate) use self::worker::{RuntimeCommand, RuntimeCommandSender, RuntimeEntry};
use self::worker::{RuntimeWorker, harvest_op_state_evaluation, start_runtime_worker};

/// Per-domain worker lane (Plan 127 P1). Lanes are separate isolates and
/// command threads inside ONE trust domain: they share the domain extension
/// set, package service, and load-entry allowlist, and never share V8
/// globals, module instances, or op state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeLane {
    /// Configuration/package evaluation, parse handlers, document analysis.
    General = 0,
    /// Completion and language-intelligence provider invocations. Handlers
    /// materialize by importing the registration's validated module specifier
    /// (document-analyzer precedent), so this lane needs no evaluation replay.
    Latency = 1,
}

impl RuntimeLane {
    /// Every lane, indexed by `RuntimeLane as usize`. Length is pinned to the
    /// `JS_RUNTIME_LANES_PER_DOMAIN` budget: adding a lane fails to compile
    /// until the variant (and its heap ceiling) exists.
    pub(crate) const ALL: [RuntimeLane; crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN] =
        [RuntimeLane::General, RuntimeLane::Latency];

    fn index(self) -> usize {
        self as usize
    }

    /// Lane serving a provider invocation (Plan 127 P1): a registration that
    /// carries a host-validated package module specifier can materialize its
    /// handler in any isolate, so it runs on the latency lane. Inline
    /// `module: {...}` registrations only exist as closures in the isolate
    /// that evaluated the package, so they stay on the general lane until the
    /// package adopts `moduleSpecifier`.
    pub(crate) fn for_provider(module_specifier: Option<&str>) -> Self {
        if module_specifier.is_some() {
            Self::Latency
        } else {
            Self::General
        }
    }
}

/// Heap ceiling for one lane: the general lane keeps the configured service
/// ceiling; the latency lane is clamped to the provider-lane budget and never
/// raised above the configured ceiling, so synthetic small-heap tests stay
/// bounded on every lane.
pub(crate) fn lane_heap_limit_bytes(
    configured_heap_limit_bytes: usize,
    lane: RuntimeLane,
) -> usize {
    match lane {
        RuntimeLane::General => configured_heap_limit_bytes,
        RuntimeLane::Latency => configured_heap_limit_bytes
            .min(crate::perf::budgets::JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES),
    }
}

/// One lane's isolate handle plus its own poison flag: a lane timeout/heap
/// limit replaces that lane's worker only (Plan 127 P1).
#[derive(Debug, Clone)]
struct LaneRuntime {
    poisoned: Arc<std::sync::atomic::AtomicBool>,
    worker: Arc<std::sync::Mutex<Arc<RuntimeWorker>>>,
}

/// One trust domain's lanes plus per-domain generation state (evaluation
/// metric, replaceable worker handles, registration ownership generation).
#[derive(Debug, Clone)]
struct DomainRuntime {
    evaluations: Arc<AtomicU64>,
    /// Per-domain runtime generation, bumped on every GENERAL-lane worker
    /// replacement (poison recovery, trusted reload). Registration ownership
    /// metadata per Plan 061 task 12; the latency lane owns no host-visible
    /// registrations, so replacing it leaves the generation untouched.
    generation: Arc<AtomicU64>,
    lanes: [LaneRuntime; crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN],
}

impl DomainRuntime {
    fn lane(&self, lane: RuntimeLane) -> &LaneRuntime {
        &self.lanes[lane.index()]
    }
}

/// Isolated server-side Clay JavaScript runtime boundary. Owns two trust
/// domains (Plan 061): one trusted runtime for configuration and bundled
/// first-party packages, and one shared third-party runtime for adopted
/// packages. Each domain owns `JS_RUNTIME_LANES_PER_DOMAIN` worker lanes
/// (Plan 127 P1): separate isolates inside the same domain with their own
/// poison/restart path, each carrying that domain's op extension set and
/// sharing the host-owned `PackageService` and package load-entry allowlist.
/// No V8 value, module instance, or op state crosses a lane or a domain.
#[derive(Clone)]
pub(crate) struct ClayJsRuntimeService {
    evaluations: Arc<AtomicU64>,
    /// Plan 099: per-generation cache of completed document mode activations
    /// (classification identity + published behavior manifest). A repeat open
    /// whose classification inputs and native grammar registration match a
    /// cached activation skips the per-open generated module evaluation
    /// entirely: the cached manifest is republished from Rust.
    mode_activation_cache: Arc<std::sync::Mutex<ModeActivationCache>>,
    /// Count of generated module evaluations served by the V8 open path, for
    /// the registry fast-path parity/measurement tests.
    pub(crate) open_activation_evaluations: Arc<AtomicU64>,
    timeout: Duration,
    heap_limit_bytes: usize,
    completion_providers:
        Arc<std::sync::Mutex<Vec<crate::server::completion::CompletionProviderMeta>>>,
    native_syntax_handlers: Arc<std::sync::Mutex<std::collections::HashSet<(u64, String, String)>>>,
    package_service: Arc<std::sync::Mutex<crate::packages::service::PackageService>>,
    load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
    workers_started: Arc<AtomicU64>,
    trusted: DomainRuntime,
    third_party: DomainRuntime,
    /// Follow-up round (`editor-control`): bounded publisher for gated
    /// programmatic editor-command execution requests. Shared by both domain
    /// workers (each op state holds a sender clone) and subscribed by every
    /// connection loop. Survives `production_reload`.
    editor_commands: tokio::sync::broadcast::Sender<crate::protocol::EditorCommandRequest>,
    /// Plan 071 caret-transport fix: runtime caret override channel plus the
    /// current-value store shared by both domain workers. Survives
    /// `production_reload` so one subscription covers reloads; the store
    /// feeds connection initial sync and lag replay.
    caret_styles: tokio::sync::broadcast::Sender<Option<crate::protocol::CaretStyle>>,
    caret_style_state: std::sync::Arc<std::sync::Mutex<Option<crate::protocol::CaretStyle>>>,
    /// Phase 26: user-owned editor wrap-policy override channel plus the
    /// current-value store. Survives `production_reload`; the store feeds
    /// connection initial sync and lag replay. Trusted-domain only.
    editor_layouts: tokio::sync::broadcast::Sender<Option<crate::protocol::WrapPolicy>>,
    editor_layout_state: std::sync::Arc<std::sync::Mutex<Option<crate::protocol::WrapPolicy>>>,
    /// Phase 22.1: shell-preferences channel plus the current-value store.
    /// Same lifetime semantics as `caret_styles`: survives reloads, feeds
    /// connection initial sync and lag replay.
    shell_preferences: tokio::sync::broadcast::Sender<crate::protocol::ShellPreferences>,
    shell_preferences_state: std::sync::Arc<std::sync::Mutex<crate::protocol::ShellPreferences>>,
    /// Plan 130 A1: server-owned agent host handle, installed into every lane
    /// op state (hostless services — unit harnesses, embedded runtimes — leave
    /// it `None` and fail closed). Carried across `production_reload` and
    /// re-installed on poison restarts, so no lane ever reads a process global.
    agent_host: std::sync::Arc<std::sync::Mutex<Option<crate::server::agent::AgentHostHandle>>>,
}

/// Per-generation mode-activation cache with least-recently-used eviction
/// (Plan 126 D6). The cap is small (`MODE_ACTIVATION_CACHE_ENTRIES`), so the
/// oldest entry is found by linear scan instead of a second index structure,
/// mirroring `SyntaxChunkCache::next_access`.
#[derive(Default)]
struct ModeActivationCache {
    entries: HashMap<
        crate::server::connection::ModeActivationKey,
        (u64, crate::server::connection::CachedModeActivation),
    >,
    access_counter: u64,
}

impl ModeActivationCache {
    fn next_access(&mut self) -> u64 {
        self.access_counter = self.access_counter.saturating_add(1);
        self.access_counter
    }

    /// Cached activation for `key`, marked most-recently used.
    fn get(
        &mut self,
        key: &crate::server::connection::ModeActivationKey,
    ) -> Option<crate::server::connection::CachedModeActivation> {
        let stamp = self.next_access();
        let (last_access, cached) = self.entries.get_mut(key)?;
        *last_access = stamp;
        Some(cached.clone())
    }

    /// Insert `key` as most-recently used. At capacity only the
    /// least-recently used entry is evicted; clearing the whole cache here
    /// (Plan 126 D6) forced every previously cached mode to re-evaluate its
    /// generated module after 64 distinct modes.
    fn insert(
        &mut self,
        key: crate::server::connection::ModeActivationKey,
        cached: crate::server::connection::CachedModeActivation,
    ) {
        if !self.entries.contains_key(&key)
            && self.entries.len() >= crate::perf::budgets::MODE_ACTIVATION_CACHE_ENTRIES
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (last_access, _))| *last_access)
                .map(|(key, _)| key.clone())
        {
            self.entries.remove(&oldest);
        }
        let stamp = self.next_access();
        self.entries.insert(key, (stamp, cached));
    }
}

impl std::fmt::Debug for ClayJsRuntimeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClayJsRuntimeService")
            .field("evaluations", &self.evaluations)
            .field("timeout", &self.timeout)
            .field("heap_limit_bytes", &self.heap_limit_bytes)
            .field("workers_started", &self.workers_started)
            .field("trusted", &self.trusted)
            .field("third_party", &self.third_party)
            .finish_non_exhaustive()
    }
}

impl Default for ClayJsRuntimeService {
    fn default() -> Self {
        Self::new(Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS))
    }
}

impl ClayJsRuntimeService {
    fn new(timeout: Duration) -> Self {
        Self::new_with_heap_limit(timeout, JS_RUNTIME_HEAP_LIMIT_BYTES)
    }

    fn new_with_heap_limit(timeout: Duration, heap_limit_bytes: usize) -> Self {
        Self::with_package_service(
            timeout,
            heap_limit_bytes,
            crate::packages::service::PackageService::new(
                PathBuf::new(),
                Box::new(crate::packages::manager::FakeBackend::new()),
            ),
        )
    }

    /// Production reload (Plan 061 task 12): a trusted-generation reload
    /// shares the CURRENT third-party domain (worker, poison/generation
    /// state, package authority, load-entry allowlist) so adopted
    /// third-party packages, their providers, and their language-server
    /// sessions survive a configuration reload untouched. Only the trusted
    /// worker is rebuilt and re-runs configuration.
    pub(crate) fn production_reload(current: &Self) -> Self {
        let workers_started = Arc::new(AtomicU64::new(0));
        let trusted = Self::start_domain_runtime(
            crate::packages::bundled::RuntimeDomain::Trusted,
            current.timeout,
            current.heap_limit_bytes,
            &current.package_service,
            &current.load_entry_allowlist,
            &workers_started,
        );
        let service = Self {
            evaluations: Arc::new(AtomicU64::new(0)),
            // A reload rebuilds the trusted worker and its registries; the
            // per-generation activation cache must not survive it.
            mode_activation_cache: Arc::new(std::sync::Mutex::new(ModeActivationCache::default())),
            open_activation_evaluations: Arc::new(AtomicU64::new(0)),
            timeout: current.timeout,
            heap_limit_bytes: current.heap_limit_bytes,
            completion_providers: Arc::new(std::sync::Mutex::new(Vec::new())),
            native_syntax_handlers: Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
            package_service: Arc::clone(&current.package_service),
            load_entry_allowlist: Arc::clone(&current.load_entry_allowlist),
            workers_started,
            trusted,
            third_party: current.third_party.clone(),
            editor_commands: current.editor_commands.clone(),
            caret_styles: current.caret_styles.clone(),
            caret_style_state: std::sync::Arc::clone(&current.caret_style_state),
            editor_layouts: current.editor_layouts.clone(),
            editor_layout_state: std::sync::Arc::clone(&current.editor_layout_state),
            shell_preferences: current.shell_preferences.clone(),
            shell_preferences_state: std::sync::Arc::clone(&current.shell_preferences_state),
            agent_host: std::sync::Arc::clone(&current.agent_host),
        };
        // Same wiring as a fresh service: the rebuilt trusted lanes attach to
        // the carried-over channels and to the surviving third-party lanes
        // (Plan 061 task 12).
        service.wire_domain_lanes();
        service
    }

    /// Production server runtime: durable approval store at the default
    /// package store root so CLI/native approvals take effect (Plan 061 task
    /// 10). A corrupt or unreadable approval store fails closed with an empty
    /// in-memory store — no third-party package can be approved into
    /// execution until the store is repaired.
    pub(crate) fn production() -> Self {
        let store_root = crate::packages::service::default_store_root();
        let service = crate::packages::service::PackageService::open_production(store_root);
        Self::with_package_service(
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
            service,
        )
    }

    fn with_package_service(
        timeout: Duration,
        heap_limit_bytes: usize,
        package_service: crate::packages::service::PackageService,
    ) -> Self {
        let package_service = Arc::new(std::sync::Mutex::new(package_service));
        let load_entry_allowlist = Arc::new(PackageLoadEntryAllowlist::default());
        let workers_started = Arc::new(AtomicU64::new(0));
        let trusted = Self::start_domain_runtime(
            crate::packages::bundled::RuntimeDomain::Trusted,
            timeout,
            heap_limit_bytes,
            &package_service,
            &load_entry_allowlist,
            &workers_started,
        );
        let third_party = Self::start_domain_runtime(
            crate::packages::bundled::RuntimeDomain::ThirdParty,
            timeout,
            heap_limit_bytes,
            &package_service,
            &load_entry_allowlist,
            &workers_started,
        );
        let (editor_commands, _) = tokio::sync::broadcast::channel(16);
        // Plan 071 caret-transport fix: runtime caret override lane.
        let (caret_styles, _) = tokio::sync::broadcast::channel(4);
        let caret_style_state =
            std::sync::Arc::new(std::sync::Mutex::new(None::<crate::protocol::CaretStyle>));
        // Phase 26: user-owned editor wrap-policy override lane. Trusted
        // domain only (the op is not registered in the package extension),
        // but the channel is shared so a reload keeps one subscription.
        let (editor_layouts, _) = tokio::sync::broadcast::channel(4);
        let editor_layout_state =
            std::sync::Arc::new(std::sync::Mutex::new(None::<crate::protocol::WrapPolicy>));
        // Phase 22.1: shell-preferences lane.
        let (shell_preferences, _) = tokio::sync::broadcast::channel(4);
        let shell_preferences_state =
            std::sync::Arc::new(std::sync::Mutex::new(crate::protocol::ShellPreferences {
                pane_focus_policy: "click".to_string(),
            }));
        let service = Self {
            evaluations: Arc::new(AtomicU64::new(0)),
            mode_activation_cache: Arc::new(std::sync::Mutex::new(ModeActivationCache::default())),
            open_activation_evaluations: Arc::new(AtomicU64::new(0)),
            timeout,
            heap_limit_bytes,
            completion_providers: Arc::new(std::sync::Mutex::new(Vec::new())),
            native_syntax_handlers: Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
            package_service,
            load_entry_allowlist,
            workers_started,
            trusted,
            third_party,
            editor_commands,
            caret_styles,
            caret_style_state,
            editor_layouts,
            editor_layout_state,
            shell_preferences,
            shell_preferences_state,
            agent_host: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        // Wire shared host channels into every lane plus the cross-domain
        // bridge: trusted general `loadPackage` of an approved third-party
        // package dispatches its load evaluation to the third-party general
        // lane (Plan 061 task 12), while both third-party lanes receive
        // active-mode replication.
        service.wire_domain_lanes();
        service
    }

    fn start_domain_runtime(
        domain: crate::packages::bundled::RuntimeDomain,
        timeout: Duration,
        heap_limit_bytes: usize,
        package_service: &Arc<std::sync::Mutex<crate::packages::service::PackageService>>,
        load_entry_allowlist: &Arc<PackageLoadEntryAllowlist>,
        workers_started: &Arc<AtomicU64>,
    ) -> DomainRuntime {
        let lanes = RuntimeLane::ALL.map(|lane| {
            workers_started.fetch_add(1, Ordering::Relaxed);
            LaneRuntime {
                poisoned: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                worker: Arc::new(std::sync::Mutex::new(start_runtime_worker(
                    timeout,
                    lane_heap_limit_bytes(heap_limit_bytes, lane),
                    domain,
                    Arc::clone(package_service),
                    Arc::clone(load_entry_allowlist),
                    lane,
                ))),
            }
        });
        DomainRuntime {
            evaluations: Arc::new(AtomicU64::new(0)),
            generation: Arc::new(AtomicU64::new(1)),
            lanes,
        }
    }

    /// Follow-up round (`editor-control`): subscribe to gated programmatic
    /// editor-command execution requests. Connection loops forward each
    /// request as `ServerMessage::EditorCommandRequest`.
    pub(crate) fn subscribe_editor_commands(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::protocol::EditorCommandRequest> {
        self.editor_commands.subscribe()
    }

    /// Plan 071 caret-transport fix: subscribe to runtime caret override
    /// updates (`None` clears). Shared across generations.
    pub(crate) fn subscribe_caret_styles(
        &self,
    ) -> tokio::sync::broadcast::Receiver<Option<crate::protocol::CaretStyle>> {
        self.caret_styles.subscribe()
    }

    /// Current runtime caret override for connection initial sync and lag
    /// replay.
    pub(crate) fn caret_style_override(&self) -> Option<crate::protocol::CaretStyle> {
        *self
            .caret_style_state
            .lock()
            .expect("caret style state mutex poisoned")
    }

    /// Phase 26: subscribe to user-owned editor wrap-policy override updates
    /// (`None` clears). Shared across generations.
    pub(crate) fn subscribe_editor_layout(
        &self,
    ) -> tokio::sync::broadcast::Receiver<Option<crate::protocol::WrapPolicy>> {
        self.editor_layouts.subscribe()
    }

    /// Current editor wrap-policy override for connection initial sync and
    /// lag replay.
    pub(crate) fn editor_layout_override(&self) -> Option<crate::protocol::WrapPolicy> {
        *self
            .editor_layout_state
            .lock()
            .expect("editor layout state mutex poisoned")
    }

    /// Phase 22.1: subscribe to shell-preferences updates.
    pub(crate) fn subscribe_shell_preferences(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::protocol::ShellPreferences> {
        self.shell_preferences.subscribe()
    }

    /// Current shell preferences for connection initial sync and lag replay.
    pub(crate) fn shell_preferences(&self) -> crate::protocol::ShellPreferences {
        self.shell_preferences_state
            .lock()
            .expect("shell preferences state mutex poisoned")
            .clone()
    }
    fn domain(&self, domain: crate::packages::bundled::RuntimeDomain) -> &DomainRuntime {
        match domain {
            crate::packages::bundled::RuntimeDomain::Trusted => &self.trusted,
            crate::packages::bundled::RuntimeDomain::ThirdParty => &self.third_party,
        }
    }

    fn lane(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        lane: RuntimeLane,
    ) -> &LaneRuntime {
        self.domain(domain).lane(lane)
    }

    fn replace_domain_worker(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        lane: RuntimeLane,
    ) {
        self.workers_started.fetch_add(1, Ordering::Relaxed);
        // Only the general lane owns host-visible registrations, so only its
        // replacement invalidates registration ownership metadata.
        if lane == RuntimeLane::General {
            self.domain(domain)
                .generation
                .fetch_add(1, Ordering::Relaxed);
        }
        let replacement = start_runtime_worker(
            self.timeout,
            lane_heap_limit_bytes(self.heap_limit_bytes, lane),
            domain,
            Arc::clone(&self.package_service),
            Arc::clone(&self.load_entry_allowlist),
            lane,
        );
        // Rewire the cross-domain bridge: the trusted general lane dispatches
        // third-party load evaluations to the CURRENT third-party general
        // lane (mode replication follows every third-party lane).
        if domain == crate::packages::bundled::RuntimeDomain::ThirdParty {
            let trusted_general =
                self.domain_worker(crate::packages::bundled::RuntimeDomain::Trusted);
            match lane {
                RuntimeLane::General => trusted_general
                    .op_state
                    .set_third_party_commands(replacement.sender.clone()),
                RuntimeLane::Latency => trusted_general
                    .op_state
                    .set_third_party_latency_sender(replacement.sender.clone()),
            }
        }
        self.wire_runtime_publishers(&replacement);
        *self
            .lane(domain, lane)
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned") = replacement;
    }

    /// Attach the shared host channels to one lane's op state. Replacement
    /// workers start unwired, so every replacement and initial construction
    /// routes through here.
    fn wire_runtime_publishers(&self, worker: &RuntimeWorker) {
        // Plan 130 A1: the agent handle rides the same wiring path as the
        // other host channels, so a replacement lane (poison restart) and a
        // reloaded lane both come up with the server's own host attached.
        if let Some(host) = self
            .agent_host
            .lock()
            .expect("agent host mutex poisoned")
            .clone()
        {
            worker.op_state.set_agent_host(host);
        }
        worker
            .op_state
            .set_editor_command_publisher(self.editor_commands.clone());
        worker.op_state.set_caret_style_publisher(
            self.caret_styles.clone(),
            std::sync::Arc::clone(&self.caret_style_state),
        );
        worker.op_state.set_editor_layout_publisher(
            self.editor_layouts.clone(),
            std::sync::Arc::clone(&self.editor_layout_state),
        );
        worker.op_state.set_shell_preferences_publisher(
            self.shell_preferences.clone(),
            std::sync::Arc::clone(&self.shell_preferences_state),
        );
    }

    /// Wire every domain lane to the shared host channels and the
    /// cross-domain bridge. The trusted latency lane deliberately gets no
    /// bridge: provider modules it imports must not drive package loading, so
    /// only the general lane can dispatch `loadPackage` into the third-party
    /// domain.
    fn wire_domain_lanes(&self) {
        for domain in [
            crate::packages::bundled::RuntimeDomain::Trusted,
            crate::packages::bundled::RuntimeDomain::ThirdParty,
        ] {
            for lane in RuntimeLane::ALL {
                let worker = self.domain_lane_worker(domain, lane);
                self.wire_runtime_publishers(&worker);
            }
        }
        let trusted_general = self.domain_worker(crate::packages::bundled::RuntimeDomain::Trusted);
        trusted_general.op_state.set_third_party_commands(
            self.domain_worker(crate::packages::bundled::RuntimeDomain::ThirdParty)
                .sender
                .clone(),
        );
        trusted_general.op_state.set_third_party_latency_sender(
            self.domain_lane_worker(
                crate::packages::bundled::RuntimeDomain::ThirdParty,
                RuntimeLane::Latency,
            )
            .sender
            .clone(),
        );
    }

    /// Plan 130 A1: install the server-owned agent authority into this
    /// service and every lane's op state. Called once by server construction;
    /// `production_reload` carries the remembered handle to the rebuilt lanes.
    pub(crate) fn set_agent_host(&self, host: crate::server::agent::AgentHostHandle) {
        *self.agent_host.lock().expect("agent host mutex poisoned") = Some(host);
        self.wire_domain_lanes();
    }

    /// Host-owned package authority shared by both domain runtimes.
    pub(crate) fn package_service(
        &self,
    ) -> &Arc<std::sync::Mutex<crate::packages::service::PackageService>> {
        &self.package_service
    }

    /// Runtime domain hosting an enabled package's code, resolved from the
    /// host-owned enabled record (never from caller-supplied manifests).
    fn enabled_record_domain(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> crate::packages::bundled::RuntimeDomain {
        self.package_service
            .lock()
            .expect("package service mutex poisoned")
            .enabled_records()
            .find(|record| {
                record.manifest.name == package_name && record.manifest.version == package_version
            })
            .map(|record| record.runtime_domain)
            .unwrap_or(crate::packages::bundled::RuntimeDomain::ThirdParty)
    }

    /// Sets a custom evaluation timeout. The default is
    /// [`JS_RUNTIME_EVALUATION_TIMEOUT_MS`]; tests use a short timeout to
    /// exercise the termination path quickly.
    #[cfg(test)]
    pub(crate) fn with_timeout(timeout: Duration) -> Self {
        Self::new(timeout)
    }

    #[cfg(test)]
    pub(crate) fn with_timeout_and_heap_limit(timeout: Duration, heap_limit_bytes: usize) -> Self {
        Self::new_with_heap_limit(timeout, heap_limit_bytes)
    }

    /// Evaluates a controlled server-owned ES module on the persistent runtime worker.
    #[cfg(test)]
    pub(crate) async fn evaluate_controlled_module(
        &self,
        source: impl Into<String> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_controlled_module_for_document(source, 1)
            .await
    }

    /// Controlled-source evaluation stamped with the runtime document the
    /// evaluation serves. Classification/open-path evaluations use the opened
    /// document's id so harvested per-document mode manifests resolve to the
    /// right layer (Phase 22.2).
    pub(crate) async fn evaluate_controlled_module_for_document(
        &self,
        source: impl Into<String> + Send + 'static,
        runtime_document_id: crate::protocol::DocumentId,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ControlledSource(source.into()),
            None,
            runtime_document_id,
            "runtime.evaluate_controlled_module",
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn load_configuration_from_root(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            None,
            1,
            "runtime.load_configuration",
        )
        .await
    }

    pub(crate) async fn load_configuration_from_root_with_workspace(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
        workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            Some(workspace),
            1,
            "runtime.load_configuration_with_workspace",
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn load_configuration_from_root_for_document(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
        runtime_document_id: crate::protocol::DocumentId,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            None,
            runtime_document_id,
            "runtime.load_configuration_for_document",
        )
        .await
    }

    async fn evaluate_entry(
        &self,
        entry: RuntimeEntry,
        workspace: Option<Arc<tokio::sync::Mutex<WorkspaceState>>>,
        runtime_document_id: crate::protocol::DocumentId,
        metric: &'static str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry_for_domain(
            crate::packages::bundled::RuntimeDomain::Trusted,
            entry,
            workspace,
            runtime_document_id,
            None,
            metric,
        )
        .await
    }

    /// Evaluate a module with host-stamped package provenance: package-facing
    /// registration/publication ops inside the evaluation resolve to
    /// `package`'s host-enabled record. Used by Rust-driven package
    /// loading/adoption and by tests; orchestration code that loads packages
    /// through the `loadPackage` op gets provenance stamped by the op itself.
    #[cfg(test)]
    pub(crate) async fn evaluate_entry_as_package(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        package: &crate::packages::record::PackageRecord,
        entry: RuntimeEntry,
        metric: &'static str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry_for_domain(
            domain,
            entry,
            None,
            0,
            Some(crate::server::ops::PackageContext::from_record(package)),
            metric,
        )
        .await
    }

    async fn evaluate_entry_for_domain(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        entry: RuntimeEntry,
        workspace: Option<Arc<tokio::sync::Mutex<WorkspaceState>>>,
        runtime_document_id: crate::protocol::DocumentId,
        package_context: Option<crate::server::ops::PackageContext>,
        metric: &'static str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        let domain_runtime = self.domain(domain);
        let general_lane = domain_runtime.lane(RuntimeLane::General);
        if general_lane.poisoned.swap(false, Ordering::Relaxed) {
            self.replace_domain_worker(domain, RuntimeLane::General);
        }
        // Recovery stays ahead of the revocation gate: a lane left poisoned by
        // an earlier command must be replaced (and its isolate dropped) no
        // matter which command arrives next.
        if let Some(context) = package_context.as_ref() {
            self.ensure_package_enabled(&context.package_name, &context.package_version)?;
        }
        let (response, receiver) = oneshot::channel();
        let command = RuntimeCommand::Evaluate {
            entry,
            workspace,
            runtime_document_id,
            package_context,
            metric,
            response,
        };
        if let Err(error) = self.domain_worker(domain).sender.send(command) {
            self.replace_domain_worker(domain, RuntimeLane::General);
            self.domain_worker(domain)
                .sender
                .send(*error.0)
                .map_err(|_| {
                    ClayRuntimeError::Runtime(
                        "persistent JavaScript runtime worker stopped".to_string(),
                    )
                })?;
        }
        let result = receiver.await.map_err(|_| {
            ClayRuntimeError::Runtime("persistent JavaScript runtime worker stopped".to_string())
        })?;
        if matches!(
            result,
            Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
        ) {
            general_lane.poisoned.store(true, Ordering::Relaxed);
        } else if let Ok(evaluation) = &result {
            self.evaluations.fetch_add(1, Ordering::Relaxed);
            domain_runtime.evaluations.fetch_add(1, Ordering::Relaxed);
            *self
                .completion_providers
                .lock()
                .expect("completion provider snapshot lock poisoned") =
                evaluation.completion_providers.clone();
        }
        result
    }

    /// Test-only evaluation entry for the third-party domain, used by trust
    /// boundary tests. Production third-party loading arrives with the
    /// adoption/activation tasks.
    #[cfg(test)]
    pub(crate) async fn evaluate_third_party_module(
        &self,
        source: impl Into<String> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry_for_domain(
            crate::packages::bundled::RuntimeDomain::ThirdParty,
            RuntimeEntry::ControlledSource(source.into()),
            None,
            1,
            None,
            "runtime.evaluate_third_party_module",
        )
        .await
    }

    pub(crate) fn completion_providers(
        &self,
    ) -> Vec<crate::server::completion::CompletionProviderMeta> {
        self.completion_providers
            .lock()
            .expect("completion provider snapshot lock poisoned")
            .clone()
    }

    pub(crate) async fn load_default_configuration_with_workspace(
        &self,
        workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    ) -> Result<Option<ClayRuntimeEvaluation>, ClayRuntimeError> {
        let Some(config_root) = ConfigurationRuntime::default_config_root() else {
            return Ok(None);
        };
        if !config_root.join("init.js").is_file() {
            return Ok(None);
        }
        self.load_configuration_from_root_with_workspace(config_root, workspace)
            .await
            .map(Some)
    }

    pub(crate) fn register_parse_handlers(
        &self,
        coordinator: &crate::server::parse_coordinator::ParseCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
    ) -> Result<
        Vec<crate::server::parse_coordinator::ParseHandlerMeta>,
        crate::server::parse_coordinator::ParseCoordinatorError,
    > {
        let mut registered = Vec::new();
        for registration in &evaluation.js_parse_handlers {
            if registration.meta.package_prefix != registration.package.manifest.clay.api_prefix {
                return Err(
                    crate::server::parse_coordinator::ParseCoordinatorError::ProvenanceMismatch,
                );
            }
            match coordinator.register_handler_for_generation(
                &registration.package,
                generation_id,
                registration.meta.mode_id.clone(),
                JsParseHandler {
                    runtime: self.clone(),
                    registration: registration.clone(),
                },
            ) {
                Ok(meta) => registered.push(meta),
                Err(crate::server::parse_coordinator::ParseCoordinatorError::HandlerAlreadyRegistered { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(registered)
    }

    pub(crate) fn register_completion_providers(
        &self,
        coordinator: &crate::server::completion::CompletionCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
    ) -> Result<
        Vec<crate::server::completion::CompletionProviderMeta>,
        crate::server::completion::CompletionProviderRegistryError,
    > {
        let mut registered = Vec::new();
        for registration in &evaluation.js_completion_providers {
            let mut registration = registration.clone();
            registration.meta.generation = generation_id;
            let meta = registration.meta.clone();
            let package = registration.package.clone();
            match coordinator.register_package_for_generation(
                &package,
                meta.clone(),
                JsCompletionProvider {
                    runtime: self.clone(),
                    registration,
                },
            ) {
                Ok(()) => registered.push(meta),
                Err(crate::server::completion::CompletionProviderRegistryError::ProviderAlreadyRegistered { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(registered)
    }

    pub(crate) fn register_language_intelligence_providers(
        &self,
        coordinator: &crate::server::language_intelligence::LanguageIntelligenceCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
    ) -> Result<
        Vec<crate::server::language_intelligence::LanguageIntelligenceProviderMeta>,
        crate::server::language_intelligence::LanguageIntelligenceProviderRegistryError,
    > {
        let mut registered = Vec::new();
        for registration in &evaluation.js_language_intelligence_providers {
            let mut meta = registration.meta.clone();
            meta.generation = generation_id;
            match coordinator.register_package_for_generation(
                &registration.package,
                meta.clone(),
                JsLanguageIntelligenceProvider {
                    runtime: self.clone(),
                    registration: registration.clone(),
                },
            ) {
                Ok(()) => registered.push(meta),
                Err(
                    crate::server::language_intelligence::LanguageIntelligenceProviderRegistryError::ProviderAlreadyRegistered {
                        ..
                    },
                ) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(registered)
    }

    pub(crate) fn register_native_syntax_handler(
        &self,
        coordinator: &crate::server::parse_coordinator::ParseCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
        path: &str,
        package_prefix: &str,
        _mode_id: &str,
    ) -> Result<
        Option<(
            crate::server::parse_coordinator::ParseHandlerMeta,
            crate::protocol::ParsePolicy,
        )>,
        crate::server::parse_coordinator::ParseCoordinatorError,
    > {
        let Some(contribution) = crate::server::syntax::select_grammar_for_path(
            &evaluation.syntax_grammars,
            &evaluation.syntax_engine_preferences,
            path,
        ) else {
            return Ok(None);
        };
        if contribution.engine_tier != crate::server::syntax::SyntaxEngineTier::Native {
            return Ok(None);
        }
        let policy = crate::protocol::ParsePolicy::new(
            contribution
                .max_window_bytes
                .unwrap_or(crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)
                as u64,
            4 * 1024,
            30 * 1024 * 1024,
            contribution.timeout_ms.unwrap_or(5_000),
        );
        let key = (
            generation_id,
            package_prefix.to_string(),
            contribution.id.clone(),
        );
        let mut native_syntax_handlers = self
            .native_syntax_handlers
            .lock()
            .expect("native syntax handler set lock poisoned");
        if native_syntax_handlers.contains(&key) {
            return Ok(Some((
                crate::server::parse_coordinator::ParseHandlerMeta {
                    package_prefix: package_prefix.to_string(),
                    mode_id: contribution.id.clone(),
                },
                policy,
            )));
        }
        let handler = crate::server::syntax::native_handler(contribution).map_err(|error| {
            crate::server::parse_coordinator::ParseCoordinatorError::HandlerFailed(
                error.to_string(),
            )
        })?;
        let Some(handler) = handler else {
            return Ok(None);
        };
        match coordinator.replace_handler_meta_for_generation(
            generation_id,
            crate::server::parse_coordinator::ParseHandlerMeta {
                package_prefix: package_prefix.to_string(),
                mode_id: contribution.id.clone(),
            },
            handler,
        ) {
            Ok(meta) => {
                native_syntax_handlers.insert(key);
                Ok(Some((meta, policy)))
            }
            Err(
                crate::server::parse_coordinator::ParseCoordinatorError::HandlerAlreadyRegistered {
                    ..
                },
            ) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Record a completed V8 open activation in the per-generation cache.
    pub(crate) fn cache_mode_activation(
        &self,
        key: crate::server::connection::ModeActivationKey,
        cached: crate::server::connection::CachedModeActivation,
    ) {
        let mut cache = self
            .mode_activation_cache
            .lock()
            .expect("mode activation cache lock poisoned");
        cache.insert(key, cached);
    }

    /// Look up a cached activation for a repeat open (registry fast path).
    pub(crate) fn cached_mode_activation(
        &self,
        key: &crate::server::connection::ModeActivationKey,
    ) -> Option<crate::server::connection::CachedModeActivation> {
        self.mode_activation_cache
            .lock()
            .expect("mode activation cache lock poisoned")
            .get(key)
    }

    /// Number of generated module evaluations the open path served from V8.
    #[cfg(test)]
    pub(crate) fn open_activation_evaluation_count(&self) -> u64 {
        self.open_activation_evaluations
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn registered_native_syntax_handler(
        &self,
        generation_id: u64,
        path: &str,
    ) -> Option<(
        crate::server::parse_coordinator::ParseHandlerMeta,
        crate::protocol::ParsePolicy,
    )> {
        let registry = crate::server::syntax::SyntaxGrammarRegistry::with_first_party_native();
        let contribution = registry.find_candidate_for_path(path)?.1;
        let key = (
            generation_id,
            contribution.package_prefix.clone(),
            contribution.id.clone(),
        );
        if !self
            .native_syntax_handlers
            .lock()
            .expect("native syntax handler set lock poisoned")
            .contains(&key)
        {
            return None;
        }
        Some((
            crate::server::parse_coordinator::ParseHandlerMeta {
                package_prefix: contribution.package_prefix.clone(),
                mode_id: contribution.id.clone(),
            },
            crate::protocol::ParsePolicy::new(
                contribution
                    .max_window_bytes
                    .unwrap_or(crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)
                    as u64,
                4 * 1024,
                30 * 1024 * 1024,
                contribution.timeout_ms.unwrap_or(5_000),
            ),
        ))
    }

    /// Dispatch one provider command to the lane that owns it inside the
    /// runtime domain that owns the registration's package (Plan 061 task 7,
    /// Plan 127 P1). Third-party callbacks run in third-party lanes, trusted
    /// callbacks in trusted lanes. On a poisoned lane only that lane's worker
    /// is replaced and the command retried; timeout/heap results poison the
    /// owning lane only.
    async fn dispatch_to_domain<T>(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        lane: RuntimeLane,
        command: RuntimeCommand,
        receiver: oneshot::Receiver<Result<T, ClayRuntimeError>>,
    ) -> Result<T, ClayRuntimeError> {
        let domain_runtime = self.domain(domain);
        let lane_runtime = domain_runtime.lane(lane);
        if lane_runtime.poisoned.swap(false, Ordering::Relaxed) {
            self.replace_domain_worker(domain, lane);
            // The latency lane materializes handlers from their validated
            // module specifiers, so only the general lane needs replay.
            if lane == RuntimeLane::General {
                self.replay_third_party_domain(domain).await?;
            }
        }
        // Recovery stays ahead of the revocation gate: a lane left poisoned by
        // an earlier command must be replaced (and its isolate dropped) no
        // matter which command arrives next.
        if let Some((package_name, package_version)) = command.package_identity() {
            self.ensure_package_enabled(package_name, package_version)?;
        }
        if let Err(error) = self.domain_lane_worker(domain, lane).sender.send(command) {
            self.replace_domain_worker(domain, lane);
            if lane == RuntimeLane::General {
                self.replay_third_party_domain(domain).await?;
            }
            self.domain_lane_worker(domain, lane)
                .sender
                .send(*error.0)
                .map_err(|_| {
                    ClayRuntimeError::Runtime(
                        "persistent JavaScript runtime worker stopped".to_string(),
                    )
                })?;
        }
        let result = receiver.await.map_err(|_| {
            ClayRuntimeError::Runtime("persistent JavaScript runtime worker stopped".to_string())
        })?;
        if matches!(
            result,
            Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
        ) {
            lane_runtime.poisoned.store(true, Ordering::Relaxed);
        } else if result.is_ok() {
            domain_runtime.evaluations.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// Replay every currently enabled, approved third-party package's load
    /// entry into a freshly replaced third-party worker (Plan 061 task 12).
    /// Only the current host-approved graph replays; deterministic
    /// registration tokens (`apiPrefix:id:index`) restore the host-side
    /// coordinator registrations in the fresh runtime. One bounded pass: a
    /// failing replay evaluation poisons the domain again instead of
    /// looping. No-op for the trusted domain (its replay is configuration
    /// reload).
    async fn replay_third_party_domain(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
    ) -> Result<(), ClayRuntimeError> {
        if domain != crate::packages::bundled::RuntimeDomain::ThirdParty {
            return Ok(());
        }
        let mut packages: Vec<(crate::server::ops::PackageContext, String)> = {
            let service = self
                .package_service
                .lock()
                .expect("package service mutex poisoned");
            let mut enabled: Vec<_> = service
                .enabled_records()
                .filter(|record| {
                    record.runtime_domain == crate::packages::bundled::RuntimeDomain::ThirdParty
                })
                .map(|record| {
                    (
                        crate::server::ops::PackageContext::from_record(record),
                        record.manifest.name.clone(),
                    )
                })
                .collect();
            enabled.sort_by(|a, b| a.1.cmp(&b.1));
            enabled
                .into_iter()
                .filter_map(|(context, name)| {
                    self.load_entry_allowlist
                        .specifier_for_package(&name)
                        .map(|specifier| (context, specifier))
                })
                .collect()
        };
        packages.dedup_by(|a, b| a.0.package_name == b.0.package_name);
        for (context, specifier) in packages {
            let source = format!(
                "const module = await import({specifier:?});\nif (typeof module.default === 'function') {{ await module.default(); }}"
            );
            let (response, receiver) = oneshot::channel();
            self.domain_worker(domain)
                .sender
                .send(RuntimeCommand::Evaluate {
                    entry: RuntimeEntry::ControlledSource(source),
                    workspace: None,
                    runtime_document_id: 1,
                    package_context: Some(context),
                    metric: "runtime.replay_third_party_domain",
                    response,
                })
                .map_err(|_| {
                    ClayRuntimeError::Runtime(
                        "persistent JavaScript runtime worker stopped".to_string(),
                    )
                })?;
            let result = receiver.await.map_err(|_| {
                ClayRuntimeError::Runtime(
                    "persistent JavaScript runtime worker stopped".to_string(),
                )
            })?;
            if let Err(error) = result {
                if matches!(
                    error,
                    ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit
                ) {
                    self.lane(domain, RuntimeLane::General)
                        .poisoned
                        .store(true, Ordering::Relaxed);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    /// Resolve the runtime domain owning a provider registration from the
    /// host-enabled package record (never from caller-supplied identity).
    fn registration_domain(
        &self,
        package: &crate::packages::record::PackageRecord,
    ) -> crate::packages::bundled::RuntimeDomain {
        self.enabled_record_domain(&package.manifest.name, &package.manifest.version)
    }

    /// Plan 127 task 6 revocation gate: a command may only reach a lane while
    /// the package it claims is still enabled at that exact version. Revoking
    /// an approval/disabling a package withdraws its authority immediately, so
    /// a registration the host still holds (coordinator table, global handler
    /// registry, module allowlist) must fail closed — identically in every lane
    /// and trust domain — instead of being replayed from whichever lane still
    /// happens to hold state for it.
    fn ensure_package_enabled(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Result<(), ClayRuntimeError> {
        let enabled = self
            .package_service
            .lock()
            .expect("package service mutex poisoned")
            .enabled_record(package_name, package_version)
            .is_some();
        if enabled {
            return Ok(());
        }
        Err(ClayRuntimeError::Revoked {
            package: package_name.to_string(),
            version: package_version.to_string(),
        })
    }

    async fn invoke_parse_handler(
        &self,
        registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
        notification: ParseEditNotification,
    ) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
        let domain = self.registration_domain(&registration.package);
        let (response, receiver) = oneshot::channel();
        self.dispatch_to_domain(
            domain,
            RuntimeLane::General,
            RuntimeCommand::Parse {
                registration,
                notification,
                response,
            },
            receiver,
        )
        .await
    }

    async fn invoke_completion_provider(
        &self,
        registration: crate::server::completion::JsCompletionProviderRegistration,
        request: crate::protocol::CompletionRequest,
        window: crate::server::completion::CompletionDocumentWindow,
    ) -> Result<crate::protocol::CompletionResultSet, ClayRuntimeError> {
        let domain = self.registration_domain(&registration.package);
        let lane = RuntimeLane::for_provider(registration.module_specifier.as_deref());
        let (response, receiver) = oneshot::channel();
        self.dispatch_to_domain(
            domain,
            lane,
            RuntimeCommand::Completion {
                registration,
                request,
                window,
                response,
            },
            receiver,
        )
        .await
    }

    pub(crate) fn document_analysis_registration_authorized(
        &self,
        registration: &crate::server::document_analysis::JsDocumentAnalyzerRegistration,
    ) -> bool {
        use crate::packages::{
            authorization::language_server_descriptor_fingerprint, permissions::PackagePermission,
        };

        if !registration
            .package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::ParseDocument)
            || !registration
                .package
                .manifest
                .clay
                .permissions
                .contains(&PackagePermission::LanguageServer)
        {
            return false;
        }
        let Some(descriptor) = registration
            .package
            .contributions
            .language_servers
            .iter()
            .find(|descriptor| descriptor.id == registration.contribution)
        else {
            return false;
        };
        let service = self
            .package_service
            .lock()
            .expect("package service mutex poisoned");
        service.enabled_records().any(|record| {
            record.manifest.name == registration.package.manifest.name
                && record.manifest.version == registration.package.manifest.version
                && record.manifest.clay.api_prefix == registration.package.manifest.clay.api_prefix
        }) && service
            .language_server_grant(
                &registration.package.manifest.name,
                &registration.contribution,
            )
            .is_some_and(|grant| {
                grant.descriptor_fingerprint == language_server_descriptor_fingerprint(descriptor)
            })
    }

    pub(crate) fn document_analysis_authorized(
        &self,
        registration: &crate::server::document_analysis::JsDocumentAnalyzerRegistration,
        workspace_root_id: crate::protocol::WorkspaceRootId,
    ) -> bool {
        self.document_analysis_registration_authorized(registration)
            && self
                .package_service
                .lock()
                .expect("package service mutex poisoned")
                .language_server_grant(
                    &registration.package.manifest.name,
                    &registration.contribution,
                )
                .is_some_and(|grant| grant.workspace_root_ids.contains(&workspace_root_id))
    }

    pub(crate) async fn invoke_document_analyzer(
        &self,
        registration: crate::server::document_analysis::JsDocumentAnalyzerRegistration,
        event: crate::server::document_analysis::DocumentAnalysisEvent,
        workspace: Arc<tokio::sync::Mutex<crate::server::workspace::WorkspaceState>>,
    ) -> Result<DocumentAnalysisInvocation, ClayRuntimeError> {
        // Route analysis through the owning domain runtime (Plan 061): no
        // additional persistent runtimes are created per analyzer/document.
        let domain = self.enabled_record_domain(
            &registration.package.manifest.name,
            &registration.package.manifest.version,
        );
        let domain_runtime = self.domain(domain);
        let general_lane = domain_runtime.lane(RuntimeLane::General);
        if general_lane.poisoned.swap(false, Ordering::Relaxed) {
            self.replace_domain_worker(domain, RuntimeLane::General);
        }
        let (response, receiver) = oneshot::channel();
        let invocation_id = self.evaluations.fetch_add(1, Ordering::Relaxed);
        let command = RuntimeCommand::DocumentAnalysis {
            registration,
            event,
            workspace,
            invocation_id,
            response,
        };
        if let Err(error) = self.domain_worker(domain).sender.send(command) {
            self.replace_domain_worker(domain, RuntimeLane::General);
            self.domain_worker(domain)
                .sender
                .send(*error.0)
                .map_err(|_| {
                    ClayRuntimeError::Runtime("document analysis worker stopped".to_string())
                })?;
        }
        let result = receiver.await.map_err(|_| {
            ClayRuntimeError::Runtime("document analysis worker stopped".to_string())
        })?;
        if matches!(
            result,
            Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
        ) {
            general_lane.poisoned.store(true, Ordering::Relaxed);
        } else if result.is_ok() {
            domain_runtime.evaluations.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    async fn invoke_language_intelligence_provider(
        &self,
        registration: crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration,
        request: crate::protocol::LanguageIntelligenceRequest,
        window: crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
    ) -> Result<crate::protocol::LanguageIntelligenceResult, ClayRuntimeError> {
        let domain = self.registration_domain(&registration.package);
        let lane = RuntimeLane::for_provider(registration.module_specifier.as_deref());
        let (response, receiver) = oneshot::channel();
        self.dispatch_to_domain(
            domain,
            lane,
            RuntimeCommand::LanguageIntelligence {
                registration,
                request,
                window,
                response,
            },
            receiver,
        )
        .await
    }

    /// Trusted-domain worker.
    fn worker(&self) -> Arc<RuntimeWorker> {
        self.domain_worker(crate::packages::bundled::RuntimeDomain::Trusted)
    }

    fn domain_worker(&self, domain: crate::packages::bundled::RuntimeDomain) -> Arc<RuntimeWorker> {
        // The general lane owns every host-visible registration (evaluation
        // snapshots, command registries, language-server sessions), so the
        // existing "the domain worker" API keeps meaning exactly that lane.
        self.domain_lane_worker(domain, RuntimeLane::General)
    }

    fn domain_lane_worker(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        lane: RuntimeLane,
    ) -> Arc<RuntimeWorker> {
        Arc::clone(
            &self
                .lane(domain, lane)
                .worker
                .lock()
                .expect("Clay runtime service worker mutex poisoned"),
        )
    }

    /// Snapshot of the third-party worker's current registration payload
    /// (Plan 061 task 12): the worker survives trusted reloads, so at
    /// generation commit the server re-registers these under the new
    /// generation instead of canceling live third-party providers.
    pub(crate) fn third_party_registrations_snapshot(&self) -> ClayRuntimeEvaluation {
        harvest_op_state_evaluation(
            &self
                .domain_worker(crate::packages::bundled::RuntimeDomain::ThirdParty)
                .op_state,
        )
    }

    /// Clone command metadata from both persistent trust-domain workers.
    /// Only inert Rust values cross this boundary; workers, V8 values, and
    /// callbacks remain owned by their runtime domains.
    pub(crate) fn command_registry_snapshots(
        &self,
    ) -> (
        Vec<crate::packages::commands::RegisteredCommand>,
        Vec<crate::packages::commands::RegisteredCommand>,
    ) {
        (
            self.worker().op_state.command_registry_snapshot(),
            self.domain_worker(crate::packages::bundled::RuntimeDomain::ThirdParty)
                .op_state
                .command_registry_snapshot(),
        )
    }

    /// Trusted-domain-only generation shutdown (Plan 061 task 12): reload
    /// shares the third-party worker across generations, so only the old
    /// trusted worker's language-server sessions end at commit (both trusted
    /// lanes, so a latency-lane provider module cannot strand a session).
    pub(crate) async fn shutdown_trusted_generation_resources(&self) -> usize {
        let mut sessions = 0;
        for lane in RuntimeLane::ALL {
            sessions += self
                .domain_lane_worker(crate::packages::bundled::RuntimeDomain::Trusted, lane)
                .op_state
                .shutdown_language_server_sessions()
                .await;
        }
        sessions
    }

    #[cfg(test)]
    pub(crate) async fn language_server_session_count(&self) -> usize {
        let mut sessions = 0;
        for domain in [
            crate::packages::bundled::RuntimeDomain::Trusted,
            crate::packages::bundled::RuntimeDomain::ThirdParty,
        ] {
            for lane in RuntimeLane::ALL {
                sessions += self
                    .domain_lane_worker(domain, lane)
                    .op_state
                    .language_server_session_count()
                    .await;
            }
        }
        sessions
    }

    /// Test-only handle to the trusted generation's language-server process service.
    #[cfg(test)]
    pub(crate) fn language_server_process_for_test(
        &self,
    ) -> crate::server::language_server::LanguageServerProcessService {
        self.worker().op_state.language_server_process()
    }

    #[cfg(test)]
    pub(crate) fn evaluation_count(&self) -> u64 {
        self.evaluations.load(Ordering::Relaxed)
    }

    /// Total persistent application runtimes started by this service
    /// (initial `2 domains × JS_RUNTIME_LANES_PER_DOMAIN` lanes plus any
    /// poison replacements). Trust-domain tests assert package/document/
    /// analyzer activity never raises it above the lane baseline.
    #[cfg(test)]
    pub(crate) fn workers_started(&self) -> u64 {
        self.workers_started.load(Ordering::Relaxed)
    }

    /// Per-lane command mailbox state (Plan 127 P2 flood tests).
    #[cfg(test)]
    pub(crate) fn lane_queue_stats(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        lane: RuntimeLane,
    ) -> self::worker::CommandQueueStats {
        self.domain_lane_worker(domain, lane).sender.queue_stats()
    }

    /// Per-domain successful evaluation count (trust-domain dispatch tests).
    #[cfg(test)]
    pub(crate) fn domain_evaluations(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
    ) -> u64 {
        self.domain(domain).evaluations.load(Ordering::Relaxed)
    }

    /// Per-domain runtime generation (bumped on every worker replacement).
    #[cfg(test)]
    pub(crate) fn domain_generation(&self, domain: crate::packages::bundled::RuntimeDomain) -> u64 {
        self.domain(domain).generation.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn test_op_state(&self) -> Arc<crate::server::ops::ClayOpState> {
        Arc::clone(&self.worker().op_state)
    }

    #[cfg(test)]
    pub(crate) fn test_lane_op_state(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        lane: RuntimeLane,
    ) -> Arc<crate::server::ops::ClayOpState> {
        Arc::clone(&self.domain_lane_worker(domain, lane).op_state)
    }
}

struct JsParseHandler {
    runtime: ClayJsRuntimeService,
    registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
}

impl crate::server::parse_coordinator::ParseHandler for JsParseHandler {
    fn parse(
        &self,
        notification: ParseEditNotification,
    ) -> crate::server::parse_coordinator::ParseHandlerFuture {
        let runtime = self.runtime.clone();
        let registration = self.registration.clone();
        Box::pin(async move {
            runtime
                .invoke_parse_handler(registration, notification)
                .await
                .map_err(|error| {
                    crate::server::parse_coordinator::ParseCoordinatorError::HandlerFailed(
                        error.to_string(),
                    )
                })
        })
    }
}

struct JsCompletionProvider {
    runtime: ClayJsRuntimeService,
    registration: crate::server::completion::JsCompletionProviderRegistration,
}

impl crate::server::completion::CompletionProvider for JsCompletionProvider {
    fn complete(
        &self,
        request: crate::protocol::CompletionRequest,
        window: crate::server::completion::CompletionDocumentWindow,
    ) -> crate::server::completion::CompletionProviderFuture {
        let runtime = self.runtime.clone();
        let registration = self.registration.clone();
        Box::pin(async move {
            runtime
                .invoke_completion_provider(registration, request, window)
                .await
                .map_err(|error| {
                    crate::server::completion::CompletionProviderError::ProviderFailed(
                        error.to_string(),
                    )
                })
        })
    }
}

struct JsLanguageIntelligenceProvider {
    runtime: ClayJsRuntimeService,
    registration: crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration,
}

impl crate::server::language_intelligence::LanguageIntelligenceProvider
    for JsLanguageIntelligenceProvider
{
    fn provide(
        &self,
        request: crate::protocol::LanguageIntelligenceRequest,
        window: crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
    ) -> crate::server::language_intelligence::LanguageIntelligenceProviderFuture {
        let runtime = self.runtime.clone();
        let registration = self.registration.clone();
        Box::pin(async move {
            runtime
                .invoke_language_intelligence_provider(registration, request, window)
                .await
                .map_err(|error| {
                    crate::server::language_intelligence::LanguageIntelligenceProviderError::ProviderFailed(
                        error.to_string(),
                    )
                })
        })
    }
}

#[cfg(test)]
mod tests;
