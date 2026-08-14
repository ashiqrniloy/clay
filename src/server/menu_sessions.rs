//! Per-connection server-owned transient menu session store (Phase 24.1).
//!
//! Server menu sessions are interactive over the protocol round-trip: the
//! client renders bounded snapshots and forwards query/selection/activate/
//! cancel intents; the server is authoritative for query, items, selection,
//! and activation. Sessions open ONLY through the built-in command execution
//! path (`controlCenter.open` / `controlCenter.openPath`); no package op can
//! create or drive a session.
//!
//! Store invariants:
//! - One active session per connection: opening a new one replaces the old,
//!   and the caller reports `TransientMenuClosed` for the replaced id.
//! - Session ids live in the high-bit partition (`1 << 63 | n`) so they can
//!   never collide with the client-local `PaneMenuSync` allocator (which
//!   starts at 1).
//! - The store is a plain field of the connection loop: it drops with the
//!   connection, so a disconnect sweeps every session with no cross-connection
//!   leak.
//! - Unknown/stale ids are dropped by handlers with a bounded
//!   `menu.unknown_session` diagnostic, never a panic or disconnect.
//!
//! Kind dispatch is an enum with a closed set of variants. Path mode (Phase
//! 24.3) added the `PathBrowser` variant here without touching the connection
//! handlers. `# ponytail: enum; trait object only if a third kind forces it`.

use std::collections::HashMap;

use crate::{
    packages::commands::CommandCatalogue,
    protocol::{
        TransientMenuActivationData, TransientMenuFocusPolicyData, TransientMenuItemData,
        TransientMenuOriginData, TransientMenuSnapshotData, TransientMenuStatusData,
    },
    server::{
        command_execution::{
            CommandExecutionDiagnostic, CommandExecutionRule, CommandExecutionTarget,
        },
        control_center::{ControlCenter, ServerMenuActivation},
        workspace::UserBrowsePage,
    },
    shell::{
        path_browser::{PathBrowserActivation, PathBrowserSession, PathBrowserTransition},
        transient_menu::{
            TransientMenuFocusPolicy, TransientMenuOrigin, TransientMenuSession,
            TransientMenuSessionId, TransientMenuStatus,
        },
    },
};

/// Session ids allocated by [`ServerMenuSessions`] carry this bit, keeping
/// them disjoint from the client-local `PaneMenuSync` allocator (ids from 1).
pub(crate) const SERVER_MENU_SESSION_ID_HIGH_BIT: u64 = 1 << 63;

/// Per-connection store of server-owned transient menu sessions.
#[derive(Debug, Default)]
pub(crate) struct ServerMenuSessions {
    next_session_id: u64,
    active: HashMap<u64, ServerMenuSession>,
}

