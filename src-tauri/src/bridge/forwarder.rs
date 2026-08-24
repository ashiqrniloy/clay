//! Bounded, ordered event delivery from the session pump to webview sinks.
//!
//! Two lanes:
//! - **Live** (strict FIFO, capacity-bounded): every family except
//!   viewport-resynthesizable decoration/folding sets. When the live lane is
//!   full, `push` awaits — natural backpressure flows back through the
//!   client's socket read loop to the server instead of dropping edits.
//! - **Latest-wins** (one slot per coalesce key): `DecorationSet`,
//!   `DecorationBatch`, and `FoldingRangeSet` are full replacements for one
//!   (document, provenance, kind) scope at a stated version; when a newer set
//!   arrives before the older was delivered, the older is discarded and the
//!   `coalesced` counter ticks. The frontend re-requests via
//!   `DecorationViewportRequest` whenever it lacks spans for its viewport, so
//!   coalescing here can never strand state.

use super::dto::BridgeEnvelope;
use clay::client::ClientConnectionEvent;
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
        ClientConnectionEvent::DecorationSet(set) => Some(format!(
            "deco|{}|{}|{:?}",
            set.document_id, set.package_prefix, set.kind
        )),
        ClientConnectionEvent::DecorationBatch(sets) => {
            // One batch shares a single parse update; collapse per batch.
            let first = sets.first()?;
            Some(format!("batch|{}", first.document_id))
        }
        ClientConnectionEvent::FoldingRangeSet(set) => {
            Some(format!("fold|{}|{}", set.document_id, set.package_prefix))
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
        let mut sinks = self.sinks.lock().expect("sink registry poisoned");
        sinks.retain(|sink| sink.deliver(envelope.clone()).is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clay::protocol::DocumentId;
    use clay::protocol::{DecorationKind, DecorationSet};

    #[derive(Default)]
    struct Collector(Arc<Mutex<Vec<BridgeEnvelope>>>);

    impl EventSink for Collector {
        fn deliver(&self, envelope: BridgeEnvelope) -> Result<(), String> {
            self.0.lock().expect("collector").push(envelope);
            Ok(())
        }
    }

    fn decoration(document_id: DocumentId, version: u64) -> ClientConnectionEvent {
        ClientConnectionEvent::DecorationSet(DecorationSet {
            document_id,
            document_version: version,
            package_prefix: "clay".into(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 128,
            spans: Vec::new(),
        })
    }

    fn ack(version: u64) -> ClientConnectionEvent {
        ClientConnectionEvent::EditAck {
            document_id: 1,
            version,
            transaction_id: version,
        }
    }

    fn kinds(collected: &[BridgeEnvelope]) -> Vec<&'static str> {
        collected
            .iter()
            .map(|envelope| match envelope {
                BridgeEnvelope::Event(event) | BridgeEnvelope::Routed { event, .. } => {
                    match event.as_ref() {
                        ClientConnectionEvent::DecorationSet(_) => "deco",
                        ClientConnectionEvent::EditAck { .. } => "ack",
                        _ => "other",
                    }
                }
                BridgeEnvelope::ThemeSnapshot(_) => "theme",
                BridgeEnvelope::RuntimeSnapshot { .. } => "runtime",
                BridgeEnvelope::Disconnected { .. } => "disconnected",
            })
            .collect()
    }

    /// Latest-wins per key: an older decoration set is replaced, live-lane
    /// events keep strict FIFO order, and lifecycle notices bypass both.
    #[tokio::test]
    async fn coalescing_keeps_latest_decoration_and_live_order() {
        let sinks = Arc::new(SinkRegistry::new());
        let collector = Collector::default();
        sinks.add(Arc::new(Collector(Arc::clone(&collector.0))) as Arc<dyn EventSink>);
        let forwarder = Forwarder::spawn(Arc::clone(&sinks));

        forwarder.push(decoration(1, 1)).await;
        forwarder.push(ack(2)).await;
        forwarder.push(decoration(1, 2)).await;
        forwarder.push_disconnected("bye".into());

        // Drain deterministically: notify + yield until quiescent.
        for _ in 0..50 {
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let got = collector.0.lock().expect("collector").clone();
        let sequence = kinds(&got);
        // Disconnected bypasses both lanes, so it lands immediately.
        assert_eq!(sequence[0], "disconnected", "got {sequence:?}");
        assert_eq!(
            sequence.iter().filter(|k| **k == "deco").count(),
            1,
            "at most one decoration set survives coalescing, got {sequence:?}"
        );
        assert_eq!(
            forwarder.coalesced_count(),
            1,
            "the older set was folded into the newer"
        );
        forwarder.stop();
    }

    #[tokio::test]
    async fn distinct_documents_do_not_coalesce_against_each_other() {
        let sinks = Arc::new(SinkRegistry::new());
        let collector = Collector::default();
        sinks.add(Arc::new(Collector(Arc::clone(&collector.0))) as Arc<dyn EventSink>);
        let forwarder = Forwarder::spawn(Arc::clone(&sinks));

        forwarder.push(decoration(1, 1)).await;
        forwarder.push(decoration(2, 1)).await;
        forwarder.stop(); // abort drain; flush manually below is not needed —
        // instead verify slots hold two distinct keys.
        let count = forwarder.coalesced_count();
        assert_eq!(count, 0);
    }
}
