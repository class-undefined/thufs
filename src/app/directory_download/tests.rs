use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

use super::*;
use crate::{
    app::pull_service::{PullResult, PullService},
    config::ConfigManager,
    transfer::ConflictPolicy,
};

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    fn json(body: Value) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: serde_json::to_vec(&body).unwrap(),
        }
    }
    fn error() -> Self {
        Self {
            status: 404,
            headers: Vec::new(),
            body: b"not found".to_vec(),
        }
    }
    fn file(body: &[u8], range: Option<&str>) -> Self {
        let mut response = Self {
            status: 200,
            headers: vec![(
                "Content-Disposition".into(),
                "attachment; filename=file.bin".into(),
            )],
            body: body.to_vec(),
        };
        if body.is_empty() && range.is_some() {
            response.status = 416;
            response
                .headers
                .push(("Content-Range".into(), "bytes */0".into()));
            return response;
        }
        if let Some(range) = range.and_then(|range| range.strip_prefix("bytes=")) {
            let (start, end) = range.split_once('-').unwrap();
            let start: usize = start.parse().unwrap();
            let end = if end.is_empty() {
                body.len() - 1
            } else {
                end.parse().unwrap()
            };
            assert!(start <= end && end < body.len());
            response.status = 206;
            response.headers.push((
                "Content-Range".into(),
                format!("bytes {start}-{end}/{}", body.len()),
            ));
            response.body = body[start..=end].to_vec();
        }
        response
    }
}

struct Server {
    base: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Server {
    fn new(handler: impl Fn(&url::Url, Option<&str>, bool) -> Response + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server_base = base.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = thread::spawn(move || {
            'connections: while !stopped.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("test server: {error}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    let bytes = match stream.read(&mut buffer) {
                        Ok(bytes) => bytes,
                        Err(error)
                            if matches!(
                                error.kind(),
                                ErrorKind::WouldBlock | ErrorKind::TimedOut
                            ) =>
                        {
                            continue 'connections;
                        }
                        Err(error) => panic!("test request: {error}"),
                    };
                    if bytes == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..bytes]);
                }
                if request.is_empty() {
                    continue;
                }
                let request = String::from_utf8(request).unwrap();
                let target = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                let url = url::Url::parse(&format!("{server_base}{target}")).unwrap();
                let range = request.lines().find_map(|line| {
                    line.split_once(':')
                        .filter(|(key, _)| key.eq_ignore_ascii_case("range"))
                        .map(|(_, value)| value.trim())
                });
                let authenticated = request
                    .lines()
                    .any(|line| line.to_ascii_lowercase().starts_with("authorization:"));
                let response = handler(&url, range, authenticated);
                let status = if response.status == 200 {
                    "OK"
                } else if response.status == 206 {
                    "Partial Content"
                } else {
                    "Error"
                };
                let mut headers = format!(
                    "HTTP/1.1 {} {status}\r\nConnection: close\r\nContent-Length: {}\r\n",
                    response.status,
                    response.body.len()
                );
                for (name, value) in response.headers {
                    headers.push_str(&format!("{name}: {value}\r\n"));
                }
                headers.push_str("\r\n");
                let _ = stream.write_all(headers.as_bytes());
                let _ = stream.write_all(&response.body);
            }
        });
        Self {
            base,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let joined = self.worker.take().unwrap().join();
        if !thread::panicking() {
            joined.unwrap();
        }
    }
}

fn repository_response(url: &url::Url, range: Option<&str>) -> Response {
    let path = url
        .query_pairs()
        .find(|(key, _)| key == "p")
        .map(|(_, path)| path.into_owned())
        .unwrap_or_default();
    match url.path() {
        "/api2/repos/" => Response::json(json!([{"id": "repo-id", "name": "Research"}])),
        "/api2/repos/repo-id/dir/" => match path.as_str() {
            "/" => Response::json(json!([{"name": "folder", "type": "dir"}])),
            "/folder" => Response::json(json!([
                {"name": "中文 子目录", "type": "dir"}, {"name": "empty", "type": "dir"},
                {"name": "root.txt", "type": "file", "size": 4},
                {"name": ".root.txt.thufs-part", "type": "file", "size": 3}
            ])),
            "/folder/中文 子目录" => {
                Response::json(json!([{"name": "data.bin", "type": "file", "size": 6}]))
            }
            "/folder/empty" => Response::json(json!([])),
            _ => Response::error(),
        },
        "/api2/repos/repo-id/file/" if path != "/folder" => {
            let mut link = url.clone();
            link.set_path("/content");
            Response::json(json!(link.as_str()))
        }
        "/content" => match path.as_str() {
            "/folder/root.txt" => Response::file(b"root", range),
            "/folder/.root.txt.thufs-part" => Response::file(b"dot", range),
            "/folder/中文 子目录/data.bin" => Response::file(b"abcdef", range),
            _ => Response::error(),
        },
        _ => Response::error(),
    }
}

