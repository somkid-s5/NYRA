use crate::{
    Finding, RunError, TlsTarget, WebDiagnosticsData, WebDiagnosticsTarget, WorkflowStep,
    WorkflowStepStatus, tcp,
};
use serde_json::json;
use std::{
    future::Future,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    time::Duration,
};
use tokio::time::{Instant, timeout_at};
use url::Host;

pub(crate) struct WorkflowExecution {
    pub finding: Option<Finding>,
    pub data: WebDiagnosticsData,
    pub error: Option<RunError>,
}

pub(crate) async fn web_diagnostics<C>(target: WebDiagnosticsTarget, cancel: C) -> WorkflowExecution
where
    C: Future<Output = Result<(), RunError>>,
{
    let mut cancel = Box::pin(cancel);
    let deadline = Instant::now() + Duration::from_secs(target.timeout_seconds);
    let hostname = match target.url.host() {
        Some(Host::Domain(domain)) => domain.to_owned(),
        Some(Host::Ipv4(address)) => address.to_string(),
        Some(Host::Ipv6(address)) => address.to_string(),
        None => String::new(),
    };
    let port = target
        .url
        .port_or_known_default()
        .unwrap_or(if target.url.scheme() == "https" {
            443
        } else {
            80
        });
    let mut steps = Vec::with_capacity(4);
    let mut selected_address = None;
    let mut finding;

    let resolved_addresses = match hostname.parse::<IpAddr>() {
        Ok(address) => {
            steps.push(step(
                "dns",
                WorkflowStepStatus::Skipped,
                None,
                0,
                Some("Target is an IP literal; DNS resolution is not needed.".into()),
                None,
            ));
            vec![SocketAddr::new(address, port)]
        }
        Err(_) => {
            let stage_started = Instant::now();
            match run_stage(
                &mut cancel,
                deadline,
                tcp::resolve_addresses(&hostname, port, deadline),
            )
            .await
            {
                Ok(addresses) => {
                    let answers = addresses
                        .iter()
                        .map(|address| address.ip().to_string())
                        .collect::<Vec<_>>();
                    steps.push(step(
                        "dns",
                        WorkflowStepStatus::Completed,
                        Some(Finding::Available),
                        stage_started.elapsed().as_millis(),
                        Some("System name resolution returned addresses.".into()),
                        Some(json!({ "hostname": hostname.clone(), "addresses": answers })),
                    ));
                    addresses
                }
                Err(error) => {
                    let (stage_status, stage_finding, run_error, reason) = match error {
                        RunError::Cancelled => (
                            WorkflowStepStatus::Cancelled,
                            None,
                            Some(error),
                            "Workflow cancelled during name resolution.",
                        ),
                        RunError::DeadlineExceeded => (
                            WorkflowStepStatus::Completed,
                            Some(Finding::TimedOut),
                            None,
                            "Name resolution reached the workflow timeout.",
                        ),
                        RunError::ResolutionFailed => (
                            WorkflowStepStatus::Completed,
                            Some(Finding::Unreachable),
                            None,
                            "The system resolver returned no usable address.",
                        ),
                        _ => (
                            WorkflowStepStatus::Failed,
                            None,
                            Some(error),
                            "Name resolution could not run.",
                        ),
                    };
                    steps.push(step(
                        "dns",
                        stage_status,
                        stage_finding,
                        stage_started.elapsed().as_millis(),
                        Some(reason.into()),
                        None,
                    ));
                    skip_remaining(&mut steps, &["tcp", "tls", "http"], reason);
                    let result_finding = stage_finding;
                    return finish(
                        target,
                        hostname,
                        port,
                        selected_address,
                        steps,
                        result_finding,
                        run_error,
                    );
                }
            }
        }
    };

    let stage_started = Instant::now();
    match run_stage(
        &mut cancel,
        deadline,
        tcp::check_addresses_with_peer(
            &resolved_addresses,
            Duration::from_secs(target.timeout_seconds),
        ),
    )
    .await
    {
        Ok((tcp_finding, peer)) => {
            finding = Some(tcp_finding);
            selected_address = peer;
            steps.push(step(
                "tcp",
                WorkflowStepStatus::Completed,
                Some(tcp_finding),
                stage_started.elapsed().as_millis(),
                peer.is_none()
                    .then(|| "No destination accepted the TCP connection.".into()),
                peer.map(|address| json!({ "peer_address": address.to_string() })),
            ));
            if tcp_finding != Finding::Open {
                let reason = "TCP did not accept a connection; later protocol checks were skipped.";
                skip_remaining(&mut steps, &["tls", "http"], reason);
                return finish(
                    target,
                    hostname,
                    port,
                    selected_address,
                    steps,
                    finding,
                    None,
                );
            }
        }
        Err(error) => {
            let (stage_status, stage_finding, run_error, reason) = handle_stage_error(
                error,
                "TCP",
                Some(Finding::TimedOut),
                "TCP check failed to execute.",
            );
            finding = stage_finding;
            steps.push(step(
                "tcp",
                stage_status,
                stage_finding,
                stage_started.elapsed().as_millis(),
                Some(reason.to_owned()),
                None,
            ));
            skip_remaining(&mut steps, &["tls", "http"], reason);
            return finish(
                target,
                hostname,
                port,
                selected_address,
                steps,
                finding,
                run_error,
            );
        }
    }

    let Some(peer) = selected_address else {
        return finish(
            target,
            hostname,
            port,
            selected_address,
            steps,
            Some(Finding::Inconclusive),
            None,
        );
    };

    if target.url.scheme() == "https" {
        let tls_target = match TlsTarget::new(hostname.clone(), port, target.timeout_seconds) {
            Ok(target) => target,
            Err(_) => {
                skip_remaining(
                    &mut steps,
                    &["tls", "http"],
                    "TLS input could not be prepared.",
                );
                return finish(
                    target,
                    hostname,
                    port,
                    selected_address,
                    steps,
                    None,
                    Some(RunError::InvalidInput),
                );
            }
        };
        let stage_started = Instant::now();
        match run_stage(
            &mut cancel,
            deadline,
            super::network::tls_with_peer(tls_target, Some(peer)),
        )
        .await
        {
            Ok((trusted, data)) => {
                let tls_finding = if trusted {
                    Finding::Available
                } else {
                    Finding::Unhealthy
                };
                finding = Some(tls_finding);
                steps.push(step(
                    "tls",
                    WorkflowStepStatus::Completed,
                    Some(tls_finding),
                    stage_started.elapsed().as_millis(),
                    (!trusted).then(|| {
                        "System certificate and hostname validation failed; HTTP was withheld."
                            .into()
                    }),
                    serde_json::to_value(data).ok(),
                ));
                if !trusted {
                    skip_remaining(
                        &mut steps,
                        &["http"],
                        "HTTP was withheld because TLS verification failed.",
                    );
                    return finish(
                        target,
                        hostname,
                        port,
                        selected_address,
                        steps,
                        finding,
                        None,
                    );
                }
            }
            Err(error) => {
                let (stage_status, stage_finding, run_error, reason) = handle_stage_error(
                    error,
                    "TLS",
                    Some(Finding::Unhealthy),
                    "TLS inspection could not run.",
                );
                finding = stage_finding;
                steps.push(step(
                    "tls",
                    stage_status,
                    stage_finding,
                    stage_started.elapsed().as_millis(),
                    Some(reason.to_owned()),
                    None,
                ));
                skip_remaining(
                    &mut steps,
                    &["http"],
                    "HTTP was skipped because TLS did not verify.",
                );
                return finish(
                    target,
                    hostname,
                    port,
                    selected_address,
                    steps,
                    finding,
                    run_error,
                );
            }
        }
    } else {
        steps.push(step(
            "tls",
            WorkflowStepStatus::Skipped,
            None,
            0,
            Some("URL uses HTTP; TLS does not apply.".into()),
            None,
        ));
    }

    let stage_started = Instant::now();
    match run_stage(
        &mut cancel,
        deadline,
        super::network::http_with_peer(target.url.clone(), target.timeout_seconds, Some(peer)),
    )
    .await
    {
        Ok(data) => {
            let http_finding = if (200..300).contains(&data.status_code) {
                Finding::Healthy
            } else {
                Finding::Unhealthy
            };
            finding = Some(http_finding);
            let reason = if (300..400).contains(&data.status_code) {
                Some("Redirect destination reported; follow-up is explicit.".into())
            } else if http_finding == Finding::Unhealthy {
                Some("HTTP returned a non-2xx status.".into())
            } else {
                None
            };
            steps.push(step(
                "http",
                WorkflowStepStatus::Completed,
                Some(http_finding),
                stage_started.elapsed().as_millis(),
                reason,
                serde_json::to_value(data).ok(),
            ));
        }
        Err(error) => {
            let (stage_status, stage_finding, run_error, reason) = handle_stage_error(
                error,
                "HTTP",
                Some(Finding::Unreachable),
                "HTTP request could not run.",
            );
            finding = stage_finding;
            steps.push(step(
                "http",
                stage_status,
                stage_finding,
                stage_started.elapsed().as_millis(),
                Some(reason.to_owned()),
                None,
            ));
            return finish(
                target,
                hostname,
                port,
                selected_address,
                steps,
                finding,
                run_error,
            );
        }
    }

    finish(
        target,
        hostname,
        port,
        selected_address,
        steps,
        finding,
        None,
    )
}

