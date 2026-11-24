use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

use super::*;

fn service() -> (tempfile::TempDir, UpdateService) {
    let temp = tempfile::tempdir().unwrap();
    let config = ConfigManager::from_path(temp.path().join("config.json"));
    let mut service = UpdateService::new(config);
    service.current_version = "0.1.9".into();
    (temp, service)
}

fn server(status: u16, body: &'static str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|part| part == b"\r\n\r\n") {
                break;
            }
        }
        sender.send(String::from_utf8(request).unwrap()).ok();
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    (format!("http://{address}/latest"), receiver)
}

const RELEASE: &str = r#"{"tag_name":"v0.1.10","draft":false,"prerelease":false}"#;

#[test]
fn update_checks_use_the_same_proxy_settings_as_cloud_requests() {
    let (_temp, mut service) = service();
    let (proxy, received) = server(200, RELEASE);
    service
        .config
        .set_proxy(
            &crate::network::ProxyConfig::from_input("manual", proxy.trim_end_matches("/latest"))
                .unwrap(),
        )
        .unwrap();
    service.endpoint = "http://thufs-update-proxy.invalid/latest".into();
    assert!(
        service
            .check_at(CheckMode::Refresh, 100)
            .unwrap()
            .update_available
    );
    let request = received.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(request.starts_with("GET http://thufs-update-proxy.invalid/latest HTTP/1.1"));
    assert!(!request.to_ascii_lowercase().contains("\r\nauthorization:"));
}

#[test]
fn checks_public_releases_with_custom_user_agent_and_no_cloud_credentials() {
    let (temp, mut service) = service();
    service.config.set_token("private-cloud-token").unwrap();
    service.config.set_user_agent("ExampleClient/2.0").unwrap();
    let (endpoint, request) = server(200, RELEASE);
    service.endpoint = endpoint;
    let result = service.check_at(CheckMode::Refresh, 100).unwrap();
    assert!(result.update_available);
    assert_eq!(result.latest_version.as_deref(), Some("0.1.10"));
    assert_eq!(result.release_url, format!("{RELEASES_URL}/tag/v0.1.10"));
    assert!(!result.cached);
    let request = request.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(
        request
            .to_ascii_lowercase()
            .contains("user-agent: exampleclient/2.0")
    );
    assert!(!request.to_ascii_lowercase().contains("authorization:"));
    assert!(!request.contains("private-cloud-token"));
    let cache = fs::read_to_string(temp.path().join("update-check.json")).unwrap();
    assert!(!cache.contains("private-cloud-token"));
    service.endpoint = "http://127.0.0.1:1/unreachable".into();
    service.current_version = "0.2.0".into();
    let cached = service.check_at(CheckMode::Cached, 101).unwrap();
    assert!(cached.cached);
    assert!(!cached.update_available);
    assert_eq!(cached.current_version, "0.2.0");
}

#[test]
fn compares_semantic_versions_without_mistaking_metadata_or_older_releases_for_updates() {
    let (_temp, mut service) = service();
    for (current, tag, available) in [
        ("0.1.9", "v0.1.10", true),
        ("0.1.10", "0.1.9", false),
        ("0.1.10", "V0.1.10", false),
        ("1.0.0-rc.1", "v1.0.0", true),
        ("1.0.0+local", "v1.0.0+release", false),
        ("1.0.0", "v2.0.0", true),
    ] {
        service.current_version = current.into();
        let result = service
            .result(
                &CachedRelease {
                    checked_at: 100,
                    tag: Some(tag.into()),
                },
                true,
            )
            .unwrap();
        assert_eq!(result.update_available, available, "{current} vs {tag}");
    }
    for tag in [
        "v1.0.0-beta.1",
        "latest",
        "v1.0.0\nInjected",
        "v1.0.0/../../invalid",
    ] {
        assert!(
            service
                .result(
                    &CachedRelease {
                        checked_at: 100,
                        tag: Some(tag.into())
                    },
                    true
                )
                .is_err()
        );
    }
}

#[test]
fn expired_future_and_corrupt_caches_are_replaced_by_an_online_check() {
    for cache in [
        r#"{"checked_at":0,"tag":"v0.1.8"}"#,
        r#"{"checked_at":999999,"tag":"v0.1.8"}"#,
        r#"{"checked_at":90000,"tag":"not-a-version"}"#,
        "invalid JSON",
    ] {
        let (temp, mut service) = service();
        fs::write(temp.path().join("update-check.json"), cache).unwrap();
        let (endpoint, _request) = server(200, RELEASE);
        service.endpoint = endpoint;
        let result = service.check_at(CheckMode::Cached, 90001).unwrap();
        assert!(!result.cached);
        assert!(result.update_available);
        assert!(service.read_cache(90002).unwrap().cached);
    }
}

#[test]
fn manual_refresh_bypasses_a_fresh_cache() {
    let (temp, mut service) = service();
    fs::write(
        temp.path().join("update-check.json"),
        r#"{"checked_at":100,"tag":"v0.1.9"}"#,
    )
    .unwrap();
    assert!(
        !service
            .check_at(CheckMode::Cached, 101)
            .unwrap()
            .update_available
    );
    let (endpoint, _request) = server(200, RELEASE);
    service.endpoint = endpoint;
    assert!(
        service
            .check_at(CheckMode::Refresh, 101)
            .unwrap()
            .update_available
    );
}

#[test]
fn missing_releases_are_distinct_from_errors_and_up_to_date_results() {
    let (_temp, mut service) = service();
    let (endpoint, _request) = server(404, "{}");
    service.endpoint = endpoint;
    let result = service.check_at(CheckMode::Refresh, 100).unwrap();
    assert!(result.latest_version.is_none());
    assert!(!result.update_available);
    assert_eq!(result.release_url, RELEASES_URL);
}

#[test]
fn rate_limits_server_errors_and_invalid_releases_never_replace_a_valid_cache() {
    for (status, body) in [
        (403, "{}"),
        (429, "{}"),
        (500, "{}"),
        (200, "not JSON"),
        (
            200,
            r#"{"tag_name":"latest","draft":false,"prerelease":false}"#,
        ),
        (
            200,
            r#"{"tag_name":"v0.2.0","draft":true,"prerelease":false}"#,
        ),
        (
            200,
            r#"{"tag_name":"v0.2.0","draft":false,"prerelease":true}"#,
        ),
        (
            200,
            r#"{"tag_name":"v0.2.0-beta.1","draft":false,"prerelease":false}"#,
        ),
    ] {
        let (temp, mut service) = service();
        let cache = r#"{"checked_at":100,"tag":"v0.1.9"}"#;
        fs::write(temp.path().join("update-check.json"), cache).unwrap();
        let (endpoint, _request) = server(status, body);
        service.endpoint = endpoint;
        assert!(service.check_at(CheckMode::Refresh, 101).is_err());
        assert_eq!(
            fs::read_to_string(temp.path().join("update-check.json")).unwrap(),
            cache
        );
    }
}

#[test]
fn an_unwritable_cache_does_not_hide_a_successful_update_check_or_leave_temporary_files() {
    let (temp, mut service) = service();
    fs::create_dir(temp.path().join("update-check.json")).unwrap();
    let (endpoint, _request) = server(200, RELEASE);
    service.endpoint = endpoint;
    assert!(
        service
            .check_at(CheckMode::Refresh, 100)
            .unwrap()
            .update_available
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}