fn service(server: &Server, authenticated: bool) -> (TempDir, PullService) {
    let config_dir = tempdir().unwrap();
    let config = ConfigManager::from_path(config_dir.path().join("config.json"))
        .with_session_token(authenticated.then(|| "test-token".to_string()));
    let client = SeafileClient::with_base_url(config.clone(), server.base.clone());
    (config_dir, PullService::new(config, client))
}

fn pull(
    service: &PullService,
    remote: &str,
    local: &Path,
    policy: ConflictPolicy,
) -> Result<PullResult> {
    service.pull(
        remote,
        Some(local),
        false,
        policy,
        DownloadMode::Sequential,
        4,
        ProgressMode::None,
    )
}

#[test]
fn repository_directory_download_preserves_nested_empty_and_special_names() {
    let server = Server::new(|url, range, authenticated| {
        assert!(authenticated);
        repository_response(url, range)
    });
    let (_config, service) = service(&server, true);
    let output = tempdir().unwrap();
    let result = pull(
        &service,
        "repo:Research/folder",
        output.path(),
        ConflictPolicy::Uniquify,
    )
    .unwrap();
    let folder = output.path().join("folder");
    assert_eq!(result.final_local_path, folder.display().to_string());
    assert!(result.is_directory);
    assert_eq!(result.bytes_written, 13);
    assert_eq!(
        fs::read(folder.join("中文 子目录/data.bin")).unwrap(),
        b"abcdef"
    );
    assert_eq!(fs::read(folder.join("root.txt")).unwrap(), b"root");
    assert_eq!(
        fs::read(folder.join(".root.txt.thufs-part")).unwrap(),
        b"dot"
    );
    assert!(folder.join("empty").is_dir());
    assert_eq!(fs::read_dir(output.path()).unwrap().count(), 1);
}

#[test]
fn a_library_root_can_be_downloaded_as_a_directory() {
    let server = Server::new(|url, range, _| repository_response(url, range));
    let (_config, service) = service(&server, true);
    let output = tempdir().unwrap();
    let result = pull(
        &service,
        "repo:Research",
        output.path(),
        ConflictPolicy::Uniquify,
    )
    .unwrap();
    assert!(result.is_directory);
    assert_eq!(
        fs::read(output.path().join("Research/folder/root.txt")).unwrap(),
        b"root"
    );
}

