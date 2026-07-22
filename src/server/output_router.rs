//! Per-connection output routing for server-published document payloads.
//!
//! Plan 060 T4 (P0-3): coordinators publish parse updates, diagnostics, and
//! document-analysis output into shared channels. Every connection previously
//! raced to drain those channels, so one client could receive (or steal)
//! another client's document payloads. `OutputRouter` keeps per-client bounded
//! senders plus a document-to-client subscription index so payloads reach only
//! connections that opened the document; connection-scoped payloads (sanitized
//! runtime diagnostics) broadcast to every subscribed connection.

use std::collections::{HashMap, HashSet};

use tokio::sync::mpsc;

use crate::protocol::{ClientId, DocumentId};

/// Bounded per-connection subscription channel. A full channel drops the
/// payload (fail closed): stale decorations recover on the next edit or
/// viewport request, and no unbounded memory grows behind a slow client.
pub(crate) const OUTPUT_SUBSCRIPTION_CAPACITY: usize = 64;

#[derive(Debug)]
pub(crate) struct OutputRouter<T> {
    client_senders: HashMap<ClientId, mpsc::Sender<T>>,
    document_index: HashMap<DocumentId, HashSet<ClientId>>,
}

impl<T> Default for OutputRouter<T> {
    fn default() -> Self {
        Self {
            client_senders: HashMap::new(),
            document_index: HashMap::new(),
        }
    }
}

impl<T: Clone> OutputRouter<T> {
    /// Register one connection's delivery channel. Called once per connection;
    /// later document subscriptions reuse the same sender.
    pub(crate) fn subscribe_client(&mut self, client_id: ClientId) -> mpsc::Receiver<T> {
        let (tx, rx) = mpsc::channel(OUTPUT_SUBSCRIPTION_CAPACITY);
        self.client_senders.insert(client_id, tx);
        rx
    }

    /// Authorize `client_id` to receive payloads for `document_id`. No-op when
    /// the connection has not registered a sender yet.
    pub(crate) fn subscribe_document(&mut self, document_id: DocumentId, client_id: ClientId) {
        if self.client_senders.contains_key(&client_id) {
            self.document_index
                .entry(document_id)
                .or_default()
                .insert(client_id);
        }
    }

    pub(crate) fn unsubscribe_document(&mut self, document_id: DocumentId, client_id: ClientId) {
        if let Some(subscribers) = self.document_index.get_mut(&document_id) {
            subscribers.remove(&client_id);
            if subscribers.is_empty() {
                self.document_index.remove(&document_id);
            }
        }
    }

    /// Remove every subscription held by one connection (disconnect/close).
    pub(crate) fn unsubscribe_client(&mut self, client_id: ClientId) {
        self.client_senders.remove(&client_id);
        self.document_index.retain(|_, subscribers| {
            subscribers.remove(&client_id);
            !subscribers.is_empty()
        });
    }

    /// Deliver one document-scoped payload to every subscribed connection.
    /// Returns the number of connections that accepted the payload.
    pub(crate) fn route_document(&self, document_id: DocumentId, payload: &T) -> usize {
        let mut delivered = 0;
        if let Some(subscribers) = self.document_index.get(&document_id) {
            for client_id in subscribers {
                if let Some(sender) = self.client_senders.get(client_id)
                    && sender.try_send(payload.clone()).is_ok()
                {
                    delivered += 1;
                }
            }
        }
        delivered
    }

    /// Deliver one connection-scoped payload (e.g. a sanitized runtime
    /// diagnostic) to every subscribed connection.
    pub(crate) fn broadcast(&self, payload: &T) -> usize {
        let mut delivered = 0;
        for sender in self.client_senders.values() {
            if sender.try_send(payload.clone()).is_ok() {
                delivered += 1;
            }
        }
        delivered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn document_payload_reaches_only_subscribed_clients() {
        let mut router = OutputRouter::<u32>::default();
        let mut client_a = router.subscribe_client(1);
        let mut client_b = router.subscribe_client(2);
        router.subscribe_document(7, 1);

        assert_eq!(router.route_document(7, &42), 1);
        assert_eq!(client_a.recv().await, Some(42));
        assert!(client_b.try_recv().is_err());

        router.subscribe_document(7, 2);
        assert_eq!(router.route_document(7, &43), 2);
        assert_eq!(client_a.recv().await, Some(43));
        assert_eq!(client_b.recv().await, Some(43));
    }

    #[tokio::test]
    async fn unsubscribe_client_stops_all_delivery() {
        let mut router = OutputRouter::<u32>::default();
        let mut client_a = router.subscribe_client(1);
        router.subscribe_document(7, 1);
        router.subscribe_document(9, 1);
        router.unsubscribe_client(1);

        assert_eq!(router.route_document(7, &1), 0);
        assert_eq!(router.route_document(9, &2), 0);
        assert!(client_a.recv().await.is_none());
    }

    #[tokio::test]
    async fn broadcast_reaches_every_subscribed_client() {
        let mut router = OutputRouter::<u32>::default();
        let mut client_a = router.subscribe_client(1);
        let mut client_b = router.subscribe_client(2);

        assert_eq!(router.broadcast(&7), 2);
        assert_eq!(client_a.recv().await, Some(7));
        assert_eq!(client_b.recv().await, Some(7));
    }

    #[tokio::test]
    async fn full_channel_drops_payload_without_blocking() {
        let mut router = OutputRouter::<u32>::default();
        let _client_a = router.subscribe_client(1);
        router.subscribe_document(7, 1);
        for value in 0..OUTPUT_SUBSCRIPTION_CAPACITY {
            assert_eq!(router.route_document(7, &(value as u32)), 1);
        }
        // Next payload is dropped (fail closed), not queued or blocking.
        assert_eq!(router.route_document(7, &999), 0);
    }
}
