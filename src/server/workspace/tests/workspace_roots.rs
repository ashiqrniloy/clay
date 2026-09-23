use super::*;

#[test]
fn bounded_ignore_grammar_supports_root_paths_and_rejects_unsupported_rules() {
    let rules = build_ignore_set("*ab\n?.rs\nbuild/\n").unwrap();
    assert!(rules.is_ignored(std::path::Path::new("aab"), false));
    assert!(rules.is_ignored(std::path::Path::new("é.rs"), false));
    assert!(!rules.is_ignored(std::path::Path::new("ab.rs"), false));
    assert!(rules.is_ignored(std::path::Path::new("build"), true));
    assert!(!rules.is_ignored(std::path::Path::new("build"), false));

    let path_rules = build_ignore_set("/cache\n/packages/markdown/node_modules/\n").unwrap();
    assert!(path_rules.is_ignored(std::path::Path::new("cache"), true));
    assert!(path_rules.is_ignored(std::path::Path::new("packages/markdown/node_modules"), true));
    assert!(!path_rules.is_ignored(std::path::Path::new("src/cache"), true));

    for unsupported in [
        "!secret", "foo\\*", "[xy]", "foo//", "**.log", "/", "foo\0bar",
    ] {
        assert!(
            build_ignore_set(unsupported).is_err(),
            "unsupported rule was accepted: {unsupported}"
        );
    }
}

#[test]
fn ignore_rule_line_pattern_and_character_counts_are_bounded() {
    let too_many_lines = "#\n".repeat(crate::perf::budgets::MAX_GITIGNORE_LINES + 1);
    assert!(build_ignore_set(&too_many_lines).is_err());

    let too_many_patterns = "x\n".repeat(crate::perf::budgets::MAX_GITIGNORE_PATTERNS + 1);
    assert!(build_ignore_set(&too_many_patterns).is_err());

    let too_long = "x".repeat(crate::perf::budgets::MAX_GITIGNORE_PATTERN_CHARS + 1);
    assert!(build_ignore_set(&too_long).is_err());
}

#[test]
fn add_root_deduplicates_by_canonical_path() {
    let root = temp_workspace("dedup-root");
    let mut workspace = WorkspaceState::new();
    let first = workspace.add_root(&root).unwrap();
    let second = workspace.add_root(&root).unwrap();
    assert_eq!(first, second);
    assert_eq!(workspace.list_root_metadata().len(), 1);
    let _ = fs::remove_dir(root);
}

#[test]
fn add_root_from_cwd_adds_current_directory_when_no_roots_exist() {
    let _guard = CWD_TEST_LOCK.lock().unwrap();
    let root = temp_workspace("cwd-root");
    let previous = std::env::current_dir().unwrap();
    std::env::set_current_dir(&root).unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root_from_cwd().unwrap();
    assert!(root_id.is_some());
    assert_eq!(workspace.list_root_metadata().len(), 1);
    std::env::set_current_dir(previous).unwrap();
    let _ = fs::remove_dir(root);
}

#[test]
fn add_root_from_cwd_is_noop_when_roots_already_configured() {
    let _guard = CWD_TEST_LOCK.lock().unwrap();
    let root = temp_workspace("cwd-root-noop");
    let other = temp_workspace("cwd-root-noop-other");
    let previous = std::env::current_dir().unwrap();
    std::env::set_current_dir(&root).unwrap();
    let mut workspace = WorkspaceState::new();
    let configured = workspace.add_root(&other).unwrap();
    let cwd = workspace.add_root_from_cwd().unwrap();
    assert_eq!(cwd, None);
    assert_eq!(workspace.list_root_metadata().len(), 1);
    assert_eq!(
        workspace.list_root_metadata()[0].workspace_root_id,
        configured
    );
    std::env::set_current_dir(previous).unwrap();
    let _ = fs::remove_dir(root);
    let _ = fs::remove_dir(other);
}

