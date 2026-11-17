use serde_json::Value;
use std::{
    fs,
    net::TcpListener,
    path::PathBuf,
    process::{Command, Output},
};

fn nyra(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nyra"))
        .args(args)
        .output()
        .expect("run nyra")
}

#[test]
fn discovers_tcp_check() {
    let help = nyra(&["--help"]);
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    let tools = nyra(&["tools", "list"]);
    assert!(tools.status.success());
    assert!(String::from_utf8_lossy(&tools.stdout).contains("network.tcp"));
}

#[test]
fn tool_catalog_includes_the_tcp_tool_summary() {
    let tools = nyra(&["tools", "list"]);
    assert!(tools.status.success());
    let output = String::from_utf8_lossy(&tools.stdout);
    assert!(output.contains("CATEGORY"));
    assert!(output.contains("DESCRIPTION"));
    assert!(output.contains("Network"));
    assert!(output.contains("Check whether one host accepts a TCP connection"));
    for tool_id in [
        "network.tcp",
        "network.dns",
        "network.reverse_dns",
        "web.http",
        "security.tls",
        "network.interfaces",
        "network.ping",
        "system.info",
        "system.cpu",
        "system.memory",
        "system.disk",
        "file.sha256",
    ] {
        assert!(output.contains(tool_id), "missing {tool_id}");
    }
}

#[test]
fn local_interfaces_are_structured_and_reported_without_network_probes() {
    let output = nyra(&["net", "interfaces", "--output", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["tool_id"], "network.interfaces");
    assert_eq!(json["finding"], "available");
    assert!(json["data"]["interfaces"].is_array());
}

#[test]
fn http_server_error_is_a_completed_negative_finding() {
    use std::io::{Read, Write};
    use std::thread;

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let url = format!("http://{address}/health");
    let output = nyra(&["web", "check", &url, "--output", "json"]);
    server.join().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["execution_status"], "completed");
    assert_eq!(json["finding"], "unhealthy");
    assert_eq!(json["data"]["status_code"], 503);
    assert!(json["error"].is_null());
}

#[test]
fn http_check_reports_status_and_redacts_url_query_values() {
    use std::io::{Read, Write};
    use std::thread;

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nServer: nyra-test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let url = format!("http://{address}/health?token=private");
    let output = nyra(&["web", "check", &url, "--output", "json"]);
    server.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["tool_id"], "web.http");
    assert_eq!(json["finding"], "healthy");
    assert_eq!(json["data"]["status_code"], 204);
    assert_eq!(json["data"]["url"], format!("http://{address}/health"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private"));
}

#[test]
fn http_redirect_destination_is_redacted_and_not_followed() {
    use std::io::{Read, Write};
    use std::thread;

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 302 Found\r\nLocation: /destination?token=private#section\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let url = format!("http://{address}/start");
    let output = nyra(&["web", "check", &url, "--output", "json"]);
    server.join().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let body = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["status_code"], 302);
    assert_eq!(
        json["data"]["redirect_location"],
        format!("http://{address}/destination")
    );
    assert!(!body.contains("private"));
    assert!(!body.contains("section"));
}

#[test]
fn remaining_diagnostic_inputs_are_validated_before_execution() {
    for args in [
        vec!["net", "dns", "127.0.0.1"],
        vec!["net", "dns", "example.com", "--record-types", "ANY"],
        vec!["net", "reverse-dns", "not-an-ip"],
        vec!["web", "check", "ftp://example.com"],
        vec!["tls", "inspect", "example.com", "--port", "0"],
        vec!["net", "ping", "example.com", "--count", "11"],
    ] {
        let output = nyra(&args);
        assert_eq!(output.status.code(), Some(2), "args={args:?}");
        assert!(output.stdout.is_empty(), "args={args:?}");
        assert!(!output.stderr.is_empty(), "args={args:?}");
    }
}

#[test]
fn bare_command_explains_interactive_terminal_requirement() {
    let output = nyra(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
}

#[test]
fn system_info_is_a_structured_local_measurement() {
    let output = nyra(&["sys", "info", "--output", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["tool_id"], "system.info");
    assert_eq!(json["target"], "local");
    assert_eq!(json["execution_status"], "completed");
    assert_eq!(json["finding"], "available");
    assert!(json["data"]["architecture"].as_str().is_some());
    assert!(json["data"]["os"].as_str().is_some());
}

#[test]
fn system_cpu_and_memory_reports_include_measurements_and_units() {
    let cpu = nyra(&["sys", "cpu", "--output", "json"]);
    assert_eq!(cpu.status.code(), Some(0));
    let cpu_json: Value = serde_json::from_slice(&cpu.stdout).unwrap();
    assert_eq!(cpu_json["tool_id"], "system.cpu");
    assert_eq!(cpu_json["finding"], "measured");
    assert!(cpu_json["data"]["logical_cores"].as_u64().unwrap() > 0);
    assert!(cpu_json["data"]["average_usage_percent"].is_number());

    let memory = nyra(&["sys", "memory", "--output", "json"]);
    assert_eq!(memory.status.code(), Some(0));
    let memory_json: Value = serde_json::from_slice(&memory.stdout).unwrap();
    assert_eq!(memory_json["tool_id"], "system.memory");
    assert_eq!(memory_json["finding"], "measured");
    assert!(memory_json["data"]["total_bytes"].is_number());
    assert!(memory_json["data"]["available_bytes"].is_number());
}

#[test]
fn disk_usage_returns_a_volume_array() {
    let output = nyra(&["sys", "disk", "--output", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["tool_id"], "system.disk");
    assert_eq!(json["finding"], "measured");
    assert!(json["data"]["volumes"].is_array());
}

#[test]
fn sha256_reports_the_known_digest_for_a_small_file() {
    let path = std::env::temp_dir().join(format!("nyra-hash-{}.txt", std::process::id()));
    fs::write(&path, b"abc").unwrap();
    let _cleanup = RemoveFile(path.clone());
    let path_text = path.to_string_lossy().into_owned();
    let output = nyra(&["file", "sha256", &path_text, "--output", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["tool_id"], "file.sha256");
    assert_eq!(json["finding"], "computed");
    assert_eq!(
        json["data"]["sha256"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(json["data"]["size_bytes"], 3);
}

struct RemoveFile(PathBuf);

impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn tcp_open_is_structured_and_quiet() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let output = nyra(&["net", "tcp", "127.0.0.1", &port, "--output", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["tool_id"], "network.tcp");
    assert_eq!(json["execution_status"], "completed");
    assert_eq!(json["finding"], "open");
    assert!(json["error"].is_null());
}

#[tokio::test(flavor = "current_thread")]
async fn tcp_refusal_is_a_completed_finding() {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let port = socket.local_addr().unwrap().port().to_string();
    let output = nyra(&["net", "tcp", "127.0.0.1", &port, "--output", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["execution_status"], "completed");
    assert_eq!(json["finding"], "refused");
    assert!(json["error"].is_null());
}

#[test]
fn invalid_input_has_no_result_on_stdout() {
    for (host, port, timeout) in [
        ("http://localhost", "80", "5"),
        ("localhost", "0", "5"),
        ("localhost", "80", "61"),
    ] {
        let output = nyra(&[
            "net",
            "tcp",
            host,
            port,
            "--timeout",
            timeout,
            "--output",
            "json",
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}