async fn run_stage<C, F, T>(
    cancel: &mut Pin<Box<C>>,
    deadline: Instant,
    future: F,
) -> Result<T, RunError>
where
    C: Future<Output = Result<(), RunError>>,
    F: Future<Output = Result<T, RunError>>,
{
    tokio::select! {
        biased;
        signal = cancel.as_mut() => Err(signal.err().unwrap_or(RunError::Cancelled)),
        result = timeout_at(deadline, future) => match result {
            Ok(result) => result,
            Err(_) => Err(RunError::DeadlineExceeded),
        },
    }
}

fn handle_stage_error(
    error: RunError,
    stage: &str,
    timeout_finding: Option<Finding>,
    fallback: &'static str,
) -> (
    WorkflowStepStatus,
    Option<Finding>,
    Option<RunError>,
    &'static str,
) {
    match error {
        RunError::Cancelled => (
            WorkflowStepStatus::Cancelled,
            None,
            Some(error),
            "Workflow cancelled; completed observations are preserved.",
        ),
        RunError::DeadlineExceeded => (
            WorkflowStepStatus::Completed,
            timeout_finding,
            None,
            "The workflow timeout expired during this step.",
        ),
        RunError::ResolutionFailed if stage == "DNS" => (
            WorkflowStepStatus::Completed,
            Some(Finding::Unreachable),
            None,
            "The system resolver returned no usable address.",
        ),
        RunError::TlsHandshakeFailed | RunError::TlsCertificateInvalid if stage == "TLS" => (
            WorkflowStepStatus::Completed,
            Some(Finding::Unhealthy),
            None,
            "TLS handshake or certificate parsing failed.",
        ),
        RunError::NetworkRequestFailed if stage == "HTTP" => (
            WorkflowStepStatus::Completed,
            Some(Finding::Unreachable),
            None,
            "TCP connected, but the HTTP response could not be read.",
        ),
        _ => (WorkflowStepStatus::Failed, None, Some(error), fallback),
    }
}