impl ServerMenuSessions {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Opens a new Control Center session (24.1's first kind) and returns its
    /// initial snapshot plus the replaced session's id. One active session per
    /// connection: the previous session (if any) is dropped and its id
    /// returned, which the caller must report as `TransientMenuClosed` before
    /// pushing the new snapshot.
    pub(crate) fn open_control_center(
        &mut self,
        catalogue: &CommandCatalogue,
        generation_id: u64,
    ) -> (TransientMenuSnapshotData, Option<u64>) {
        self.next_session_id += 1;
        let id = SERVER_MENU_SESSION_ID_HIGH_BIT | self.next_session_id;
        let replaced_id = self.active.keys().next().copied();
        self.active.clear();
        let session = ServerMenuSession::control_center(
            ControlCenter::open_catalogue(catalogue, id),
            id,
            generation_id,
        );
        let snapshot = snapshot_from_session(&session.session());
        self.active.insert(id, session);
        (snapshot, replaced_id)
    }

    /// Opens a new Path Browser session (Phase 24.3's second kind) seeded
    /// with a caller-resolved canonical starting directory and its initial
    /// bounded listing already installed. Mirrors [`Self::open_control_center`]
    /// exactly: one active session per connection, replaced id reported by
    /// the caller as `TransientMenuClosed` before the new snapshot.
    pub(crate) fn open_path_browser(
        &mut self,
        session: PathBrowserSession,
        generation_id: u64,
    ) -> (TransientMenuSnapshotData, Option<u64>) {
        self.next_session_id += 1;
        let id = SERVER_MENU_SESSION_ID_HIGH_BIT | self.next_session_id;
        let replaced_id = self.active.keys().next().copied();
        self.active.clear();
        let session = ServerMenuSession::path_browser(session, id, generation_id);
        let snapshot = snapshot_from_session(&session.session());
        self.active.insert(id, session);
        (snapshot, replaced_id)
    }

    pub(crate) fn get_mut(&mut self, session_id: u64) -> Option<&mut ServerMenuSession> {
        self.active.get_mut(&session_id)
    }

    /// Removes the session (activation and cancel both consume it).
    pub(crate) fn cancel(&mut self, session_id: u64) -> Option<ServerMenuSession> {
        self.active.remove(&session_id)
    }

    /// Cancels the connection's active session (if any) and returns its id.
    /// Invariant: at most one active session per connection, so this is the
    /// sweep used by tab switches and other per-tab lifecycle events; the
    /// caller reports the id as `TransientMenuClosed`.
    pub(crate) fn cancel_active(&mut self) -> Option<u64> {
        let id = self.active.keys().next().copied()?;
        self.active.remove(&id);
        Some(id)
    }
}

/// Outcome of [`ServerMenuSession::activate`]: either a dispatchable
/// activation (the caller consumes the session, pushes `TransientMenuClosed`,
/// then routes it), a path-mode navigation (the session stays open and the
/// caller relists the target directory, pushing a fresh snapshot), or a
/// path-mode file open (the caller consumes the session, pushes
/// `TransientMenuClosed`, then runs the selected-file open; the browse
/// activation itself is the authorization event that converts to a
/// `SingleFile` grant).
#[derive(Debug)]
pub(crate) enum ServerMenuActivateOutcome {
    Dispatch(ServerMenuActivation),
    Navigate(std::path::PathBuf),
    OpenFile(std::path::PathBuf),
    OpenWorkspace(std::path::PathBuf),
}

/// Result of an edit intent: the projected snapshot plus an optional relist
/// target. Only the Path Browser arm ever produces a relist (its directory
/// prefix changed); the connection runs the bounded listing, installs the
/// page, and re-projects.
#[derive(Debug)]
pub(crate) struct MenuEdit {
    pub(crate) snapshot: TransientMenuSession,
    pub(crate) relist: Option<std::path::PathBuf>,
}

/// One active server menu session. Kind dispatch is an enum (closed set):
/// handlers never match on kinds themselves, they call the shared methods.
#[derive(Debug)]
pub(crate) struct ServerMenuSession {
    /// Opaque session id (high-bit partition); needed by the Path Browser's
    /// projection, which takes the id explicitly rather than owning it.
    session_id: u64,
    generation_id: u64,
    kind: ServerMenuSessionKind,
}

#[derive(Debug)]
pub(crate) enum ServerMenuSessionKind {
    ControlCenter(ControlCenter),
    PathBrowser(PathBrowserSession),
}

impl ServerMenuSession {
    fn control_center(center: ControlCenter, session_id: u64, generation_id: u64) -> Self {
        Self {
            session_id,
            generation_id,
            kind: ServerMenuSessionKind::ControlCenter(center),
        }
    }

    fn path_browser(session: PathBrowserSession, session_id: u64, generation_id: u64) -> Self {
        Self {
            session_id,
            generation_id,
            kind: ServerMenuSessionKind::PathBrowser(session),
        }
    }

    /// Replaces the filter query (clamped at the shared query budget before
    /// reaching session state) and returns the projected snapshot plus an
    /// optional relist target (path mode only, when the directory prefix
    /// changed).
    pub(crate) fn set_query(&mut self, query: &str) -> MenuEdit {
        let query: String = query
            .chars()
            .take(crate::perf::budgets::TRANSIENT_MENU_MAX_QUERY_CHARS)
            .collect();
        match &mut self.kind {
            ServerMenuSessionKind::ControlCenter(center) => MenuEdit {
                snapshot: center.set_query(query),
                relist: None,
            },
            // Phase 24.3: full-value path input replacement. A `FilterOnly`
            // transition scores the installed entries locally with no
            // filesystem work; a `Relist` transition (directory prefix
            // changed) is executed by the connection (plan 083 task 8).
            ServerMenuSessionKind::PathBrowser(session) => {
                let transition = session.set_input(&query);
                let relist = match transition {
                    PathBrowserTransition::FilterOnly => None,
                    PathBrowserTransition::Relist { target } => Some(target),
                };
                MenuEdit {
                    snapshot: session.menu_session(TransientMenuSessionId(self.session_id)),
                    relist,
                }
            }
        }
    }

    /// Generic semantic Backspace (Phase 24.3): the session kind decides
    /// whether Backspace deletes query text (Control Center) or ascends when
    /// the filter is empty (path mode). An ascent produces a relist target
    /// the connection executes; a root no-op produces none.
    pub(crate) fn backspace(&mut self) -> MenuEdit {
        match &mut self.kind {
            ServerMenuSessionKind::ControlCenter(center) => MenuEdit {
                snapshot: center.backspace(),
                relist: None,
            },
            ServerMenuSessionKind::PathBrowser(session) => {
                let transition = session.backspace();
                let relist = match transition {
                    PathBrowserTransition::FilterOnly => None,
                    PathBrowserTransition::Relist { target } => Some(target),
                };
                MenuEdit {
                    snapshot: session.menu_session(TransientMenuSessionId(self.session_id)),
                    relist,
                }
            }
        }
    }

