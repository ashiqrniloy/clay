//! Clay-owned dired-style path browser session state.
//!
//! `PathBrowserSession` is the server-side state behind the built-in path
//! mode (Phase 24.3): a canonical current directory, the displayed path
//! input (the editable path bar), a derived final-component filter, bounded
//! installed entries, a persisted selection, and an optional error status.
//! It performs no filesystem reads itself — the server turns a
//! `PathBrowserTransition::Relist` into a bounded user-browse listing and
//! installs the resulting page.
//!
//! The input model is dired-style: the input is always a path string whose
//! final component (after the last platform separator) is the filter
//! fragment; everything before it is the directory part. An input with no
//! separator keeps the session in its current directory and filters the
//! installed entries. Filtering uses the shared Phase 24.2 fuzzy scorer.
//!
//! Activation resolves only from server-held installed entries, never from
//! client-supplied paths: `activate` maps the selected entry to a
//! `PathBrowserActivation` carrying the entry's canonical path.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use crate::perf::budgets::{TRANSIENT_MENU_MAX_ITEMS, TRANSIENT_MENU_MAX_QUERY_CHARS};
use crate::server::workspace::UserBrowsePage;

use super::{
    file_browser::FileBrowserEntryKind,
    fuzzy::fuzzy_score,
    transient_menu::{
        TransientMenuAction, TransientMenuItem, TransientMenuOrigin, TransientMenuSession,
        TransientMenuSessionId,
    },
};

const MAX_ITEMS: usize = TRANSIENT_MENU_MAX_ITEMS;
const MAX_INPUT_CHARS: usize = TRANSIENT_MENU_MAX_QUERY_CHARS;

/// Outcome of an input edit: whether the server must list a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PathBrowserTransition {
    /// Only the derived filter/selection changed; no filesystem work.
    FilterOnly,
    /// The path targets a different directory; list `target` and install.
    Relist { target: PathBuf },
}

/// Activation resolved from a server-held installed entry. The `PathBuf`
/// values are canonical activation paths from the installed listing, never
/// client-supplied paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PathBrowserActivation {
    Descend(PathBuf),
    OpenFile(PathBuf),
    OpenWorkspace(PathBuf),
}

/// One installed depth-1 entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PathBrowserEntry {
    pub(crate) name: String,
    pub(crate) kind: FileBrowserEntryKind,
    pub(crate) canonical_path: PathBuf,
    pub(crate) size: Option<u64>,
}

/// Server-side state for the built-in path session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PathBrowserSession {
    /// Canonical current directory (adopted from the last installed page).
    canonical_dir: PathBuf,
    /// Displayed path input (the editable path bar / query line).
    input: String,
    /// Directory part (with trailing separator) of the input that produced
    /// the current listing. A textual match means no relist is needed.
    input_dir: String,
    /// Bounded installed entries, name-sorted from the listing service.
    entries: Vec<PathBrowserEntry>,
    /// Indices into `entries` in display order (directory-first for an
    /// empty filter, fuzzy-ranked otherwise).
    filtered: Vec<usize>,
    /// Persisted selection into `filtered`, clamped on filter changes.
    selected_index: usize,
    /// Sticky listing failure; suppresses items and activation until the
    /// next successful install.
    error: Option<String>,
    /// Whether the listing was truncated at the entry cap.
    truncated: bool,
}

impl PathBrowserSession {
    /// Seed with a canonical starting directory. The displayed input is the
    /// seed as a path string with a trailing separator; the server relists
    /// the seed immediately and installs the page.
    pub(crate) fn new(seed: PathBuf) -> Self {
        let input = dir_display(&seed);
        Self {
            canonical_dir: seed,
            input_dir: input.clone(),
            input,
            entries: Vec::new(),
            filtered: Vec::new(),
            selected_index: 0,
            error: None,
            truncated: false,
        }
    }

    pub(crate) fn canonical_dir(&self) -> &Path {
        &self.canonical_dir
    }

    pub(crate) fn input(&self) -> &str {
        &self.input
    }

    /// The derived filter: the final component of the displayed input.
    pub(crate) fn filter_fragment(&self) -> &str {
        parse_input(&self.input).1
    }

    pub(crate) fn truncated(&self) -> bool {
        self.truncated
    }

