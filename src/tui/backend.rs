use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::{
    app::{
        App,
        account_service::{AccountInfoResult, RepoItem},
        list_service::ListItem,
        share_service::{ShareItem, ShareOptions, ShareResult},
    },
    config::ConfigManager,
    seafile::SeafileClient,
};

use super::model::{LocalEntry, Location, TransferSpec};

// Deliberately not Debug: requests can contain a share password.
pub enum Request {
    Account,
    Repositories {
        id: u64,
    },
    Directory {
        id: u64,
        location: Location,
    },
    Shares {
        id: u64,
    },
    LocalDirectory {
        id: u64,
        path: PathBuf,
        hidden: bool,
    },
    CreateShare {
        id: u64,
        remote: String,
        password: Option<String>,
        expire_days: Option<u32>,
    },
    CreateRepository {
        id: u64,
        name: String,
    },
    CreateDirectory {
        id: u64,
        remote: String,
    },
    Revoke {
        id: u64,
        token: String,
    },
    Transfer {
        id: u64,
        spec: TransferSpec,
    },
    CancelTransfer {
        id: u64,
    },
    Copy(String),
}

pub enum Operation {
    Shared(ShareResult),
    RepositoryCreated(String),
    DirectoryCreated,
    Revoked,
}
pub struct TransferOutcome {
    pub path: String,
    pub bytes: u64,
    pub uniquified: bool,
}

pub enum Message {
    Account(Result<AccountInfoResult, String>),
    Repositories {
        id: u64,
        result: Result<Vec<RepoItem>, String>,
    },
    Directory {
        id: u64,
        result: Result<Vec<ListItem>, String>,
    },
    Shares {
        id: u64,
        result: Result<Vec<ShareItem>, String>,
    },
    LocalDirectory {
        id: u64,
        result: Result<Vec<LocalEntry>, String>,
    },
    Operation {
        id: u64,
        result: Result<Operation, String>,
    },
    Progress {
        id: u64,
        transferred: u64,
        total: Option<u64>,
        warning: Option<String>,
    },
    TransferFinished {
        id: u64,
        result: Result<TransferOutcome, String>,
        cancelled: bool,
    },
    Copied(Result<(), String>),
}

struct TransferWorker {
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

pub struct Backend {
    sender: Sender<Message>,
    demo: bool,
    transfers: HashMap<u64, TransferWorker>,
    pending: VecDeque<Request>,
    workers: Vec<JoinHandle<()>>,
}

impl Backend {
    pub fn new(sender: Sender<Message>, demo: bool) -> Self {
        Self {
            sender,
            demo,
            transfers: HashMap::new(),
            pending: VecDeque::new(),
            workers: Vec::new(),
        }
    }