    /// Installs a fresh bounded listing page into the Path Browser session
    /// (a completed relist) and re-projects. The Control Center arm is a
    /// no-op: relist targets are only ever produced by the path arm, so this
    /// cannot fire on a Control Center session.
    pub(crate) fn install_path_browser(&mut self, page: UserBrowsePage) -> TransientMenuSession {
        match &mut self.kind {
            ServerMenuSessionKind::ControlCenter(_) => {}
            ServerMenuSessionKind::PathBrowser(session) => session.install(page),
        }
        self.session()
    }

    /// Records a failed relist as the Path Browser's sticky error status
    /// (items suppressed, activation fails closed, input stays recoverable).
    pub(crate) fn set_path_browser_error(&mut self, message: String) -> TransientMenuSession {
        match &mut self.kind {
            ServerMenuSessionKind::ControlCenter(_) => {}
            ServerMenuSessionKind::PathBrowser(session) => session.set_error(message),
        }
        self.session()
    }

    /// Relative selection movement, clamped/wrapped per
    /// `TransientMenuSession::select_next` semantics.
    pub(crate) fn move_selection(&mut self, delta: i64) -> TransientMenuSession {
        match &mut self.kind {
            ServerMenuSessionKind::ControlCenter(center) => center.move_selection(delta),
            ServerMenuSessionKind::PathBrowser(session) => {
                session.move_selection(delta);
                session.menu_session(TransientMenuSessionId(self.session_id))
            }
        }
    }

    /// Current session (query + persisted selection) for snapshot pushes.
    pub(crate) fn session(&self) -> TransientMenuSession {
        match &self.kind {
            ServerMenuSessionKind::ControlCenter(center) => center.session(),
            ServerMenuSessionKind::PathBrowser(session) => {
                session.menu_session(TransientMenuSessionId(self.session_id))
            }
        }
    }

    /// Produces the typed activation for the currently selected item: a
    /// `CommandExecutionRequest` (server/package commands, dispatched through
    /// the shared connection dispatcher) or a narrow shell command id
    /// (`ClientUiCommand` items, re-parsed client-side deny-by-default), or a
    /// path-mode navigation that keeps the session open. Consumes nothing;
    /// the caller removes the session and pushes `TransientMenuClosed` only
    /// for the `Dispatch` outcome. `kind` is interpreted per session kind
    /// (the Control Center activates the same selection for primary and
    /// secondary; path mode descends on primary directory activation).
    pub(crate) fn activate(
        &self,
        target: CommandExecutionTarget,
        kind: TransientMenuActivationData,
        current_generation_id: u64,
    ) -> Result<ServerMenuActivateOutcome, CommandExecutionDiagnostic> {
        if self.generation_id != current_generation_id {
            return Err(CommandExecutionDiagnostic {
                command_id: String::new(),
                rule: CommandExecutionRule::StaleRuntimeGeneration,
                message: "menu session belongs to a replaced runtime generation".to_string(),
            });
        }
        match &self.kind {
            ServerMenuSessionKind::ControlCenter(center) => {
                let _ = kind; // Control Center: both kinds activate the selection
                center
                    .selected_activation(target)
                    .map(ServerMenuActivateOutcome::Dispatch)
            }
            // Phase 24.3: primary activation on a directory descends (the
            // session stays open and the caller relists the canonical
            // target); primary activation on a file resolves to the
            // server-held canonical path and the caller runs the ordinary
            // selected-file open (plan 083 task 9), which converts the
            // browse authority into a `SingleFile` grant; secondary
            // activation on a directory resolves to a workspace open
            // (task 10), which converts the browse authority into a
            // `Directory` root grant for the bound tab.
            ServerMenuSessionKind::PathBrowser(session) => {
                let activation = match kind {
                    TransientMenuActivationData::Primary => session.activate(),
                    TransientMenuActivationData::Secondary => session.activate_workspace(),
                };
                match activation {
                    Some(PathBrowserActivation::Descend(target)) => {
                        Ok(ServerMenuActivateOutcome::Navigate(target))
                    }
                    Some(PathBrowserActivation::OpenFile(path)) => {
                        Ok(ServerMenuActivateOutcome::OpenFile(path))
                    }
                    Some(PathBrowserActivation::OpenWorkspace(path)) => {
                        Ok(ServerMenuActivateOutcome::OpenWorkspace(path))
                    }
                    None => Err(CommandExecutionDiagnostic {
                        command_id: String::new(),
                        rule: CommandExecutionRule::UnknownCommand,
                        message: "no activation for the selected path browser item".to_string(),
                    }),
                }
            }
        }
    }
}

/// Projection from the shell session model to the inert protocol snapshot.
/// Inert display data only: no actions, paths, or authority fields cross the
/// wire; activation is by opaque session id.
pub(crate) fn snapshot_from_session(session: &TransientMenuSession) -> TransientMenuSnapshotData {
    TransientMenuSnapshotData::new(
        session.session_id().0,
        session.prompt(),
        session.query(),
        session
            .items()
            .iter()
            .map(|item| {
                TransientMenuItemData::new(
                    item.id.clone(),
                    item.label.clone(),
                    item.detail.clone(),
                    item.accessibility_label.clone(),
                )
            })
            .collect(),
        session.selected_index() as u32,
        match session.status() {
            TransientMenuStatus::Active | TransientMenuStatus::Cancelled => {
                // `Cancelled` never crosses the wire; `TransientMenuClosed`
                // is the terminal message for ended sessions.
                TransientMenuStatusData::Active
            }
            TransientMenuStatus::Empty { message } => TransientMenuStatusData::Empty {
                message: message.clone(),
            },
        },
        match session.focus_policy() {
            TransientMenuFocusPolicy::Modal => TransientMenuFocusPolicyData::Modal,
            TransientMenuFocusPolicy::Modeless => TransientMenuFocusPolicyData::Modeless,
        },
        match session.origin() {
            TransientMenuOrigin::CommandPalette => TransientMenuOriginData::CommandPalette,
            // Completion sessions are client-local and never serialize through
            // the server-owned menu snapshot protocol.
            TransientMenuOrigin::Completion => TransientMenuOriginData::CommandPalette,
            TransientMenuOrigin::ContextMenu => TransientMenuOriginData::ContextMenu,
            TransientMenuOrigin::MenuBar => TransientMenuOriginData::MenuBar,
            TransientMenuOrigin::Centered => TransientMenuOriginData::Centered,
        },
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::{
        packages::{
            commands::{
                CommandCatalogue, CommandRegistry, PackageCommandDeclaration, RegisteredCommand,
            },
            manifest::validate_manifest_value,
            permissions::PackagePermission,
        },
        protocol::{KeyBindingRule, KeyCode, RoutingPolicy},
    };

    fn package_manifest() -> crate::packages::manifest::ClayPackageManifest {
        validate_manifest_value(&json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "clay": {
                "apiPrefix": "markdown",
                "permissions": ["command-registration", "parse-document"],
                "modes": ["markdown"],
                "entry": "./dist/index.js"
            }
        }))
        .expect("valid package manifest")
    }

