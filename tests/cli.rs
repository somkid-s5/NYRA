use serde_json::Value;
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn nyra(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nyra"))
        .args(args)
        .output()
        .expect("run nyra")
}

fn nyra_with_config(args: &[&str], config_root: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nyra"));
    #[cfg(target_os = "windows")]
    command.env("APPDATA", config_root);
    #[cfg(target_os = "linux")]
    command.env("XDG_CONFIG_HOME", config_root);
    #[cfg(target_os = "macos")]
    command.env("HOME", config_root);
    command
        .args(args)
        .output()
        .expect("run nyra with isolated config")
}

#[test]
fn custom_theme_cli_validates_imports_and_exports_without_accidental_replacement() {
    let workspace = tempfile::tempdir().unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("custom-theme.toml");
    let source_arg = source.to_string_lossy().into_owned();

    let validation = nyra(&["theme", "validate", &source_arg]);
    assert_eq!(validation.status.code(), Some(0));
    assert!(validation.stderr.is_empty());
    assert!(String::from_utf8_lossy(&validation.stdout).contains("Midnight Circuit"));

    let imported = nyra_with_config(&["theme", "import", &source_arg], workspace.path());
    assert_eq!(imported.status.code(), Some(0));
    assert!(imported.stderr.is_empty());
    assert!(String::from_utf8_lossy(&imported.stdout).contains("installed"));

    let duplicate = nyra_with_config(&["theme", "import", &source_arg], workspace.path());
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(duplicate.stdout.is_empty());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("--force"));

    let invalid_source = workspace.path().join("invalid.toml");
    fs::write(&invalid_source, "version = 2\n").unwrap();
    let invalid_arg = invalid_source.to_string_lossy().into_owned();
    let invalid_validation = nyra(&["theme", "validate", &invalid_arg]);
    assert_eq!(invalid_validation.status.code(), Some(2));
    assert!(invalid_validation.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid_validation.stderr).contains("version"));

    let invalid_import = nyra_with_config(
        &["theme", "import", &invalid_arg, "--force"],
        workspace.path(),
    );
    assert_eq!(invalid_import.status.code(), Some(2));

    let exported_path = workspace.path().join("exported.toml");
    let exported_arg = exported_path.to_string_lossy().into_owned();
    let exported = nyra_with_config(&["theme", "export", &exported_arg], workspace.path());
    assert_eq!(exported.status.code(), Some(0));
    assert!(exported.stderr.is_empty());
    assert_eq!(fs::read(exported_path).unwrap(), fs::read(source).unwrap());

    let duplicate_export = nyra_with_config(&["theme", "export", &exported_arg], workspace.path());
    assert_eq!(duplicate_export.status.code(), Some(2));
    let forced_export = nyra_with_config(
        &["theme", "export", &exported_arg, "--force"],
        workspace.path(),
    );
    assert_eq!(forced_export.status.code(), Some(0));
}

