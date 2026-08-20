//! Bounded client multi-document session store (Phase 20).
//!
//! Server `WorkspaceState` remains the open-registry / lease / dirty authority.
//! This store retains local shadow editor state so opening another file no
//! longer destroys the previous session, and so activate-by-id can restore
//! caret/viewport/history without re-downloading text.

use std::collections::{HashMap, VecDeque};

use crate::client::PendingEdit;
use crate::perf::budgets::CLIENT_DOCUMENT_SESSION_MAX;
use crate::protocol::{DocumentId, DocumentVersion, WorkspaceRootId};

use super::surface::EditorSurface;

/// Snapshot of one inactive (or about-to-become-inactive) document session.
#[derive(Debug)]
pub(crate) struct RetainedDocumentSession {
    pub(crate) surface: EditorSurface,
    pub(crate) dirty: bool,
    pub(crate) document_display_name: Option<String>,
    pub(crate) confirmed_version: DocumentVersion,
    pub(crate) pending: Vec<PendingEdit>,
    /// Monotonic activation stamp for LRU eviction among inactive sessions.
    pub(crate) last_activated_order: u64,
    /// Phase 22.3: the document's open identity (workspace root id + relative
    /// path), retained so a reconnected tab can re-open this document through
    /// the plain `OpenDocument` path (fresh connections hold no selected-file
    /// capability for previously open documents).
    pub(crate) workspace_root_id: WorkspaceRootId,
    pub(crate) path: String,
}

#[derive(Debug, Default)]
pub(crate) struct DocumentSessionStore {
    sessions: HashMap<DocumentId, RetainedDocumentSession>,
    /// LRU order of inactive document ids (front = oldest / evict first).
    lru: VecDeque<DocumentId>,
    activation_clock: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionListEntry {
    pub(crate) document_id: DocumentId,
    pub(crate) display_name: String,
    pub(crate) dirty: bool,
    pub(crate) active: bool,
}

impl DocumentSessionStore {
    pub(crate) fn len(&self) -> usize {
        self.sessions.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub(crate) fn contains(&self, document_id: DocumentId) -> bool {
        self.sessions.contains_key(&document_id)
    }

    /// All retained document ids (used by pane-close cleanup).
    pub(crate) fn document_ids(&self) -> Vec<DocumentId> {
        self.sessions.keys().copied().collect()
    }

    /// Drop every retained session (pane close).
    pub(crate) fn clear(&mut self) {
        self.sessions.clear();
        self.lru.clear();
    }

    pub(crate) fn get_mut(
        &mut self,
        document_id: DocumentId,
    ) -> Option<&mut RetainedDocumentSession> {
        self.sessions.get_mut(&document_id)
    }

    /// Insert or replace a retained session. Returns the sanitized eviction
    /// notice plus the evicted document IDs (so the caller can notify the
    /// server with `CloseDocument`) when inactive LRU sessions were dropped
    /// to stay within the ceiling.
    pub(crate) fn insert(
        &mut self,
        document_id: DocumentId,
        mut session: RetainedDocumentSession,
    ) -> SessionEviction {
        self.activation_clock = self.activation_clock.saturating_add(1);
        session.last_activated_order = self.activation_clock;
        self.touch_lru(document_id);
        self.sessions.insert(document_id, session);
        self.evict_if_needed()
    }

    pub(crate) fn document_id_for_path(
        &self,
        workspace_root_id: WorkspaceRootId,
        path: &str,
    ) -> Option<DocumentId> {
        let normalized = path.replace('\\', "/");
        self.sessions.iter().find_map(|(&document_id, session)| {
            (session.workspace_root_id == workspace_root_id
                && session.path.replace('\\', "/") == normalized)
                .then_some(document_id)
        })
    }

    /// Phase 22.3: the open identities of every retained session, for a
    /// reconnecting tab to re-open (root id + relative path).
    pub(crate) fn reopen_documents(&self) -> Vec<(WorkspaceRootId, String)> {
        self.sessions
            .values()
            .map(|session| (session.workspace_root_id, session.path.clone()))
            .collect()
    }

    pub(crate) fn remove(&mut self, document_id: DocumentId) -> Option<RetainedDocumentSession> {
        self.lru.retain(|id| *id != document_id);
        self.sessions.remove(&document_id)
    }

    /// Mark `document_id` as most-recently-activated without requiring a full
    /// re-insert (used when the active document stays active).
    #[allow(dead_code)]
    pub(crate) fn touch_active(&mut self, document_id: DocumentId) {
        if self.sessions.contains_key(&document_id) {
            self.activation_clock = self.activation_clock.saturating_add(1);
            if let Some(session) = self.sessions.get_mut(&document_id) {
                session.last_activated_order = self.activation_clock;
            }
            self.touch_lru(document_id);
        }
    }

    pub(crate) fn list_with_active(
        &self,
        active_document_id: DocumentId,
        active_display_name: Option<&str>,
        active_dirty: bool,
    ) -> Vec<SessionListEntry> {
        let mut entries = Vec::with_capacity(self.sessions.len().saturating_add(1));
        entries.push(SessionListEntry {
            document_id: active_document_id,
            display_name: active_display_name
                .map(str::to_string)
                .unwrap_or_else(|| format!("doc {active_document_id}")),
            dirty: active_dirty,
            active: true,
        });
        let mut inactive: Vec<_> = self
            .sessions
            .iter()
            .map(|(&document_id, session)| SessionListEntry {
                document_id,
                display_name: session
                    .document_display_name
                    .clone()
                    .unwrap_or_else(|| format!("doc {document_id}")),
                dirty: session.dirty,
                active: false,
            })
            .collect();
        inactive.sort_by(|a, b| {
            a.display_name
                .cmp(&b.display_name)
                .then(a.document_id.cmp(&b.document_id))
        });
        entries.extend(inactive);
        entries
    }

    /// Phase 22.2: retained sessions only (no active-document entry), for
    /// cross-pane open-documents menu aggregation. Sorted by display name.
    pub(crate) fn list_retained(&self) -> Vec<SessionListEntry> {
        let mut inactive: Vec<_> = self
            .sessions
            .iter()
            .map(|(&document_id, session)| SessionListEntry {
                document_id,
                display_name: session
                    .document_display_name
                    .clone()
                    .unwrap_or_else(|| format!("doc {document_id}")),
                dirty: session.dirty,
                active: false,
            })
            .collect();
        inactive.sort_by(|a, b| {
            a.display_name
                .cmp(&b.display_name)
                .then(a.document_id.cmp(&b.document_id))
        });
        inactive
    }

    fn touch_lru(&mut self, document_id: DocumentId) {
        self.lru.retain(|id| *id != document_id);
        self.lru.push_back(document_id);
    }

    fn evict_if_needed(&mut self) -> SessionEviction {
        // Active document is never stored in this map; ceiling applies to retained
        // inactive sessions only. Total client sessions ≈ retained + 1 active.
        let max_retained = CLIENT_DOCUMENT_SESSION_MAX.saturating_sub(1).max(1);
        let mut eviction = SessionEviction::default();
        while self.sessions.len() > max_retained {
            let Some(evict_id) = self.lru.pop_front() else {
                break;
            };
            if self.sessions.remove(&evict_id).is_some() {
                eviction.evicted.push(evict_id);
                eviction.notice = Some(format!(
                    "Closed least-recently used document session (doc {evict_id}) to stay within the open-document limit."
                ));
            }
        }
        eviction
    }
}

/// Result of a retained-session insert: an optional user-facing eviction
/// notice and the server document IDs whose sessions were dropped (the caller
/// notifies the server so document state is released).
#[derive(Debug, Default)]
pub(crate) struct SessionEviction {
    pub(crate) notice: Option<String>,
    pub(crate) evicted: Vec<DocumentId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DocumentAccess;