#[tokio::test]
async fn discover_root_for_path_finds_marker_ancestor() {
    let root = temp_workspace("discover-marker");
    fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let nested = root.join("src").join("nested");
    fs::create_dir_all(&nested).unwrap();
    let file = nested.join("lib.rs");
    fs::write(&file, "fn main() {}").unwrap();

    let mut workspace = WorkspaceState::new();
    let root_id = workspace.discover_root_for_path(&file).await.unwrap();
    assert!(root_id.is_some());
    assert_eq!(workspace.list_root_metadata().len(), 1);
    assert!(
        workspace.list_root_metadata()[0]
            .display_name
            .contains("discover-marker")
    );

    let _ = fs::remove_file(file);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn discover_root_for_path_returns_existing_root_when_already_covered() {
    let root = temp_workspace("discover-covered");
    let nested = root.join("deep");
    fs::create_dir_all(&nested).unwrap();
    let file = nested.join("note.txt");
    fs::write(&file, "hello").unwrap();

    let mut workspace = WorkspaceState::new();
    let existing = workspace.add_root(&root).unwrap();
    let discovered = workspace.discover_root_for_path(&file).await.unwrap();
    assert_eq!(discovered, Some(existing));
    assert_eq!(workspace.list_root_metadata().len(), 1);

    let _ = fs::remove_file(file);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn discover_root_for_path_without_marker_returns_none() {
    let root = temp_workspace("discover-no-marker");
    let file = root.join("note.txt");
    fs::write(&file, "hello").unwrap();

    let mut workspace = WorkspaceState::new();
    let discovered = workspace.discover_root_for_path(&file).await.unwrap();
    assert_eq!(discovered, None);
    assert!(workspace.list_root_metadata().is_empty());

    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn discover_root_for_path_ignores_unknown_marker() {
    let root = temp_workspace("discover-unknown-marker");
    fs::write(root.join("myproject.marker"), "").unwrap();
    let file = root.join("note.txt");
    fs::write(&file, "hello").unwrap();

    let mut workspace = WorkspaceState::new();
    let discovered = workspace.discover_root_for_path(&file).await.unwrap();
    assert_eq!(discovered, None);

    let _ = fs::remove_file(file);
    let _ = fs::remove_file(root.join("myproject.marker"));
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn explicit_user_grant_adds_directory_root() {
    let root = temp_workspace("grant-dir");
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_explicit_user_grant(&root).await.unwrap();
    assert_eq!(workspace.list_root_metadata().len(), 1);
    assert_eq!(workspace.list_root_metadata()[0].workspace_root_id, root_id);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn explicit_user_grant_adds_file_as_single_file_grant() {
    let root = temp_workspace("grant-file");
    let file = root.join("note.md");
    fs::write(&file, "# note").unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_explicit_user_grant(&file).await.unwrap();
    // Single-file grants are not listed by list_root_metadata.
    assert!(workspace.list_root_metadata().is_empty());
    assert_eq!(root_id, 1);
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn explicit_user_grant_deduplicates_single_file_grant() {
    let root = temp_workspace("grant-file-dedup");
    let file = root.join("note.md");
    fs::write(&file, "# note").unwrap();
    let mut workspace = WorkspaceState::new();
    let first = workspace.add_explicit_user_grant(&file).await.unwrap();
    let second = workspace.add_explicit_user_grant(&file).await.unwrap();
    assert_eq!(first, second);
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn explicit_user_grant_rejects_missing_path() {
    let root = temp_workspace("grant-missing");
    let missing = root.join("missing");
    let mut workspace = WorkspaceState::new();
    let error = workspace
        .add_explicit_user_grant(&missing)
        .await
        .unwrap_err();
    assert!(matches!(error, WorkspaceError::RootUnavailable { .. }));
    let _ = fs::remove_dir(root);
}

/// Plan 134 P3: the async open path surfaces the existing `FileUnavailable`
/// mapping (not a panic or a new error) when the root disappears after the
/// tokio::fs canonicalize/metadata probe would find it gone.
#[tokio::test]
async fn open_existing_file_reports_file_unavailable_when_root_disappears() {
    let root = temp_workspace("open-vanished-root");
    let file = root.join("note.txt");
    fs::write(&file, "hello").unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();
    fs::remove_dir_all(&root).unwrap();

    let error = workspace
        .open_existing_file(root_id, "note.txt", 1)
        .await
        .unwrap_err();
    assert!(matches!(error, WorkspaceError::FileUnavailable { .. }));
}

#[tokio::test]
async fn discover_root_for_path_rejects_directory() {
    let root = temp_workspace("discover-dir");
    let mut workspace = WorkspaceState::new();
    let error = workspace.discover_root_for_path(&root).await.unwrap_err();
    assert!(matches!(error, WorkspaceError::DirectoryOpen));
    let _ = fs::remove_dir(root);
}