    /// Full-value replacement of the displayed path input (the client sends
    /// the whole edited value). Relists only when the directory prefix
    /// changes; filter-only edits score the installed entries locally.
    pub(crate) fn set_input(&mut self, input: &str) -> PathBrowserTransition {
        self.input = truncate_chars(input, MAX_INPUT_CHARS);
        let (dir_part, _) = parse_input(&self.input);
        if dir_part.is_empty() || dir_part == self.input_dir {
            self.refresh_filter();
            PathBrowserTransition::FilterOnly
        } else {
            self.input_dir = dir_part.to_string();
            PathBrowserTransition::Relist {
                target: relist_target(&self.canonical_dir, dir_part),
            }
        }
    }

    /// Semantic Backspace: with a non-empty derived filter, delete its last
    /// character; with an empty filter, ascend to the parent directory.
    /// At the filesystem root there is nothing to ascend to.
    pub(crate) fn backspace(&mut self) -> PathBrowserTransition {
        if !self.filter_fragment().is_empty() {
            self.input.pop();
            self.refresh_filter();
            PathBrowserTransition::FilterOnly
        } else if let Some(parent) = self.canonical_dir.parent() {
            let parent = parent.to_path_buf();
            let input = dir_display(&parent);
            self.input_dir = input.clone();
            self.input = input;
            PathBrowserTransition::Relist { target: parent }
        } else {
            PathBrowserTransition::FilterOnly
        }
    }

    /// Install a freshly listed page: adopt the canonical directory, rewrite
    /// the displayed input to the canonical location, reset the selection,
    /// and clear any error.
    pub(crate) fn install(&mut self, page: UserBrowsePage) {
        let input = dir_display(&page.canonical_dir);
        self.canonical_dir = page.canonical_dir;
        self.entries = page
            .entries
            .into_iter()
            .take(MAX_ITEMS)
            .map(|entry| PathBrowserEntry {
                name: entry.name,
                kind: entry.kind.into(),
                canonical_path: entry.canonical_path,
                size: entry.size,
            })
            .collect();
        self.truncated = page.truncated;
        self.input_dir = input.clone();
        self.input = input;
        self.error = None;
        self.selected_index = 0;
        self.refresh_filter();
    }

