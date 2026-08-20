//! Registry reconcile: the server-authoritative `TabRegistrySnapshot` diff
//! → driver state (tab ids, removals, active tab, cards) and shell
//! reconciliation. Phase 22.3 (plan 078; findings A5/D4).
use super::*;

impl Driver {
    /// Phase 22.3: reconcile the server-authoritative registry snapshot into
    /// driver state (pure; no shell access): fill each tab's server-assigned
    /// `TabId`, compute tabs whose entries vanished (close), the tab to
    /// activate (a rejected optimistic `Activate` reverts via the pushed
    /// snapshot's `active`), and the tab bar cards (registry order/names,
    /// mounted tabs awaiting their entry appended with close disabled).
    ///
    /// Removals are only reconciled against a non-empty registry: an empty
    /// registry means the server restarted (the in-memory registry is lost)
    /// and the lifecycle task re-registers tabs via `Reclaim`/`New`.
    /// Ordering guard for registry relays: a connection's handshake replay
    /// races the broadcast of its own pending tab command, and relays from
    /// different connections interleave, so snapshots can land out of order.
    /// Accept only snapshots that advance the applied revision; a stale
    /// replay would otherwise delete tabs a newer broadcast confirmed. An
    /// empty snapshot is always accepted: it means the server restarted
    /// (in-memory registry lost) and resets the revision baseline — the
    /// removal guard already keeps mounted tabs alive against it.
    pub(crate) fn accept_registry_snapshot(&mut self, snapshot: &TabRegistrySnapshot) -> bool {
        if snapshot.tabs.is_empty() {
            self.registry_revision = Some(snapshot.revision);
            return true;
        }
        if self
            .registry_revision
            .is_some_and(|last| snapshot.revision <= last)
        {
            return false;
        }
        self.registry_revision = Some(snapshot.revision);
        true
    }

    pub(crate) fn apply_tab_registry(
        &mut self,
        snapshot: TabRegistrySnapshot,
    ) -> TabRegistryReconcile {
        if snapshot.tabs.is_empty() {
            // A server restart clears its in-memory registry. Old TabIds are
            // no longer valid, so clear them before the first replacement
            // `New` snapshot arrives; otherwise that partial snapshot would
            // look like a close and remove every tab not reconnected yet.
            for state in self.tabs.values_mut() {
                state.tab_id = None;
            }
        }
        for (client_id, state) in &mut self.tabs {
            if let Some(entry) = snapshot
                .tabs
                .iter()
                .find(|entry| entry.client_id == *client_id)
            {
                state.tab_id = Some(entry.tab_id);
            }
        }
        let mut removed = Vec::new();
        if !snapshot.tabs.is_empty() {
            // Removals key on the server-assigned `TabId`, never the raw
            // client id: a tab still awaiting its registry entry (`tab_id`
            // None) must survive replayed snapshots that race its own
            // `TabCommand::New` (stale entries from dead connections keep
            // the snapshot non-empty), and a reconnected tab matches its
            // entry through the `Reclaim` rebind despite the new client id.
            removed = self
                .tabs
                .iter()
                .filter_map(|(client_id, state)| {
                    let tab_id = state.tab_id?;
                    if snapshot.tabs.iter().any(|entry| entry.tab_id == tab_id) {
                        None
                    } else {
                        Some(*client_id)
                    }
                })
                .collect();
        }
        let mut new_active = None;
        if let Some(active_tab_id) = snapshot.active
            && let Some(entry) = snapshot
                .tabs
                .iter()
                .find(|entry| entry.tab_id == active_tab_id)
            && self.tabs.contains_key(&entry.client_id)
            && entry.client_id != self.active_tab
        {
            new_active = Some(entry.client_id);
        }
        if removed.contains(&self.active_tab) {
            new_active = self
                .tabs
                .keys()
                .copied()
                .find(|client_id| !removed.contains(client_id));
        }
        let mut cards: Vec<clay::masonry_shell::TabCard> = snapshot
            .tabs
            .iter()
            .filter(|entry| self.tabs.contains_key(&entry.client_id))
            .map(|entry| clay::masonry_shell::TabCard {
                client_id: entry.client_id,
                name: tab_card_display_name(&entry.workspace_root),
                closable: true,
            })
            .collect();
        // Phase 22.4: entry-less mounted tabs append after the registry
        // entries, matching `tab_order` (the shared card order commands
        // resolve against; the registry mirror is updated below, so the two
        // orders agree once the reconcile lands).
        for (client_id, state) in &self.tabs {
            if !snapshot
                .tabs
                .iter()
                .any(|entry| entry.client_id == *client_id)
                && !removed.contains(client_id)
            {
                cards.push(clay::masonry_shell::TabCard {
                    client_id: *client_id,
                    name: tab_card_display_name(&state.workspace_root),
                    closable: false,
                });
            }
        }
        self.registry = snapshot;
        TabRegistryReconcile {
            removed,
            new_active,
            cards,
        }
    }