    fn register_command(
        registry: &mut CommandRegistry,
        command_id: &str,
        display_name: &str,
    ) -> RegisteredCommand {
        let manifest = package_manifest();
        registry
            .register_command(
                &manifest,
                PackageCommandDeclaration {
                    package_name: "@clay/markdown".to_string(),
                    package_version: "0.1.0".to_string(),
                    api_prefix: "markdown".to_string(),
                    command_id: command_id.to_string(),
                    display_name: display_name.to_string(),
                    routing_policy: RoutingPolicy::ServerFirst,
                    key_bindings: vec![KeyBindingRule::single(
                        command_id,
                        KeyCode::Character("x".to_string()),
                    )],
                    custom_properties: BTreeMap::new(),
                    permissions: vec![PackagePermission::ParseDocument],
                },
            )
            .expect("register command")
    }

    fn registry_with_commands() -> CommandRegistry {
        let mut registry = CommandRegistry::new();
        register_command(&mut registry, "markdown.togglePreview", "Toggle Preview");
        register_command(&mut registry, "markdown.refreshPreview", "Refresh Preview");
        register_command(&mut registry, "markdown.toggleList", "Toggle List");
        registry
    }

    fn catalogue_for_registry(registry: &CommandRegistry) -> CommandCatalogue {
        CommandCatalogue::from_sources(
            vec![registry.snapshot()],
            &crate::protocol::BehaviorManifest::minimal_text_editing(1),
        )
        .expect("test catalogue should validate")
    }

    fn open_id(store: &mut ServerMenuSessions, registry: &CommandRegistry) -> u64 {
        let catalogue = catalogue_for_registry(registry);
        let (snapshot, _) = store.open_control_center(&catalogue, 1);
        snapshot.session_id
    }

    #[test]
    fn open_allocates_high_bit_ids_and_replaces() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let catalogue = catalogue_for_registry(&registry);
        let (snapshot, replaced) = store.open_control_center(&catalogue, 1);
        assert_eq!(replaced, None);
        let first_id = snapshot.session_id;
        assert!(first_id & SERVER_MENU_SESSION_ID_HIGH_BIT != 0);
        assert!(first_id > SERVER_MENU_SESSION_ID_HIGH_BIT);

