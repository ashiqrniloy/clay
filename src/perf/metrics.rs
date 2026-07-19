use std::{
    env,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

/// Environment variable that enables Clay's internal developer performance recorder.
pub const PERF_PROFILE_ENV: &str = "CLAY_PERF_PROFILE";
/// Developer CLI flag accepted by smoke/client/server workflows to enable profiling.
pub const PERF_PROFILE_FLAG: &str = "--profile-perf";
/// Maximum retained developer metric snapshots per recorder.
pub const PERF_SNAPSHOT_CAPACITY: usize = 4_096;

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
    pub document_id: Option<u64>,
    pub client_id: Option<u64>,
    pub transaction_id: Option<u64>,
    pub version: Option<u64>,
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

    pub fn transaction(
        document_id: u64,
        client_id: u64,
        transaction_id: u64,
        version: u64,
    ) -> Self {
        Self {
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

#[derive(Debug, Clone)]
pub struct PerfRecorder {
    inner: Option<Arc<Mutex<Vec<MetricSnapshot>>>>,
}

impl PerfRecorder {
    pub fn from_config(config: PerfConfig) -> Self {
        if config.is_enabled() {
            Self {
                inner: Some(Arc::new(Mutex::new(Vec::new()))),
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
            .map(|inner| inner.lock().expect("perf recorder poisoned").clone())
            .unwrap_or_default()
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
    inner: Option<Arc<Mutex<Vec<MetricSnapshot>>>>,
}

impl PerfScope {
    const fn noop() -> Self {
        Self {
            name: "",
            start: None,
            metadata: MetricMetadata {
                document_id: None,
                client_id: None,
                transaction_id: None,
                version: None,
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

fn push_snapshot(snapshots: &mut Vec<MetricSnapshot>, snapshot: MetricSnapshot) {
    if snapshots.len() < PERF_SNAPSHOT_CAPACITY {
        snapshots.push(snapshot);
    }
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
}