    fn sample_session(document_id: DocumentId, name: &str) -> RetainedDocumentSession {
        let mut surface = EditorSurface::default();
        surface.load_snapshot(
            document_id,
            1,
            format!("text-{document_id}"),
            DocumentAccess::Editable { lease_id: 1 },
        );
        RetainedDocumentSession {
            surface,
            dirty: false,
            document_display_name: Some(name.to_string()),
            confirmed_version: 1,
            pending: Vec::new(),
            last_activated_order: 0,
            workspace_root_id: 77,
            path: format!("doc-{document_id}.md"),
        }
    }

    #[test]
    fn insert_retains_and_lists_inactive_sessions() {
        let mut store = DocumentSessionStore::default();
        assert!(
            store
                .insert(10, sample_session(10, "a.md"))
                .notice
                .is_none()
        );
        assert!(
            store
                .insert(11, sample_session(11, "b.md"))
                .notice
                .is_none()
        );
        assert_eq!(store.len(), 2);
        let list = store.list_with_active(12, Some("c.md"), true);
        assert_eq!(list.len(), 3);
        assert!(list[0].active);
        assert_eq!(list[0].document_id, 12);
        assert!(list.iter().any(|e| e.document_id == 10 && !e.active));
    }

    #[test]
    fn eviction_drops_least_recently_touched_inactive_session() {
        let mut store = DocumentSessionStore::default();
        let max_retained = CLIENT_DOCUMENT_SESSION_MAX.saturating_sub(1).max(1);
        for id in 1..=max_retained {
            assert!(
                store
                    .insert(id as DocumentId, sample_session(id as DocumentId, "x"))
                    .notice
                    .is_none()
            );
        }
        let eviction = store.insert(
            (max_retained as DocumentId) + 1,
            sample_session((max_retained as DocumentId) + 1, "newest"),
        );
        assert!(eviction.notice.is_some(), "expected eviction notice");
        assert_eq!(eviction.evicted, vec![1], "evicted id must be reported");
        assert!(!store.contains(1), "oldest session should be evicted");
        assert!(store.contains((max_retained as DocumentId) + 1));
        assert_eq!(store.len(), max_retained);
    }

    #[test]
    fn remove_returns_retained_surface_text() {
        let mut store = DocumentSessionStore::default();
        store.insert(7, sample_session(7, "note.md"));
        let restored = store.remove(7).expect("session");
        assert_eq!(restored.surface.visible_text(), "text-7");
        assert_eq!(restored.document_display_name.as_deref(), Some("note.md"));
        assert!(!store.contains(7));
    }
}
