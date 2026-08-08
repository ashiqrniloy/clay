//! Phase 22.3: server-authoritative in-memory tab registry.
//!
//! Each tab is a real separate client connection; the registry binds a stable
//! `TabId` to a `ClientId` and a workspace root. The server owns the registry
//! (order, active tab, per-tab workspace + client binding) so tab structure
//! survives client reconnects, consistent with the client-authority model,
//! lease accounting, and connection caps.
//!
//! Invariants:
//! - In-memory only in 22.3; disk persistence (tab order, active tab, per-tab
//!   workspace, per-tab split trees, per-pane documents) is Phase 22.5
//!   (`layout.json` extension).
//! - Order is server-authoritative and reorderable since 22.4 via
//!   `move_left`/`move_right`/`move_to` (the Phase 22.4 keyboard tab commands);
//!   positions are 1-based and out-of-range moves are rejected, never clamped.
//! - Moving a tab never changes the active tab: `active` is tracked by `TabId`,
//!   so the active tab keeps its status at its new position.
//! - One `ClientId` per tab. The registry grants nothing: it binds
//!   already-authorized connections to stable tab identities. Documents,
//!   leases, and modes stay per-connection via the existing primitives.
//! - Reclaim re-points a tab's `ClientId` at a reconnecting connection (local
//!   single-client reclaim in 22.3). Multi-client reclaim needs a stable
//!   client-instance identity (Phase 21).

use crate::protocol::{ClientId, TabEntry, TabId, TabRegistrySnapshot, WorkspaceRootId};

/// Server-authoritative tab registry (in-memory).
#[derive(Debug, Default)]
pub(crate) struct TabRegistry {
    tabs: Vec<TabEntry>,
    active: Option<TabId>,
    next_tab_id: TabId,
}