#[test]
fn directory_conflict_policies_rename_fail_and_merge_without_removing_unrelated_files() {
    let server = Server::new(|url, range, _| repository_response(url, range));
    let (_config, service) = service(&server, true);
    let output = tempdir().unwrap();
    let folder = output.path().join("folder");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("root.txt"), b"old").unwrap();
    fs::write(folder.join("keep.txt"), b"keep").unwrap();
    assert!(
        pull(
            &service,
            "repo:Research/folder/",
            output.path(),
            ConflictPolicy::Fail
        )
        .unwrap_err()
        .to_string()
        .contains("already exists")
    );
    let renamed = pull(
        &service,
        "repo:Research/folder/",
        output.path(),
        ConflictPolicy::Uniquify,
    )
    .unwrap();
    assert!(renamed.uniquified);
    assert_eq!(
        renamed.final_local_path,
        output.path().join("folder-(1)").display().to_string()
    );
    assert_eq!(fs::read(folder.join("root.txt")).unwrap(), b"old");
    let replaced = pull(
        &service,
        "repo:Research/folder/",
        output.path(),
        ConflictPolicy::Overwrite,
    )
    .unwrap();
    assert!(replaced.overwritten);
    assert_eq!(replaced.final_local_path, folder.display().to_string());
    assert_eq!(fs::read(folder.join("root.txt")).unwrap(), b"root");
    assert_eq!(fs::read(folder.join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn interrupted_directories_resume_partial_files_without_redownloading_completed_files() {
    let failed = Arc::new(AtomicBool::new(false));
    let fail = failed.clone();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let server = Server::new(move |url, range, _| {
        if url.path() == "/content" {
            let path = url
                .query_pairs()
                .find(|(key, _)| key == "p")
                .unwrap()
                .1
                .into_owned();
            recorded
                .lock()
                .unwrap()
                .push((path.clone(), range.map(str::to_string)));
            if path.ends_with("data.bin") && !fail.swap(true, Ordering::Relaxed) {
                return Response::error();
            }
        }
        repository_response(url, range)
    });
    let (_config, service) = service(&server, true);
    let output = tempdir().unwrap();
    let destination = output.path().join("folder");
    assert!(
        pull(
            &service,
            "repo:Research/folder/",
            output.path(),
            ConflictPolicy::Uniquify
        )
        .is_err()
    );
    assert!(!destination.exists());
    let staging = staging_path(
        &destination,
        &DirectorySource::Repository {
            repo_id: "repo-id".into(),
            path: "/folder".into(),
        },
    )
    .unwrap();
    fs::write(staging.join("parts/中文 子目录/data.bin"), b"abc").unwrap();
    fs::write(staging.join("tree/stale-file.txt"), b"stale").unwrap();
    let result = pull(
        &service,
        "repo:Research/folder/",
        output.path(),
        ConflictPolicy::Uniquify,
    )
    .unwrap();
    assert_eq!(result.bytes_written, 13);
    assert_eq!(
        fs::read(destination.join("中文 子目录/data.bin")).unwrap(),
        b"abcdef"
    );
    assert!(!destination.join("stale-file.txt").exists());
    assert!(!staging.exists());
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|(path, _)| path.ends_with("/root.txt"))
            .count(),
        1
    );
    assert!(requests.iter().any(|(path, range)| path.ends_with("data.bin") && range.as_deref() == Some("bytes=3-")));
}

#[test]
fn shared_directory_download_works_anonymously_and_preserves_empty_directories() {
    let server = Server::new(|url, range, authenticated| {
        assert!(!authenticated);
        if url.path() == "/api/v2.1/share-links/dir-token/dirents/" {
            let path = url
                .query_pairs()
                .find(|(key, _)| key == "path")
                .unwrap()
                .1
                .into_owned();
            return if path == "/" {
                Response::json(json!({"dir_path": "/published/共享目录/", "dirent_list": [
                    {"folder_name": "空目录", "is_dir": true},
                    {"file_name": "zero.bin", "is_dir": false, "size": 0},
                    {"file_name": "中文 文件.txt", "is_dir": false, "size": 4}
                ]}))
            } else {
                Response::json(json!({"dirent_list": []}))
            };
        }
        if url.path() == "/d/dir-token/files/" {
            if url
                .query_pairs()
                .any(|(key, value)| key == "p" && value == "/zero.bin")
            {
                return Response::file(b"", range);
            }
            assert_eq!(
                url.query_pairs().find(|(key, _)| key == "p").unwrap().1,
                "/中文 文件.txt"
            );
            return Response::file(b"data", range);
        }
        Response::error()
    });
    let (_config, service) = service(&server, false);
    let output = tempdir().unwrap();
    let result = pull(
        &service,
        "https://cloud.tsinghua.edu.cn/d/dir-token/?dl=1",
        output.path(),
        ConflictPolicy::Uniquify,
    )
    .unwrap();
    assert!(result.is_directory);
    assert_eq!(result.bytes_written, 4);
    assert_eq!(
        fs::read(output.path().join("共享目录/中文 文件.txt")).unwrap(),
        b"data"
    );
    assert!(output.path().join("共享目录/空目录").is_dir());
    assert_eq!(
        fs::metadata(output.path().join("共享目录/zero.bin"))
            .unwrap()
            .len(),
        0
    );
    let bare_output = tempdir().unwrap();
    let bare = service
        .pull(
            "dir-token",
            Some(bare_output.path()),
            true,
            ConflictPolicy::Uniquify,
            DownloadMode::Sequential,
            4,
            ProgressMode::None,
        )
        .unwrap();
    assert!(bare.is_directory);
    assert_eq!(
        fs::read(bare_output.path().join("共享目录/中文 文件.txt")).unwrap(),
        b"data"
    );
}

