use std::sync::{Arc, Mutex, MutexGuard};

use crate::protocol::{DocumentId, LockOwner};

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "range/document/workspace acquisition is the generic primitive for later scoped mutations"
    )
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScopedLockTarget {
    Range {
        document_id: DocumentId,
        start: u64,
        end: u64,
    },
    Document {
        document_id: DocumentId,
    },
    Behavior,
    Workspace,
}

impl ScopedLockTarget {
    fn validate(&self) -> Result<(), ScopedLockError> {
        match self {
            Self::Range {
                document_id,
                start,
                end,
            } if *document_id == 0 || start >= end => Err(ScopedLockError::InvalidTarget),
            Self::Document { document_id } if *document_id == 0 => {
                Err(ScopedLockError::InvalidTarget)
            }
            _ => Ok(()),
        }
    }

    fn conflicts_with(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Workspace, _) | (_, Self::Workspace) => true,
            (Self::Behavior, Self::Behavior) => true,
            (
                Self::Range {
                    document_id: left_document,
                    start: left_start,
                    end: left_end,
                },
                Self::Range {
                    document_id: right_document,
                    start: right_start,
                    end: right_end,
                },
            ) => {
                left_document == right_document
                    && ranges_overlap(*left_start, *left_end, *right_start, *right_end)
            }
            (
                Self::Range { document_id, .. },
                Self::Document {
                    document_id: other_document,
                },
            )
            | (
                Self::Document {
                    document_id: other_document,
                },
                Self::Range { document_id, .. },
            ) => document_id == other_document,
            (
                Self::Document {
                    document_id: left_document,
                },
                Self::Document {
                    document_id: right_document,
                },
            ) => left_document == right_document,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopedLockConflict {
    pub(crate) lock_id: u64,
    pub(crate) target: ScopedLockTarget,
    pub(crate) owner: LockOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScopedLockError {
    InvalidTarget,
    Conflict(ScopedLockConflict),
}

#[derive(Debug, Clone)]
pub(crate) struct ScopedLockManager {
    inner: Arc<Mutex<ScopedLockState>>,
}

#[derive(Debug, Default)]
struct ScopedLockState {
    next_lock_id: u64,
    active: Vec<ActiveScopedLock>,
}

#[derive(Debug, Clone)]
struct ActiveScopedLock {
    lock_id: u64,
    target: ScopedLockTarget,
    owner: LockOwner,
}

impl Default for ScopedLockManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ScopedLockState {
                next_lock_id: 1,
                active: Vec::new(),
            })),
        }
    }
}

impl ScopedLockManager {
    pub(crate) fn try_acquire(
        &self,
        target: ScopedLockTarget,
        owner: LockOwner,
    ) -> Result<ScopedLockGuard, ScopedLockError> {
        target.validate()?;
        let mut state = lock_state(&self.inner);
        if let Some(active) = state
            .active
            .iter()
            .find(|active| target.conflicts_with(&active.target))
        {
            return Err(ScopedLockError::Conflict(ScopedLockConflict {
                lock_id: active.lock_id,
                target: active.target.clone(),
                owner: active.owner.clone(),
            }));
        }

        let lock_id = state.next_lock_id;
        state.next_lock_id = state.next_lock_id.saturating_add(1);
        state.active.push(ActiveScopedLock {
            lock_id,
            target,
            owner,
        });
        Ok(ScopedLockGuard {
            lock_id,
            inner: Arc::clone(&self.inner),
        })
    }
}

#[derive(Debug)]
pub(crate) struct ScopedLockGuard {
    lock_id: u64,
    inner: Arc<Mutex<ScopedLockState>>,
}

impl Drop for ScopedLockGuard {
    fn drop(&mut self) {
        lock_state(&self.inner)
            .active
            .retain(|active| active.lock_id != self.lock_id);
    }
}

pub(crate) fn ranges_overlap(
    left_start: u64,
    left_end: u64,
    right_start: u64,
    right_end: u64,
) -> bool {
    left_start < right_end && right_start < left_end
}

fn lock_state(inner: &Mutex<ScopedLockState>) -> MutexGuard<'_, ScopedLockState> {
    inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_document_workspace_lock_conflicts_reuse_generic_manager() {
        let manager = ScopedLockManager::default();
        let range = manager
            .try_acquire(
                ScopedLockTarget::Range {
                    document_id: 7,
                    start: 2,
                    end: 5,
                },
                LockOwner::Server,
            )
            .expect("range lock");

        let conflict = manager
            .try_acquire(
                ScopedLockTarget::Document { document_id: 7 },
                LockOwner::Client { client_id: 4 },
            )
            .expect_err("document conflicts with active range");
        assert_eq!(
            conflict,
            ScopedLockError::Conflict(ScopedLockConflict {
                lock_id: 1,
                target: ScopedLockTarget::Range {
                    document_id: 7,
                    start: 2,
                    end: 5,
                },
                owner: LockOwner::Server,
            })
        );
        assert!(
            manager
                .try_acquire(
                    ScopedLockTarget::Range {
                        document_id: 8,
                        start: 2,
                        end: 5,
                    },
                    LockOwner::Server,
                )
                .is_ok()
        );
        assert!(matches!(
            manager.try_acquire(ScopedLockTarget::Workspace, LockOwner::Server),
            Err(ScopedLockError::Conflict(_))
        ));

        drop(range);
        assert!(
            manager
                .try_acquire(
                    ScopedLockTarget::Document { document_id: 7 },
                    LockOwner::Server,
                )
                .is_ok()
        );
    }

    #[test]
    fn behavior_lock_releases_on_guard_drop() {
        let manager = ScopedLockManager::default();
        let guard = manager
            .try_acquire(ScopedLockTarget::Behavior, LockOwner::Server)
            .expect("behavior lock");
        assert!(matches!(
            manager.try_acquire(ScopedLockTarget::Behavior, LockOwner::Server),
            Err(ScopedLockError::Conflict(_))
        ));
        drop(guard);
        assert!(
            manager
                .try_acquire(ScopedLockTarget::Behavior, LockOwner::Server)
                .is_ok()
        );
    }
}