    /// Apply a registry reconciliation to the shell: uninstall removed tabs,
    /// switch + focus the reconciled active tab, and push the tab bar cards.
    pub(crate) fn apply_registry_reconcile(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        reconcile: TabRegistryReconcile,
    ) {
        let active_removed = reconcile.removed.contains(&self.active_tab);
        if active_removed {
            // The active tab's window-level surface cannot survive its
            // server-authoritative removal.
            self.remove_centered_layer(ctx.render_root(window_id));
        }
        if !reconcile.removed.is_empty() {
            with_shell(
                ctx.render_root(window_id),
                self.shell_widget_id,
                |shell, shell_ctx| {
                    for client_id in &reconcile.removed {
                        shell.remove_tab(shell_ctx, *client_id);
                    }
                },
            );
            for client_id in &reconcile.removed {
                // A retrying reconnect task must stop trying for a closed tab.
                if let Some(cancel) = self.reconnect_cancel.remove(client_id) {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                self.tabs.remove(client_id);
            }
        }
        // A rejected optimistic activate (or a close of the active tab)
        // switches the mounted tab; `switch_tab` updates the mirrors + focus.
        if let Some(new_active) = reconcile.new_active {
            self.switch_tab(ctx, window_id, new_active);
        } else if active_removed && self.tabs.len() == 1 {
            // The last remaining tab is now active: refresh the mirror.
            let client_id = *self.tabs.iter().next().expect("one tab remains").0;
            self.active_tab = client_id;
            if let Some(chrome_id) = with_shell(
                ctx.render_root(window_id),
                self.shell_widget_id,
                |shell, _| shell.editor_widget_id_for(client_id),
            )
            .flatten()
            {
                self.editor_widget_id = chrome_id;
                self.sync_centered_layer(ctx, window_id, chrome_id);
                if let Some(target) = with_shell(
                    ctx.render_root(window_id),
                    self.shell_widget_id,
                    |shell, _| shell.active_pane_target_for(client_id),
                )
                .flatten()
                {
                    let _ = ctx.render_root(window_id).focus_on(Some(target));
                }
            }
        }
        let cards = reconcile.cards;
        with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, shell_ctx| {
                shell.set_tab_cards(shell_ctx, cards);
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{tab_snapshot, tab_state_with_queue, test_driver_with_tabs};
    use super::*;
    use clay::protocol::TabEntry;

    fn registry_snap(revision: u64, entry: Option<&TabEntry>) -> TabRegistrySnapshot {
        TabRegistrySnapshot {
            tabs: entry.into_iter().cloned().collect(),
            active: None,
            revision,
        }
    }

    #[test]
    fn tab_card_display_name_never_falls_back_to_an_absolute_path() {
        assert_eq!(tab_card_display_name("/tmp/workspace"), "workspace");
        assert_eq!(tab_card_display_name("/"), "Workspace");
        assert_eq!(tab_card_display_name("\u{1f}"), "Workspace");
    }

    #[test]
    fn accept_registry_snapshot_rejects_stale_relays_and_resets_on_restart() {
        // Registry relays interleave across connections (a connection's
        // handshake replay races its own pending tab command): only
        // revision-advancing snapshots may be applied, or a stale replay
        // deletes tabs a newer broadcast already confirmed.
        let entry = clay::protocol::TabEntry {
            tab_id: 1,
            workspace_root_id: 1,
            client_id: 1,
            workspace_root: "/tmp/root".to_string(),
        };
        let mut driver = test_driver_with_tabs(BTreeMap::new());
        assert!(driver.accept_registry_snapshot(&registry_snap(1, Some(&entry))));
        assert!(
            !driver.accept_registry_snapshot(&registry_snap(1, Some(&entry))),
            "equal revision is a duplicate relay"
        );
        assert!(
            !driver.accept_registry_snapshot(&registry_snap(0, Some(&entry))),
            "older revision is a stale replay"
        );
        assert!(driver.accept_registry_snapshot(&registry_snap(2, Some(&entry))));
        // Server restart: the empty registry resets the revision baseline
        // even at a lower revision; apply_tab_registry also clears stale
        // TabIds before replacement New snapshots arrive.
        assert!(driver.accept_registry_snapshot(&registry_snap(0, None)));
        assert!(driver.accept_registry_snapshot(&registry_snap(1, Some(&entry))));
    }

    #[test]
    fn apply_tab_registry_fills_server_assigned_tab_ids_and_builds_cards() {
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, _receiver_b) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
        ]));

