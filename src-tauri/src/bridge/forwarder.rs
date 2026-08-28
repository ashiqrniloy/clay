//! Bounded, ordered event delivery from the session pump to webview sinks.
//!
//! Two lanes:
//! - **Live** (strict FIFO, capacity-bounded): every family except
//!   viewport-resynthesizable whole patches. When the live lane is full,
//!   `push` awaits — natural backpressure flows back through the client's
//!   socket read loop to the server instead of dropping edits.
//! - **Latest-wins** (one slot per document): protocol v29
//!   `ViewportRenderPatch` values are complete atomic answers to one request
//!   id. A newer patch for the same document supersedes any undelivered older
//!   patch wholesale (the client drops stale request ids anyway); members
//!   inside one patch never coalesce against each other, so sibling
//!   ranges/packages/features cannot overwrite one another in Tauri.

use super::dto::BridgeEnvelope;
use clay::client::ClientConnectionEvent;
use clay::perf::metrics::{BRIDGE_FORWARDER_DELIVERY, MetricMetadata, global_recorder};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// A subscribed webview endpoint. Errors mean the subscriber is gone
/// (window closed, channel dropped) and the subscription must be removed.
pub trait EventSink: Send + Sync + 'static {
    fn deliver(&self, envelope: BridgeEnvelope) -> Result<(), String>;
}

/// Stable identity of one replaceable stream slot.
pub fn coalesce_key(event: &ClientConnectionEvent) -> Option<String> {
    match event {
        ClientConnectionEvent::ViewportRenderPatch(patch) => {
            Some(format!("vrpatch|{}", patch.document_id))
        }
        _ => None,
    }
}

const LIVE_CAPACITY: usize = 512;

pub(crate) struct Forwarder {
    sinks: Arc<SinkRegistry>,
    drain: tokio::task::AbortHandle,
    live_tx: mpsc::Sender<BridgeEnvelope>,
    /// Newest-wins slots keyed by [`coalesce_key`]; flushed after each live
    /// delivery or notification. Bounded by distinct keys in flight.
    latest: Arc<Mutex<HashMap<String, BridgeEnvelope>>>,
    flush_notify: Arc<tokio::sync::Notify>,
    coalesced: AtomicU64,
}

impl Forwarder {
    /// Spawns the drain task. Stop it with [`Forwarder::stop`] on session
    /// teardown.
    pub(crate) fn spawn(sinks: Arc<SinkRegistry>) -> Self {
        let (live_tx, mut live_rx) = mpsc::channel(LIVE_CAPACITY);
        let latest: Arc<Mutex<HashMap<String, BridgeEnvelope>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let flush_notify = Arc::new(tokio::sync::Notify::new());

        let task_latest = Arc::clone(&latest);
        let task_notify = Arc::clone(&flush_notify);
        let task_sinks = Arc::clone(&sinks);
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = live_rx.recv() => match incoming {
                        Some(envelope) => task_sinks.deliver_all(envelope),
                        None => break,
                    },
                    _ = task_notify.notified() => {}
                }
                let batch: Vec<BridgeEnvelope> = task_latest
                    .lock()
                    .expect("latest lock poisoned")
                    .drain()
                    .map(|(_, envelope)| envelope)
                    .collect();
                for envelope in batch {
                    task_sinks.deliver_all(envelope);
                }
            }
        });

        Self {
            sinks,
            drain: handle.abort_handle(),
            live_tx,
            latest,
            flush_notify,
            coalesced: AtomicU64::new(0),
        }
    }

    pub(crate) fn stop(&self) {
        self.drain.abort();
    }

    pub(crate) async fn push(&self, event: ClientConnectionEvent) {
        if let Some(key) = coalesce_key(&event) {
            let mut slot = self.latest.lock().expect("latest lock poisoned");
            self.coalesced.fetch_add(
                u64::from(
                    slot.insert(key, BridgeEnvelope::Event(Box::new(event)))
                        .is_some(),
                ),
                Ordering::Relaxed,
            );
            drop(slot);
            self.flush_notify.notify_one();
            return;
        }
        let envelope = BridgeEnvelope::Event(Box::new(event));
        // Live lane: bounded; blocking here applies backpressure upstream.
        match self.live_tx.try_send(envelope) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(back)) => {
                let _ = self.live_tx.send(back).await;
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                // Drain task gone: session is being torn down.
            }
        }
    }

    /// Lifecycle notice: bypasses both lanes so it reaches the webview even
    /// while decoration snapshots are pending or the live lane is saturated.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn push_disconnected(&self, reason: String) {
        self.push_disconnected_from(reason, None, None);
    }

    pub(crate) fn push_disconnected_from(
        &self,
        reason: String,
        client_id: Option<clay::protocol::ClientId>,
        tab_id: Option<clay::protocol::TabId>,
    ) {
        self.sinks.deliver_all(BridgeEnvelope::Disconnected {
            reason,
            client_id,
            tab_id,
        });
        self.flush_notify.notify_one();
    }

    pub(crate) async fn push_routed(
        &self,
        client_id: clay::protocol::ClientId,
        tab_id: Option<clay::protocol::TabId>,
        event: ClientConnectionEvent,
    ) {
        if let Some(key) = coalesce_key(&event) {
            let mut slot = self.latest.lock().expect("latest lock poisoned");
            self.coalesced.fetch_add(
                u64::from(
                    slot.insert(
                        key,
                        BridgeEnvelope::Routed {
                            client_id,
                            tab_id,
                            event: Box::new(event),
                        },
                    )
                    .is_some(),
                ),
                Ordering::Relaxed,
            );
            drop(slot);
            self.flush_notify.notify_one();
            return;
        }
        let envelope = BridgeEnvelope::Routed {
            client_id,
            tab_id,
            event: Box::new(event),
        };
        match self.live_tx.try_send(envelope) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(back)) => {
                let _ = self.live_tx.send(back).await;
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {}
        }
    }

    /// Resolved theme snapshot: bypasses the coalescing slots (each theme
    /// change must reach the DOM in order; snapshots are small and rare).
    pub(crate) fn push_theme_snapshot(&self, snapshot: super::dto::ThemeSnapshotDto) {
        self.sinks
            .deliver_all(BridgeEnvelope::ThemeSnapshot(snapshot));
        self.flush_notify.notify_one();
    }

    pub(crate) fn push_runtime_snapshot(
        &self,
        client_id: clay::protocol::ClientId,
        tab_id: Option<clay::protocol::TabId>,
        snapshot: super::dto::RuntimeSnapshotDto,
    ) {
        self.sinks.deliver_all(BridgeEnvelope::RuntimeSnapshot {
            client_id,
            tab_id,
            snapshot: Box::new(snapshot),
        });
        self.flush_notify.notify_one();
    }

    pub(crate) fn coalesced_count(&self) -> u64 {
        self.coalesced.load(Ordering::Relaxed)
    }
}

