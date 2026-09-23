use std::{
    collections::BTreeMap,
    ffi::OsStr,
    future::Future,
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    fs,
    time::{self, MissedTickBehavior},
};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const QUIET_PERIOD: Duration = Duration::from_millis(300);
const MAX_SCAN_FILES: usize = 256;
const MAX_SCAN_DEPTH: usize = 8;
const MAX_DEBOUNCE_RESCANS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    modified: SystemTime,
    length: u64,
}

type Snapshot = BTreeMap<PathBuf, FileStamp>;

pub(crate) async fn watch_configuration_root<F, Fut>(root: PathBuf, reload: F)
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    watch_configuration_root_with_intervals(root, reload, POLL_INTERVAL, QUIET_PERIOD).await;
}

pub(crate) async fn watch_configuration_root_with_intervals<F, Fut>(
    root: PathBuf,
    mut reload: F,
    poll_interval: Duration,
    quiet_period: Duration,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    let Ok(root) = fs::canonicalize(root).await else {
        return;
    };
    let Ok(mut baseline) = scan_configuration_root(&root).await else {
        return;
    };

    let mut interval = time::interval(poll_interval);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    interval.tick().await;

    loop {
        interval.tick().await;
        let Ok(snapshot) = scan_configuration_root(&root).await else {
            continue;
        };
        if snapshot == baseline {
            continue;
        }

        let Some(stable_snapshot) = wait_for_quiet_period(&root, snapshot, quiet_period).await
        else {
            continue;
        };
        reload().await;
        // Keep the pre-reload stable snapshot as the baseline instead of
        // adopting a post-reload scan: a change that arrives while the reload
        // runs (for example the recovery write after a failed reload) must
        // re-trigger the next poll, never be absorbed. Reloads themselves do
        // not write watched files — preferences.json is written only by the
        // settings command handlers before they reload — so a post-reload
        // difference is a genuine external change.
        baseline = stable_snapshot;
    }
}

async fn wait_for_quiet_period(
    root: &Path,
    mut snapshot: Snapshot,
    quiet_period: Duration,
) -> Option<Snapshot> {
    for _ in 0..MAX_DEBOUNCE_RESCANS {
        time::sleep(quiet_period).await;
        let next = scan_configuration_root(root).await.ok()?;
        if next == snapshot {
            return Some(snapshot);
        }
        snapshot = next;
    }
    Some(snapshot)
}

async fn scan_configuration_root(root: &Path) -> io::Result<Snapshot> {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut snapshot = BTreeMap::new();

    while let Some((directory, depth)) = pending.pop() {
        let mut entries = fs::read_dir(directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            if ignored_name(&name) {
                continue;
            }

            let file_type = entry.file_type().await?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                if depth < MAX_SCAN_DEPTH {
                    pending.push((path, depth + 1));
                }
                continue;
            }
            if !file_type.is_file() || !is_watched_file(&name) {
                continue;
            }
            if snapshot.len() >= MAX_SCAN_FILES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "configuration watcher file limit exceeded",
                ));
            }

            let metadata = entry.metadata().await?;
            snapshot.insert(
                path,
                FileStamp {
                    modified: metadata.modified().unwrap_or(UNIX_EPOCH),
                    length: metadata.len(),
                },
            );
        }
    }

    Ok(snapshot)
}

fn ignored_name(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return true;
    };
    name.starts_with('.') || name.ends_with(".tmp") || name.ends_with('~')
}