        // Opening a second session replaces the first and reports its id.
        let (_, replaced) = store.open_control_center(&catalogue, 1);
        assert_eq!(replaced, Some(first_id));
        assert!(store.get_mut(first_id).is_none());
        assert!(store.cancel(first_id).is_none());
    }

    #[test]
    fn open_path_browser_replaces_active_session_and_projects() {
        use crate::server::workspace::{UserBrowseEntry, UserBrowseEntryKind, UserBrowsePage};
        use crate::shell::path_browser::PathBrowserSession;

        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let control_center_id = open_id(&mut store, &registry);

        // Install a page before opening: the store opens the session with
        // its listing already installed (the connection lists once first).
        let mut session = PathBrowserSession::new(std::path::PathBuf::from("/home"));
        session.install(UserBrowsePage {
            canonical_dir: std::path::PathBuf::from("/usr"),
            entries: vec![UserBrowseEntry {
                name: "bin".to_string(),
                kind: UserBrowseEntryKind::Directory,
                canonical_path: std::path::PathBuf::from("/usr/bin"),
                size: None,
            }],
            truncated: false,
        });
        let (snapshot, replaced) = store.open_path_browser(session, 1);
        assert_eq!(
            replaced,
            Some(control_center_id),
            "path browser replaces the active session"
        );
        let path_id = snapshot.session_id;
        assert_ne!(path_id, control_center_id);
        assert_eq!(snapshot.prompt, "Browse · /usr");
        assert_eq!(snapshot.query, "/usr/");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].label, "bin");

        // Query updates go through the shared session methods: a filter-only
        // edit scores installed entries locally (no filesystem work).
        let updated = store.get_mut(path_id).unwrap().set_query("b");
        assert_eq!(updated.snapshot.items().len(), 1);
        let updated = store.get_mut(path_id).unwrap().set_query("x");
        assert_eq!(updated.snapshot.items().len(), 0);

        // Reopening the Control Center replaces the path session in turn.
        let (_, replaced) = store.open_control_center(&catalogue_for_registry(&registry), 1);
        assert_eq!(replaced, Some(path_id));
    }

    #[test]
    fn path_browser_navigation_transitions_target_relists() {
        use crate::server::workspace::{UserBrowseEntry, UserBrowseEntryKind, UserBrowsePage};
        use crate::shell::path_browser::PathBrowserSession;

        let mut store = ServerMenuSessions::new();
        let mut session = PathBrowserSession::new(std::path::PathBuf::from("/home"));
        session.install(UserBrowsePage {
            canonical_dir: std::path::PathBuf::from("/home"),
            entries: vec![
                UserBrowseEntry {
                    name: "src".to_string(),
                    kind: UserBrowseEntryKind::Directory,
                    canonical_path: std::path::PathBuf::from("/home/src"),
                    size: None,
                },
                UserBrowseEntry {
                    name: "README.md".to_string(),
                    kind: UserBrowseEntryKind::File,
                    canonical_path: std::path::PathBuf::from("/home/README.md"),
                    size: Some(7),
                },
            ],
            truncated: false,
        });
        let (snapshot, _) = store.open_path_browser(session, 1);
        let id = snapshot.session_id;

        // Filter-only edits never produce a relist target: no filesystem
        // work happens for ordinary fuzzy characters.
        let edit = store.get_mut(id).unwrap().set_query("RE");
        assert!(edit.relist.is_none());
        assert_eq!(edit.snapshot.items().len(), 1);

        // A changed directory prefix (absolute jump) produces a relist.
        let edit = store.get_mut(id).unwrap().set_query("/usr/");
        assert_eq!(edit.relist, Some(std::path::PathBuf::from("/usr")));

        // Installing the completed listing re-projects the same session id
        // with the canonical directory and a reset selection.
        let installed = store
            .get_mut(id)
            .unwrap()
            .install_path_browser(UserBrowsePage {
                canonical_dir: std::path::PathBuf::from("/usr"),
                entries: vec![UserBrowseEntry {
                    name: "local".to_string(),
                    kind: UserBrowseEntryKind::Directory,
                    canonical_path: std::path::PathBuf::from("/usr/local"),
                    size: None,
                }],
                truncated: false,
            });
        assert_eq!(installed.session_id().0, id, "session id stays stable");
        assert_eq!(installed.prompt(), "Browse · /usr");
        assert_eq!(installed.query(), "/usr/");
        assert_eq!(installed.items().len(), 1);

        // Semantic Backspace: a filter is deleted before an ascent; an empty
        // filter ascends to the canonical parent (a relist target).
        store.get_mut(id).unwrap().set_query("loc");
        let edit = store.get_mut(id).unwrap().backspace();
        assert!(edit.relist.is_none(), "filter deletion never relists");
        assert_eq!(edit.snapshot.query(), "lo");
        let edit = store.get_mut(id).unwrap().backspace();
        assert!(edit.relist.is_none());
        assert_eq!(edit.snapshot.query(), "l");
        // The filter is now empty; the next Backspace ascends to the parent.
        let edit = store.get_mut(id).unwrap().backspace();
        assert!(edit.relist.is_none(), "empty filter needs the next press");
        assert_eq!(edit.snapshot.query(), "");
        let edit = store.get_mut(id).unwrap().backspace();
        assert_eq!(edit.relist, Some(std::path::PathBuf::from("/")));
        assert_eq!(edit.snapshot.query(), "/");

        // A failed relist becomes the sticky error status: items suppressed,
        // input preserved, next successful install self-corrects.
        let errored = store
            .get_mut(id)
            .unwrap()
            .set_path_browser_error("cannot browse /missing: No such file".to_string());
        assert!(errored.items().is_empty());
        assert!(errored.query().ends_with('/'));
        let recovered = store
            .get_mut(id)
            .unwrap()
            .install_path_browser(UserBrowsePage {
                canonical_dir: std::path::PathBuf::from("/home"),
                entries: Vec::new(),
                truncated: false,
            });
        assert_eq!(recovered.prompt(), "Browse · /home");
        assert!(recovered.items().is_empty(), "empty directory");
    }

    #[test]
    fn path_browser_activation_dispatch_and_navigate() {
        use crate::server::workspace::{UserBrowseEntry, UserBrowseEntryKind, UserBrowsePage};
        use crate::shell::path_browser::PathBrowserSession;

        let mut store = ServerMenuSessions::new();
        let mut session = PathBrowserSession::new(std::path::PathBuf::from("/home"));
        session.install(UserBrowsePage {
            canonical_dir: std::path::PathBuf::from("/home"),
            entries: vec![
                UserBrowseEntry {
                    name: "src".to_string(),
                    kind: UserBrowseEntryKind::Directory,
                    canonical_path: std::path::PathBuf::from("/home/src"),
                    size: None,
                },
                UserBrowseEntry {
                    name: "README.md".to_string(),
                    kind: UserBrowseEntryKind::File,
                    canonical_path: std::path::PathBuf::from("/home/README.md"),
                    size: Some(7),
                },
            ],
            truncated: false,
        });
        let (snapshot, _) = store.open_path_browser(session, 1);
        let id = snapshot.session_id;
        let target = CommandExecutionTarget::ActiveDocument { document_id: 7 };

        // Primary on the selected directory (index 0) navigates: the session
        // stays open and the caller relists the canonical target.
        let outcome = store
            .get_mut(id)
            .unwrap()
            .activate(
                target.clone(),
                crate::protocol::TransientMenuActivationData::Primary,
                1,
            )
            .unwrap();
        assert!(matches!(
            outcome,
            ServerMenuActivateOutcome::Navigate(path) if path == *"/home/src"
        ));

        // Secondary on a directory is workspace-open (plan 083 task 10):
        // the activation resolves to the server-held canonical path and the
        // caller rebinds the bound tab; never a dispatch.
        let outcome = store.get_mut(id).unwrap().activate(
            target.clone(),
            crate::protocol::TransientMenuActivationData::Secondary,
            1,
        );
        assert!(matches!(
            outcome,
            Ok(ServerMenuActivateOutcome::OpenWorkspace(path)) if path == *"/home/src"
        ));

        // Primary on a file is file-open (plan 083 task 9): the activation
        // resolves to the server-held canonical path; the caller runs the
        // selected-file open (session consumed, `SingleFile` grant).
        store.get_mut(id).unwrap().set_query("README");
        let outcome = store.get_mut(id).unwrap().activate(
            target.clone(),
            crate::protocol::TransientMenuActivationData::Primary,
            1,
        );
        assert!(matches!(
            outcome,
            Ok(ServerMenuActivateOutcome::OpenFile(path)) if path == *"/home/README.md"
        ));

        // A stale generation stamp rejects activation fail-closed.
        let outcome = store.get_mut(id).unwrap().activate(
            target,
            crate::protocol::TransientMenuActivationData::Primary,
            999,
        );
        assert!(outcome.is_err());
    }

    #[test]
    fn path_browser_helpers_are_noops_on_control_center() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);

        let untouched = store.get_mut(id).unwrap().install_path_browser(
            crate::server::workspace::UserBrowsePage {
                canonical_dir: std::path::PathBuf::from("/tmp"),
                entries: Vec::new(),
                truncated: false,
            },
        );
        assert_eq!(untouched.query(), "");
        let untouched = store
            .get_mut(id)
            .unwrap()
            .set_path_browser_error("browse failed".to_string());
        assert_eq!(untouched.query(), "");
        assert!(untouched.items().len() > 1);
    }

    #[test]
    fn path_browser_cancel_clears_store_and_reopen_allocates_fresh_id() {
        use crate::shell::path_browser::PathBrowserSession;

        let mut store = ServerMenuSessions::new();
        let mut session = PathBrowserSession::new(std::path::PathBuf::from("/home"));
        let (snapshot, _) = store.open_path_browser(session, 1);
        let first_id = snapshot.session_id;

        // Cancel (Escape) removes the session; reopening allocates a fresh
        // high-bit id rather than reusing the cancelled one.
        assert_eq!(store.cancel_active(), Some(first_id));
        assert!(store.get_mut(first_id).is_none());

        session = PathBrowserSession::new(std::path::PathBuf::from("/home"));
        let (snapshot, _) = store.open_path_browser(session, 1);
        assert_ne!(snapshot.session_id, first_id, "fresh id after cancel");
        assert!(store.get_mut(snapshot.session_id).is_some());
    }

    #[test]
    fn path_browser_snapshot_stays_under_frame_ceiling() {
        use crate::{
            protocol::codec::DEFAULT_MAX_FRAME_SIZE,
            server::workspace::{UserBrowseEntry, UserBrowseEntryKind, UserBrowsePage},
            shell::path_browser::PathBrowserSession,
        };

        // A full 256-entry listing (the entry cap) must serialize well under
        // the 1 MiB codec frame ceiling with the longest possible names.
        let mut session = PathBrowserSession::new(std::path::PathBuf::from("/home"));
        let entries: Vec<UserBrowseEntry> = (0..crate::perf::budgets::TRANSIENT_MENU_MAX_ITEMS)
            .map(|index| UserBrowseEntry {
                name: format!("very-long-file-name-{index:04}.md"),
                kind: UserBrowseEntryKind::File,
                canonical_path: std::path::PathBuf::from(format!(
                    "/home/very-long-file-name-{index:04}.md"
                )),
                size: Some(index as u64),
            })
            .collect();
        session.install(UserBrowsePage {
            canonical_dir: std::path::PathBuf::from("/home"),
            entries,
            truncated: false,
        });
        let menu = session.menu_session(crate::shell::transient_menu::TransientMenuSessionId(
            1 << 63 | 1,
        ));
        let snapshot = super::snapshot_from_session(&menu);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot).map(|bytes| bytes.len());
        let size = bytes.expect("snapshot serializes");
        assert!(
            size < DEFAULT_MAX_FRAME_SIZE,
            "256-entry path browser snapshot {size} bytes exceeds the frame ceiling"
        );
    }

    #[test]
    fn backspace_deletes_one_query_character() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);

        // Fresh session: Backspace on an empty query is a harmless no-op.
        assert_eq!(store.get_mut(id).unwrap().backspace().snapshot.query(), "");

        store.get_mut(id).unwrap().set_query("markdown");
        assert_eq!(
            store.get_mut(id).unwrap().backspace().snapshot.query(),
            "markdow"
        );
        assert_eq!(
            store.get_mut(id).unwrap().backspace().snapshot.query(),
            "markdo"
        );
    }

    #[test]
    fn query_update_filters_and_resets_selection() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);
        store.get_mut(id).unwrap().move_selection(1);

        // "markdown" matches the three package commands by id; the merged
        // built-in command set contains no markdown matches.
        let session = store.get_mut(id).unwrap().set_query("markdown");
        assert_eq!(session.snapshot.items().len(), 3);
        assert_eq!(session.snapshot.selected_index(), 0);

        let session = store.get_mut(id).unwrap().set_query("zzz-nope");
        assert!(matches!(
            session.snapshot.status(),
            TransientMenuStatus::Empty { .. }
        ));
        assert_eq!(session.snapshot.items().len(), 0);
    }

    #[test]
    fn selection_moves_and_wraps() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);
        store.get_mut(id).unwrap().set_query("markdown"); // 3 items

        let session = store.get_mut(id).unwrap().move_selection(1);
        assert_eq!(session.selected_index(), 1);

        // Forward wrap: index 1 + 2 steps → 0.
        let session = store.get_mut(id).unwrap().move_selection(2);
        assert_eq!(session.selected_index(), 0);

        // Backward wrap: from 0, -1 → last item.
        let session = store.get_mut(id).unwrap().move_selection(-1);
        assert_eq!(session.selected_index(), session.items().len() - 1);

        // Huge deltas are modulo-reduced, not walked.
        let session = store.get_mut(id).unwrap().move_selection(i64::MAX);
        let len = session.items().len();
        assert_eq!(
            session.selected_index(),
            ((len - 1) + (i64::MAX as usize % len)) % len
        );
    }

    #[test]
    fn activate_produces_typed_command_activation_and_consumes() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);

        let session = store.get_mut(id).unwrap();
        session.set_query("markdown");
        session.move_selection(1); // sorted: Refresh, Toggle List, Toggle Preview
        let activation = session
            .activate(
                CommandExecutionTarget::ActiveDocument { document_id: 1 },
                TransientMenuActivationData::Primary,
                1,
            )
            .expect("activate selected");
        let ServerMenuActivateOutcome::Dispatch(ServerMenuActivation::Command(request)) =
            activation
        else {
            panic!("expected command activation")
        };
        assert_eq!(request.command_id, "markdown.toggleList");
        assert_eq!(
            request.target,
            CommandExecutionTarget::ActiveDocument { document_id: 1 }
        );

        // Activation does not consume; the handler removes the session and
        // pushes TransientMenuClosed (asserted at the connection level).
        assert!(store.cancel(id).is_some());
        assert!(store.cancel(id).is_none());
    }

    #[test]
    fn stale_generation_cannot_activate_a_catalogue_item() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);
        let result = store
            .get_mut(id)
            .expect("session remains until handler consumes it")
            .activate(
                CommandExecutionTarget::Global,
                TransientMenuActivationData::Primary,
                2,
            );

        assert!(matches!(
            result,
            Err(CommandExecutionDiagnostic {
                rule: CommandExecutionRule::StaleRuntimeGeneration,
                ..
            })
        ));
    }

    #[test]
    fn cancel_drops_session_and_unknown_ids_stay_unknown() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);

        assert!(store.cancel(id).is_some());
        assert!(store.get_mut(id).is_none());
        assert!(store.cancel(id).is_none());
    }

    #[test]
    fn snapshot_projection_is_inert_and_bounded() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);
        store.get_mut(id).unwrap().set_query("markdown");

        let snapshot = snapshot_from_session(&store.get_mut(id).unwrap().session());
        assert_eq!(snapshot.session_id, id);
        assert_eq!(snapshot.prompt, "Control Center");
        assert_eq!(snapshot.query, "markdown");
        assert_eq!(snapshot.items.len(), 3);
        // Items are label-sorted; the three package commands match.
        assert_eq!(snapshot.items[0].id, "markdown.refreshPreview");
        assert_eq!(snapshot.items[0].label, "Refresh Preview");
        assert_eq!(snapshot.focus_policy, TransientMenuFocusPolicyData::Modal);
        assert_eq!(snapshot.origin, TransientMenuOriginData::Centered);
    }

    #[test]
    fn cancel_active_returns_the_single_active_session_id() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();
        let id = open_id(&mut store, &registry);

        // One active session per connection: the sweep returns its id and
        // empties the store (the caller reports `TransientMenuClosed`).
        assert_eq!(store.cancel_active(), Some(id));
        assert!(store.get_mut(id).is_none());
        assert_eq!(store.cancel_active(), None, "nothing active");
    }

    #[test]
    fn adversarial_intent_ordering_never_panics_and_keeps_one_active_session() {
        let mut store = ServerMenuSessions::new();
        let registry = registry_with_commands();

        // Fuzz-lite: arbitrary intent ordering against an empty store must
        // be a no-op, never a panic, and the store must stay consistent.
        assert!(store.cancel(1 << 63 | 7).is_none(), "cancel before open");
        assert_eq!(store.cancel_active(), None, "cancel-active before open");
        assert!(store.get_mut(1 << 63 | 7).is_none(), "query before open");

        // Cancel then activate on a stale id: both are no-ops.
        let id = open_id(&mut store, &registry);
        assert!(store.cancel(id).is_some());
        assert!(store.cancel(id).is_none(), "double cancel");
        assert!(
            store.get_mut(id).is_none(),
            "query after close stays unknown"
        );

        // Activate with nothing selected is a bounded diagnostic, not a
        // panic, and the session stays for cancel.
        let id = open_id(&mut store, &registry);
        store.get_mut(id).unwrap().set_query("zzz-nope");
        let result = store.get_mut(id).unwrap().activate(
            crate::server::command_execution::CommandExecutionTarget::Global,
            TransientMenuActivationData::Primary,
            1,
        );
        assert!(matches!(
            result,
            Err(CommandExecutionDiagnostic {
                rule: crate::server::command_execution::CommandExecutionRule::UnknownCommand,
                ..
            })
        ));
        assert!(store.cancel_active().is_some());

        // A mixed sequence (open → move → replace → stale ops) never yields
        // more than one active session, and every id stays in the server
        // partition.
        let catalogue = catalogue_for_registry(&registry);
        let mut ids = Vec::new();
        for _ in 0..8 {
            let (snapshot, replaced) = store.open_control_center(&catalogue, 1);
            assert!(snapshot.session_id & SERVER_MENU_SESSION_ID_HIGH_BIT != 0);
            if let Some(previous) = ids.last().copied() {
                assert_eq!(replaced, Some(previous));
            } else {
                assert_eq!(replaced, None);
            }
            ids.push(snapshot.session_id);
            let _ = store
                .get_mut(snapshot.session_id)
                .unwrap()
                .set_query("markdown");
            let _ = store
                .get_mut(snapshot.session_id)
                .unwrap()
                .move_selection(-3);
        }
        let live = ids.last().copied().unwrap();
        for stale in &ids[..ids.len() - 1] {
            assert!(store.get_mut(*stale).is_none(), "stale id is gone");
        }
        assert!(store.get_mut(live).is_some(), "only the newest lives");
    }
}