#[test]
fn a_completely_empty_shared_directory_is_saved_successfully() {
    let server = Server::new(|url, _, _| {
        if url.path() == "/api/v2.1/share-links/empty/dirents/" {
            Response::json(json!({"dir_path": "/empty folder/", "dirent_list": []}))
        } else {
            Response::error()
        }
    });
    let (_config, service) = service(&server, false);
    let output = tempdir().unwrap();
    let result = pull(
        &service,
        "https://cloud.tsinghua.edu.cn/d/empty/",
        output.path(),
        ConflictPolicy::Uniquify,
    )
    .unwrap();
    assert!(result.is_directory);
    assert_eq!(result.bytes_written, 0);
    assert!(output.path().join("empty folder").is_dir());
    assert_eq!(
        fs::read_dir(output.path().join("empty folder"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn directory_download_handles_unicode_normalization_without_pruning_downloaded_files() {
    let server = Server::new(|url, range, _| {
        if url.path() == "/api2/repos/repo-id/dir/" {
            return Response::json(json!([{"name": "café.txt", "type": "file", "size": 4}]));
        }
        if url.path() == "/content" {
            return Response::file(b"data", range);
        }
        repository_response(url, range)
    });
    let (_config, service) = service(&server, true);
    let output = tempdir().unwrap();
    pull(
        &service,
        "repo:Research/folder/",
        output.path(),
        ConflictPolicy::Uniquify,
    )
    .unwrap();
    assert_eq!(
        fs::read(output.path().join("folder/café.txt")).unwrap(),
        b"data"
    );
}

#[test]
fn remote_names_that_alias_locally_are_never_silently_combined() {
    let server = Server::new(|url, range, _| {
        if url.path() == "/api2/repos/repo-id/dir/" {
            return Response::json(json!([
                {"name": "File.txt", "type": "file", "size": 4},
                {"name": "file.txt", "type": "file", "size": 4}
            ]));
        }
        if url.path() == "/content" {
            return Response::file(b"data", range);
        }
        repository_response(url, range)
    });
    let (_config, service) = service(&server, true);
    let output = tempdir().unwrap();
    match pull(
        &service,
        "repo:Research/folder/",
        output.path(),
        ConflictPolicy::Uniquify,
    ) {
        Ok(result) => {
            assert_eq!(result.bytes_written, 8);
            assert_eq!(
                fs::read_dir(output.path().join("folder")).unwrap().count(),
                2
            );
        }
        Err(error) => {
            assert!(error.to_string().contains("collide"));
            assert!(!output.path().join("folder").exists());
        }
    }
}

#[test]
fn unsafe_names_and_paths_are_rejected() {
    for name in ["", ".", "..", "../escape", "/absolute", "a/b", "a\\b"] {
        assert!(validate_name(name).is_err(), "{name:?}");
    }
    for path in ["/folder/../escape", "/other/file", "/folder/a/../../escape"] {
        assert!(relative_path(path, "/folder").is_err());
    }
    assert_eq!(
        relative_path("/folder/中文 文件.txt", "/folder").unwrap(),
        PathBuf::from("中文 文件.txt")
    );
}

#[cfg(unix)]
#[test]
fn directory_overwrite_rejects_symlinked_descendants_without_writing_outside_the_target() {
    let server = Server::new(|url, range, _| repository_response(url, range));
    let (_config, service) = service(&server, true);
    let output = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::create_dir(output.path().join("folder")).unwrap();
    std::os::unix::fs::symlink(outside.path(), output.path().join("folder/中文 子目录")).unwrap();
    fs::write(outside.path().join("data.bin"), b"keep").unwrap();
    assert!(
        pull(
            &service,
            "repo:Research/folder/",
            output.path(),
            ConflictPolicy::Overwrite
        )
        .unwrap_err()
        .to_string()
        .contains("symbolic link")
    );
    assert_eq!(fs::read(outside.path().join("data.bin")).unwrap(), b"keep");
}