fn step(
    id: &'static str,
    status: WorkflowStepStatus,
    finding: Option<Finding>,
    duration_ms: u128,
    reason: Option<String>,
    data: Option<serde_json::Value>,
) -> WorkflowStep {
    WorkflowStep {
        id,
        status,
        finding,
        duration_ms,
        reason,
        data,
    }
}

fn skip_remaining(steps: &mut Vec<WorkflowStep>, ids: &[&'static str], reason: &str) {
    steps.extend(ids.iter().map(|id| {
        step(
            id,
            WorkflowStepStatus::Skipped,
            None,
            0,
            Some(reason.to_owned()),
            None,
        )
    }));
}

fn finish(
    target: WebDiagnosticsTarget,
    hostname: String,
    port: u16,
    selected_address: Option<SocketAddr>,
    steps: Vec<WorkflowStep>,
    finding: Option<Finding>,
    error: Option<RunError>,
) -> WorkflowExecution {
    let summary = match (error, finding) {
        (Some(RunError::Cancelled), _) => {
            "Workflow cancelled; completed observations are preserved.".to_owned()
        }
        (Some(_), _) => "A workflow step could not execute; see the step details.".to_owned(),
        (None, Some(Finding::Healthy)) => {
            "All applicable protocol layers completed successfully.".to_owned()
        }
        (None, Some(value)) => steps
            .iter()
            .rev()
            .find(|step| {
                matches!(
                    step.finding,
                    Some(
                        Finding::Refused
                            | Finding::TimedOut
                            | Finding::Unreachable
                            | Finding::Inconclusive
                            | Finding::Unhealthy
                    )
                )
            })
            .map_or_else(
                || format!("The workflow completed with {value}."),
                |step| format!("The {} step reported {value}.", step.id),
            ),
        (None, None) => "The workflow did not produce a final protocol finding.".to_owned(),
    };
    let blocking_step = steps.iter().rev().find(|step| {
        matches!(
            step.finding,
            Some(
                Finding::Refused
                    | Finding::TimedOut
                    | Finding::Unreachable
                    | Finding::Inconclusive
                    | Finding::Unhealthy
            )
        )
    });
    let next_checks = match (finding, blocking_step.map(|step| step.id)) {
        (Some(Finding::Healthy), _) => Vec::new(),
        (Some(Finding::TimedOut), _) => vec![
            "Compare from another authorized observer or repeat with a longer timeout.".into(),
        ],
        (Some(Finding::Unreachable), Some("dns")) => vec![
            "Verify the hostname and system resolver configuration; this finding is limited to name resolution.".into(),
        ],
        (Some(Finding::Unreachable), Some("http")) => vec![
            "Check whether the selected peer is accepting HTTP after its TCP connection succeeded.".into(),
        ],
        (Some(Finding::Unreachable), _) => vec![
            "Check routing and firewall policy from this observer.".into(),
        ],
        (Some(Finding::Refused), _) => vec![
            "Check whether the service is listening on the selected peer and port.".into(),
            "Review firewall rules between this observer and the peer.".into(),
        ],
        (Some(Finding::Unhealthy), Some("tls")) => vec![
            "Review certificate validity, hostname coverage, and the system trust chain.".into(),
            "HTTP was withheld because certificate verification failed.".into(),
        ],
        (Some(Finding::Unhealthy), Some("http")) => vec![
            "Inspect the reported HTTP status and selected headers.".into(),
            "Follow a reported redirect only as a separate explicit check.".into(),
        ],
        _ => vec!["Review the completed step findings and their reasons.".into()],
    };
    WorkflowExecution {
        finding,
        data: WebDiagnosticsData {
            url: super::network::safe_url(&target.url),
            hostname,
            port,
            selected_address: selected_address.map(|address| address.to_string()),
            steps,
            summary,
            next_checks,
        },
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test(flavor = "current_thread")]
    async fn hostname_flow_resolves_and_carries_the_selected_peer_to_http() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (first, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .expect("TCP diagnostic should connect")
                .unwrap();
            drop(first);
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .expect("HTTP check should reuse the selected peer")
                .unwrap();
            let mut request = [0_u8; 2048];
            assert!(stream.read(&mut request).await.unwrap() > 0);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });

        let target =
            WebDiagnosticsTarget::new(format!("http://localhost:{}/health", address.port()), 5)
                .unwrap();
        let result = web_diagnostics(target, std::future::pending::<Result<(), RunError>>()).await;
        server.await.unwrap();

        let expected_address = address.to_string();
        let expected_ip = address.ip().to_string();
        assert_eq!(result.finding, Some(Finding::Healthy));
        assert!(result.error.is_none());
        assert_eq!(
            result.data.selected_address.as_deref(),
            Some(expected_address.as_str())
        );
        assert!(matches!(
            result.data.steps[0].status,
            WorkflowStepStatus::Completed
        ));
        assert_eq!(result.data.steps[0].finding, Some(Finding::Available));
        let resolved = result.data.steps[0].data.as_ref().unwrap()["addresses"]
            .as_array()
            .unwrap();
        assert!(
            resolved
                .iter()
                .any(|value| value.as_str() == Some(expected_ip.as_str()))
        );
        assert_eq!(result.data.steps[1].finding, Some(Finding::Open));
        assert!(matches!(
            result.data.steps[2].status,
            WorkflowStepStatus::Skipped
        ));
        assert_eq!(result.data.steps[3].finding, Some(Finding::Healthy));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn failed_https_validation_withholds_the_http_request() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (first, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .expect("TCP diagnostic should connect")
                .unwrap();
            drop(first);
            let (second, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .expect("TLS inspection should connect")
                .unwrap();
            drop(second);
        });

        let target = WebDiagnosticsTarget::new(format!("https://{address}/health"), 5).unwrap();
        let result = web_diagnostics(target, std::future::pending::<Result<(), RunError>>()).await;
        server.await.unwrap();

        assert_eq!(result.finding, Some(Finding::Unhealthy));
        assert!(result.error.is_none());
        assert_eq!(result.data.steps[1].finding, Some(Finding::Open));
        assert_eq!(result.data.steps[2].finding, Some(Finding::Unhealthy));
        assert!(matches!(
            result.data.steps[3].status,
            WorkflowStepStatus::Skipped
        ));
        assert!(
            result.data.steps[3]
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("TLS did not verify"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancellation_preserves_prior_steps_and_skips_remaining_checks() {
        let target = WebDiagnosticsTarget::new("http://127.0.0.1:1/health".into(), 5).unwrap();
        let result = web_diagnostics(target, async { Err(RunError::Cancelled) }).await;

        assert!(result.finding.is_none());
        assert!(matches!(result.error, Some(RunError::Cancelled)));
        assert_eq!(result.data.steps.len(), 4);
        assert!(matches!(
            result.data.steps[0].status,
            WorkflowStepStatus::Skipped
        ));
        assert!(matches!(
            result.data.steps[1].status,
            WorkflowStepStatus::Cancelled
        ));
        assert!(matches!(
            result.data.steps[2].status,
            WorkflowStepStatus::Skipped
        ));
        assert!(matches!(
            result.data.steps[3].status,
            WorkflowStepStatus::Skipped
        ));
    }
}