    /// Record a failed relist; the error suppresses items and activation
    /// until the next successful install.
    pub(crate) fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(truncate_chars(&message.into(), MAX_INPUT_CHARS));
        self.filtered.clear();
        self.selected_index = 0;
    }

    /// Move the persisted selection by `delta`, wrapping within the filtered
    /// view (matching the Control Center's `move_selection` semantics).
    pub(crate) fn move_selection(&mut self, delta: i64) {
        if self.filtered.is_empty() {
            self.selected_index = 0;
            return;
        }
        self.selected_index =
            (self.selected_index as i64 + delta).rem_euclid(self.filtered.len() as i64) as usize;
    }

    /// Resolve the selected installed entry to a primary activation. Never
    /// resolves a client-supplied path: the canonical path comes from the
    /// installed entry. Fails closed while a listing error is pending.
    pub(crate) fn activate(&self) -> Option<PathBrowserActivation> {
        if self.error.is_some() {
            return None;
        }
        let entry = self.selected_entry()?;
        match entry.kind {
            FileBrowserEntryKind::Directory => {
                Some(PathBrowserActivation::Descend(entry.canonical_path.clone()))
            }
            _ => Some(PathBrowserActivation::OpenFile(
                entry.canonical_path.clone(),
            )),
        }
    }

    /// Resolve the selected entry to a secondary activation: opening a
    /// directory as a workspace for the current tab. Files have no secondary
    /// activation.
    pub(crate) fn activate_workspace(&self) -> Option<PathBrowserActivation> {
        if self.error.is_some() {
            return None;
        }
        let entry = self.selected_entry()?;
        (entry.kind == FileBrowserEntryKind::Directory)
            .then(|| PathBrowserActivation::OpenWorkspace(entry.canonical_path.clone()))
    }

    /// After activation resolved to `Descend`, target the new directory.
    pub(crate) fn descend(&mut self, canonical_target: PathBuf) -> PathBrowserTransition {
        let input = dir_display(&canonical_target);
        self.input_dir = input.clone();
        self.input = input;
        PathBrowserTransition::Relist {
            target: canonical_target,
        }
    }

    fn selected_entry(&self) -> Option<&PathBrowserEntry> {
        self.filtered
            .get(self.selected_index)
            .and_then(|&index| self.entries.get(index))
    }

    /// Recompute the display order from the derived filter: directory-first
    /// stable (name) order for an empty filter, shared fuzzy ranking
    /// otherwise. Scores only the bounded installed entries.
    fn refresh_filter(&mut self) {
        let filter = self.filter_fragment();
        if filter.is_empty() {
            // Entries arrive name-sorted from the listing, so a stable
            // partition by kind keeps deterministic name order within each
            // group.
            let mut filtered = Vec::with_capacity(self.entries.len());
            filtered.extend(
                self.entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| entry.kind == FileBrowserEntryKind::Directory)
                    .map(|(index, _)| index),
            );
            filtered.extend(
                self.entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| entry.kind != FileBrowserEntryKind::Directory)
                    .map(|(index, _)| index),
            );
            self.filtered = filtered;
        } else {
            let mut scored: Vec<(i32, usize)> = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    fuzzy_score(filter, &entry.name).map(|score| (score, index))
                })
                .collect();
            scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
            self.filtered = scored
                .into_iter()
                .take(MAX_ITEMS)
                .map(|(_, index)| index)
                .collect();
        }
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        self.selected_index = if self.filtered.is_empty() {
            0
        } else {
            self.selected_index.min(self.filtered.len() - 1)
        };
    }

    /// Project the session onto the shared transient-menu display session.
    /// Items carry no activation action: path activation resolves by opaque
    /// session id on the server, never from item actions.
    pub(crate) fn menu_session(&self, session_id: TransientMenuSessionId) -> TransientMenuSession {
        let prompt = format!("Browse · {}", self.canonical_dir.display());
        let mut session = TransientMenuSession::new(session_id, prompt)
            .with_query(&self.input)
            .with_origin(TransientMenuOrigin::Centered);
        if let Some(message) = &self.error {
            // Items stay suppressed while a listing error is pending.
            return session.with_empty_status(message);
        }
        let items: Vec<TransientMenuItem> = self
            .filtered
            .iter()
            .filter_map(|&index| self.entries.get(index))
            .enumerate()
            .map(|(index, entry)| {
                TransientMenuItem::new(
                    index.to_string(),
                    entry.name.clone(),
                    TransientMenuAction::new(""),
                )
                .with_detail(kind_label(entry.kind))
            })
            .collect();
        session = session.with_items(items);
        if session.items().is_empty() {
            let message = if self.filter_fragment().is_empty() {
                "Empty directory".to_string()
            } else {
                format!("No matches for '{}'", self.filter_fragment())
            };
            session = session.with_empty_status(message);
        } else {
            session = session.with_selected_index(self.selected_index);
        }
        session
    }
}

fn kind_label(kind: FileBrowserEntryKind) -> &'static str {
    match kind {
        FileBrowserEntryKind::Directory => "folder",
        FileBrowserEntryKind::File => "file",
        FileBrowserEntryKind::Symlink => "link",
        FileBrowserEntryKind::Other => "other",
    }
}

/// Split a path input into (directory part, filter fragment). The directory
/// part keeps its trailing separator when present; an input without a
/// separator has an empty directory part and the whole input is the filter.
fn parse_input(input: &str) -> (&str, &str) {
    match input.rfind(std::path::is_separator) {
        None => ("", input),
        Some(pos) => (&input[..=pos], &input[pos + 1..]),
    }
}

/// Resolve a non-empty directory part to a listing target: absolute parts
/// are used as-is, relative parts resolve from the current canonical
/// directory.
fn relist_target(canonical_dir: &Path, dir_part: &str) -> PathBuf {
    let part = Path::new(dir_part);
    if part.is_absolute() {
        part.to_path_buf()
    } else {
        canonical_dir.join(part)
    }
}

/// Render a canonical directory as a displayed path string with a trailing
/// separator (the root "/" keeps its single separator).
fn dir_display(dir: &Path) -> String {
    let display = dir.to_string_lossy().to_string();
    if display.ends_with(std::path::MAIN_SEPARATOR) {
        display
    } else {
        format!("{display}{}", std::path::MAIN_SEPARATOR)
    }
}