#[test]
fn knowledge_cli_lists_and_displays_read_only_command_cards() {
    let list = nyra(&["knowledge", "list"]);
    assert_eq!(list.status.code(), Some(0));
    assert!(list.stderr.is_empty());
    let list_text = String::from_utf8_lossy(&list.stdout);
    assert!(list_text.contains("linux.filesystem-space"));
    assert!(list_text.contains("windows.tcp-connectivity"));
    assert!(list_text.contains("macos.tcp-connectivity"));
    assert!(list_text.contains("solaris.local-tcp-sockets"));

    let search = nyra(&["knowledge", "search", "check", "port"]);
    assert_eq!(search.status.code(), Some(0));
    let search_text = String::from_utf8_lossy(&search.stdout);
    assert!(search_text.contains("linux.tcp-connectivity"));
    assert!(search_text.contains("windows.tcp-connectivity"));
    assert!(search_text.contains("macos.tcp-connectivity"));
    assert!(search_text.contains("solaris.tcp-connectivity"));

    let solaris_card = nyra(&["knowledge", "show", "solaris.tcp-connectivity"]);
    assert_eq!(solaris_card.status.code(), Some(0));
    assert!(solaris_card.stderr.is_empty());
    let solaris_card_text = String::from_utf8_lossy(&solaris_card.stdout);
    assert!(solaris_card_text.contains("nc -z"));
    assert!(solaris_card_text.contains("never runs the command"));

    let card = nyra(&["knowledge", "show", "linux.find-large-files"]);
    assert_eq!(card.status.code(), Some(0));
    assert!(card.stderr.is_empty());
    let card_text = String::from_utf8_lossy(&card.stdout);
    assert!(card_text.contains("find /var/log -type f -size +100M -print"));
    assert!(card_text.contains("does not change or delete files"));
    assert!(card_text.contains("never runs the command"));

    let missing = nyra(&["knowledge", "show", "unknown.card"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("knowledge list"));
}

#[test]
fn discovers_tcp_check() {
    let help = nyra(&["--help"]);
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--log-level"));
    let tools = nyra(&["tools", "list"]);
    assert!(tools.status.success());
    assert!(String::from_utf8_lossy(&tools.stdout).contains("network.tcp"));
}

#[test]
fn dns_lookup_reports_localhost_a_and_aaaa_records() {
    let output = nyra(&[
        "net",
        "dns",
        "localhost",
        "--record-types",
        "A,AAAA",
        "--output",
        "json",
    ]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["tool_id"], "network.dns");
    assert_eq!(json["execution_status"], "completed");
    assert_eq!(json["finding"], "available");
    let records = json["data"]["records"].as_array().unwrap();
    for (record_type, expected_address) in [("A", "127.0.0.1"), ("AAAA", "::1")] {
        let record = records
            .iter()
            .find(|record| record["record_type"] == record_type)
            .unwrap();
        assert!(
            record["answers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|answer| answer == expected_address),
            "expected {record_type} answer {expected_address}, got {}",
            record["answers"]
        );
    }
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
        "workflow.web_diagnostics",
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
fn opt_in_debug_logs_stay_on_stderr_and_omit_url_details() {
    use std::io::{Read, Write};
    use std::thread;

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "HTTP client did not connect"
                    );
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("failed to accept HTTP connection: {error}"),
            }
        };
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let url = format!("http://{address}/health?token=private");
    let output = nyra(&[
        "web",
        "check",
        &url,
        "--output",
        "json",
        "--log-level",
        "debug",
    ]);
    server.join().unwrap();

    assert_eq!(output.status.code(), Some(0));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["finding"], "healthy");
    let logs = String::from_utf8_lossy(&output.stderr);
    assert!(logs.contains("diagnostic run started"));
    assert!(logs.contains("diagnostic run completed"));
    assert!(logs.contains("web.http"));
    assert!(!logs.contains("private"));
    assert!(!logs.contains(&address.to_string()));
    assert!(!logs.contains("/health"));
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
    assert!(!body.contains("token=private"));
    assert!(!body.contains("section"));
}

#[test]
fn remaining_diagnostic_inputs_are_validated_before_execution() {
    for args in [
        vec!["net", "dns", "127.0.0.1"],
        vec!["net", "dns", "example.com", "--record-types", "ANY"],
        vec!["net", "reverse-dns", "not-an-ip"],
        vec!["web", "check", "ftp://example.com"],
        vec!["web", "check", "http://example.com:0"],
        vec!["tls", "inspect", "example.com", "--port", "0"],
        vec!["net", "ping", "example.com", "--count", "11"],
        vec![
            "workflow",
            "run",
            "web-unreachable",
            "--target",
            "ftp://example.com",
        ],
    ] {
        let output = nyra(&args);
        assert_eq!(output.status.code(), Some(2), "args={args:?}");
        assert!(output.stdout.is_empty(), "args={args:?}");
        assert!(!output.stderr.is_empty(), "args={args:?}");
    }
}