/// Registry of subscribed webview channels.
#[derive(Default)]
pub struct SinkRegistry {
    sinks: Mutex<Vec<Arc<dyn EventSink>>>,
}

impl SinkRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, sink: Arc<dyn EventSink>) {
        let mut sinks = self.sinks.lock().expect("sink registry poisoned");
        sinks.push(sink);
    }

    /// Removes every sink (unsubscribe). Returns how many were held.
    pub fn clear(&self) -> usize {
        let mut sinks = self.sinks.lock().expect("sink registry poisoned");
        std::mem::take(&mut *sinks).len()
    }

    fn deliver_all(&self, envelope: BridgeEnvelope) {
        let trace_id = envelope_trace_id(&envelope);
        let recorder = global_recorder();
        let delivery = recorder
            .is_enabled()
            .then(|| trace_id)
            .flatten()
            .map(|trace_id| {
                recorder.scope_with_metadata(
                    BRIDGE_FORWARDER_DELIVERY,
                    MetricMetadata::default().with_trace_id(Some(trace_id)),
                )
            });
        let mut sinks = self.sinks.lock().expect("sink registry poisoned");
        sinks.retain(|sink| sink.deliver(envelope.clone()).is_ok());
        if let Some(delivery) = delivery {
            delivery.finish();
        }
    }
}

