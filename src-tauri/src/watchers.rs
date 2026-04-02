use crate::{indexing, storage, utils::unix_timestamp};
use anyhow::Result;
use notify::{
    event::{CreateKind, ModifyKind, RemoveKind},
    recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const WATCH_DEBOUNCE: Duration = Duration::from_millis(900);
const WATCH_POLL_INTERVAL: Duration = Duration::from_millis(300);

pub struct RootWatchService {
    sender: mpsc::Sender<WatchCommand>,
}

enum WatchCommand {
    Event(Event),
    WatchRoot { root_id: i64, path: PathBuf },
    UnwatchRoot { root_id: i64, path: PathBuf },
}

#[derive(Default)]
struct PendingRootEvents {
    changed_paths: HashSet<String>,
    removed_paths: HashSet<String>,
    needs_full_rescan: bool,
    last_event_at: Option<Instant>,
}

impl RootWatchService {
    pub fn new(
        db_path: PathBuf,
        vector_db_path: PathBuf,
        model_cache_dir: PathBuf,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel::<WatchCommand>();
        let notify_sender = sender.clone();
        let watcher = recommended_watcher(move |result| {
            if let Ok(event) = result {
                let _ = notify_sender.send(WatchCommand::Event(event));
            }
        })?;

        thread::spawn(move || {
            run_watch_loop(receiver, watcher, db_path, vector_db_path, model_cache_dir)
        });

        Ok(Self { sender })
    }

    pub fn watch_root(&self, root_id: i64, path: impl Into<PathBuf>) {
        let _ = self.sender.send(WatchCommand::WatchRoot {
            root_id,
            path: path.into(),
        });
    }

    pub fn unwatch_root(&self, root_id: i64, path: impl Into<PathBuf>) {
        let _ = self.sender.send(WatchCommand::UnwatchRoot {
            root_id,
            path: path.into(),
        });
    }
}

fn run_watch_loop(
    receiver: mpsc::Receiver<WatchCommand>,
    mut watcher: RecommendedWatcher,
    db_path: PathBuf,
    vector_db_path: PathBuf,
    model_cache_dir: PathBuf,
) {
    let mut watched_roots = HashMap::<i64, PathBuf>::new();
    let mut pending_events = HashMap::<i64, PendingRootEvents>::new();

    loop {
        match receiver.recv_timeout(WATCH_POLL_INTERVAL) {
            Ok(command) => handle_command(
                command,
                &mut watcher,
                &db_path,
                &mut watched_roots,
                &mut pending_events,
            ),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        flush_pending_events(
            &db_path,
            &vector_db_path,
            &model_cache_dir,
            &watched_roots,
            &mut pending_events,
        );
    }
}

fn handle_command(
    command: WatchCommand,
    watcher: &mut RecommendedWatcher,
    db_path: &Path,
    watched_roots: &mut HashMap<i64, PathBuf>,
    pending_events: &mut HashMap<i64, PendingRootEvents>,
) {
    match command {
        WatchCommand::WatchRoot { root_id, path } => {
            if watcher.watch(&path, RecursiveMode::Recursive).is_ok() {
                watched_roots.insert(root_id, path);
                if let Ok(conn) = storage::open_connection(db_path) {
                    let _ = storage::mark_root_watch_state(
                        &conn,
                        root_id,
                        "watching",
                        unix_timestamp(),
                    );
                }
            }
        }
        WatchCommand::UnwatchRoot { root_id, path } => {
            let _ = watcher.unwatch(&path);
            watched_roots.remove(&root_id);
            pending_events.remove(&root_id);
        }
        WatchCommand::Event(event) => record_event(event, db_path, watched_roots, pending_events),
    }
}

fn record_event(
    event: Event,
    db_path: &Path,
    watched_roots: &HashMap<i64, PathBuf>,
    pending_events: &mut HashMap<i64, PendingRootEvents>,
) {
    let now = Instant::now();
    let kind = event.kind;

    for path in event.paths {
        let Some((root_id, root_path)) = resolve_root(&path, watched_roots) else {
            continue;
        };

        let pending = pending_events.entry(root_id).or_default();
        pending.last_event_at = Some(now);

        match kind {
            EventKind::Create(CreateKind::File) | EventKind::Modify(ModifyKind::Data(_)) => {
                if path.is_file() && !indexing::is_ignored(&path) {
                    pending
                        .changed_paths
                        .insert(path.to_string_lossy().into_owned());
                    pending
                        .removed_paths
                        .remove(&path.to_string_lossy().into_owned());
                }
            }
            EventKind::Remove(RemoveKind::File) => {
                pending
                    .removed_paths
                    .insert(path.to_string_lossy().into_owned());
                pending
                    .changed_paths
                    .remove(&path.to_string_lossy().into_owned());
            }
            EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Create(CreateKind::Folder)
            | EventKind::Remove(RemoveKind::Folder)
            | EventKind::Modify(ModifyKind::Metadata(_))
            | EventKind::Modify(ModifyKind::Other)
            | EventKind::Other
            | EventKind::Any => {
                if path.starts_with(root_path) {
                    pending.needs_full_rescan = true;
                }
            }
            _ => {
                if path.starts_with(root_path) {
                    pending.needs_full_rescan = true;
                }
            }
        }

        if let Ok(conn) = storage::open_connection(db_path) {
            let _ = storage::mark_root_change_detected(&conn, root_id, unix_timestamp());
        }
    }
}

fn flush_pending_events(
    db_path: &Path,
    vector_db_path: &Path,
    model_cache_dir: &Path,
    watched_roots: &HashMap<i64, PathBuf>,
    pending_events: &mut HashMap<i64, PendingRootEvents>,
) {
    let ready_roots = pending_events
        .iter()
        .filter_map(|(root_id, pending)| {
            let elapsed = pending.last_event_at?;
            if elapsed.elapsed() >= WATCH_DEBOUNCE {
                Some(*root_id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for root_id in ready_roots {
        let Some(root_path) = watched_roots.get(&root_id).cloned() else {
            pending_events.remove(&root_id);
            continue;
        };

        let Some(_pending) = pending_events.get(&root_id) else {
            continue;
        };

        let Ok(conn) = storage::open_connection(db_path) else {
            continue;
        };
        let Ok(Some(root)) = storage::lookup_root_record(&conn, root_id) else {
            pending_events.remove(&root_id);
            continue;
        };

        if root.status == "indexing" || root.sync_status == "syncing" {
            continue;
        }

        let pending = pending_events.remove(&root_id).unwrap_or_default();

        if pending.needs_full_rescan {
            if let Ok(job_id) = storage::create_index_job(&conn, root_id, unix_timestamp()) {
                indexing::spawn_index_job(
                    db_path.to_path_buf(),
                    vector_db_path.to_path_buf(),
                    model_cache_dir.to_path_buf(),
                    root_id,
                    job_id,
                    root_path.to_string_lossy().into_owned(),
                );
            }
            continue;
        }

        if pending.changed_paths.is_empty() && pending.removed_paths.is_empty() {
            let _ = storage::mark_root_synced(&conn, root_id, unix_timestamp());
            continue;
        }

        indexing::spawn_incremental_sync_job(
            db_path.to_path_buf(),
            vector_db_path.to_path_buf(),
            model_cache_dir.to_path_buf(),
            root_id,
            root_path.to_string_lossy().into_owned(),
            pending.changed_paths.into_iter().collect(),
            pending.removed_paths.into_iter().collect(),
        );
    }
}

fn resolve_root<'a>(
    path: &Path,
    watched_roots: &'a HashMap<i64, PathBuf>,
) -> Option<(i64, &'a PathBuf)> {
    watched_roots
        .iter()
        .filter(|(_, root_path)| path.starts_with(root_path))
        .max_by_key(|(_, root_path)| root_path.components().count())
        .map(|(root_id, root_path)| (*root_id, root_path))
}
