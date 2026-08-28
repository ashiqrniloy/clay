use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

/// Environment variable that enables Clay's internal developer performance recorder.
pub const PERF_PROFILE_ENV: &str = "CLAY_PERF_PROFILE";
/// Directory that receives per-process performance summaries at exit when set
/// by the editor performance harness. Unset in production.
pub const PERF_REPORT_DIR_ENV: &str = "CLAY_PERF_REPORT_DIR";
/// Developer CLI flag accepted by smoke/client/server workflows to enable profiling.
pub const PERF_PROFILE_FLAG: &str = "--profile-perf";
/// Schema version for content-free cross-process performance traces.
pub const PERF_TRACE_SCHEMA_VERSION: u32 = 1;
/// Maximum retained developer metric snapshots per recorder.
pub const PERF_SNAPSHOT_CAPACITY: usize = 4_096;

pub const EDITOR_OPEN: &str = "editor.open";
pub const EDITOR_READY: &str = "editor.ready";
pub const BROWSER_INPUT: &str = "browser.input";
pub const CODEMIRROR_UPDATE: &str = "codemirror.update";
pub const EDITOR_TYPING: &str = "editor.typing";
pub const BROWSER_VIEWPORT: &str = "browser.viewport";
pub const EDITOR_SCROLL: &str = "editor.scroll";
pub const EDITOR_SYNTAX_FRESH: &str = "editor.syntax_fresh";
pub const REACT_COMMIT: &str = "react.commit";
pub const EDITOR_COMPARTMENT_RECONFIGURE: &str = "editor.compartment_reconfigure";
pub const EDITOR_LONG_TASK: &str = "editor.long_task";
pub const BRIDGE_ENQUEUE: &str = "bridge.enqueue";
pub const BRIDGE_CLIENT_DELIVERY: &str = "bridge.client_delivery";
pub const BRIDGE_SERVER_DELIVERY: &str = "bridge.server_delivery";
pub const BRIDGE_FORWARDER_DELIVERY: &str = "bridge.forwarder_delivery";
pub const BRIDGE_PATCH_DELIVERY: &str = "bridge.patch_delivery";
pub const EDITOR_PATCH_APPLY: &str = "editor.patch_apply";
pub const EDITOR_PAINT_ADJACENT: &str = "editor.paint_adjacent";
pub const SERVER_RECEIVE: &str = "server.receive";
pub const SERVER_EDIT_ACK: &str = "server.edit_ack";
pub const SYNTAX_QUEUE: &str = "syntax.queue";
pub const SYNTAX_START: &str = "syntax.start";
pub const SYNTAX_END: &str = "syntax.end";

pub const SYNTAX_LOGICAL_WORK_ITEMS: &str = "syntax.parse.logical_work_items";
pub const SYNTAX_PARSE_INVOCATIONS: &str = "syntax.parse.invocations";
pub const SYNTAX_PARSE_INCREMENTAL: &str = "syntax.parse.incremental";
pub const SYNTAX_PARSE_FULL: &str = "syntax.parse.full";
pub const SYNTAX_QUERY_RANGES: &str = "syntax.query.ranges";
pub const SYNTAX_QUERY_BYTES: &str = "syntax.query.bytes";
pub const SYNTAX_DECORATION_CHUNKS: &str = "syntax.decoration.chunks";
pub const SYNTAX_CANCELLED_SUPERSEDED: &str = "syntax.parse.cancelled_superseded";
pub const SYNTAX_EDIT_TO_PUBLISH: &str = "syntax.edit_to_publish";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerfConfig {
    enabled: bool,
}

impl PerfConfig {
    pub const fn disabled() -> Self {
        Self { enabled: false }
    }

    pub const fn enabled() -> Self {
        Self { enabled: true }
    }

    pub fn from_env() -> Self {
        Self {
            enabled: env::var(PERF_PROFILE_ENV)
                .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on")),
        }
    }

    pub const fn with_flag(self, flag_present: bool) -> Self {
        Self {
            enabled: self.enabled || flag_present,
        }
    }

