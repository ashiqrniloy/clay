//! Bounded broadcast lanes shared by the JS runtime, op state, and the
//! connection loops (Plan 132 U2/C5).
//!
//! Two shapes cover every host-owned lane: [`Fanout`] for transient advice
//! (lagged receivers drop), and [`StateFanout`] for state that also carries a
//! current value (lagged receivers and late subscribers replay it).

use crate::lock_util::LockOrRecover;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::broadcast;

/// Bounded broadcast lane with no current value.
///
/// Advisory lanes carry transient notifications where only live delivery
/// matters: a lagged receiver drops the messages it missed instead of
/// replaying them.
pub(crate) struct Fanout<T> {
    tx: broadcast::Sender<T>,
}

impl<T: Clone> Fanout<T> {
    /// Bounded channel holding `capacity` messages per subscriber.
    pub(crate) fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Send `value` to live subscribers. Sends with no subscriber are
    /// dropped, so the lane never blocks or buffers unboundedly.
    pub(crate) fn publish(&self, value: T) {
        let _ = self.tx.send(value);
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<T> {
        self.tx.subscribe()
    }
}

impl<T> Clone for Fanout<T> {
    /// Shares the channel; subscribers of every clone observe one stream.
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

/// Lane handles debug as their shape: the state itself belongs to the store
/// (and is not a lane's to print), so no `T: Debug` bound is required.
impl<T> std::fmt::Debug for Fanout<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Fanout")
    }
}

impl<T> std::fmt::Debug for StateFanout<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StateFanout")
    }
}

/// Bounded broadcast lane plus the shared current value.
///
/// State lanes replay: `current` is the recovery source for connection
/// initial sync and for a receiver that observed
/// [`broadcast::error::RecvError::Lagged`] (Plan 071 state policy). Every
/// `publish` records the value before sending, so the store is never behind
/// the channel.
pub(crate) struct StateFanout<T> {
    fanout: Fanout<T>,
    current: Arc<Mutex<T>>,
}

impl<T: Clone> StateFanout<T> {
    /// Bounded channel plus a seeded current value.
    pub(crate) fn new(capacity: usize, initial: T) -> Self {
        Self {
            fanout: Fanout::new(capacity),
            current: Arc::new(Mutex::new(initial)),
        }
    }

    /// Record `value` as the current value and send it to live subscribers.
    pub(crate) fn publish(&self, value: T) {
        *self.lock() = value.clone();
        self.fanout.publish(value);
    }

    /// Current value, cloned out for initial sync and lag replay.
    pub(crate) fn current(&self) -> T {
        self.lock().clone()
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<T> {
        self.fanout.subscribe()
    }

    fn lock(&self) -> MutexGuard<'_, T> {
        self.current.lock_or_recover()
    }
}

impl<T> Clone for StateFanout<T> {
    /// Shares the channel and the current-value store.
    fn clone(&self) -> Self {
        Self {
            fanout: self.fanout.clone(),
            current: Arc::clone(&self.current),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast::error::{RecvError, TryRecvError};

    /// A subscriber that arrives after a publish finds no buffered message —
    /// `current` is the replay source connection initial sync uses.
    #[tokio::test]
    async fn late_subscriber_replays_current() {
        let fanout = StateFanout::new(4, 0u32);
        fanout.publish(7);

        let mut late = fanout.subscribe();
        assert!(matches!(late.try_recv(), Err(TryRecvError::Empty)));
        assert_eq!(fanout.current(), 7);
    }

    /// Plan 071 lag replay: a receiver that overflowed the channel recovers
    /// from `current`, not from the remaining backlog.
    #[tokio::test]
    async fn lagged_receiver_replays_current() {
        let fanout = StateFanout::new(2, 0u32);
        let mut receiver = fanout.subscribe();
        for value in 1..=4 {
            fanout.publish(value);
        }

        assert!(matches!(
            receiver.recv().await,
            Err(RecvError::Lagged(skipped)) if skipped >= 2
        ));
        assert_eq!(fanout.current(), 4);
    }

    /// Capacity is the constructor's, not the publisher's: a receiver that
    /// never falls behind sees every buffered message in order, and exactly
    /// `capacity` missed messages are reported once it overflows.
    #[tokio::test]
    async fn publish_honors_configured_capacity() {
        let fanout = Fanout::new(4);
        let mut receiver = fanout.subscribe();
        for value in 0..4 {
            fanout.publish(value);
        }
        for value in 0..4 {
            assert_eq!(receiver.try_recv().expect("buffered message"), value);
        }
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));

        // Five publishes into a capacity-4 channel overflow by exactly one.
        for value in 0..5 {
            fanout.publish(value);
        }
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Lagged(1))));
        assert_eq!(receiver.try_recv().expect("backlog survives"), 1);
    }

    /// A poisoned recovered-class lock keeps serving (plan 134 D5): the lane's
    /// state store recovers, so a panicked critical section cannot kill the
    /// connection task that publishes or replays through it.
    #[tokio::test]
    async fn poisoned_state_mutex_recovers_service() {
        let fanout = StateFanout::new(4, 1u32);
        let poisoner = fanout.clone();
        std::thread::spawn(move || {
            let _guard = poisoner.lock();
            panic!("poison the state store");
        })
        .join()
        .ok();

        let mut receiver = fanout.subscribe();
        fanout.publish(7);
        assert_eq!(fanout.current(), 7);
        assert_eq!(receiver.recv().await.expect("lane stays open"), 7);
    }

    /// Clones share one channel and one store, which is what lets a reloaded
    /// service and the op states publish into the same lane.
    #[tokio::test]
    async fn clones_share_channel_and_state() {
        let fanout = StateFanout::new(4, 0u32);
        let clone = fanout.clone();
        let mut receiver = fanout.subscribe();

        clone.publish(3);

        assert_eq!(receiver.recv().await.expect("lane stays open"), 3);
        assert_eq!(fanout.current(), 3);
    }
}
