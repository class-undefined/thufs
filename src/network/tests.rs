use std::{
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    sync::mpsc,
    thread,
    time::Duration,
};

use super::*;

fn server() -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let request = read_request(&mut stream);
        let _ = sender.send(request);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });
    (format!("http://{address}"), receiver)
}

fn read_request(stream: &mut impl Read) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0; 1024];
    loop {
        let read = stream.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}

fn get(config: &Config, url: &str) {
    let client = client_builder(config)
        .unwrap()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let text = runtime.block_on(async {
        client
            .get(url)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap()
    });
    assert_eq!(text, "ok");
}

#[test]
fn proxy_urls_validate_schemes_hosts_ports_and_hide_credentials() {
    for input in [
        "http://127.0.0.1:7890",
        "https://proxy.example:443",
        "socks5://127.0.0.1:1080",
        "socks5h://[::1]:1080",
    ] {
        assert!(ProxyUrl::parse(input).is_ok());
    }
    for input in [
        "",
        "127.0.0.1:7890",
        "file:///tmp/data",
        "http://",
        "http://proxy:0",
        "http://proxy:99999",
        "http://proxy/path",
        "http://proxy?query",
        "http://proxy/#fragment",
        "http://proxy/\n",
    ] {
        assert!(ProxyUrl::parse(input).is_err(), "{input:?}");
    }
    let proxy = ProxyConfig::from_input(
        "manual",
        "http://private-user:private-password@127.0.0.1:7890",
    )
    .unwrap();
    let text = format!("{proxy:?} {}", proxy.description());
    assert!(!text.contains("private-user"));
    assert!(!text.contains("private-password"));
    assert!(text.contains("127.0.0.1:7890"));
}

#[test]
fn manual_http_proxy_receives_absolute_urls_and_authentication() {
    let (url, received) = server();
    let url = url.replacen("http://", "http://test-user:test-password@", 1);
    let config = Config {
        proxy: ProxyConfig::from_input("manual", &url).unwrap(),
        ..Config::default()
    };
    get(&config, "http://thufs-proxy-test.invalid/data");
    let request = received.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(request.starts_with("GET http://thufs-proxy-test.invalid/data HTTP/1.1"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("proxy-authorization: basic ")
    );
    assert!(!request.to_ascii_lowercase().contains("\r\nauthorization:"));
}

#[test]
fn direct_mode_connects_to_the_origin() {
    let (url, received) = server();
    get(
        &Config {
            proxy: ProxyConfig::Direct,
            ..Config::default()
        },
        &format!("{url}/data"),
    );
    let request = received.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(request.starts_with("GET /data HTTP/1.1"));
    assert!(
        !request
            .to_ascii_lowercase()
            .contains("proxy-authorization:")
    );
}

#[test]
fn auto_mode_uses_proxy_environment_variables_and_no_proxy_without_mutating_the_test_process() {
    const TARGET: &str = "THUFS_PROXY_TEST_TARGET";
    if let Ok(url) = std::env::var(TARGET) {
        get(&Config::default(), &url);
        return;
    }
    for key in ["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"] {
        let (proxy, received) = server();
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args(["--exact", "network::tests::auto_mode_uses_proxy_environment_variables_and_no_proxy_without_mutating_the_test_process"]);
        for name in [
            "HTTP_PROXY",
            "http_proxy",
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
            "NO_PROXY",
            "no_proxy",
            "REQUEST_METHOD",
        ] {
            child.env_remove(name);
        }
        let output = child
            .env(key, proxy)
            .env(TARGET, "http://thufs-proxy-test.invalid/data")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            received
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .starts_with("GET http://thufs-proxy-test.invalid/data HTTP/1.1")
        );
    }
    let (origin, received) = server();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "network::tests::auto_mode_uses_proxy_environment_variables_and_no_proxy_without_mutating_the_test_process"])
        .env(TARGET, format!("{origin}/data"))
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("NO_PROXY", "127.0.0.1")
        .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        received
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .starts_with("GET /data HTTP/1.1")
    );
}

#[test]
fn socks5h_sends_the_hostname_to_the_proxy_for_remote_dns_resolution() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut greeting = [0; 2];
        stream.read_exact(&mut greeting).unwrap();
        assert_eq!(greeting[0], 5);
        let mut methods = vec![0; greeting[1] as usize];
        stream.read_exact(&mut methods).unwrap();
        stream.write_all(&[5, 0]).unwrap();
        let mut request = [0; 5];
        stream.read_exact(&mut request).unwrap();
        assert_eq!(&request[..4], &[5, 1, 0, 3]);
        let mut host = vec![0; request[4] as usize];
        stream.read_exact(&mut host).unwrap();
        let mut port = [0; 2];
        stream.read_exact(&mut port).unwrap();
        assert_eq!(u16::from_be_bytes(port), 80);
        stream
            .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 80])
            .unwrap();
        let request = read_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        sender
            .send((String::from_utf8(host).unwrap(), request))
            .unwrap();
    });
    let config = Config {
        proxy: ProxyConfig::from_input("manual", &format!("socks5h://{address}")).unwrap(),
        ..Config::default()
    };
    get(&config, "http://thufs-proxy-test.invalid/data");
    let (host, request) = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(host, "thufs-proxy-test.invalid");
    assert!(request.starts_with("GET /data HTTP/1.1"));
    worker.join().unwrap();
}