    pub const fn is_enabled(self) -> bool {
        self.enabled
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetricValue {
    Duration { nanos: u128 },
    Counter { amount: u64 },
    Gauge { value: u64 },
    Bytes { bytes: u64 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricMetadata {
    pub trace_id: Option<crate::protocol::PerformanceTraceId>,
    pub document_id: Option<u64>,
    pub client_id: Option<u64>,
    pub transaction_id: Option<u64>,
    pub version: Option<u64>,
    pub byte_count: Option<u64>,
    pub sanitized_path: Option<String>,
}

impl MetricMetadata {
    pub fn document(document_id: u64, version: u64) -> Self {
        Self {
            document_id: Some(document_id),
            version: Some(version),
            ..Self::default()
        }
    }

    pub fn with_trace_id(mut self, trace_id: Option<crate::protocol::PerformanceTraceId>) -> Self {
        self.trace_id = trace_id;
        self
    }

    pub fn with_byte_count(mut self, byte_count: u64) -> Self {
        self.byte_count = Some(byte_count);
        self
    }

    pub fn transaction(
        document_id: u64,
        client_id: u64,
        transaction_id: u64,
        version: u64,
    ) -> Self {
        Self {
            trace_id: Some(transaction_id),
            document_id: Some(document_id),
            client_id: Some(client_id),
            transaction_id: Some(transaction_id),
            version: Some(version),
            ..Self::default()
        }
    }

    pub fn with_sanitized_path(path: impl AsRef<Path>) -> Self {
        Self {
            sanitized_path: Some(sanitize_path(path.as_ref())),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricSnapshot {
    pub name: &'static str,
    pub value: MetricValue,
    pub metadata: MetricMetadata,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSummary {
    pub count: u64,
    pub duration_samples: u64,
    pub p50_nanos: u128,
    pub p95_nanos: u128,
    pub max_nanos: u128,
    #[serde(skip)]
    durations: Vec<u128>,
}

impl MetricSummary {
    fn finish(&mut self) {
        self.duration_samples = self.durations.len() as u64;
        if self.durations.is_empty() {
            return;
        }
        self.durations.sort_unstable();
        self.p50_nanos = percentile(&self.durations, 50);
        self.p95_nanos = percentile(&self.durations, 95);
        self.max_nanos = *self.durations.last().expect("duration is non-empty");
        self.durations.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfSummary {
    pub schema_version: u32,
    pub enabled: bool,
    pub retained_events: usize,
    pub dropped_events: u64,
    pub metrics: BTreeMap<String, MetricSummary>,
}

#[derive(Debug, Clone)]
pub struct PerfRecorder {
    inner: Option<Arc<Mutex<PerfBuffer>>>,
}

#[derive(Debug, Default)]
struct PerfBuffer {
    snapshots: Vec<MetricSnapshot>,
    dropped: u64,
}

impl PerfRecorder {
    pub fn from_config(config: PerfConfig) -> Self {
        if config.is_enabled() {
            Self {
                inner: Some(Arc::new(Mutex::new(PerfBuffer::default()))),
            }
        } else {
            Self::noop()
        }
    }

    pub const fn noop() -> Self {
        Self { inner: None }
    }

    pub fn for_test(enabled: bool) -> Self {
        Self::from_config(if enabled {
            PerfConfig::enabled()
        } else {
            PerfConfig::disabled()
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub fn scope(&self, name: &'static str) -> PerfScope {
        match &self.inner {
            Some(inner) => PerfScope {
                name,
                start: Some(Instant::now()),
                metadata: MetricMetadata::default(),
                inner: Some(Arc::clone(inner)),
            },
            None => PerfScope::noop(),
        }
    }

    pub fn scope_with_metadata(&self, name: &'static str, metadata: MetricMetadata) -> PerfScope {
        match &self.inner {
            Some(inner) => PerfScope {
                name,
                start: Some(Instant::now()),
                metadata,
                inner: Some(Arc::clone(inner)),
            },
            None => PerfScope::noop(),
        }
    }

    pub fn record_counter(&self, name: &'static str, amount: u64) {
        self.record(
            name,
            MetricValue::Counter { amount },
            MetricMetadata::default(),
        );
    }

    pub fn record_gauge(&self, name: &'static str, value: u64) {
        self.record(
            name,
            MetricValue::Gauge { value },
            MetricMetadata::default(),
        );
    }

    pub fn record_bytes(&self, name: &'static str, bytes: u64) {
        self.record(
            name,
            MetricValue::Bytes { bytes },
            MetricMetadata::default(),
        );
    }

    pub fn record_with_metadata(
        &self,
        name: &'static str,
        value: MetricValue,
        metadata: MetricMetadata,
    ) {
        self.record(name, value, metadata);
    }

    pub fn snapshots(&self) -> Vec<MetricSnapshot> {
        self.inner
            .as_ref()
            .map(|inner| {
                inner
                    .lock()
                    .expect("perf recorder poisoned")
                    .snapshots
                    .clone()
            })
            .unwrap_or_default()
    }

    pub fn dropped_snapshots(&self) -> u64 {
        self.inner
            .as_ref()
            .map(|inner| inner.lock().expect("perf recorder poisoned").dropped)
            .unwrap_or_default()
    }

    pub fn summary(&self) -> PerfSummary {
        let snapshots = self.snapshots();
        let mut metrics: BTreeMap<String, MetricSummary> = BTreeMap::new();
        for snapshot in &snapshots {
            let summary = metrics.entry(snapshot.name.to_string()).or_default();
            summary.count += 1;
            if let MetricValue::Duration { nanos } = &snapshot.value {
                summary.durations.push(*nanos);
            }
        }
        for summary in metrics.values_mut() {
            summary.finish();
        }
        PerfSummary {
            schema_version: PERF_TRACE_SCHEMA_VERSION,
            enabled: self.is_enabled(),
            retained_events: snapshots.len(),
            dropped_events: self.dropped_snapshots(),
            metrics,
        }
    }

    fn record(&self, name: &'static str, value: MetricValue, metadata: MetricMetadata) {
        if let Some(inner) = &self.inner {
            push_snapshot(
                &mut inner.lock().expect("perf recorder poisoned"),
                MetricSnapshot {
                    name,
                    value,
                    metadata,
                },
            );
        }
    }
}

impl Default for PerfRecorder {
    fn default() -> Self {
        Self::noop()
    }
}

#[derive(Debug)]
pub struct PerfScope {
    name: &'static str,
    start: Option<Instant>,
    metadata: MetricMetadata,
    inner: Option<Arc<Mutex<PerfBuffer>>>,
}

impl PerfScope {
    const fn noop() -> Self {
        Self {
            name: "",
            start: None,
            metadata: MetricMetadata {
                trace_id: None,
                document_id: None,
                client_id: None,
                transaction_id: None,
                version: None,
                byte_count: None,
                sanitized_path: None,
            },
            inner: None,
        }
    }

    pub fn finish(mut self) -> Option<Duration> {
        let elapsed = self.start.take().map(|start| start.elapsed());
        if let (Some(duration), Some(inner)) = (elapsed, &self.inner) {
            push_snapshot(
                &mut inner.lock().expect("perf recorder poisoned"),
                MetricSnapshot {
                    name: self.name,
                    value: MetricValue::Duration {
                        nanos: duration.as_nanos(),
                    },
                    metadata: self.metadata.clone(),
                },
            );
        }
        elapsed
    }
}

impl Drop for PerfScope {
    fn drop(&mut self) {
        let Some(start) = self.start.take() else {
            return;
        };
        let Some(inner) = &self.inner else {
            return;
        };
        push_snapshot(
            &mut inner.lock().expect("perf recorder poisoned"),
            MetricSnapshot {
                name: self.name,
                value: MetricValue::Duration {
                    nanos: start.elapsed().as_nanos(),
                },
                metadata: self.metadata.clone(),
            },
        );
    }
}

fn push_snapshot(buffer: &mut PerfBuffer, snapshot: MetricSnapshot) {
    if buffer.snapshots.len() < PERF_SNAPSHOT_CAPACITY {
        buffer.snapshots.push(snapshot);
    } else {
        buffer.dropped = buffer.dropped.saturating_add(1);
    }
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let index = (sorted.len() * percentile).div_ceil(100).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

static GLOBAL_RECORDER: OnceLock<PerfRecorder> = OnceLock::new();

pub fn install_global_recorder(config: PerfConfig) {
    let _ = GLOBAL_RECORDER.set(PerfRecorder::from_config(config));
}

pub fn global_recorder() -> PerfRecorder {
    GLOBAL_RECORDER
        .get_or_init(|| PerfRecorder::from_config(PerfConfig::from_env()))
        .clone()
}

/// Writes this process's sanitized performance summary under
/// [`PERF_REPORT_DIR_ENV`] as `<label>-perf-summary.json` (atomic rename).
/// Returns the written path, or `None` when the harness environment is unset
/// (the production default) or the label is empty after sanitization.
pub fn write_perf_report(label: &str) -> Option<PathBuf> {
    let dir = PathBuf::from(env::var_os(PERF_REPORT_DIR_ENV)?);
    let slug = sanitize_report_label(label)?;
    let json = serde_json::to_vec(&global_recorder().summary()).ok()?;
    write_report_file(&dir, &format!("{slug}-perf-summary.json"), &json)
}

/// Reduces a harness label to `[A-Za-z0-9-]`; `None` when nothing survives.
pub fn sanitize_report_label(label: &str) -> Option<String> {
    let slug: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if slug.is_empty() { None } else { Some(slug) }
}

fn write_report_file(dir: &Path, file_name: &str, bytes: &[u8]) -> Option<PathBuf> {
    let path = dir.join(file_name);
    let tmp = dir.join(format!("{file_name}.tmp"));
    std::fs::write(&tmp, bytes).ok()?;
    std::fs::rename(&tmp, &path).ok()?;
    Some(path)
}

pub fn sanitize_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(|name| format!("<redacted-path>/{name}"))
        .unwrap_or_else(|| "<redacted-path>".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_recorder_is_disabled_without_env_or_flag() {
        let recorder = PerfRecorder::from_config(PerfConfig::disabled());
        let _scope = recorder.scope("editor.visible_extraction");
        recorder.record_counter("editor.input", 1);

        assert!(!recorder.is_enabled());
        assert!(recorder.snapshots().is_empty());
    }

    #[test]
    fn perf_recorder_enables_only_with_env_flag_or_test_helper() {
        assert!(PerfConfig::disabled().with_flag(true).is_enabled());
        assert!(PerfRecorder::for_test(true).is_enabled());
        assert!(!PerfRecorder::for_test(false).is_enabled());
    }

    #[test]
    fn perf_recorder_noop_does_not_allocate_snapshots() {
        let recorder = PerfRecorder::noop();
        for _ in 0..100 {
            let _span = recorder.scope("editor.input");
            recorder.record_bytes("ipc.payload_bytes", 12);
        }
        assert!(recorder.snapshots().is_empty());
    }

    #[test]
    fn write_perf_report_sanitizes_labels_and_requires_env() {
        let dir = env::temp_dir().join("clay-perf-report-test");
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: single-threaded env mutation inside this test process scope.
        unsafe { env::set_var(PERF_REPORT_DIR_ENV, &dir) };
        let written = write_perf_report("run/1").expect("report written");
        assert!(
            written
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("run-1-")
        );
        let parsed: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&written).unwrap()).unwrap();
        assert_eq!(
            parsed["schemaVersion"],
            serde_json::json!(PERF_TRACE_SCHEMA_VERSION)
        );
        // SAFETY: same single-threaded scope as above.
        unsafe { env::remove_var(PERF_REPORT_DIR_ENV) };
        assert!(write_perf_report("run").is_none());
        let slug = sanitize_report_label("reference/host run").unwrap();
        assert!(slug.contains('-'));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn perf_snapshot_sanitizes_paths_and_content() {
        let recorder = PerfRecorder::for_test(true);
        recorder.record_with_metadata(
            "workspace.open.path",
            MetricValue::Counter { amount: 1 },
            MetricMetadata::with_sanitized_path("/home/alice/project/secret-note.txt"),
        );

        let snapshot = recorder.snapshots().pop().expect("snapshot recorded");
        assert_eq!(
            snapshot.metadata.sanitized_path.as_deref(),
            Some("<redacted-path>/secret-note.txt")
        );
        let debug = format!("{snapshot:?}");
        assert!(!debug.contains("/home/alice"));
        assert!(!debug.contains("document text"));
        assert!(!debug.contains("function secret"));
    }

    #[test]
    fn scope_records_duration_when_enabled() {
        let recorder = PerfRecorder::for_test(true);
        {
            let _scope = recorder.scope("editor.visible_extraction");
        }
        assert_eq!(recorder.snapshots()[0].name, "editor.visible_extraction");
    }

    #[test]
    fn summary_reports_duration_percentiles_and_counts() {
        let recorder = PerfRecorder::for_test(true);
        for nanos in [1, 2, 3, 4, 5] {
            recorder.record_with_metadata(
                EDITOR_PAINT_ADJACENT,
                MetricValue::Duration { nanos },
                MetricMetadata::default().with_trace_id(Some(7)),
            );
        }
        let summary = recorder.summary();
        let metric = summary
            .metrics
            .get(EDITOR_PAINT_ADJACENT)
            .expect("metric summary");
        assert_eq!(metric.count, 5);
        assert_eq!(metric.p50_nanos, 3);
        assert_eq!(metric.p95_nanos, 5);
        assert_eq!(metric.max_nanos, 5);
    }

    #[test]
    fn capacity_drops_events_without_growing_buffer() {
        let recorder = PerfRecorder::for_test(true);
        for _ in 0..=PERF_SNAPSHOT_CAPACITY {
            recorder.record_counter(SERVER_RECEIVE, 1);
        }
        assert_eq!(recorder.snapshots().len(), PERF_SNAPSHOT_CAPACITY);
        assert_eq!(recorder.dropped_snapshots(), 1);
    }
}