#[test]
fn web_diagnostics_reuses_one_peer_and_reports_each_protocol_layer() {
    use std::io::{Read, Write};
    use std::{
        io::ErrorKind,
        thread,
        time::{Duration, Instant},
    };

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(6);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => return,
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0_u8; 2048];
            let count = match stream.read(&mut request) {
                Ok(count) => count,
                Err(_) => return,
            };
            if count > 0 {
                stream
                    .write_all(b"HTTP/1.1 503 Service Unavailable\r\nX-Internal-Token: private-header\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
            }
        }
    });
    let workspace = tempfile::tempdir().unwrap();
    let report_path = workspace.path().join("report.json");
    let report_arg = report_path.to_string_lossy().into_owned();
    let url = format!("http://{address}/private-path/health?token=private");
    let output = nyra(&[
        "workflow",
        "run",
        "web-unreachable",
        "--target",
        &url,
        "--timeout",
        "5",
        "--output",
        "json",
        "--report-json",
        &report_arg,
    ]);
    server.join().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "JSON report exported."
    );
    let body = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["tool_id"], "workflow.web_diagnostics");
    assert_eq!(json["finding"], "unhealthy");
    assert_eq!(json["data"]["steps"][0]["status"], "skipped");
    assert_eq!(json["data"]["steps"][1]["finding"], "open");
    assert_eq!(json["data"]["steps"][2]["status"], "skipped");
    assert_eq!(json["data"]["steps"][3]["finding"], "unhealthy");
    assert_eq!(json["data"]["steps"][3]["data"]["status_code"], 503);
    assert_eq!(json["data"]["selected_address"], address.to_string());
    assert_eq!(
        json["target"],
        format!("http://{address}/private-path/health")
    );
    assert!(!body.contains("token=private"));

    let report_contents = fs::read_to_string(report_path).unwrap();
    let report: Value = serde_json::from_str(&report_contents).unwrap();
    assert_eq!(report["report_schema_version"], 1);
    assert_eq!(report["tool_id"], "workflow.web_diagnostics");
    assert_eq!(report["execution_status"], "completed");
    assert_eq!(report["finding"], "unhealthy");
    assert_eq!(report["target"]["display"], "[redacted]");
    assert_eq!(report["target"]["redacted"], true);
    assert_eq!(report["steps"][3]["status_code"], 503);
    assert!(report["exported_at_utc"].as_str().is_some());
    assert!(report.get("observer").is_none());
    assert!(report.get("data").is_none());
    assert!(report["steps"][3].get("data").is_none());
    assert!(report_contents.len() <= 64 * 1024);
    assert!(!report_contents.contains("127.0.0.1"));
    assert!(!report_contents.contains("private-path"));
    assert!(!report_contents.contains("private"));
    assert!(!report_contents.contains("private-header"));
}