        let reconcile = driver.apply_tab_registry(TabRegistrySnapshot {
            tabs: vec![
                clay::protocol::TabEntry {
                    tab_id: 101,
                    workspace_root_id: 7,
                    client_id: 11,
                    workspace_root: "/tmp/alpha".to_string(),
                },
                clay::protocol::TabEntry {
                    tab_id: 102,
                    workspace_root_id: 8,
                    client_id: 22,
                    workspace_root: "/tmp/beta".to_string(),
                },
            ],
            active: Some(102),
            revision: 0,
        });

        assert_eq!(driver.tabs[&11].tab_id, Some(101));
        assert_eq!(driver.tabs[&22].tab_id, Some(102));
        assert_eq!(driver.registry.active, Some(102));
        assert_eq!(driver.registry.tabs.len(), 2);
        // No removals; the server's active tab (client 22) wins over the
        // driver mirror (0).
        assert!(reconcile.removed.is_empty());
        assert_eq!(reconcile.new_active, Some(22));
        // Cards follow registry order and carry display names + close.
        assert_eq!(reconcile.cards.len(), 2);
        assert_eq!(reconcile.cards[0].client_id, 11);
        assert_eq!(reconcile.cards[0].name, "alpha");
        assert!(reconcile.cards[0].closable);
        assert_eq!(reconcile.cards[1].name, "beta");
    }

    #[test]
    fn apply_tab_registry_removes_closed_tab_and_activates_remaining() {
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, _receiver_b) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
        ]));
        driver.active_tab = 22;

        // The server closed tab 102 (client 22): the registry no longer
        // carries it, and the remaining tab becomes active. Both tabs carry
        // server-assigned ids (removals key on `tab_id`).
        driver.tabs.get_mut(&11).unwrap().tab_id = Some(101);
        driver.tabs.get_mut(&22).unwrap().tab_id = Some(102);
        let reconcile = driver.apply_tab_registry(TabRegistrySnapshot {
            tabs: vec![clay::protocol::TabEntry {
                tab_id: 101,
                workspace_root_id: 7,
                client_id: 11,
                workspace_root: "/tmp/alpha".to_string(),
            }],
            active: Some(101),
            revision: 0,
        });

        assert_eq!(reconcile.removed, vec![22]);
        assert_eq!(reconcile.new_active, Some(11));
        assert_eq!(reconcile.cards.len(), 1);
        assert_eq!(reconcile.cards[0].client_id, 11);
    }

    #[test]
    fn apply_tab_registry_keeps_unregistered_tab_against_stale_snapshot() {
        // Startup race: the handshake replays a non-empty registry holding
        // stale entries from dead connections while this client's own tab is
        // still awaiting its `TabCommand::New` confirmation (`tab_id` None).
        // The unregistered tab must survive — removing the last mounted tab
        // leaves `active_tab` dangling and panics the shell paint.
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let mut driver =
            test_driver_with_tabs(BTreeMap::from([(3, tab_state_with_queue(queue_a))]));
        driver.active_tab = 3;

        let reconcile = driver.apply_tab_registry(TabRegistrySnapshot {
            tabs: vec![clay::protocol::TabEntry {
                tab_id: 101,
                workspace_root_id: 7,
                client_id: 9,
                workspace_root: "/tmp/stale".to_string(),
            }],
            active: None,
            revision: 0,
        });

        assert!(reconcile.removed.is_empty());
        assert_eq!(reconcile.new_active, None);
        // The entry-less mounted tab appends after the stale entries' cards.
        assert_eq!(reconcile.cards.len(), 1);
        assert_eq!(reconcile.cards[0].client_id, 3);
        assert!(!reconcile.cards[0].closable);
    }

    #[test]
    fn apply_tab_registry_skips_removals_on_empty_snapshot() {
        // An empty registry means the server restarted (in-memory registry
        // lost): mounted tabs survive for the lifecycle task's re-registration
        // (Reclaim/New). No removals, no activation churn.
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, _receiver_b) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
        ]));
        driver.tabs.get_mut(&11).unwrap().tab_id = Some(101);

        let reconcile = driver.apply_tab_registry(TabRegistrySnapshot {
            tabs: Vec::new(),
            active: None,
            revision: 0,
        });

        assert!(reconcile.removed.is_empty());
        assert_eq!(reconcile.new_active, None);
        assert_eq!(driver.tabs.len(), 2);
        assert_eq!(driver.tabs[&11].tab_id, None, "stale ids are cleared");
        // Both mounted tabs still get cards (entry-less → close disabled).
        assert_eq!(reconcile.cards.len(), 2);
        assert!(reconcile.cards.iter().all(|card| !card.closable));

        // The first replacement tab can arrive before the other reconnects;
        // entry-less tabs remain mounted and await their own New snapshot.
        let reconcile = driver.apply_tab_registry(TabRegistrySnapshot {
            tabs: vec![clay::protocol::TabEntry {
                tab_id: 201,
                workspace_root_id: 7,
                client_id: 11,
                workspace_root: "/tmp/alpha".to_string(),
            }],
            active: Some(201),
            revision: 1,
        });
        assert!(reconcile.removed.is_empty());
        assert_eq!(driver.tabs[&11].tab_id, Some(201));
        assert_eq!(driver.tabs[&22].tab_id, None);
        assert_eq!(reconcile.cards.len(), 2);
    }

    #[test]
    fn apply_tab_registry_reorder_preserves_per_tab_state() {
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, _receiver_b) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
        ]));
        driver.tabs.get_mut(&22).unwrap().pending_opens.insert(
            PaneId(2),
            PendingOpenRequest {
                path: None,
                root_id: Some(8),
                relative_path: Some("docs/intro.md".to_string()),
            },
        );
        // Order [11, 22] establishes tab ids.
        let reconcile = driver.apply_tab_registry(tab_snapshot(&[(11, 101), (22, 102)]));
        assert_eq!(
            reconcile
                .cards
                .iter()
                .map(|card| card.client_id)
                .collect::<Vec<_>>(),
            vec![11, 22]
        );
        // Reordered snapshot [22, 11]: cards follow the new order, and every
        // tab's internal state (tab id, workspace, pending opens, queue)
        // survives untouched.
        let reconcile = driver.apply_tab_registry(tab_snapshot(&[(22, 102), (11, 101)]));
        assert!(reconcile.removed.is_empty());
        assert_eq!(
            reconcile
                .cards
                .iter()
                .map(|card| card.client_id)
                .collect::<Vec<_>>(),
            vec![22, 11]
        );
        assert_eq!(driver.tabs[&11].tab_id, Some(101));
        assert_eq!(driver.tabs[&22].tab_id, Some(102));
        // Mount-time state untouched by reorder (card names read the
        // registry entries, not this field).
        assert_eq!(driver.tabs[&11].workspace_root, "/tmp/root");
        assert_eq!(driver.tabs[&22].workspace_root, "/tmp/root");
        assert_eq!(driver.tabs[&22].pending_opens.len(), 1);
        assert!(driver.tabs[&11].edit_queue.is_some());
        assert!(driver.tabs[&22].edit_queue.is_some());
    }

    #[test]
    fn ordered_tab_clients_registry_order_then_entry_less() {
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, _receiver_b) = ClientEditQueue::bounded(4);
        let (queue_c, _receiver_c) = ClientEditQueue::bounded(4);
        let driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
            (33, tab_state_with_queue(queue_c)),
        ]));
        // Registry order [22, 11]; tab 33 is mounted but entry-less.
        let registry = tab_snapshot(&[(22, 202), (11, 101)]);
        assert_eq!(
            ordered_tab_clients(&registry, &driver.tabs),
            vec![22, 11, 33]
        );
        // Empty registry: mounted tabs in client-id order.
        let registry = tab_snapshot(&[]);
        assert_eq!(
            ordered_tab_clients(&registry, &driver.tabs),
            vec![11, 22, 33]
        );
    }
}
