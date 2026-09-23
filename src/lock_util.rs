//! Poison policy for server-owned `std::sync::Mutex` locks (plan 134 D5).
//!
//! A `std::sync::Mutex` becomes poisoned when a thread panics while holding
//! its guard, and every later `lock()` returns an error. Treating that as
//! fatal with `expect("…poisoned")` means one panicked critical section kills
//! the whole server process and every editor connection with it.
//!
//! **Rule.** A lock whose guarded value is self-healing state — a registry,
//! cache, snapshot, counter, mailbox, or routing table that a later write
//! fully replaces or that only needs per-entry validation — is *recoverable*:
//! take the guard with [`LockOrRecover::lock_or_recover`] and keep serving.
//! Recovery never bypasses an authority check: permission state (package
//! allowlists, enabled sets, executing-package provenance) is re-validated by
//! the ordinary checks on the next operation, and a torn write can only leave
//! an entry absent (fail closed) or fully written and already validated.
//!
//! A lock guarding an irreversible hand-off may keep a loud `expect`: poison
//! there means the guarded state must not be reused. The current example is
//! the embedded tree-sitter `Parser` in `server::syntax` — reusing a parser
//! whose FFI state was interrupted mid-parse is not safe, so that site keeps
//! `expect` with a comment naming the corruption.
//!
//! Test-only locks (`#[cfg(test)]` hooks and harness gates) keep `expect`:
//! tests should fail loudly, and no production path can poison them.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Lock a recoverable state mutex, taking the poison's inner value.
pub(crate) trait LockOrRecover<T> {
    /// Like [`Mutex::lock`], but a poisoned lock yields its inner guard
    /// instead of a [`PoisonError`].
    fn lock_or_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> LockOrRecover<T> for Mutex<T> {
    fn lock_or_recover(&self) -> MutexGuard<'_, T> {
        // Healthy path is exactly `Mutex::lock`; only the poison arm differs.
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_state_mutex_recovers_service() {
        let state = std::sync::Arc::new(Mutex::new(vec![1u32, 2, 3]));
        let poisoner = std::sync::Arc::clone(&state);
        let result = std::thread::spawn(move || {
            let mut guard = poisoner.lock().expect("unpoisoned");
            guard.push(4);
            panic!("poison the state mutex");
        })
        .join();
        assert!(result.is_err(), "the poisoner must panic");
        assert!(state.lock().is_err(), "the mutex must be poisoned");

        // Recovery continues service on the state as far as it was written.
        state.lock_or_recover().push(5);
        assert_eq!(*state.lock_or_recover(), vec![1, 2, 3, 4, 5]);
    }
}