impl TabRegistry {
    pub(crate) fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            next_tab_id: 1,
        }
    }

    pub(crate) fn snapshot(&self) -> TabRegistrySnapshot {
        TabRegistrySnapshot {
            tabs: self.tabs.clone(),
            active: self.active,
        }
    }

    /// Create a tab bound to `client_id`, `workspace_root_id`, and the
    /// validated root path, append it, and make it active. Returns the new
    /// `TabId`.
    pub(crate) fn create_tab(
        &mut self,
        client_id: ClientId,
        workspace_root_id: WorkspaceRootId,
        workspace_root: String,
    ) -> TabId {
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.saturating_add(1);
        self.tabs.push(TabEntry {
            tab_id,
            workspace_root_id,
            client_id,
            workspace_root,
        });
        self.active = Some(tab_id);
        tab_id
    }

    /// Rebind a tab's workspace root. Only the tab's bound connection may
    /// change it.
    pub(crate) fn open_workspace(
        &mut self,
        tab_id: TabId,
        client_id: ClientId,
        workspace_root_id: WorkspaceRootId,
        workspace_root: String,
    ) -> bool {
        let Some(entry) = self.tabs.iter_mut().find(|entry| entry.tab_id == tab_id) else {
            return false;
        };
        if entry.client_id != client_id {
            return false;
        }
        entry.workspace_root_id = workspace_root_id;
        entry.workspace_root = workspace_root;
        true
    }

    /// Remove a tab. Only the tab's bound connection may close it. Returns
    /// true when the tab existed and was removed.
    pub(crate) fn close_tab(&mut self, tab_id: TabId, client_id: ClientId) -> bool {
        let Some(index) = self
            .tabs
            .iter()
            .position(|entry| entry.tab_id == tab_id && entry.client_id == client_id)
        else {
            return false;
        };
        self.tabs.remove(index);
        if self.active == Some(tab_id) {
            self.active = self.tabs.last().map(|entry| entry.tab_id);
        }
        true
    }

    /// Set the active tab. Only the tab's bound connection may activate it.
    pub(crate) fn activate(&mut self, tab_id: TabId, client_id: ClientId) -> bool {
        let Some(entry) = self.tabs.iter().find(|entry| entry.tab_id == tab_id) else {
            return false;
        };
        if entry.client_id != client_id {
            return false;
        }
        self.active = Some(tab_id);
        true
    }

    /// Re-point a tab's `ClientId` binding at a reconnecting connection.
    /// Local single-client reclaim in 22.3: any connection may reclaim any
    /// tab. Multi-client reclaim needs a stable client-instance identity
    /// (Phase 21) — recorded as a known ceiling.
    pub(crate) fn reclaim(&mut self, tab_id: TabId, client_id: ClientId) -> bool {
        let Some(entry) = self.tabs.iter_mut().find(|entry| entry.tab_id == tab_id) else {
            return false;
        };
        entry.client_id = client_id;
        true
    }

    /// Move a tab one position toward the front. Only the tab's bound
    /// connection may move it. Moving at the first position is a no-op — no
    /// wraparound (explicit Phase 22.4 policy). The active tab keeps its
    /// status wherever it moves.
    pub(crate) fn move_left(&mut self, tab_id: TabId, client_id: ClientId) -> bool {
        let Some(index) = self.tabs.iter().position(|entry| entry.tab_id == tab_id) else {
            return false;
        };
        if self.tabs[index].client_id != client_id {
            return false;
        }
        if index == 0 {
            return false;
        }
        self.tabs.swap(index, index - 1);
        true
    }

    /// Move a tab one position toward the back. Only the tab's bound
    /// connection may move it. Moving at the last position is a no-op — no
    /// wraparound (explicit Phase 22.4 policy). The active tab keeps its
    /// status wherever it moves.
    pub(crate) fn move_right(&mut self, tab_id: TabId, client_id: ClientId) -> bool {
        let Some(index) = self.tabs.iter().position(|entry| entry.tab_id == tab_id) else {
            return false;
        };
        if self.tabs[index].client_id != client_id {
            return false;
        }
        if index + 1 >= self.tabs.len() {
            return false;
        }
        self.tabs.swap(index, index + 1);
        true
    }

    /// Move a tab to a 1-based position. Only the tab's bound connection may
    /// move it. Positions outside `1..=tab_count` are rejected (no-op, never
    /// clamped); moving a tab to its current position is a no-op. The active
    /// tab keeps its status wherever it moves.
    pub(crate) fn move_to(&mut self, tab_id: TabId, client_id: ClientId, position: u32) -> bool {
        let Some(index) = self.tabs.iter().position(|entry| entry.tab_id == tab_id) else {
            return false;
        };
        if self.tabs[index].client_id != client_id {
            return false;
        }
        if position == 0 || position as usize > self.tabs.len() {
            return false;
        }
        let target = position as usize - 1; // 1-based -> 0-based
        if target == index {
            return false;
        }
        let entry = self.tabs.remove(index);
        self.tabs.insert(target, entry);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_with_tabs() -> (TabRegistry, TabId, TabId) {
        let mut registry = TabRegistry::new();
        let first = registry.create_tab(1, 10, "/tmp/alpha".to_string());
        let second = registry.create_tab(2, 20, "/tmp/beta".to_string());
        (registry, first, second)
    }

    #[test]
    fn create_tab_appends_sets_active_and_assigns_stable_ids() {
        let mut registry = TabRegistry::new();
        let first = registry.create_tab(1, 10, "/tmp/alpha".to_string());
        let second = registry.create_tab(2, 20, "/tmp/beta".to_string());
        assert_eq!(first, 1);
        assert_eq!(second, 2);
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.tabs.len(), 2);
        assert_eq!(snapshot.tabs[0].tab_id, first);
        assert_eq!(snapshot.tabs[0].client_id, 1);
        assert_eq!(snapshot.tabs[0].workspace_root_id, 10);
        assert_eq!(snapshot.tabs[1].tab_id, second);
        assert_eq!(snapshot.tabs[1].client_id, 2);
        assert_eq!(snapshot.active, Some(second));
    }

    #[test]
    fn close_tab_requires_bound_client_and_falls_back_to_last_tab() {
        let (mut registry, first, second) = registry_with_tabs();
        // Wrong client cannot close.
        assert!(!registry.close_tab(first, 99));
        assert_eq!(registry.snapshot().tabs.len(), 2);
        // Bound client closes; active falls back to the last remaining tab.
        assert!(registry.close_tab(second, 2));
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.tabs.len(), 1);
        assert_eq!(snapshot.active, Some(first));
        // Closing the last tab leaves an empty registry with no active tab.
        assert!(registry.close_tab(first, 1));
        let snapshot = registry.snapshot();
        assert!(snapshot.tabs.is_empty());
        assert_eq!(snapshot.active, None);
    }

    #[test]
    fn activate_and_open_workspace_require_bound_client() {
        let (mut registry, first, _) = registry_with_tabs();
        assert!(!registry.activate(first, 99));
        assert_eq!(registry.snapshot().active, Some(2));
        assert!(registry.activate(first, 1));
        assert_eq!(registry.snapshot().active, Some(first));
        assert!(!registry.open_workspace(first, 99, 30, "/tmp/gamma".to_string()));
        assert!(registry.open_workspace(first, 1, 30, "/tmp/alpha".to_string()));
        assert_eq!(registry.snapshot().tabs[0].workspace_root_id, 30);
    }

    #[test]
    fn reclaim_repoints_client_binding() {
        let (mut registry, first, _) = registry_with_tabs();
        assert!(registry.reclaim(first, 7));
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.tabs[0].client_id, 7);
        // Reclaimed tab is now owned by the new connection.
        assert!(registry.activate(first, 7));
        assert!(!registry.activate(first, 1));
        // Unknown tab ids are rejected.
        assert!(!registry.reclaim(99, 7));
    }

    #[test]
    fn move_left_right_reorder_and_preserve_active_status() {
        let (mut registry, first, second) = registry_with_tabs();
        let third = registry.create_tab(3, 30, "/tmp/gamma".to_string());
        // [1, 2, 3] active=3. Move the active tab left: [1, 3, 2], still active.
        assert!(registry.move_left(third, 3));
        let snapshot = registry.snapshot();
        assert_eq!(
            snapshot
                .tabs
                .iter()
                .map(|entry| entry.tab_id)
                .collect::<Vec<_>>(),
            vec![first, third, second]
        );
        assert_eq!(snapshot.active, Some(third));
        // Moving a non-active tab right: [3, 1, 2], active unchanged.
        assert!(registry.move_right(first, 1));
        let snapshot = registry.snapshot();
        assert_eq!(
            snapshot
                .tabs
                .iter()
                .map(|entry| entry.tab_id)
                .collect::<Vec<_>>(),
            vec![third, first, second]
        );
        assert_eq!(snapshot.active, Some(third));
    }

    #[test]
    fn move_boundaries_are_noops_without_wraparound() {
        let (mut registry, first, _) = registry_with_tabs();
        // First position: move_left is a no-op (no wrap to the back).
        assert!(!registry.move_left(first, 1));
        assert_eq!(registry.snapshot().tabs.len(), 2);
        // Last position: move_right is a no-op (no wrap to the front).
        assert!(!registry.move_right(2, 2));
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.tabs[0].tab_id, first);
        assert_eq!(snapshot.tabs[1].tab_id, 2);
        // Single-tab registry: both directions are no-ops.
        let mut single = TabRegistry::new();
        let only = single.create_tab(1, 10, "/tmp/alpha".to_string());
        assert!(!single.move_left(only, 1));
        assert!(!single.move_right(only, 1));
        assert!(!single.move_to(only, 1, 1));
    }

    #[test]
    fn move_requires_bound_client_and_existing_tab() {
        let (mut registry, first, _) = registry_with_tabs();
        // Foreign client cannot move.
        assert!(!registry.move_left(first, 99));
        assert!(!registry.move_right(first, 99));
        assert!(!registry.move_to(first, 99, 1));
        // Unknown tab ids are rejected.
        assert!(!registry.move_left(99, 1));
        assert!(!registry.move_right(99, 1));
        assert!(!registry.move_to(99, 1, 1));
        // Order is unchanged by all rejections.
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.tabs[0].tab_id, first);
    }

    #[test]
    fn move_to_validates_position_and_reorders() {
        let (mut registry, first, second) = registry_with_tabs();
        let third = registry.create_tab(3, 30, "/tmp/gamma".to_string());
        // [1, 2, 3] -> move tab 3 to position 1: [3, 1, 2].
        assert!(registry.move_to(third, 3, 1));
        let snapshot = registry.snapshot();
        assert_eq!(
            snapshot
                .tabs
                .iter()
                .map(|entry| entry.tab_id)
                .collect::<Vec<_>>(),
            vec![third, first, second]
        );
        // Position 0 and position > tab_count are rejected without mutation.
        assert!(!registry.move_to(second, 2, 0));
        assert!(!registry.move_to(second, 2, 4));
        // Moving to the current position is a no-op.
        assert!(!registry.move_to(second, 2, 3));
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.tabs[2].tab_id, second);
        // [3, 1, 2] -> move tab 1 to position 2: [3, 1, 2] is already that; use
        // position 3: [3, 2, 1].
        assert!(registry.move_to(first, 1, 3));
        let snapshot = registry.snapshot();
        assert_eq!(
            snapshot
                .tabs
                .iter()
                .map(|entry| entry.tab_id)
                .collect::<Vec<_>>(),
            vec![third, second, first]
        );
        // Active tab status survives move_to.
        assert_eq!(snapshot.active, Some(third));
    }

    /// Phase 22.6 (plan 077 task 6): the registry "grants nothing" — entries
    /// bind identity only (tab id, bound connection, workspace root) with no
    /// grant, lease, capability, or package field. The literal below pins
    /// the shape: adding any grant-carrying field breaks this test's
    /// compilation, so the invariant is asserted, not just commented.
    #[test]
    fn tab_entries_carry_identity_bindings_only_and_grants_nothing() {
        let mut registry = TabRegistry::new();
        let tab_id = registry.create_tab(7, 70, "/tmp/identity".to_string());
        let snapshot = registry.snapshot();
        assert_eq!(
            snapshot.tabs,
            vec![TabEntry {
                tab_id,
                workspace_root_id: 70,
                client_id: 7,
                workspace_root: "/tmp/identity".to_string(),
            }]
        );
        assert_eq!(snapshot.active, Some(tab_id));
    }

    /// Phase 22.6 (plan 077 task 6): reclaim re-points only the reclaimed
    /// tab's connection binding. The superseded connection loses every
    /// registry operation, the reclaiming connection gains them, and no
    /// other tab's binding is touched.
    #[test]
    fn reclaim_rebinds_only_the_reclaiming_connection() {
        let (mut registry, first, second) = registry_with_tabs();
        assert!(registry.reclaim(first, 99));

        // The superseded connection (1) can no longer operate the tab.
        assert!(!registry.activate(first, 1));
        assert!(!registry.close_tab(first, 1));
        assert!(!registry.open_workspace(first, 1, 11, "/tmp/x".to_string()));
        assert!(!registry.move_left(first, 1));
        assert!(!registry.move_right(first, 1));
        assert!(!registry.move_to(first, 1, 2));

        // The reclaiming connection (99) is bound to the reclaimed tab.
        assert!(registry.activate(first, 99));
        assert!(registry.open_workspace(first, 99, 71, "/tmp/new".to_string()));
        assert!(registry.move_right(first, 99));

        // Cross-tab rebinding fails: 99 cannot operate tab 2, and tab 2's
        // own binding (2) is untouched.
        assert!(!registry.activate(second, 99));
        assert!(!registry.close_tab(second, 99));
        assert!(!registry.open_workspace(second, 99, 21, "/tmp/x".to_string()));
        assert!(registry.activate(second, 2));
    }

    #[test]
    fn move_ops_change_order_only_and_preserve_entry_contents() {
        let (mut registry, first, second) = registry_with_tabs();
        let third = registry.create_tab(3, 30, "/tmp/gamma".to_string());
        let contents = |registry: &TabRegistry| {
            let mut entries: Vec<(TabId, ClientId, WorkspaceRootId, String)> = registry
                .snapshot()
                .tabs
                .iter()
                .map(|entry| {
                    (
                        entry.tab_id,
                        entry.client_id,
                        entry.workspace_root_id,
                        entry.workspace_root.clone(),
                    )
                })
                .collect();
            entries.sort_by_key(|(tab_id, _, _, _)| *tab_id);
            entries
        };
        let before = contents(&registry);
        // [1, 2, 3] -> move_to(1, 3): [2, 3, 1] -> move_left(3): [3, 2, 1]
        // -> move_right(2): [3, 1, 2]. Three distinct reorder shapes.
        assert!(registry.move_to(first, 1, 3));
        assert!(registry.move_left(third, 3));
        assert!(registry.move_right(second, 2));
        assert_eq!(contents(&registry), before, "reorder changes order only");
    }
}