#[test]
fn workflow_json_report_refuses_to_replace_an_existing_file() {
    let workspace = tempfile::tempdir().unwrap();
    let report_path = workspace.path().join("report.json");
    fs::write(&report_path, "keep existing report").unwrap();
    let report_arg = report_path.to_string_lossy().into_owned();
    let output = nyra(&[
        "workflow",
        "run",
        "web-unreachable",
        "--target",
        "http://127.0.0.1:1/",
        "--report-json",
        &report_arg,
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(
        fs::read_to_string(report_path).unwrap(),
        "keep existing report"
    );
}

#[test]
fn invalid_workflow_input_does_not_create_a_json_report() {
    let workspace = tempfile::tempdir().unwrap();
    let report_path = workspace.path().join("report.json");
    let report_arg = report_path.to_string_lossy().into_owned();
    let output = nyra(&[
        "workflow",
        "run",
        "web-unreachable",
        "--target",
        "ftp://example.com/",
        "--report-json",
        &report_arg,
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!report_path.exists());
}

#[test]
fn workflow_html_and_csv_reports_are_redacted_before_writing() {
    let workspace = tempfile::tempdir().unwrap();
    let target = "http://127.0.0.1:1/private-path?token=private";

    for (format, extension, success_message) in [
        ("--report-html", "html", "HTML report exported."),
        ("--report-csv", "csv", "CSV report exported."),
    ] {
        let report_path = workspace.path().join(format!("report.{extension}"));
        let report_arg = report_path.to_string_lossy().into_owned();
        let output = nyra(&[
            "workflow",
            "run",
            "web-unreachable",
            "--target",
            target,
            "--timeout",
            "2",
            format,
            &report_arg,
        ]);

        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            success_message
        );
        let report = fs::read_to_string(report_path).unwrap();
        assert!(report.len() <= 64 * 1024);
        assert!(report.contains("workflow.web_diagnostics"));
        assert!(report.contains("[redacted]"));
        assert!(!report.contains("127.0.0.1"));
        assert!(!report.contains("private-path"));
        assert!(!report.contains("token=private"));
    }
}

#[test]
fn workflow_report_formats_are_mutually_exclusive() {
    let workspace = tempfile::tempdir().unwrap();
    let json_path = workspace.path().join("report.json");
    let html_path = workspace.path().join("report.html");
    let json_arg = json_path.to_string_lossy().into_owned();
    let html_arg = html_path.to_string_lossy().into_owned();
    let output = nyra(&[
        "workflow",
        "run",
        "web-unreachable",
        "--target",
        "http://127.0.0.1:1/",
        "--report-json",
        &json_arg,
        "--report-html",
        &html_arg,
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
    assert!(!json_path.exists());
    assert!(!html_path.exists());
}

#[test]
fn direct_tool_report_formats_redact_local_identity_and_keep_json_stdout_separate() {
    let workspace = tempfile::tempdir().unwrap();
    let input_path = workspace.path().join("nyra-sensitive-artifact-path.bin");
    fs::write(&input_path, b"abc").unwrap();
    let input_arg = input_path.to_string_lossy().into_owned();

    for (option, extension, success_message) in [
        ("--report-json", "json", "JSON report exported."),
        ("--report-html", "html", "HTML report exported."),
        ("--report-csv", "csv", "CSV report exported."),
    ] {
        let report_path = workspace.path().join(format!("tool-report.{extension}"));
        let report_arg = report_path.to_string_lossy().into_owned();
        let output = nyra(&[
            "file",
            "sha256",
            &input_arg,
            "--output",
            "json",
            option,
            &report_arg,
        ]);

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            success_message
        );
        let raw_stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(raw_stdout["tool_id"], "file.sha256");
        assert_eq!(raw_stdout["target"], input_arg);
        assert_eq!(raw_stdout["data"]["path"], input_arg);
        assert_eq!(
            raw_stdout["data"]["sha256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let report_contents = fs::read_to_string(&report_path).unwrap();

        match extension {
            "json" => {
                let report: Value = serde_json::from_str(&report_contents).unwrap();
                assert_eq!(report["report_schema_version"], 1);
                assert_eq!(report["tool_id"], "file.sha256");
                assert_eq!(report["target"]["display"], "[redacted]");
                assert_eq!(report["target"]["redacted"], true);
                assert!(report.get("observer").is_none());
                assert!(report.get("data").is_none());
            }
            "html" | "csv" => {
                assert!(report_contents.contains("file.sha256"));
                assert!(report_contents.contains("[redacted]"));
            }
            _ => unreachable!(),
        }
        assert!(!report_contents.contains("nyra-sensitive-artifact-path.bin"));
        assert!(
            !report_contents
                .contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
}

#[test]
fn direct_tool_report_formats_are_mutually_exclusive() {
    let workspace = tempfile::tempdir().unwrap();
    let json_path = workspace.path().join("report.json");
    let csv_path = workspace.path().join("report.csv");
    let json_arg = json_path.to_string_lossy().into_owned();
    let csv_arg = csv_path.to_string_lossy().into_owned();

    let output = nyra(&[
        "sys",
        "info",
        "--report-json",
        &json_arg,
        "--report-csv",
        &csv_arg,
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
    assert!(!json_path.exists());
    assert!(!csv_path.exists());
}

#[test]
fn direct_tool_report_refuses_to_replace_an_existing_file() {
    let workspace = tempfile::tempdir().unwrap();
    let report_path = workspace.path().join("existing.json");
    fs::write(&report_path, "keep this report").unwrap();
    let report_arg = report_path.to_string_lossy().into_owned();

    let output = nyra(&["sys", "info", "--report-json", &report_arg]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(fs::read_to_string(report_path).unwrap(), "keep this report");
}

#[test]
fn workflow_file_cli_validates_definitions_without_running_them() {
    let workspace = tempfile::tempdir().unwrap();
    let valid_path = workspace.path().join("valid-workflow.toml");
    fs::write(
        &valid_path,
        r#"
id = "local-health"
version = 1
description = "Collect local system health measurements"

[[steps]]
id = "info"
tool = "system.info"
"#,
    )
    .unwrap();
    let valid_arg = valid_path.to_string_lossy().into_owned();

    let valid = nyra(&["workflow", "validate", &valid_arg]);
    assert_eq!(valid.status.code(), Some(0));
    assert!(valid.stderr.is_empty());
    assert!(
        String::from_utf8_lossy(&valid.stdout)
            .to_ascii_lowercase()
            .contains("valid")
    );

    let invalid_path = workspace.path().join("invalid-workflow.toml");
    fs::write(
        &invalid_path,
        r#"
id = "private-workflow-marker"
version = 999
description = "Invalid version"

[[steps]]
id = "info"
tool = "system.info"
"#,
    )
    .unwrap();
    let invalid_arg = invalid_path.to_string_lossy().into_owned();

    let invalid = nyra(&["workflow", "validate", &invalid_arg]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let error = String::from_utf8_lossy(&invalid.stderr);
    assert!(error.to_ascii_lowercase().contains("invalid"));
    assert!(!error.contains("private-workflow-marker"));
}

#[test]
fn custom_workflow_reports_redact_identifiers_and_preserve_stdout() {
    let workspace = tempfile::tempdir().unwrap();
    let definition = workspace.path().join("workflow.toml");
    fs::write(
        &definition,
        r#"
id = "private-workflow"
version = 1
description = "Read local system information"

[[steps]]
id = "private-step"
tool = "system.info"
"#,
    )
    .unwrap();
    let definition = definition.to_str().unwrap();

    for (flag, filename) in [
        ("--report-json", "report.json"),
        ("--report-html", "report.html"),
        ("--report-csv", "report.csv"),
    ] {
        let path = workspace.path().join(filename);
        let path = path.to_str().unwrap();
        let args = [
            "workflow", "run-file", definition, "--output", "json", flag, path,
        ];
        let output = nyra(&args);
        assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
        let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(stdout["workflow_id"], "private-workflow");
        let report = fs::read_to_string(path).unwrap();
        assert!(report.contains("[redacted]"));
        assert!(!report.contains("private-workflow"));
        assert!(!report.contains("private-step"));
        if flag == "--report-json" {
            let json: Value = serde_json::from_str(&report).unwrap();
            assert_eq!(json["report_schema_version"], 1);
            assert_eq!(json["steps"][0]["step_index"], 1);
            assert_eq!(json["steps"][0]["status"], "completed");
        }

        let repeated = nyra(&args);
        assert_eq!(repeated.status.code(), Some(2));
        assert!(repeated.stdout.is_empty());
        assert_eq!(fs::read_to_string(path).unwrap(), report);
    }

    let json = workspace.path().join("conflict.json");
    let csv = workspace.path().join("conflict.csv");
    let conflict = nyra(&[
        "workflow",
        "run-file",
        definition,
        "--report-json",
        json.to_str().unwrap(),
        "--report-csv",
        csv.to_str().unwrap(),
    ]);
    assert_eq!(conflict.status.code(), Some(2));
    assert!(!json.exists());
    assert!(!csv.exists());
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn workflow_file_cli_rejects_a_fifo_without_blocking() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt, time::Duration};

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "nyra-workflow-fifo-{}-{unique}",
        std::process::id()
    ));
    let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let _cleanup = RemoveFile(path.clone());
    let path_text = path.to_string_lossy().into_owned();

    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_nyra"));
    command
        .args(["workflow", "validate", &path_text])
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(2), command.output())
        .await
        .expect("workflow file inspection must not block on a FIFO")
        .expect("run nyra");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unavailable"));
}

#[test]
fn workflow_file_cli_runs_local_tools_and_a_status_condition() {
    let workspace = tempfile::tempdir().unwrap();
    let workflow_path = workspace.path().join("local-health.toml");
    fs::write(
        &workflow_path,
        r#"
id = "local-health"
version = 1
description = "Collect local system health measurements"

[[steps]]
id = "info"
tool = "system.info"

[[steps]]
id = "cpu"
tool = "system.cpu"
run_if = { operator = "all", predicates = [{ kind = "step_status", step_id = "info", equals = "completed" }] }
"#,
    )
    .unwrap();
    let path_arg = workflow_path.to_string_lossy().into_owned();

    let output = nyra(&["workflow", "run-file", &path_arg, "--output", "json"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["workflow_id"], "local-health");
    assert_eq!(result["execution_status"], "completed");
    let steps = result["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["id"], "info");
    assert_eq!(steps[0]["status"], "completed");
    assert_eq!(steps[1]["id"], "cpu");
    assert_eq!(steps[1]["status"], "completed");
}

#[test]
fn workflow_file_text_output_explains_skipped_step_reasons() {
    let workspace = tempfile::tempdir().unwrap();
    let workflow_path = workspace.path().join("conditional.toml");
    fs::write(
        &workflow_path,
        r#"
id = "conditional-check"
version = 1
description = "Skip CPU collection unless system info fails"

[[steps]]
id = "info"
tool = "system.info"

[[steps]]
id = "cpu"
tool = "system.cpu"
run_if = { operator = "all", predicates = [{ kind = "step_status", step_id = "info", equals = "failed" }] }
"#,
    )
    .unwrap();
    let path_arg = workflow_path.to_string_lossy().into_owned();

    let output = nyra(&["workflow", "run-file", &path_arg, "--output", "text"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cpu: Skipped"));
    assert!(stdout.contains("Issue: condition not met"));
}

#[test]
fn workflow_file_cli_runs_typed_inputs_through_the_shared_tool_executor() {
    let workspace = tempfile::tempdir().unwrap();
    let input_path = workspace.path().join("artifact.bin");
    fs::write(&input_path, b"abc").unwrap();
    let workflow_path = workspace.path().join("file-integrity.toml");
    fs::write(
        &workflow_path,
        r#"
id = "file-integrity"
version = 1
description = "Compute a file digest"

[[inputs]]
name = "path"
value_type = "file_path"

[[steps]]
id = "digest"
tool = "file.sha256"

[[steps.parameters]]
parameter = "path"
source = { kind = "input", name = "path" }
"#,
    )
    .unwrap();
    let workflow_arg = workflow_path.to_string_lossy().into_owned();
    let input_arg = format!("path={}", input_path.display());

    let output = nyra(&[
        "workflow",
        "run-file",
        &workflow_arg,
        "--input",
        &input_arg,
        "--output",
        "json",
    ]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["execution_status"], "completed");
    assert_eq!(result["steps"][0]["status"], "completed");
    assert_eq!(result["steps"][0]["finding"], "computed");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("artifact.bin"));
}

#[test]
fn workflow_file_cli_does_not_echo_invalid_input_values() {
    let workspace = tempfile::tempdir().unwrap();
    let workflow_path = workspace.path().join("http-check.toml");
    fs::write(
        &workflow_path,
        r#"
id = "http-check"
version = 1
description = "Check an authorized web endpoint"

[[inputs]]
name = "target"
value_type = "url"

[[steps]]
id = "http"
tool = "web.http"

[[steps.parameters]]
parameter = "url"
source = { kind = "input", name = "target" }
"#,
    )
    .unwrap();
    let path_arg = workflow_path.to_string_lossy().into_owned();
    let untrusted_value = "https://user:private-token@example.com/";

    let output = nyra(&[
        "workflow",
        "run-file",
        &path_arg,
        "--input",
        &format!("target={untrusted_value}"),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-token"));
}

#[cfg(target_os = "linux")]
#[test]
fn web_diagnostics_verifies_trusted_local_https_before_http() {
    use rustls::{
        ServerConfig,
        pki_types::{CertificateDer, PrivateKeyDer},
    };
    use std::{net::TcpListener, thread, time::Duration};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let certificate =
        CertificateDer::from(include_bytes!("fixtures/tls/localhost-cert.der").to_vec());
    let private_key =
        PrivateKeyDer::try_from(include_bytes!("fixtures/tls/localhost-key.der").to_vec()).unwrap();
    let provider = std::sync::Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let server_config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![certificate], private_key)
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(server_config));
    let server = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            for request_index in 0..3 {
                let deadline = Duration::from_secs(10);
                let (stream, _) = tokio::time::timeout(deadline, listener.accept())
                    .await
                    .expect("workflow should connect to the local endpoint")
                    .unwrap();
                if request_index == 0 {
                    drop(stream);
                    continue;
                }
                let stream = tokio::time::timeout(deadline, acceptor.accept(stream))
                    .await
                    .expect("TLS handshake should complete")
                    .unwrap();
                if request_index == 1 {
                    drop(stream);
                    continue;
                }
                let mut stream = stream;
                let mut request = [0_u8; 2048];
                assert!(
                    tokio::time::timeout(deadline, stream.read(&mut request))
                        .await
                        .expect("HTTP request should arrive")
                        .unwrap()
                        > 0
                );
                stream
                    .write_all(
                        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await
                    .unwrap();
            }
        });
    });

    let trust_bundle =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls/localhost-ca.pem");
    let url = format!("https://localhost:{}/health?token=private", address.port());
    let output = Command::new(env!("CARGO_BIN_EXE_nyra"))
        .env("SSL_CERT_FILE", trust_bundle)
        .args([
            "workflow",
            "run",
            "web-unreachable",
            "--target",
            &url,
            "--timeout",
            "10",
            "--output",
            "json",
        ])
        .output()
        .expect("run nyra against local HTTPS fixture");
    server.join().unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["finding"], "healthy");
    assert_eq!(json["data"]["selected_address"], address.to_string());
    assert_eq!(json["data"]["steps"][0]["finding"], "available");
    assert_eq!(json["data"]["steps"][1]["finding"], "open");
    assert_eq!(json["data"]["steps"][2]["finding"], "available");
    assert_eq!(
        json["data"]["steps"][2]["data"]["trust_validation"],
        "system validation passed; inspection-only handshake"
    );
    assert_eq!(json["data"]["steps"][3]["finding"], "healthy");
    assert_eq!(json["data"]["steps"][3]["data"]["status_code"], 204);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private"));
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
    assert!(
        json["observer"]
            .as_str()
            .is_some_and(|name| !name.is_empty())
    );
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
    assert!(json["data"]["volumes"].is_array());
    if json["data"]["volumes"]
        .as_array()
        .is_some_and(|volumes| !volumes.is_empty())
    {
        assert_eq!(json["finding"], "measured");
        assert_eq!(json["data"]["availability"], "available");
    } else {
        assert_eq!(json["finding"], "inconclusive");
        assert_eq!(json["data"]["availability"], "unavailable");
    }
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

#[test]
fn workflow_file_cli_rejects_invalid_ip_input_before_running_a_step() {
    let workspace = tempfile::tempdir().unwrap();
    let workflow_path = workspace.path().join("reverse-dns.toml");
    fs::write(
        &workflow_path,
        r#"
id = "reverse-dns"
version = 1
description = "Look up a reverse DNS name"

[[inputs]]
name = "address"
value_type = "ip_address"

[[steps]]
id = "reverse"
tool = "network.reverse_dns"

[[steps.parameters]]
parameter = "address"
source = { kind = "input", name = "address" }
"#,
    )
    .unwrap();
    let workflow_arg = workflow_path.to_string_lossy().into_owned();
    let invalid_input = "address=private-marker";

    let output = nyra(&[
        "workflow",
        "run-file",
        &workflow_arg,
        "--input",
        invalid_input,
        "--output",
        "json",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-marker"));
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn sha256_rejects_a_fifo_without_blocking() {
    use std::{
        ffi::CString,
        os::unix::ffi::OsStrExt,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("nyra-fifo-{}-{unique}", std::process::id()));
    let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let _cleanup = RemoveFile(path.clone());
    let path_text = path.to_string_lossy().into_owned();

    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_nyra"));
    command
        .args(["file", "sha256", &path_text, "--output", "json"])
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(2), command.output())
        .await
        .expect("SHA-256 on a FIFO must not block")
        .expect("run nyra");
    assert_eq!(output.status.code(), Some(3));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["error"], "file_open_failed");
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