fn envelope_trace_id(envelope: &BridgeEnvelope) -> Option<clay::protocol::PerformanceTraceId> {
    let event = match envelope {
        BridgeEnvelope::Event(event) => event.as_ref(),
        BridgeEnvelope::Routed { event, .. } => event.as_ref(),
        _ => return None,
    };
    match event {
        ClientConnectionEvent::EditAck { transaction_id, .. }
        | ClientConnectionEvent::EditRejected { transaction_id, .. } => Some(*transaction_id),
        ClientConnectionEvent::DecorationSet(set) => set.trace_id,
        ClientConnectionEvent::DecorationBatch(sets) => sets.first().and_then(|set| set.trace_id),
        ClientConnectionEvent::ViewportRenderPatch(patch) => patch.trace_id,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clay::protocol::DocumentId;
    use clay::protocol::{
        DecorationKind, DecorationSet, ViewportRenderPatch, ViewportRenderStatus,
    };

    #[derive(Default)]
    struct Collector(Arc<Mutex<Vec<BridgeEnvelope>>>);

    impl EventSink for Collector {
        fn deliver(&self, envelope: BridgeEnvelope) -> Result<(), String> {
            self.0.lock().expect("collector").push(envelope);
            Ok(())
        }
    }

    fn patch(document_id: DocumentId, request_id: u64, members: usize) -> ClientConnectionEvent {
        ClientConnectionEvent::ViewportRenderPatch(ViewportRenderPatch {
            request_id,
            document_id,
            document_version: 2,
            status: ViewportRenderStatus::Complete,
            reason: None,
            covered_ranges: Vec::new(),
            decorations: (0..members)
                .map(|index| DecorationSet {
                    document_id,
                    document_version: 2,
                    package_prefix: "clay".into(),
                    kind: if index % 2 == 0 {
                        DecorationKind::Syntax
                    } else {
                        DecorationKind::Semantic
                    },
                    viewport_byte_start: index as u64 * 128,
                    viewport_byte_end: (index as u64 + 1) * 128,
                    spans: Vec::new(),
                    trace_id: None,
                })
                .collect(),
            diagnostics: Vec::new(),
            folds: Vec::new(),
            trace_id: None,
        })
    }

    fn ack(version: u64) -> ClientConnectionEvent {
        ClientConnectionEvent::EditAck {
            document_id: 1,
            version,
            transaction_id: version,
        }
    }

    fn delivered_patches(collected: &[BridgeEnvelope]) -> Vec<(u64, usize)> {
        collected
            .iter()
            .filter_map(|envelope| match envelope {
                BridgeEnvelope::Event(event) | BridgeEnvelope::Routed { event, .. } => {
                    match event.as_ref() {
                        ClientConnectionEvent::ViewportRenderPatch(patch) => {
                            Some((patch.request_id, patch.decorations.len()))
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect()
    }

    async fn drain() {
        for _ in 0..50 {
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    /// Obsolete whole patches coalesce per document; live-lane events keep
    /// strict FIFO order; lifecycle notices bypass both lanes.
    #[tokio::test]
    async fn coalescing_keeps_latest_whole_patch_and_live_order() {
        let sinks = Arc::new(SinkRegistry::new());
        let collector = Collector::default();
        sinks.add(Arc::new(Collector(Arc::clone(&collector.0))) as Arc<dyn EventSink>);
        let forwarder = Forwarder::spawn(Arc::clone(&sinks));

        forwarder.push(patch(1, 1, 24)).await;
        forwarder.push(ack(2)).await;
        forwarder.push(patch(1, 2, 24)).await;
        forwarder.push_disconnected("bye".into());

        drain().await;
        let got = collector.0.lock().expect("collector").clone();
        // Disconnected bypasses both lanes, so it lands immediately.
        assert!(matches!(
            got.first(),
            Some(BridgeEnvelope::Disconnected { .. })
        ));
        // The latest whole patch survives with all 24 sibling members intact.
        assert_eq!(delivered_patches(&got), vec![(2, 24)]);
        assert_eq!(
            forwarder.coalesced_count(),
            1,
            "the obsolete whole patch was folded into the newer one"
        );
        forwarder.stop();
    }

    /// Sibling ranges/packages/features inside one patch never overwrite one
    /// another: 24 mixed syntax/semantic members stay one complete patch.
    #[tokio::test]
    async fn sibling_members_stay_one_complete_patch() {
        let sinks = Arc::new(SinkRegistry::new());
        let collector = Collector::default();
        sinks.add(Arc::new(Collector(Arc::clone(&collector.0))) as Arc<dyn EventSink>);
        let forwarder = Forwarder::spawn(Arc::clone(&sinks));

        forwarder.push(patch(1, 5, 24)).await;
        drain().await;
        let got = collector.0.lock().expect("collector").clone();
        assert_eq!(delivered_patches(&got), vec![(5, 24)]);
        forwarder.stop();
    }

    /// Distinct documents never coalesce against each other.
    #[tokio::test]
    async fn distinct_documents_do_not_coalesce_against_each_other() {
        let sinks = Arc::new(SinkRegistry::new());
        let collector = Collector::default();
        sinks.add(Arc::new(Collector(Arc::clone(&collector.0))) as Arc<dyn EventSink>);
        let forwarder = Forwarder::spawn(Arc::clone(&sinks));

        forwarder.push(patch(1, 1, 2)).await;
        forwarder.push(patch(2, 1, 2)).await;
        drain().await;
        let got = collector.0.lock().expect("collector").clone();
        assert_eq!(
            delivered_patches(&got).len(),
            2,
            "both documents deliver their patch"
        );
        assert_eq!(forwarder.coalesced_count(), 0);
        forwarder.stop();
    }

    /// Edit-driven member frames travel the live lane in strict FIFO order.
    #[tokio::test]
    async fn edit_driven_members_keep_fifo_order() {
        let sinks = Arc::new(SinkRegistry::new());
        let collector = Collector::default();
        sinks.add(Arc::new(Collector(Arc::clone(&collector.0))) as Arc<dyn EventSink>);
        let forwarder = Forwarder::spawn(Arc::clone(&sinks));

        forwarder.push(ack(1)).await;
        forwarder.push(ack(2)).await;
        drain().await;
        let got = collector.0.lock().expect("collector").clone();
        let acks: Vec<u64> = got
            .iter()
            .filter_map(|envelope| match envelope {
                BridgeEnvelope::Event(event) => match event.as_ref() {
                    ClientConnectionEvent::EditAck { version, .. } => Some(*version),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(acks, vec![1, 2]);
        forwarder.stop();
    }
}