fn is_watched_file(name: &OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        name == "preferences.json" || Path::new(name).extension() == Some(OsStr::new("js"))
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "clay-config-watch-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create watcher fixture root");
        root
    }

    fn remove_root(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn scan_tracks_config_files_and_ignores_unrelated_or_temporary_entries() {
        let root = temp_root("scan");
        fs::create_dir(root.join("packages")).expect("create packages directory");
        fs::write(root.join("init.js"), "").expect("write init");
        fs::write(root.join("packages").join("package.js"), "").expect("write package");
        fs::write(root.join("preferences.json"), "{}").expect("write preferences");
        fs::write(root.join("notes.txt"), "").expect("write unrelated file");
        fs::write(root.join(".hidden.js"), "").expect("write hidden file");
        fs::write(root.join("package.js.tmp"), "").expect("write temporary file");

        let snapshot = scan_configuration_root(&root).await.expect("scan succeeds");

        assert_eq!(snapshot.len(), 3);
        assert!(snapshot.contains_key(&root.join("init.js")));
        assert!(snapshot.contains_key(&root.join("packages").join("package.js")));
        assert!(snapshot.contains_key(&root.join("preferences.json")));
        remove_root(&root);
    }

    #[tokio::test]
    async fn watcher_debounces_rapid_saves_into_one_reload() {
        let root = temp_root("debounce");
        fs::write(root.join("init.js"), "one").expect("write init");
        let reloads = Arc::new(AtomicUsize::new(0));
        let reloads_for_task = Arc::clone(&reloads);
        let watcher = tokio::spawn(watch_configuration_root_with_intervals(
            root.clone(),
            move || {
                let reloads = Arc::clone(&reloads_for_task);
                async move {
                    reloads.fetch_add(1, Ordering::Relaxed);
                }
            },
            Duration::from_millis(10),
            Duration::from_millis(25),
        ));

        time::sleep(Duration::from_millis(40)).await;
        fs::write(root.join("init.js"), "two").expect("write first edit");
        time::sleep(Duration::from_millis(5)).await;
        fs::write(root.join("init.js"), "three").expect("write second edit");
        time::sleep(Duration::from_millis(100)).await;

        watcher.abort();
        assert_eq!(reloads.load(Ordering::Relaxed), 1);
        remove_root(&root);
    }

    #[tokio::test]
    async fn watcher_detects_new_and_deleted_watched_files() {
        let root = temp_root("create-delete");
        fs::write(root.join("init.js"), "").expect("write init");
        let reloads = Arc::new(AtomicUsize::new(0));
        let reloads_for_task = Arc::clone(&reloads);
        let watcher = tokio::spawn(watch_configuration_root_with_intervals(
            root.clone(),
            move || {
                let reloads = Arc::clone(&reloads_for_task);
                async move {
                    reloads.fetch_add(1, Ordering::Relaxed);
                }
            },
            Duration::from_millis(10),
            Duration::from_millis(15),
        ));

        time::sleep(Duration::from_millis(35)).await;
        fs::write(root.join("new.js"), "").expect("create new module");
        time::sleep(Duration::from_millis(70)).await;
        fs::remove_file(root.join("new.js")).expect("delete module");
        time::sleep(Duration::from_millis(70)).await;
        fs::write(root.join("preferences.json"), "{}").expect("create preferences");
        time::sleep(Duration::from_millis(70)).await;

        watcher.abort();
        assert_eq!(reloads.load(Ordering::Relaxed), 3);
        remove_root(&root);
    }

    #[tokio::test]
    async fn watcher_reloads_for_a_change_that_lands_during_a_reload() {
        // A change arriving while a reload is in flight must not be absorbed
        // into the post-reload baseline: the watcher re-detects it and reloads
        // again (regression for the failed-then-recovered configuration
        // watcher test, where the recovery write landed mid-reload and the
        // generation never advanced).
        let root = temp_root("during-reload");
        fs::write(root.join("init.js"), "one").expect("write init");
        let reloads = Arc::new(AtomicUsize::new(0));
        let reloads_for_task = Arc::clone(&reloads);
        let root_for_task = root.clone();
        let watcher = tokio::spawn(watch_configuration_root_with_intervals(
            root.clone(),
            move || {
                let reloads = Arc::clone(&reloads_for_task);
                let root = root_for_task.clone();
                async move {
                    let count = reloads.fetch_add(1, Ordering::Relaxed) + 1;
                    if count == 1 {
                        // Simulate an external write landing while the first
                        // reload runs.
                        time::sleep(Duration::from_millis(50)).await;
                        fs::write(root.join("init.js"), "two").expect("write during reload");
                    }
                }
            },
            Duration::from_millis(10),
            Duration::from_millis(15),
        ));

        time::sleep(Duration::from_millis(35)).await;
        fs::write(root.join("init.js"), "one").expect("touch init");
        time::sleep(Duration::from_millis(200)).await;

        watcher.abort();
        assert_eq!(
            reloads.load(Ordering::Relaxed),
            2,
            "the change written during the first reload must trigger a second reload"
        );
        remove_root(&root);
    }
}
