//! Bounded per-document syntax sessions (Plan 099).
//!
//! One session owns the mailbox of the latest syntax job for one
//! document/grammar pair and one worker task. The worker snapshots nothing on
//! the hot path: the connection only enqueues an already-validated
//! notification, the mailbox keeps the latest compatible job, and the worker
//! runs CPU-bound native parsing on the bounded blocking executor
//! (`spawn_blocking` under [`SYNTAX_EXECUTOR_MAX_JOBS`] permits) or awaits
//! async package handlers on the normal runtime. Newer versions/viewports
//! coalesce latest-wins; a running job is never aborted mid-parse — its
//! result is simply dropped at publication time when a newer job superseded
//! it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tokio::sync::{Semaphore, watch};

use crate::perf::budgets::SYNTAX_EXECUTOR_MAX_JOBS;
use crate::protocol::ParseEditNotification;

/// Monotonic job sequence inside one session mailbox so the worker never
/// re-runs or misses a job even after the pending slot was drained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) struct JobSeq(u64);

#[derive(Debug, Clone)]
pub(crate) struct SessionJob {
    pub(crate) seq: JobSeq,
    pub(crate) notification: ParseEditNotification,
    /// Owning client of a request-scoped job (for completion routing).
    pub(crate) client_id: Option<crate::protocol::ClientId>,
    /// Schedule timestamp so the worker can measure queue wait.
    pub(crate) queued_at: std::time::Instant,
}

/// Latest-wins mailbox for one syntax session. The coordinator's session
/// entry holds the [`SessionMailbox`] (sender side); the session worker holds
/// the [`SessionReceiver`].
#[derive(Debug)]
pub(crate) struct SessionMailbox {
    tx: watch::Sender<Option<SessionJob>>,
    /// Kept alive so `send` never fails for want of a receiver between
    /// session creation and worker start.
    _rx: watch::Receiver<Option<SessionJob>>,
    next_seq: AtomicU64,
    closed: Arc<AtomicBool>,
    /// Latest job sequence the worker has taken, shared so `close` can tell a
    /// genuinely pending job from one the worker already drained (watch keeps
    /// the last value visible after delivery).
    observed: Arc<AtomicU64>,
}

