use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, SystemTime},
};

use tokio::{
    io::AsyncReadExt,
    process::Command,
    sync::{Mutex, Notify},
    task::JoinSet,
    time::timeout,
};

use super::workspace::{WorkspaceRootId, WorkspaceState};

pub(crate) const GIT_DISCOVERY_TIMEOUT: Duration = Duration::from_millis(750);
pub(crate) const GIT_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(5);
const GIT_OUTPUT_MAX_BYTES: usize = 256 * 1024;
const GIT_DIAGNOSTIC_MAX_CHARS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitStatusSnapshot {
    pub(crate) workspace_root_id: WorkspaceRootId,
    pub(crate) workspace_root: PathBuf,
    pub(crate) repository_root: Option<PathBuf>,
    pub(crate) head: GitHeadState,
    pub(crate) dirty: bool,
    pub(crate) changed_file_count: usize,
    pub(crate) last_refresh: GitRefreshStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitHeadState {
    Branch(String),
    Detached(String),
    Unborn,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitRefreshStatus {
    Success,
    NonRepository,
    Timeout,
    CommandError {
        command: &'static str,
        message: String,
    },
    InvalidOutput {
        command: &'static str,
        message: String,
    },
    BoundaryRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitDiscoveryCommand {
    RepositoryRoot,
    Branch,
    DetachedHead,
    StatusShort,
}

impl GitDiscoveryCommand {
    fn name(self) -> &'static str {
        match self {
            Self::RepositoryRoot => "rev-parse --show-toplevel",
            Self::Branch => "symbolic-ref --quiet --short HEAD",
            Self::DetachedHead => "rev-parse --short HEAD",
            Self::StatusShort => "status --porcelain=v1",
        }
    }

    fn args(self) -> &'static [&'static str] {
        match self {
            Self::RepositoryRoot => &["rev-parse", "--show-toplevel"],
            Self::Branch => &["symbolic-ref", "--quiet", "--short", "HEAD"],
            Self::DetachedHead => &["rev-parse", "--short", "HEAD"],
            Self::StatusShort => &["status", "--porcelain=v1", "--untracked-files=normal"],
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GitDiscoveryService {
    git_binary: PathBuf,
    timeout: Duration,
    max_output_bytes: usize,
}

impl Default for GitDiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

impl GitDiscoveryService {
    pub(crate) fn new() -> Self {
        Self {
            git_binary: PathBuf::from("git"),
            timeout: GIT_DISCOVERY_TIMEOUT,
            max_output_bytes: GIT_OUTPUT_MAX_BYTES,
        }
    }

    #[cfg(test)]
    fn with_git_binary(git_binary: impl Into<PathBuf>) -> Self {
        Self {
            git_binary: git_binary.into(),
            timeout: GIT_DISCOVERY_TIMEOUT,
            max_output_bytes: GIT_OUTPUT_MAX_BYTES,
        }
    }

    #[cfg(test)]
    fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub(crate) async fn discover_workspace_statuses(
        &self,
        workspace: &WorkspaceState,
    ) -> Vec<GitStatusSnapshot> {
        let mut snapshots = Vec::new();
        for root in workspace.directory_roots() {
            snapshots.push(
                self.discover_root_status(root.workspace_root_id, &root.canonical_path)
                    .await,
            );
        }
        snapshots
    }

    pub(crate) async fn discover_root_status(
        &self,
        workspace_root_id: WorkspaceRootId,
        workspace_root: impl AsRef<Path>,
    ) -> GitStatusSnapshot {
        let Ok(canonical_root) = canonical_directory(workspace_root.as_ref()) else {
            return GitStatusSnapshot::error(
                workspace_root_id,
                workspace_root.as_ref().to_path_buf(),
                GitRefreshStatus::BoundaryRejected,
            );
        };

        let repo_root_output = match self
            .run(GitDiscoveryCommand::RepositoryRoot, &canonical_root)
            .await
        {
            Ok(output) => output,
            Err(GitRunError::Timeout) => {
                return GitStatusSnapshot::error(
                    workspace_root_id,
                    canonical_root,
                    GitRefreshStatus::Timeout,
                );
            }
            Err(GitRunError::CommandFailed { stderr, .. })
                if looks_like_non_repository(&stderr) =>
            {
                return GitStatusSnapshot::error(
                    workspace_root_id,
                    canonical_root,
                    GitRefreshStatus::NonRepository,
                );
            }
            Err(error) => {
                return GitStatusSnapshot::error(
                    workspace_root_id,
                    canonical_root,
                    error.into_refresh_status(GitDiscoveryCommand::RepositoryRoot),
                );
            }
        };

        let repository_root = match parse_single_line_path(&repo_root_output.stdout) {
            Ok(path) => path,
            Err(message) => {
                return GitStatusSnapshot::error(
                    workspace_root_id,
                    canonical_root,
                    GitRefreshStatus::InvalidOutput {
                        command: GitDiscoveryCommand::RepositoryRoot.name(),
                        message,
                    },
                );
            }
        };

        if !repository_root.starts_with(&canonical_root) {
            return GitStatusSnapshot::error(
                workspace_root_id,
                canonical_root,
                GitRefreshStatus::BoundaryRejected,
            );
        }

        let head = match self.read_head(&canonical_root).await {
            Ok(head) => head,
            Err(GitRunError::Timeout) => {
                return GitStatusSnapshot::with_repo_error(
                    workspace_root_id,
                    canonical_root,
                    repository_root,
                    GitRefreshStatus::Timeout,
                );
            }
            Err(error) => {
                return GitStatusSnapshot::with_repo_error(
                    workspace_root_id,
                    canonical_root,
                    repository_root,
                    error.into_refresh_status(GitDiscoveryCommand::DetachedHead),
                );
            }
        };

        let status_output = match self
            .run(GitDiscoveryCommand::StatusShort, &canonical_root)
            .await
        {
            Ok(output) => output,
            Err(GitRunError::Timeout) => {
                return GitStatusSnapshot::with_repo_error(
                    workspace_root_id,
                    canonical_root,
                    repository_root,
                    GitRefreshStatus::Timeout,
                );
            }
            Err(error) => {
                return GitStatusSnapshot::with_repo_error(
                    workspace_root_id,
                    canonical_root,
                    repository_root,
                    error.into_refresh_status(GitDiscoveryCommand::StatusShort),
                );
            }
        };

        let changed_file_count = count_status_paths(&status_output.stdout);
        GitStatusSnapshot {
            workspace_root_id,
            workspace_root: canonical_root,
            repository_root: Some(repository_root),
            head,
            dirty: changed_file_count > 0,
            changed_file_count,
            last_refresh: GitRefreshStatus::Success,
        }
    }

    async fn read_head(&self, cwd: &Path) -> Result<GitHeadState, GitRunError> {
        match self.run(GitDiscoveryCommand::Branch, cwd).await {
            Ok(output) => parse_single_line(&output.stdout)
                .map(GitHeadState::Branch)
                .map_err(GitRunError::InvalidOutput),
            Err(GitRunError::CommandFailed { .. }) => {
                match self.run(GitDiscoveryCommand::DetachedHead, cwd).await {
                    Ok(output) => parse_single_line(&output.stdout)
                        .map(GitHeadState::Detached)
                        .map_err(GitRunError::InvalidOutput),
                    Err(GitRunError::CommandFailed { .. }) => Ok(GitHeadState::Unborn),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    async fn run(
        &self,
        command: GitDiscoveryCommand,
        cwd: &Path,
    ) -> Result<CappedOutput, GitRunError> {
        let mut child = Command::new(&self.git_binary)
            .args(["--no-optional-locks"])
            .args(command.args())
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(GitRunError::Spawn)?;

        let stdout = child.stdout.take().ok_or(GitRunError::MissingPipe)?;
        let stderr = child.stderr.take().ok_or(GitRunError::MissingPipe)?;
        let max_stdout = self.max_output_bytes;
        let max_stderr = self.max_output_bytes.min(16 * 1024);
        let stdout_task = tokio::spawn(read_capped(stdout, max_stdout));
        let stderr_task = tokio::spawn(read_capped(stderr, max_stderr));
        let wait = child.wait();

        let status = match timeout(self.timeout, wait).await {
            Ok(result) => result.map_err(GitRunError::Wait)?,
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(GitRunError::Timeout);
            }
        };

        let stdout = stdout_task
            .await
            .map_err(|_| GitRunError::MissingPipe)?
            .map_err(GitRunError::Wait)?;
        let stderr = stderr_task
            .await
            .map_err(|_| GitRunError::MissingPipe)?
            .map_err(GitRunError::Wait)?;

        if stdout.truncated || stderr.truncated {
            return Err(GitRunError::InvalidOutput(
                "git output exceeded byte cap".to_string(),
            ));
        }

        if !status.success() {
            return Err(GitRunError::CommandFailed {
                code: status.code(),
                stderr: sanitize_diagnostic(&stderr.bytes),
            });
        }

        Ok(CappedOutput {
            stdout: stdout.bytes,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GitStatusCache {
    discovery: GitDiscoveryService,
    inner: Arc<Mutex<HashMap<WorkspaceRootId, GitCacheEntry>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCachedStatus {
    pub(crate) workspace_root_id: WorkspaceRootId,
    pub(crate) workspace_root: PathBuf,
    pub(crate) snapshot: Option<GitStatusSnapshot>,
    pub(crate) refresh_state: GitRefreshState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitRefreshState {
    Idle,
    Refreshing {
        started_at: SystemTime,
    },
    LastSuccess {
        finished_at: SystemTime,
    },
    LastError {
        finished_at: SystemTime,
        status: GitRefreshStatus,
    },
}

#[derive(Debug, Clone)]
struct GitCacheEntry {
    workspace_root: PathBuf,
    snapshot: Option<GitStatusSnapshot>,
    refresh_state: GitRefreshState,
    notify: Arc<Notify>,
}

impl GitStatusCache {
    pub(crate) fn new(discovery: GitDiscoveryService) -> Self {
        Self {
            discovery,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) async fn list_cached(&self, workspace: &WorkspaceState) -> Vec<GitCachedStatus> {
        let roots = workspace.directory_roots();
        let inner = self.inner.lock().await;
        roots
            .into_iter()
            .map(|root| {
                inner
                    .get(&root.workspace_root_id)
                    .map(|entry| entry.status(root.workspace_root_id))
                    .unwrap_or(GitCachedStatus {
                        workspace_root_id: root.workspace_root_id,
                        workspace_root: root.canonical_path,
                        snapshot: None,
                        refresh_state: GitRefreshState::Idle,
                    })
            })
            .collect()
    }

    pub(crate) async fn refresh_workspace(
        &self,
        workspace: &WorkspaceState,
    ) -> Vec<GitCachedStatus> {
        let mut tasks = JoinSet::new();
        for root in workspace.directory_roots() {
            let cache = self.clone();
            tasks.spawn(async move {
                cache
                    .refresh_root(root.workspace_root_id, root.canonical_path)
                    .await
            });
        }

        let mut statuses = Vec::new();
        while let Some(result) = tasks.join_next().await {
            if let Ok(status) = result {
                statuses.push(status);
            }
        }
        statuses.sort_by_key(|status| status.workspace_root_id);
        statuses
    }

    pub(crate) async fn refresh_stale_workspace(
        &self,
        workspace: &WorkspaceState,
    ) -> Vec<GitCachedStatus> {
        let now = SystemTime::now();
        let roots = workspace.directory_roots();
        let stale_roots = {
            let inner = self.inner.lock().await;
            roots
                .iter()
                .filter(|root| {
                    inner
                        .get(&root.workspace_root_id)
                        .is_none_or(|entry| entry.should_poll(now))
                })
                .cloned()
                .collect::<Vec<_>>()
        };

        let mut tasks = JoinSet::new();
        for root in stale_roots {
            let cache = self.clone();
            tasks.spawn(async move {
                cache
                    .refresh_root(root.workspace_root_id, root.canonical_path)
                    .await
            });
        }
        while tasks.join_next().await.is_some() {}
        self.list_cached(workspace).await
    }

    pub(crate) async fn refresh_root(
        &self,
        workspace_root_id: WorkspaceRootId,
        workspace_root: impl AsRef<Path>,
    ) -> GitCachedStatus {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        loop {
            let wait_for = {
                let mut inner = self.inner.lock().await;
                let entry = inner
                    .entry(workspace_root_id)
                    .or_insert_with(|| GitCacheEntry::new(workspace_root.clone()));
                entry.workspace_root = workspace_root.clone();
                match entry.refresh_state {
                    GitRefreshState::Refreshing { .. } => Some(entry.notify.clone()),
                    _ => {
                        entry.refresh_state = GitRefreshState::Refreshing {
                            started_at: SystemTime::now(),
                        };
                        entry.notify = Arc::new(Notify::new());
                        None
                    }
                }
            };

            if let Some(notify) = wait_for {
                notify.notified().await;
                if let Some(status) = self.cached_root(workspace_root_id).await {
                    return status;
                }
                continue;
            }
            break;
        }

        let snapshot = self
            .discovery
            .discover_root_status(workspace_root_id, &workspace_root)
            .await;
        self.finish_refresh(workspace_root_id, workspace_root, snapshot)
            .await
    }

    async fn cached_root(&self, workspace_root_id: WorkspaceRootId) -> Option<GitCachedStatus> {
        let inner = self.inner.lock().await;
        inner
            .get(&workspace_root_id)
            .map(|entry| entry.status(workspace_root_id))
    }

    async fn finish_refresh(
        &self,
        workspace_root_id: WorkspaceRootId,
        workspace_root: PathBuf,
        snapshot: GitStatusSnapshot,
    ) -> GitCachedStatus {
        let mut inner = self.inner.lock().await;
        let entry = inner
            .entry(workspace_root_id)
            .or_insert_with(|| GitCacheEntry::new(workspace_root.clone()));
        entry.workspace_root = workspace_root;
        let finished_at = SystemTime::now();
        if snapshot.last_refresh == GitRefreshStatus::Success {
            entry.snapshot = Some(snapshot);
            entry.refresh_state = GitRefreshState::LastSuccess { finished_at };
        } else {
            let status = snapshot.last_refresh.clone();
            if entry.snapshot.is_none() {
                entry.snapshot = Some(snapshot);
            }
            entry.refresh_state = GitRefreshState::LastError {
                finished_at,
                status,
            };
        }
        let notify = entry.notify.clone();
        let status = entry.status(workspace_root_id);
        notify.notify_waiters();
        status
    }
}

impl GitCacheEntry {
    fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            snapshot: None,
            refresh_state: GitRefreshState::Idle,
            notify: Arc::new(Notify::new()),
        }
    }

    fn status(&self, workspace_root_id: WorkspaceRootId) -> GitCachedStatus {
        GitCachedStatus {
            workspace_root_id,
            workspace_root: self.workspace_root.clone(),
            snapshot: self.snapshot.clone(),
            refresh_state: self.refresh_state.clone(),
        }
    }

    fn should_poll(&self, now: SystemTime) -> bool {
        match self.refresh_state {
            GitRefreshState::Idle => true,
            GitRefreshState::Refreshing { .. } => false,
            GitRefreshState::LastSuccess { finished_at }
            | GitRefreshState::LastError { finished_at, .. } => now
                .duration_since(finished_at)
                .is_ok_and(|age| age >= GIT_STATUS_POLL_INTERVAL),
        }
    }
}

impl Default for GitStatusCache {
    fn default() -> Self {
        Self::new(GitDiscoveryService::new())
    }
}

impl GitStatusSnapshot {
    fn error(
        workspace_root_id: WorkspaceRootId,
        workspace_root: PathBuf,
        last_refresh: GitRefreshStatus,
    ) -> Self {
        Self {
            workspace_root_id,
            workspace_root,
            repository_root: None,
            head: GitHeadState::Unknown,
            dirty: false,
            changed_file_count: 0,
            last_refresh,
        }
    }

    fn with_repo_error(
        workspace_root_id: WorkspaceRootId,
        workspace_root: PathBuf,
        repository_root: PathBuf,
        last_refresh: GitRefreshStatus,
    ) -> Self {
        Self {
            workspace_root_id,
            workspace_root,
            repository_root: Some(repository_root),
            head: GitHeadState::Unknown,
            dirty: false,
            changed_file_count: 0,
            last_refresh,
        }
    }
}

#[derive(Debug)]
struct CappedOutput {
    stdout: Vec<u8>,
}

#[derive(Debug)]
struct CappedBytes {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
enum GitRunError {
    Spawn(io::Error),
    Wait(io::Error),
    MissingPipe,
    Timeout,
    CommandFailed { code: Option<i32>, stderr: String },
    InvalidOutput(String),
}

impl GitRunError {
    fn into_refresh_status(self, command: GitDiscoveryCommand) -> GitRefreshStatus {
        match self {
            Self::Timeout => GitRefreshStatus::Timeout,
            Self::InvalidOutput(message) => GitRefreshStatus::InvalidOutput {
                command: command.name(),
                message,
            },
            Self::CommandFailed { code, stderr } => GitRefreshStatus::CommandError {
                command: command.name(),
                message: match code {
                    Some(code) if !stderr.is_empty() => format!("exit {code}: {stderr}"),
                    Some(code) => format!("exit {code}"),
                    None => stderr,
                },
            },
            Self::Spawn(error) | Self::Wait(error) => GitRefreshStatus::CommandError {
                command: command.name(),
                message: sanitize_text(&error.to_string()),
            },
            Self::MissingPipe => GitRefreshStatus::CommandError {
                command: command.name(),
                message: "missing process pipe".to_string(),
            },
        }
    }
}

async fn read_capped<R>(mut reader: R, max_bytes: usize) -> io::Result<CappedBytes>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(CappedBytes {
                bytes,
                truncated: false,
            });
        }
        let remaining = max_bytes.saturating_sub(bytes.len());
        if read > remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            return Ok(CappedBytes {
                bytes,
                truncated: true,
            });
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

fn canonical_directory(path: &Path) -> Result<PathBuf, io::Error> {
    let canonical = std::fs::canonicalize(path)?;
    let metadata = std::fs::metadata(&canonical)?;
    if metadata.is_dir() {
        Ok(canonical)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a directory",
        ))
    }
}

fn parse_single_line_path(bytes: &[u8]) -> Result<PathBuf, String> {
    parse_single_line(bytes).map(PathBuf::from)
}

fn parse_single_line(bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "output is not UTF-8".to_string())?;
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.contains('\n')
        || trimmed.contains('\r')
        || trimmed.contains('\0')
    {
        return Err("expected one non-empty line".to_string());
    }
    Ok(trimmed.to_string())
}

fn count_status_paths(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    let mut paths = HashSet::new();
    for line in text.lines().filter(|line| line.len() >= 4) {
        let path_text = if line.starts_with("R ") || line.starts_with("C ") {
            line[3..]
                .rsplit_once(" -> ")
                .map_or(&line[3..], |(_, new)| new)
        } else {
            &line[3..]
        };
        let path = path_text.trim();
        if !path.is_empty() {
            paths.insert(path.to_string());
        }
    }
    paths.len()
}

fn looks_like_non_repository(stderr: &str) -> bool {
    let message = stderr.to_ascii_lowercase();
    message.contains("not a git repository") || message.contains("not a gitdir")
}

fn sanitize_diagnostic(bytes: &[u8]) -> String {
    sanitize_text(&String::from_utf8_lossy(bytes))
}

fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
        .take(GIT_DIAGNOSTIC_MAX_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workspace::WorkspaceState;
    use std::{ffi::OsStr, fs, time::SystemTime};

    #[tokio::test]
    async fn repo_root_and_branch_status_are_discovered() {
        let root = temp_dir("git-discovery-repo");
        init_repo(&root);
        fs::write(root.join("tracked.txt"), "base").unwrap();
        git(&root, ["add", "."]);
        git(&root, ["commit", "-m", "initial"]);
        fs::write(root.join("tracked.txt"), "changed").unwrap();
        fs::write(root.join("untracked.txt"), "new").unwrap();

        let snapshot = GitDiscoveryService::new()
            .discover_root_status(7, &root)
            .await;

        assert_eq!(snapshot.workspace_root_id, 7);
        assert_eq!(snapshot.repository_root.as_deref(), Some(root.as_path()));
        assert_eq!(snapshot.head, GitHeadState::Branch("main".to_string()));
        assert_eq!(snapshot.changed_file_count, 2);
        assert!(snapshot.dirty);
        assert_eq!(snapshot.last_refresh, GitRefreshStatus::Success);
    }

    #[tokio::test]
    async fn non_repo_root_returns_typed_status() {
        let root = temp_dir("git-discovery-non-repo");
        let snapshot = GitDiscoveryService::new()
            .discover_root_status(1, &root)
            .await;

        assert_eq!(snapshot.repository_root, None);
        assert_eq!(snapshot.head, GitHeadState::Unknown);
        assert_eq!(snapshot.last_refresh, GitRefreshStatus::NonRepository);
    }

    #[tokio::test]
    async fn detached_head_returns_short_sha() {
        let root = temp_dir("git-discovery-detached");
        init_repo(&root);
        fs::write(root.join("tracked.txt"), "base").unwrap();
        git(&root, ["add", "."]);
        git(&root, ["commit", "-m", "initial"]);
        let sha = git_output(&root, ["rev-parse", "--short", "HEAD"]);
        git(&root, ["checkout", "--detach", "HEAD"]);

        let snapshot = GitDiscoveryService::new()
            .discover_root_status(1, &root)
            .await;

        assert_eq!(snapshot.head, GitHeadState::Detached(sha));
        assert_eq!(snapshot.last_refresh, GitRefreshStatus::Success);
    }

    #[tokio::test]
    async fn cwd_outside_workspace_root_is_rejected() {
        let root = temp_dir("git-discovery-parent");
        init_repo(&root);
        let subdir = root.join("nested");
        fs::create_dir_all(&subdir).unwrap();

        let snapshot = GitDiscoveryService::new()
            .discover_root_status(1, &subdir)
            .await;

        assert_eq!(snapshot.repository_root, None);
        assert_eq!(snapshot.last_refresh, GitRefreshStatus::BoundaryRejected);
    }

    #[tokio::test]
    async fn workspace_statuses_use_known_directory_roots() {
        let root = temp_dir("git-discovery-workspace-roots");
        init_repo(&root);
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let snapshots = GitDiscoveryService::new()
            .discover_workspace_statuses(&workspace)
            .await;

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].workspace_root_id, root_id);
        assert_eq!(snapshots[0].last_refresh, GitRefreshStatus::Success);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn command_timeout_is_typed() {
        let root = temp_dir("git-discovery-timeout");
        let fake_git = fake_git(&root, "sleep 2\n");
        tokio::time::sleep(Duration::from_millis(20)).await;

        let snapshot = GitDiscoveryService::with_git_binary(&fake_git)
            .with_timeout(Duration::from_millis(50))
            .discover_root_status(1, &root)
            .await;

        assert_eq!(snapshot.last_refresh, GitRefreshStatus::Timeout);
    }

    #[tokio::test]
    async fn cache_returns_cached_snapshot_until_explicit_refresh() {
        let root = temp_dir("git-cache-explicit-refresh");
        init_repo(&root);
        fs::write(root.join("tracked.txt"), "base").unwrap();
        git(&root, ["add", "."]);
        git(&root, ["commit", "-m", "initial"]);
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let cache = GitStatusCache::default();

        let cold = cache.list_cached(&workspace).await;
        assert_eq!(cold[0].refresh_state, GitRefreshState::Idle);
        assert!(cold[0].snapshot.is_none());

        let clean = cache.refresh_root(root_id, &root).await;
        assert_eq!(clean.snapshot.as_ref().unwrap().changed_file_count, 0);
        fs::write(root.join("tracked.txt"), "changed").unwrap();

        let cached = cache.list_cached(&workspace).await;
        assert_eq!(cached[0].snapshot.as_ref().unwrap().changed_file_count, 0);

        let refreshed = cache.refresh_root(root_id, &root).await;
        assert_eq!(refreshed.snapshot.unwrap().changed_file_count, 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn concurrent_refreshes_for_same_root_are_coalesced() {
        // Rendezvous design (starvation-proof): the leader's first command
        // appends to the count file and then blocks until a release sentinel
        // exists. Because the leader cannot complete until the test writes the
        // sentinel, a second refresh started in that window is guaranteed to
        // find the entry Refreshing and coalesce instead of starting its own
        // git run — regardless of how the runtime is descheduled under load.
        let root = temp_dir("git-cache-coalesce");
        let count_path = root.join("count");
        let release_path = root.join("release");
        let fake_git = fake_git(
            &root,
            &format!(
                r#"echo "$@" >> "{}"
case "$*" in
  *"rev-parse --show-toplevel"*)
    while [ ! -f "{}" ]; do sleep 0.02; done
    pwd ;;
  *"symbolic-ref"*) echo main ;;
  *"status --porcelain"*) ;;
  *) echo "fatal fake" >&2; exit 2 ;;
esac
"#,
                count_path.display(),
                release_path.display()
            ),
        );
        let cache = GitStatusCache::new(
            GitDiscoveryService::with_git_binary(fake_git).with_timeout(Duration::from_secs(30)),
        );

        let cache_a = cache.clone();
        let first_root = root.clone();
        let first = tokio::spawn(async move { cache_a.refresh_root(1, &first_root).await });

        // Wait until the leader's git run has started (rev-parse appended to
        // the count file) and is now blocked on the release sentinel.
        while read_count(&count_path) < 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Start the second refresh. On the next yield it coalesces (the leader
        // is Refreshing and blocked), so it must not start its own git run.
        let second_root = root.clone();
        let second = tokio::spawn(async move { cache.refresh_root(1, &second_root).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        // Only the leader's first command has run: the second coalesced.
        assert_eq!(read_count(&count_path), 1);

        // Release the leader; both finish sharing the single git run.
        fs::write(&release_path, "").unwrap();
        let (first_result, second_result) = tokio::join!(first, second);
        let first_result = first_result.unwrap();
        let second_result = second_result.unwrap();

        assert_eq!(first_result.refresh_state, second_result.refresh_state);
        assert_eq!(
            first_result.snapshot.unwrap().last_refresh,
            GitRefreshStatus::Success
        );
        assert_eq!(read_count(&count_path), 3);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn refreshes_for_different_roots_do_not_globally_serialize() {
        // Widened timing band: a 2s discovery timeout makes a serialized fast
        // root wait the full slow timeout (>=2s) before it can run, while a
        // concurrent fast root completes in well under 1.5s even under heavy
        // parallel test load. This gives a wide, starvation-resistant margin.
        let root = temp_dir("git-cache-multi-root");
        let slow = root.join("slow");
        let fast = root.join("fast");
        fs::create_dir_all(&slow).unwrap();
        fs::create_dir_all(&fast).unwrap();
        let fake_git = fake_git(
            &root,
            r#"case "$(pwd)" in *slow*) sleep 3 ;; esac
case "$*" in
  *"rev-parse --show-toplevel"*) pwd ;;
  *"symbolic-ref"*) echo main ;;
  *"status --porcelain"*) ;;
  *) echo "fatal fake" >&2; exit 2 ;;
esac
"#,
        );
        let cache = GitStatusCache::new(
            GitDiscoveryService::with_git_binary(fake_git).with_timeout(Duration::from_secs(2)),
        );
        let slow_cache = cache.clone();
        let slow_task = tokio::spawn(async move { slow_cache.refresh_root(1, slow).await });
        tokio::time::sleep(Duration::from_millis(50)).await;

        let started = tokio::time::Instant::now();
        let fast_status = cache.refresh_root(2, &fast).await;

        assert!(started.elapsed() < Duration::from_millis(1500));
        assert_eq!(
            fast_status.snapshot.unwrap().last_refresh,
            GitRefreshStatus::Success
        );
        assert!(matches!(
            slow_task.await.unwrap().refresh_state,
            GitRefreshState::LastError {
                status: GitRefreshStatus::Timeout,
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn refresh_error_keeps_previous_snapshot_with_diagnostics() {
        let root = temp_dir("git-cache-diagnostics");
        let fail_path = root.join("fail");
        let fake_git = fake_git(
            &root,
            &format!(
                r#"if [ -f "{}" ]; then echo "fatal: nope" >&2; exit 2; fi
case "$*" in
  *"rev-parse --show-toplevel"*) pwd ;;
  *"symbolic-ref"*) echo main ;;
  *"status --porcelain"*) ;;
  *) echo "fatal fake" >&2; exit 2 ;;
esac
"#,
                fail_path.display()
            ),
        );
        let cache = GitStatusCache::new(
            GitDiscoveryService::with_git_binary(fake_git).with_timeout(Duration::from_secs(30)),
        );

        let first = cache.refresh_root(1, &root).await;
        fs::write(&fail_path, "fail").unwrap();
        let second = cache.refresh_root(1, &root).await;

        assert_eq!(
            first.snapshot.as_ref().unwrap().last_refresh,
            GitRefreshStatus::Success
        );
        assert_eq!(
            second.snapshot.as_ref().unwrap().last_refresh,
            GitRefreshStatus::Success
        );
        assert!(matches!(
            second.refresh_state,
            GitRefreshState::LastError {
                status: GitRefreshStatus::CommandError { .. },
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stale_polling_refreshes_only_after_interval() {
        let root = temp_dir("git-cache-stale-poll");
        let count_path = root.join("count");
        let fake_git = fake_git(
            &root,
            &format!(
                r#"echo "$@" >> "{}"
case "$*" in
  *"rev-parse --show-toplevel"*) pwd ;;
  *"symbolic-ref"*) echo main ;;
  *"status --porcelain"*) ;;
  *) echo "fatal fake" >&2; exit 2 ;;
esac
"#,
                count_path.display()
            ),
        );
        let cache = GitStatusCache::new(
            GitDiscoveryService::with_git_binary(fake_git).with_timeout(Duration::from_secs(30)),
        );
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        cache.refresh_root(root_id, &root).await;
        // Pin finished_at to now so the "fresh → skip" assertion is immune to
        // wall-clock drift under parallel test load (the poll interval is 5s).
        {
            let mut inner = cache.inner.lock().await;
            inner.get_mut(&root_id).unwrap().refresh_state = GitRefreshState::LastSuccess {
                finished_at: SystemTime::now(),
            };
        }
        cache.refresh_stale_workspace(&workspace).await;
        assert_eq!(read_count(&count_path), 3);

        {
            let mut inner = cache.inner.lock().await;
            inner.get_mut(&root_id).unwrap().refresh_state = GitRefreshState::LastSuccess {
                finished_at: SystemTime::UNIX_EPOCH,
            };
        }
        cache.refresh_stale_workspace(&workspace).await;
        assert_eq!(read_count(&count_path), 6);
    }

    #[test]
    fn status_parser_counts_unique_paths_and_renames() {
        let output = b" M src/lib.rs\nM  src/lib.rs\n?? README.md\nR  old.rs -> new.rs\n";

        assert_eq!(count_status_paths(output), 3);
    }

    #[test]
    fn closed_command_table_is_read_only() {
        // Phase 18.13: GitDiscoveryCommand is a closed enum. Prove no variant
        // accepts arbitrary argv and none maps to a mutating subcommand. If a
        // future variant is added, this test forces an explicit read-only choice.
        const READ_ONLY_ROOTS: &[&str] = &["rev-parse", "symbolic-ref", "status"];
        const MUTATING_SUBCOMMANDS: &[&str] = &[
            "add",
            "commit",
            "checkout",
            "switch",
            "reset",
            "rebase",
            "stash",
            "push",
            "pull",
            "fetch",
            "merge",
            "cherry-pick",
            "clone",
            "mv",
            "rm",
            "tag",
            "apply",
            "restore",
            "bisect",
        ];
        for variant in [
            GitDiscoveryCommand::RepositoryRoot,
            GitDiscoveryCommand::Branch,
            GitDiscoveryCommand::DetachedHead,
            GitDiscoveryCommand::StatusShort,
        ] {
            let args = variant.args();
            assert!(
                READ_ONLY_ROOTS.contains(&args[0]),
                "Git discovery command {:?} must start with a read-only subcommand",
                variant
            );
            for &sub in MUTATING_SUBCOMMANDS {
                assert!(
                    !args.contains(&sub),
                    "Git discovery command {:?} must not run mutating subcommand `{sub}`",
                    variant
                );
            }
        }
    }

    fn init_repo(root: &Path) {
        git(root, ["init", "-b", "main"]);
        git(root, ["config", "user.email", "clay@example.invalid"]);
        git(root, ["config", "user.name", "Clay Test"]);
    }

    fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
        let status = command("git", cwd, args).status().unwrap();
        assert!(status.success(), "git command failed: {args:?}");
    }

    fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> String {
        let output = command("git", cwd, args).output().unwrap();
        assert!(output.status.success(), "git command failed: {args:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn command<const N: usize>(
        program: impl AsRef<OsStr>,
        cwd: &Path,
        args: [&str; N],
    ) -> std::process::Command {
        let mut command = std::process::Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "");
        command
    }

    #[cfg(unix)]
    fn read_count(count_path: &Path) -> usize {
        fs::read_to_string(count_path)
            .map(|content| content.lines().count())
            .unwrap_or(0)
    }

    #[cfg(unix)]
    fn fake_git(root: &Path, body: &str) -> PathBuf {
        use std::{io::Write, os::unix::fs::PermissionsExt};
        let path = root.join("fake-git");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"#!/bin/sh\n").unwrap();
        file.write_all(body.as_bytes()).unwrap();
        file.sync_all().unwrap();
        drop(file);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        // Linux can briefly return ETXTBSY if an executable is spawned right
        // after being written; keep fake-git tests deterministic under load.
        std::thread::sleep(Duration::from_millis(20));
        path
    }

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("clay-{label}-{unique}"));
        fs::create_dir_all(&root).unwrap();
        fs::canonicalize(root).unwrap()
    }
}