fn truncate_chars(input: &str, max: usize) -> String {
    input.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workspace::{UserBrowseEntry, UserBrowseEntryKind, UserBrowsePage};

    fn page(canonical_dir: &str, entries: Vec<(&str, UserBrowseEntryKind)>) -> UserBrowsePage {
        UserBrowsePage {
            canonical_dir: PathBuf::from(canonical_dir),
            entries: entries
                .into_iter()
                .map(|(name, kind)| UserBrowseEntry {
                    name: name.to_string(),
                    kind,
                    canonical_path: PathBuf::from(canonical_dir).join(name),
                    size: None,
                })
                .collect(),
            truncated: false,
        }
    }

    #[test]
    fn parse_input_splits_directory_part_and_filter() {
        let cases: &[(&str, &str, &str)] = &[
            ("", "", ""),
            ("pro", "", "pro"),
            ("src/", "src/", ""),
            ("src/ma", "src/", "ma"),
            ("/", "/", ""),
            ("/home/arn/", "/home/arn/", ""),
            ("/home/arn/pro", "/home/arn/", "pro"),
            ("/a/b", "/a/", "b"),
            ("..", "", ".."),
            ("../", "../", ""),
            ("../../x", "../../", "x"),
            ("./", "./", ""),
        ];
        for (input, dir_part, filter) in cases {
            assert_eq!(parse_input(input), (*dir_part, *filter), "input {input:?}");
        }
    }

    #[test]
    fn path_browser_new_seeds_display_with_canonical_directory() {
        let session = PathBrowserSession::new(PathBuf::from("/home/arn"));
        assert_eq!(session.canonical_dir(), Path::new("/home/arn"));
        assert_eq!(session.input(), "/home/arn/");
        assert_eq!(session.filter_fragment(), "");
        assert!(!session.truncated());

        let root = PathBrowserSession::new(PathBuf::from("/"));
        assert_eq!(root.input(), "/");
    }

    #[test]
    fn path_browser_set_input_filters_without_relisting() {
        let mut session = PathBrowserSession::new(PathBuf::from("/home/arn"));
        session.install(page(
            "/home/arn",
            vec![("proj", UserBrowseEntryKind::Directory)],
        ));

        // Same directory prefix, filter fragment appended.
        let transition = session.set_input("/home/arn/pro");
        assert_eq!(transition, PathBrowserTransition::FilterOnly);
        assert_eq!(session.filter_fragment(), "pro");

        // Back to the plain directory display.
        let transition = session.set_input("/home/arn/");
        assert_eq!(transition, PathBrowserTransition::FilterOnly);
        assert_eq!(session.filter_fragment(), "");

        // A bare fragment keeps the current directory.
        let transition = session.set_input("pro");
        assert_eq!(transition, PathBrowserTransition::FilterOnly);
        assert_eq!(session.filter_fragment(), "pro");

        // Empty input filters nothing.
        let transition = session.set_input("");
        assert_eq!(transition, PathBrowserTransition::FilterOnly);
        assert_eq!(session.filter_fragment(), "");
    }

    #[test]
    fn path_browser_set_input_relists_on_directory_prefix_change() {
        let mut session = PathBrowserSession::new(PathBuf::from("/home/arn"));
        session.install(page("/home/arn", vec![]));

        // Absolute directory jump.
        let transition = session.set_input("/home/arn/src/");
        assert_eq!(
            transition,
            PathBrowserTransition::Relist {
                target: PathBuf::from("/home/arn/src/")
            }
        );

        // Relative directory jump resolves from the current directory.
        session.install(page("/home/arn/src", vec![]));
        let transition = session.set_input("lib/");
        assert_eq!(
            transition,
            PathBrowserTransition::Relist {
                target: PathBuf::from("/home/arn/src/lib/")
            }
        );

        // Root.
        let transition = session.set_input("/");
        assert_eq!(
            transition,
            PathBrowserTransition::Relist {
                target: PathBuf::from("/")
            }
        );

        // Parent-relative: joined textually; the listing canonicalizes.
        let transition = session.set_input("../");
        assert_eq!(
            transition,
            PathBrowserTransition::Relist {
                target: PathBuf::from("/home/arn/src/../")
            }
        );
    }

    #[test]
    fn path_browser_backspace_deletes_filter_first_then_ascends() {
        let mut session = PathBrowserSession::new(PathBuf::from("/home/arn"));
        session.install(page("/home/arn", vec![]));

        let transition = session.set_input("/home/arn/fi");
        assert_eq!(transition, PathBrowserTransition::FilterOnly);

        // Filter backspace deletes one character.
        assert_eq!(session.backspace(), PathBrowserTransition::FilterOnly);
        assert_eq!(session.input(), "/home/arn/f");
        assert_eq!(session.backspace(), PathBrowserTransition::FilterOnly);
        assert_eq!(session.input(), "/home/arn/");

        // Empty filter backspace ascends to the parent.
        let transition = session.backspace();
        assert_eq!(
            transition,
            PathBrowserTransition::Relist {
                target: PathBuf::from("/home")
            }
        );
        assert_eq!(session.input(), "/home/");

        // At the root there is nothing to ascend to.
        let mut root = PathBrowserSession::new(PathBuf::from("/"));
        assert_eq!(root.backspace(), PathBrowserTransition::FilterOnly);
    }

    #[test]
    fn path_browser_install_adopts_canonical_directory_and_resets_selection() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        session.install(page(
            "/a",
            vec![
                ("a.txt", UserBrowseEntryKind::File),
                ("bdir", UserBrowseEntryKind::Directory),
                ("c.txt", UserBrowseEntryKind::File),
            ],
        ));
        assert_eq!(session.canonical_dir(), Path::new("/a"));
        assert_eq!(session.input(), "/a/");
        assert_eq!(session.install_entries_len(), 3);

        session.move_selection(1);
        session.install(page("/b", vec![("x", UserBrowseEntryKind::File)]));
        assert_eq!(session.canonical_dir(), Path::new("/b"));
        assert_eq!(session.input(), "/b/");
        assert_eq!(
            session
                .menu_session(TransientMenuSessionId(7))
                .selected_index(),
            0
        );
    }

    #[test]
    fn path_browser_empty_filter_orders_directories_first_stably() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        session.install(page(
            "/a",
            vec![
                ("a.txt", UserBrowseEntryKind::File),
                ("bdir", UserBrowseEntryKind::Directory),
                ("c.txt", UserBrowseEntryKind::File),
                ("ddir", UserBrowseEntryKind::Directory),
            ],
        ));
        let menu = session.menu_session(TransientMenuSessionId(1));
        let labels: Vec<&str> = menu
            .items()
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(labels, vec!["bdir", "ddir", "a.txt", "c.txt"]);
    }

    #[test]
    fn path_browser_fuzzy_filter_ranks_and_reports_no_match() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        session.install(page(
            "/a",
            vec![
                ("main.rs", UserBrowseEntryKind::File),
                ("README.md", UserBrowseEntryKind::File),
                ("Cargo.toml", UserBrowseEntryKind::File),
            ],
        ));

        // Only README.md contains "md" in order.
        session.set_input("/a/md");
        let menu = session.menu_session(TransientMenuSessionId(1));
        let labels: Vec<&str> = menu
            .items()
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(labels, vec!["README.md"]);

        // No match surfaces the empty state with the filter in the message.
        session.set_input("/a/zzz");
        let menu = session.menu_session(TransientMenuSessionId(2));
        assert!(menu.items().is_empty());
        assert_eq!(
            menu.status(),
            &crate::shell::transient_menu::TransientMenuStatus::Empty {
                message: "No matches for 'zzz'".to_string()
            }
        );
    }

    #[test]
    fn path_browser_selection_clamps_and_wraps() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        session.install(page(
            "/a",
            vec![
                ("a.txt", UserBrowseEntryKind::File),
                ("b.txt", UserBrowseEntryKind::File),
                ("c.txt", UserBrowseEntryKind::File),
            ],
        ));

        session.move_selection(5);
        assert_eq!(
            session
                .menu_session(TransientMenuSessionId(1))
                .selected_index(),
            2
        );
        session.move_selection(1);
        assert_eq!(
            session
                .menu_session(TransientMenuSessionId(2))
                .selected_index(),
            0
        );
        session.move_selection(-1);
        assert_eq!(
            session
                .menu_session(TransientMenuSessionId(3))
                .selected_index(),
            2
        );

        // Filter change to a single match clamps the persisted selection.
        session.move_selection(2);
        session.set_input("/a/c");
        let menu = session.menu_session(TransientMenuSessionId(4));
        assert_eq!(menu.items().len(), 1);
        assert_eq!(menu.items()[0].label, "c.txt");
        assert_eq!(menu.selected_index(), 0);
    }

    #[test]
    fn path_browser_oversize_input_and_entries_clamp_without_panic() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        let oversize: String = "x".repeat(TRANSIENT_MENU_MAX_QUERY_CHARS + 100);
        session.set_input(&oversize);
        assert!(session.input().chars().count() <= TRANSIENT_MENU_MAX_QUERY_CHARS);

        let mut many = Vec::new();
        for index in 0..(TRANSIENT_MENU_MAX_ITEMS + 50) {
            many.push((format!("f{index:03}.txt"), UserBrowseEntryKind::File));
        }
        let big_page = UserBrowsePage {
            canonical_dir: PathBuf::from("/a"),
            entries: many
                .into_iter()
                .map(|(name, kind)| {
                    let canonical = PathBuf::from("/a").join(&name);
                    UserBrowseEntry {
                        name,
                        kind,
                        canonical_path: canonical,
                        size: None,
                    }
                })
                .collect(),
            truncated: true,
        };
        session.install(big_page);
        assert!(session.install_entries_len() <= TRANSIENT_MENU_MAX_ITEMS);
        assert!(session.truncated());
        // No panic on activation/selection after clamping.
        let _ = session.activate();
        session.move_selection(10_000);
    }

    #[test]
    fn path_browser_error_status_suppresses_items_and_activation() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        session.install(page("/a", vec![("x.txt", UserBrowseEntryKind::File)]));

        session.set_error("cannot browse /bad/: No such file or directory");
        assert!(session.activate().is_none());
        assert!(session.activate_workspace().is_none());
        let menu = session.menu_session(TransientMenuSessionId(1));
        assert!(menu.items().is_empty());
        assert!(matches!(
            menu.status(),
            crate::shell::transient_menu::TransientMenuStatus::Empty { message }
                if message.contains("cannot browse")
        ));

        session.install(page("/a", vec![("y.txt", UserBrowseEntryKind::File)]));
        assert_eq!(
            session.activate(),
            Some(PathBrowserActivation::OpenFile(PathBuf::from("/a/y.txt")))
        );
    }

    #[test]
    fn path_browser_activation_resolves_from_installed_entries() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        session.install(page(
            "/a",
            vec![
                ("sub", UserBrowseEntryKind::Directory),
                ("file.txt", UserBrowseEntryKind::File),
            ],
        ));

        // Directory-first order: sub is selected first.
        assert_eq!(
            session.activate(),
            Some(PathBrowserActivation::Descend(PathBuf::from("/a/sub")))
        );
        assert_eq!(
            session.activate_workspace(),
            Some(PathBrowserActivation::OpenWorkspace(PathBuf::from(
                "/a/sub"
            )))
        );

        // File: primary opens, secondary has no activation.
        session.move_selection(1);
        assert_eq!(
            session.activate(),
            Some(PathBrowserActivation::OpenFile(PathBuf::from(
                "/a/file.txt"
            )))
        );
        assert_eq!(session.activate_workspace(), None);
    }

    #[test]
    fn path_browser_descend_targets_entry_directory() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        session.install(page("/a", vec![("sub", UserBrowseEntryKind::Directory)]));
        let transition = session.descend(PathBuf::from("/a/sub"));
        assert_eq!(
            transition,
            PathBrowserTransition::Relist {
                target: PathBuf::from("/a/sub")
            }
        );
        assert_eq!(session.input(), "/a/sub/");
    }

    #[test]
    fn path_browser_entry_kind_conversion_has_no_symlink_variant() {
        assert_eq!(
            FileBrowserEntryKind::from(UserBrowseEntryKind::Directory),
            FileBrowserEntryKind::Directory
        );
        assert_eq!(
            FileBrowserEntryKind::from(UserBrowseEntryKind::File),
            FileBrowserEntryKind::File
        );
        assert_eq!(
            FileBrowserEntryKind::from(UserBrowseEntryKind::Other),
            FileBrowserEntryKind::Other
        );
    }

    #[test]
    fn path_browser_menu_projection_carries_path_and_kind_details() {
        let mut session = PathBrowserSession::new(PathBuf::from("/a"));
        session.install(page(
            "/a",
            vec![
                ("sub", UserBrowseEntryKind::Directory),
                ("file.txt", UserBrowseEntryKind::File),
            ],
        ));
        let menu = session.menu_session(TransientMenuSessionId(9));
        assert!(menu.prompt().contains("/a"));
        assert_eq!(menu.query(), "/a/");
        assert_eq!(menu.items()[0].label, "sub");
        assert_eq!(menu.items()[0].detail.as_deref(), Some("folder"));
        assert_eq!(menu.items()[1].label, "file.txt");
        assert_eq!(menu.items()[1].detail.as_deref(), Some("file"));
        assert_eq!(menu.selected_index(), 0);
    }

    // Test-only accessor: installed entry count.
    impl PathBrowserSession {
        fn install_entries_len(&self) -> usize {
            self.entries.len()
        }
    }
}