impl Default for SessionMailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMailbox {
    pub(crate) fn new() -> Self {
        let (tx, rx) = watch::channel(None);
        Self {
            tx,
            _rx: rx,
            next_seq: AtomicU64::new(1),
            closed: Arc::new(AtomicBool::new(false)),
            observed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Enqueue the latest job, replacing any pending job (latest-wins).
    /// Returns the superseded pending job so the caller can complete a
    /// superseded viewport-render request id and keep the connection's
    /// pending-patch counter exact.
    pub(crate) fn push(
        &self,
        notification: ParseEditNotification,
        client_id: Option<crate::protocol::ClientId>,
        queued_at: std::time::Instant,
    ) -> Option<SessionJob> {
        let seq = JobSeq(self.next_seq.fetch_add(1, Ordering::Relaxed));
        let job = SessionJob {
            seq,
            notification,
            client_id,
            queued_at,
        };
        // Only a job the worker has NOT taken yet is genuinely superseded;
        // watch keeps the last delivered value visible after a drain.
        let superseded = self
            .tx
            .borrow()
            .clone()
            .filter(|job| job.seq.0 > self.observed.load(std::sync::atomic::Ordering::Acquire));
        let _ = self.tx.send(Some(job));
        superseded
    }

    /// Close the mailbox: any pending job is dropped (returned to the caller
    /// for request completion) and the worker exits after its current job.
    /// A running job finishes and its output is discarded by the coordinator.
    pub(crate) fn close(&self) -> Option<SessionJob> {
        let pending = self
            .tx
            .borrow()
            .clone()
            .filter(|job| job.seq.0 > self.observed.load(Ordering::Acquire));
        self.closed.store(true, Ordering::Release);
        let _ = self.tx.send(None);
        pending
    }

    pub(crate) fn receiver(&self) -> SessionReceiver {
        SessionReceiver {
            rx: self.tx.subscribe(),
            last_seq: JobSeq::default(),
            closed: Arc::clone(&self.closed),
            observed: Arc::clone(&self.observed),
        }
    }
}

/// Worker-side view of one session mailbox. `Send + 'static` so it can live
/// inside the spawned session worker task.
pub(crate) struct SessionReceiver {
    rx: watch::Receiver<Option<SessionJob>>,
    last_seq: JobSeq,
    closed: Arc<AtomicBool>,
    observed: Arc<AtomicU64>,
}

impl SessionReceiver {
    /// Wait for and return the latest unobserved job. Returns `None` once the
    /// mailbox is closed and drained.
    pub(crate) async fn recv(&mut self) -> Option<SessionJob> {
        loop {
            let visible = self.rx.borrow_and_update().clone();
            match visible.filter(|job| job.seq > self.last_seq) {
                Some(job) => {
                    self.last_seq = job.seq;
                    self.observed.store(job.seq.0, Ordering::Release);
                    return Some(job);
                }
                None => {
                    if self.closed.load(Ordering::Acquire) {
                        return None;
                    }
                    if self.rx.changed().await.is_err() {
                        return None;
                    }
                }
            }
        }
    }
}

/// Shared bounded blocking executor for native syntax jobs. A permit is held
/// for the whole CPU-bound parse so at most [`SYNTAX_EXECUTOR_MAX_JOBS`]
/// parser jobs run at once, regardless of how many documents or connections
/// are active; the permits also cap worst-case concurrent syntax memory.
#[derive(Debug, Clone)]
pub(crate) struct SyntaxExecutor {
    permits: Arc<Semaphore>,
}

impl Default for SyntaxExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxExecutor {
    pub(crate) fn new() -> Self {
        Self {
            permits: Arc::new(Semaphore::new(SYNTAX_EXECUTOR_MAX_JOBS)),
        }
    }

    /// Acquire one 'static permit that can be moved into a blocking task.
    pub(crate) async fn acquire(&self) -> tokio::sync::OwnedSemaphorePermit {
        Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .expect("syntax executor semaphore closed")
    }

    #[cfg(test)]
    pub(crate) fn available_permits(&self) -> usize {
        self.permits.available_permits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ParseByteRange, ParseEditNotification};

    fn notification(version: u64) -> ParseEditNotification {
        ParseEditNotification {
            document_id: 7,
            document_version: version,
            behavior_version: 1,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            viewport: ParseByteRange::new(0, 16),
            invalidated_ranges: Vec::new(),
            accepted_edit: None,
            parse_windows: Vec::new(),
            memory_budget: None,
            trace_id: None,
            request_id: None,
        }
    }

    #[tokio::test]
    async fn mailbox_is_latest_wins_and_close_drains() {
        let mailbox = SessionMailbox::new();
        let mut worker = mailbox.receiver();
        assert!(
            mailbox
                .push(notification(1), None, std::time::Instant::now())
                .is_none()
        );
        let superseded = mailbox
            .push(notification(2), None, std::time::Instant::now())
            .expect("job 1 superseded");
        assert_eq!(superseded.notification.document_version, 1);
        let job = worker.recv().await.expect("latest job delivered");
        assert_eq!(job.notification.document_version, 2);
        assert_eq!(job.seq, JobSeq(2), "job sequences stay monotonic");
        assert!(
            mailbox.close().is_none(),
            "close after drain has no pending"
        );
        assert!(
            worker.recv().await.is_none(),
            "closed mailbox drains to None"
        );
    }

    #[tokio::test]
    async fn close_completes_pending_and_terminates_worker() {
        let mailbox = SessionMailbox::new();
        let mut worker = mailbox.receiver();
        mailbox.push(notification(3), None, std::time::Instant::now());
        let pending = mailbox.close().expect("pending job returned on close");
        assert_eq!(pending.notification.document_version, 3);
        assert!(worker.recv().await.is_none());
    }

    #[tokio::test]
    async fn worker_sees_jobs_pushed_after_drain() {
        let mailbox = SessionMailbox::new();
        let mut worker = mailbox.receiver();
        mailbox.push(notification(1), None, std::time::Instant::now());
        let job = worker.recv().await.expect("first job");
        assert_eq!(job.notification.document_version, 1);
        mailbox.push(notification(2), None, std::time::Instant::now());
        let job = worker.recv().await.expect("second job");
        assert_eq!(job.notification.document_version, 2);
    }

    #[tokio::test]
    async fn executor_bounds_concurrent_blocking_jobs() {
        let executor = SyntaxExecutor::new();
        assert_eq!(executor.available_permits(), SYNTAX_EXECUTOR_MAX_JOBS);
        let mut permits = Vec::new();
        for _ in 0..SYNTAX_EXECUTOR_MAX_JOBS {
            permits.push(executor.acquire().await);
        }
        assert_eq!(executor.available_permits(), 0);
        drop(permits);
    }
}
