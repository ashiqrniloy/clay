use std::sync::{Arc, Mutex, MutexGuard};

/// Non-blocking behavior-scope lock for server-owned behavior mutation.
///
/// Runtime reload holds it across the generation compare-and-swap commit, so a
/// concurrent behavior mutation fails fast with `BehaviorLocked` instead of
/// queueing; `ScopedLockGuard` releases on every return/unwind path.
#[derive(Debug, Clone)]
pub(crate) struct ScopedLockManager {
    held: Arc<Mutex<bool>>,
}

impl Default for ScopedLockManager {
    fn default() -> Self {
        Self {
            held: Arc::new(Mutex::new(false)),
        }
    }
}

/// The behavior scope is already held by another server operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BehaviorLocked;

impl ScopedLockManager {
    pub(crate) fn try_acquire(&self) -> Result<ScopedLockGuard, BehaviorLocked> {
        let mut held = lock_state(&self.held);
        if *held {
            return Err(BehaviorLocked);
        }
        *held = true;
        drop(held);
        Ok(ScopedLockGuard {
            held: Arc::clone(&self.held),
        })
    }
}

#[derive(Debug)]
pub(crate) struct ScopedLockGuard {
    held: Arc<Mutex<bool>>,
}

impl Drop for ScopedLockGuard {
    fn drop(&mut self) {
        *lock_state(&self.held) = false;
    }
}

fn lock_state(held: &Mutex<bool>) -> MutexGuard<'_, bool> {
    held.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behavior_lock_rejects_concurrent_acquire_and_releases_on_drop() {
        let manager = ScopedLockManager::default();
        let guard = manager.try_acquire().expect("behavior lock");
        assert!(matches!(manager.try_acquire(), Err(BehaviorLocked)));
        drop(guard);
        assert!(manager.try_acquire().is_ok());
    }
}