    pub fn dispatch(&mut self, request: Request) {
        if let Request::CancelTransfer { id } = request {
            if let Some(worker) = self.transfers.get(&id) {
                worker.cancel.store(true, Ordering::Relaxed);
            }
            return;
        }
        let sender = self.sender.clone();
        if let Request::Transfer { id, spec } = request {
            if self.demo {
                let _ = sender.send(Message::TransferFinished {
                    id,
                    result: Err("Demo is read-only".into()),
                    cancelled: false,
                });
                return;
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            let handle = thread::spawn(move || {
                let result = transfer(id, &spec, &worker_cancel, &sender).map_err(error_text);
                let cancelled = worker_cancel.load(Ordering::Relaxed) && result.is_err();
                let _ = sender.send(Message::TransferFinished {
                    id,
                    result,
                    cancelled,
                });
            });
            if let Some(previous) = self.transfers.insert(id, TransferWorker { cancel, handle }) {
                let _ = previous.handle.join();
            }
            // Reap completed workers so a long session does not retain thread resources.
            self.transfers
                .retain(|_, worker| !worker.handle.is_finished());
        } else {
            if matches!(
                request,
                Request::Account
                    | Request::Repositories { .. }
                    | Request::Directory { .. }
                    | Request::Shares { .. }
                    | Request::LocalDirectory { .. }
                    | Request::Copy(_)
            ) {
                self.pending.retain(|queued| {
                    std::mem::discriminant(queued) != std::mem::discriminant(&request)
                });
            }
            self.pending.push_back(request);
            self.pump();
        }
    }

    /// Bound metadata work and replace obsolete queued reads during rapid navigation.
    pub fn pump(&mut self) {
        self.workers.retain(|worker| !worker.is_finished());
        while self.workers.len() < 4 {
            let Some(request) = self.pending.pop_front() else {
                break;
            };
            let sender = self.sender.clone();
            let demo = self.demo;
            self.workers.push(thread::spawn(move || {
                let message = if demo {
                    super::demo::respond(request)
                } else {
                    execute(request)
                };
                let _ = sender.send(message);
            }));
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        for worker in self.transfers.values() {
            worker.cancel.store(true, Ordering::Relaxed);
        }
        for (_, worker) in self.transfers.drain() {
            let _ = worker.handle.join();
        }
    }
}

fn error_text(error: anyhow::Error) -> String {
    format!("{error:#}")
}

fn app() -> App {
    App::with_request_timeout(Duration::from_secs(30))
}

pub fn execute(request: Request) -> Message {
    match request {
        Request::Account => {
            Message::Account(app().account_service.account_info().map_err(error_text))
        }
        Request::Repositories { id } => Message::Repositories {
            id,
            result: app()
                .account_service
                .repositories()
                .map(|r| r.repos)
                .map_err(error_text),
        },
        Request::Directory { id, location } => {
            // Use the already resolved library ID; avoid re-fetching every library and N+1 timestamp requests on each keypress.
            let result =
                SeafileClient::with_request_timeout(ConfigManager::new(), Duration::from_secs(30))
                    .list_directory_entries_with_time(&location.repo_id, &location.path, false)
                    .map(|entries| entries.into_iter().map(ListItem::from).collect())
                    .map_err(error_text);
            Message::Directory { id, result }
        }
        Request::Shares { id } => Message::Shares {
            id,
            result: app()
                .share_service
                .list_shares(None, 1, 100, true)
                .map(|r| r.shares)
                .map_err(error_text),
        },
        Request::LocalDirectory { id, path, hidden } => Message::LocalDirectory {
            id,
            result: local_entries(&path, hidden).map_err(error_text),
        },
        Request::CreateShare {
            id,
            remote,
            password,
            expire_days,
        } => Message::Operation {
            id,
            result: app()
                .share_service
                .share(
                    &remote,
                    ShareOptions {
                        password,
                        expire_days,
                    },
                )
                .map(Operation::Shared)
                .map_err(error_text),
        },
        Request::CreateRepository { id, name } => Message::Operation {
            id,
            result: app()
                .create_service
                .create_repo(&name)
                .map(|r| Operation::RepositoryCreated(r.repo))
                .map_err(error_text),
        },
        Request::CreateDirectory { id, remote } => Message::Operation {
            id,
            result: app()
                .create_service
                .create_dir(&remote)
                .map(|_| Operation::DirectoryCreated)
                .map_err(error_text),
        },
        Request::Revoke { id, token } => Message::Operation {
            id,
            result: app()
                .share_service
                .unshare(&token)
                .map(|_| Operation::Revoked)
                .map_err(error_text),
        },
        Request::Copy(text) => Message::Copied(copy(&text).map_err(error_text)),
        Request::Transfer { .. } | Request::CancelTransfer { .. } => {
            unreachable!("transfers are dispatched separately")
        }
    }
}

pub fn local_entries(path: &PathBuf, hidden: bool) -> Result<Vec<LocalEntry>> {
    let mut entries = Vec::new();
    for entry in
        std::fs::read_dir(path).with_context(|| format!("Cannot open {}", path.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !hidden && name.starts_with('.') {
            continue;
        }
        let metadata = match std::fs::metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if !metadata.is_file() && !metadata.is_dir() {
            continue;
        }
        entries.push(LocalEntry {
            name,
            path: entry.path(),
            is_dir: metadata.is_dir(),
            size: if metadata.is_file() {
                Some(metadata.len())
            } else {
                None
            },
        });
    }
    entries.sort_by(|a, b| {
        (!a.is_dir)
            .cmp(&(!b.is_dir))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// Kill and reap a transfer even when an early error or panic interrupts the worker.
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn transfer_arguments(spec: &TransferSpec) -> Vec<std::ffi::OsString> {
    let (verb, policy) = match spec {
        TransferSpec::Upload { policy, .. } => ("upload", policy),
        TransferSpec::Download { policy, .. } => ("download", policy),
    };
    let mut args: Vec<std::ffi::OsString> = [
        "--json",
        verb,
        "--progress",
        "jsonl",
        "--conflict",
        policy,
        "--",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    match spec {
        TransferSpec::Upload { local, remote, .. } => {
            args.push(local.as_os_str().to_owned());
            args.push(remote.into());
        }
        TransferSpec::Download { remote, local, .. } => {
            args.push(remote.into());
            args.push(local.as_os_str().to_owned());
        }
    }
    args
}

fn transfer(
    id: u64,
    spec: &TransferSpec,
    cancel: &AtomicBool,
    sender: &Sender<Message>,
) -> Result<TransferOutcome> {
    let mut command = Command::new(std::env::current_exe()?);
    command.args(transfer_arguments(spec));
    monitor_transfer(id, cancel, sender, command)
}

fn monitor_transfer(
    id: u64,
    cancel: &AtomicBool,
    sender: &Sender<Message>,
    mut command: Command,
) -> Result<TransferOutcome> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Cancelled");
    }
    let mut child = ChildGuard(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Cannot start transfer process")?,
    );
    let stdout = child.0.stdout.take().context("Missing transfer output")?;
    let stderr = child
        .0
        .stderr
        .take()
        .context("Missing transfer progress stream")?;
    let out = thread::spawn(move || {
        let mut text = String::new();
        stdout
            .take(1024 * 1024)
            .read_to_string(&mut text)
            .map(|_| text)
    });
    let progress_sender = sender.clone();
    let err = thread::spawn(move || {
        let mut errors = String::new();
        let mut last_emit = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else {
                break;
            };
            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                if event.get("event").is_some() {
                    let warning = event
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    if last_emit.elapsed() >= Duration::from_millis(80)
                        || warning.is_some()
                        || event["event"] == "progress-finished"
                    {
                        let _ = progress_sender.send(Message::Progress {
                            id,
                            transferred: event["transferred_bytes"].as_u64().unwrap_or(0),
                            total: event["total_bytes"].as_u64(),
                            warning,
                        });
                        last_emit = Instant::now();
                    }
                    continue;
                }
            }
            if errors.len() < 64 * 1024 {
                if !errors.is_empty() {
                    errors.push('\n');
                }
                errors.push_str(&line);
            }
        }
        errors
    });
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.0.kill();
            break child.0.wait()?;
        }
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(40));
    };
    let output = out
        .join()
        .map_err(|_| anyhow::anyhow!("Transfer output reader stopped"))??;
    let errors = err
        .join()
        .unwrap_or_else(|_| "Transfer progress reader stopped".to_string());
    if !status.success() {
        bail!(
            "{}",
            if errors.trim().is_empty() {
                format!("Transfer exited with {status}")
            } else {
                errors.trim().to_string()
            }
        );
    }
    parse_transfer_outcome(&output)
}

pub fn parse_transfer_outcome(output: &str) -> Result<TransferOutcome> {
    let result: Value = serde_json::from_str(output).context("Cannot decode transfer result")?;
    let path = if let Some(local) = result.get("final_local_path").and_then(Value::as_str) {
        local.to_string()
    } else {
        let remote = result
            .get("final_remote_path")
            .and_then(Value::as_str)
            .context("Transfer did not return its saved path")?;
        let repo = result
            .get("repo")
            .and_then(Value::as_str)
            .context("Upload did not return its library")?;
        format!("repo:{repo}{remote}")
    };
    Ok(TransferOutcome {
        path,
        bytes: result
            .get("bytes_written")
            .or_else(|| result.get("size"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        uniquified: result["uniquified"].as_bool().unwrap_or(false),
    })
}

fn copy(text: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let candidates: &[(&str, &[&str])] = &[("pbcopy", &[])];
    #[cfg(target_os = "windows")]
    let candidates: &[(&str, &[&str])] = &[("clip", &[])];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];
    for (program, args) in candidates {
        let Ok(child) = Command::new(program)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        let mut child = ChildGuard(child);
        if let Some(mut stdin) = child.0.stdin.take() {
            if stdin.write_all(text.as_bytes()).is_err() {
                continue;
            }
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(2) {
            if let Some(status) = child.0.try_wait()? {
                if status.success() {
                    return Ok(());
                }
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
    bail!("Clipboard unavailable. Select the text using thufs tui --no-mouse.")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn rapid_navigation_bounds_workers_and_only_keeps_the_latest_queued_read() {
        let (sender, _receiver) = mpsc::channel();
        let mut backend = Backend::new(sender, false);
        let gate = Arc::new(std::sync::Barrier::new(5));
        for _ in 0..4 {
            let gate = gate.clone();
            backend.workers.push(thread::spawn(move || {
                gate.wait();
            }));
        }
        let location = Location {
            repo: "Research".into(),
            repo_id: "demo".into(),
            path: "/".into(),
        };
        for id in 1..=100 {
            backend.dispatch(Request::Directory {
                id,
                location: location.clone(),
            });
        }
        backend.dispatch(Request::Shares { id: 101 });
        assert_eq!(backend.workers.len(), 4);
        assert_eq!(backend.pending.len(), 2);
        assert!(matches!(
            backend.pending.front(),
            Some(Request::Directory { id: 100, .. })
        ));
        gate.wait();
        for worker in backend.workers.drain(..) {
            worker.join().unwrap();
        }
    }

    #[test]
    fn transfer_worker_reads_progress_warnings_and_the_saved_path_from_a_real_child() {
        let (sender, receiver) = mpsc::channel();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", r#"
            printf '%s\n' '{"event":"progress","transferred_bytes":4,"total_bytes":10}' >&2
            printf '%s\n' '{"event":"warning","message":"retrying","transferred_bytes":4,"total_bytes":10}' >&2
            printf '%s\n' '{"event":"progress-finished","transferred_bytes":10,"total_bytes":10}' >&2
            printf '%s\n' '{"final_local_path":"/tmp/result-(1).txt","bytes_written":10,"uniquified":true}'
        "#]);
        let outcome = monitor_transfer(7, &AtomicBool::new(false), &sender, command).unwrap();
        assert_eq!(outcome.path, "/tmp/result-(1).txt");
        assert!(outcome.uniquified);
        let messages: Vec<_> = receiver.try_iter().collect();
        assert!(messages.iter().any(|m| matches!(
            m,
            Message::Progress {
                id: 7,
                transferred: 4,
                total: Some(10),
                warning: None
            }
        )));
        assert!(messages.iter().any(
            |m| matches!(m, Message::Progress { warning: Some(text), .. } if text == "retrying")
        ));
        assert!(messages.iter().any(|m| matches!(
            m,
            Message::Progress {
                transferred: 10,
                ..
            }
        )));
    }

    #[test]
    fn cancellation_kills_and_reaps_the_transfer_process_promptly() {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let handle = thread::spawn(move || {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", r#"printf '%s\n' '{"event":"progress-started","transferred_bytes":0}' >&2; exec sleep 30"#]);
            monitor_transfer(8, &worker_cancel, &sender, command)
        });
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("child started");
        let cancelled_at = Instant::now();
        cancel.store(true, Ordering::Relaxed);
        assert!(handle.join().unwrap().is_err());
        assert!(cancelled_at.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn transfer_failures_preserve_the_cli_error_message() {
        let (sender, _receiver) = mpsc::channel();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf 'permission denied\\n' >&2; exit 7"]);
        let error = monitor_transfer(9, &AtomicBool::new(false), &sender, command)
            .err()
            .unwrap();
        assert!(error.to_string().contains("permission denied"));
    }

    #[test]
    fn local_file_picker_follows_valid_symlinks_and_filters_hidden_files() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("visible.txt"), "test").unwrap();
        std::fs::write(temp.path().join(".hidden"), "secret").unwrap();
        std::fs::create_dir(temp.path().join("folder")).unwrap();
        std::os::unix::fs::symlink(
            temp.path().join("visible.txt"),
            temp.path().join("link.txt"),
        )
        .unwrap();
        let entries = local_entries(&temp.path().to_path_buf(), false).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_dir);
        assert!(
            entries
                .iter()
                .any(|e| e.name == "link.txt" && e.size == Some(4))
        );
        assert_eq!(
            local_entries(&temp.path().to_path_buf(), true)
                .unwrap()
                .len(),
            4
        );
    }
}
