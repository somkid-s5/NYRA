use crate::{
    DnsData, DnsRecord, HttpData, InterfaceData, InterfaceEntry, PingData, PingTarget, RunError,
    TlsData, TlsTarget,
};
use hickory_resolver::{
    TokioResolver,
    proto::rr::{Name, RecordType},
};
use reqwest::{Client, Url, redirect::Policy};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use std::{
    ffi::OsString,
    fmt,
    net::IpAddr,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{net::TcpStream, process::Command, time::timeout_at};
use tokio_rustls::TlsConnector;
use x509_parser::{extensions::GeneralName, parse_x509_certificate};

const MAX_DNS_TYPES: usize = 5;
const MAX_DNS_ANSWERS: usize = 20;
const MAX_INTERFACES: usize = 64;

pub(crate) async fn dns(
    hostname: String,
    record_types: Vec<String>,
    timeout_seconds: u64,
) -> Result<(bool, DnsData), RunError> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_seconds);
    let mut builder = TokioResolver::builder_tokio().map_err(|_| RunError::ResolverUnavailable)?;
    builder.options_mut().timeout = Duration::from_secs(timeout_seconds);
    let resolver = builder.build().map_err(|_| RunError::ResolverUnavailable)?;
    let mut records = Vec::with_capacity(record_types.len());
    for record_type in record_types.into_iter().take(MAX_DNS_TYPES) {
        if tokio::time::Instant::now() >= deadline {
            return Err(RunError::DeadlineExceeded);
        }
        let kind = parse_record_type(&record_type).ok_or(RunError::InvalidInput)?;
        let lookup = timeout_at(deadline, resolver.lookup(hostname.as_str(), kind)).await;
        let answers = match lookup {
            Ok(Ok(lookup)) => lookup
                .answers()
                .iter()
                .take(MAX_DNS_ANSWERS)
                .map(|answer| answer.data.to_string())
                .collect(),
            Ok(Err(error)) if error.is_no_records_found() => Vec::new(),
            Ok(Err(_)) => return Err(RunError::ResolutionFailed),
            Err(_) => return Err(RunError::DeadlineExceeded),
        };
        records.push(DnsRecord {
            record_type,
            answers,
        });
    }
    let found = records.iter().any(|record| !record.answers.is_empty());
    Ok((found, DnsData { hostname, records }))
}

pub(crate) async fn reverse_dns(
    address: IpAddr,
    timeout_seconds: u64,
) -> Result<(bool, DnsData), RunError> {
    let mut builder = TokioResolver::builder_tokio().map_err(|_| RunError::ResolverUnavailable)?;
    builder.options_mut().timeout = Duration::from_secs(timeout_seconds);
    let resolver = builder.build().map_err(|_| RunError::ResolverUnavailable)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_seconds);
    let lookup = timeout_at(deadline, resolver.reverse_lookup(Name::from(address)))
        .await
        .map_err(|_| RunError::DeadlineExceeded)?;
    let answers = match lookup {
        Ok(lookup) => lookup
            .answers()
            .iter()
            .take(MAX_DNS_ANSWERS)
            .map(|answer| answer.data.to_string())
            .collect::<Vec<_>>(),
        Err(error) if error.is_no_records_found() => Vec::new(),
        Err(_) => return Err(RunError::ResolutionFailed),
    };
    let found = !answers.is_empty();
    Ok((
        found,
        DnsData {
            hostname: address.to_string(),
            records: vec![DnsRecord {
                record_type: "PTR".into(),
                answers,
            }],
        },
    ))
}

pub(crate) async fn http(url: Url, timeout_seconds: u64) -> Result<HttpData, RunError> {
    let started = Instant::now();
    let response = request_client(timeout_seconds)?
        .get(url.clone())
        .send()
        .await
        .map_err(map_request_error)?;
    let status_code = response.status().as_u16();
    let redirect_location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| safe_redirect_location(response.url(), value));
    let headers = [
        "content-type",
        "content-length",
        "server",
        "cache-control",
        "last-modified",
    ]
    .iter()
    .filter_map(|name| {
        response
            .headers()
            .get(*name)
            .and_then(|value| value.to_str().ok())
            .map(|value| ((*name).to_owned(), value.to_owned()))
    })
    .collect();
    Ok(HttpData {
        url: safe_url(&url),
        status_code,
        elapsed_ms: started.elapsed().as_millis(),
        headers,
        redirect_location,
    })
}

pub(crate) async fn tls(target: TlsTarget) -> Result<(bool, TlsData), RunError> {
    let started = Instant::now();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(target.timeout_seconds);
    let socket = timeout_at(
        deadline,
        TcpStream::connect((target.host.as_str(), target.port)),
    )
    .await
    .map_err(|_| RunError::DeadlineExceeded)?
    .map_err(|_| RunError::SocketFailed)?;

    // This verifier records the normal system validation result, then permits only
    // the certificate-inspection handshake to finish. No application data is sent.
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let validator = rustls_platform_verifier::Verifier::new(Arc::clone(&provider))
        .map_err(|_| RunError::TlsVerifierUnavailable)?;
    let validation = Arc::new(Mutex::new(None));
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| RunError::TlsVerifierUnavailable)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(InspectingVerifier {
            validator: Arc::new(validator),
            validation: Arc::clone(&validation),
        }))
        .with_no_client_auth();
    let server_name =
        ServerName::try_from(target.host.clone()).map_err(|_| RunError::InvalidInput)?;
    let connector = TlsConnector::from(Arc::new(config));
    let stream = timeout_at(deadline, connector.connect(server_name, socket))
        .await
        .map_err(|_| RunError::DeadlineExceeded)?
        .map_err(|_| RunError::TlsHandshakeFailed)?;
    let certificate = stream
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certificates| certificates.first())
        .ok_or(RunError::TlsCertificateUnavailable)?;
    let (_, certificate) = parse_x509_certificate(certificate.as_ref())
        .map_err(|_| RunError::TlsCertificateInvalid)?;
    let certificate = &certificate.tbs_certificate;
    let trust_validated = validation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .unwrap_or(false);
    let subject_alt_names = certificate
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|extension| {
            extension
                .value
                .general_names
                .iter()
                .filter_map(|name| match name {
                    GeneralName::DNSName(name) => Some((*name).to_owned()),
                    GeneralName::IPAddress(address) if address.len() == 4 => {
                        Some(IpAddr::from(<[u8; 4]>::try_from(*address).ok()?).to_string())
                    }
                    GeneralName::IPAddress(address) if address.len() == 16 => {
                        Some(IpAddr::from(<[u8; 16]>::try_from(*address).ok()?).to_string())
                    }
                    _ => None,
                })
                .take(64)
                .collect()
        })
        .unwrap_or_default();
    let not_before_unix = certificate
        .validity()
        .not_before
        .to_datetime()
        .unix_timestamp();
    let not_after_unix = certificate
        .validity()
        .not_after
        .to_datetime()
        .unix_timestamp();
    Ok((
        trust_validated,
        TlsData {
            host: target.host,
            port: target.port,
            subject: certificate.subject().to_string(),
            issuer: certificate.issuer().to_string(),
            subject_alt_names,
            not_before_unix,
            not_after_unix,
            elapsed_ms: started.elapsed().as_millis(),
            trust_validation: if trust_validated {
                "system validation passed; inspection-only handshake".into()
            } else {
                "system validation failed; inspection-only handshake".into()
            },
        },
    ))
}

pub(crate) fn interfaces() -> Result<InterfaceData, RunError> {
    let entries = get_if_addrs::get_if_addrs().map_err(|_| RunError::InterfaceQueryFailed)?;
    Ok(InterfaceData {
        interfaces: entries
            .into_iter()
            .take(MAX_INTERFACES)
            .map(|entry| {
                let loopback = entry.is_loopback();
                let address = entry.ip().to_string();
                let netmask = match &entry.addr {
                    get_if_addrs::IfAddr::V4(address) => address.netmask.to_string(),
                    get_if_addrs::IfAddr::V6(address) => address.netmask.to_string(),
                };
                InterfaceEntry {
                    name: entry.name,
                    address,
                    netmask,
                    loopback,
                }
            })
            .collect(),
    })
}

pub(crate) async fn ping(target: PingTarget) -> Result<(bool, PingData), RunError> {
    let mut command = ping_command(&target);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(target.timeout_seconds);
    let output = timeout_at(deadline, command.output())
        .await
        .map_err(|_| RunError::DeadlineExceeded)?
        .map_err(|_| RunError::DependencyUnavailable)?;
    let output_text = String::from_utf8_lossy(&output.stdout);
    let reachable = ping_reachable(output.status.code(), output.stderr.is_empty(), &output_text)?;
    let latency_ms = parse_ping_latency(&output_text);
    Ok((
        reachable,
        PingData {
            host: target.host,
            count: target.count,
            reachable,
            latency_ms,
            note: "Uses the system ping utility; ICMP may be filtered or unavailable.".into(),
        },
    ))
}

fn parse_record_type(value: &str) -> Option<RecordType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "A" => Some(RecordType::A),
        "AAAA" => Some(RecordType::AAAA),
        "CNAME" => Some(RecordType::CNAME),
        "MX" => Some(RecordType::MX),
        "TXT" => Some(RecordType::TXT),
        _ => None,
    }
}

fn request_client(timeout_seconds: u64) -> Result<Client, RunError> {
    Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|_| RunError::HttpClientFailed)
}

struct InspectingVerifier {
    validator: Arc<dyn ServerCertVerifier>,
    validation: Arc<Mutex<Option<bool>>>,
}

impl fmt::Debug for InspectingVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InspectingVerifier")
    }
}

impl ServerCertVerifier for InspectingVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let result = self.validator.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        );
        *self
            .validation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(result.is_ok());
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.validator.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.validator.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.validator.supported_verify_schemes()
    }
}

fn map_request_error(error: reqwest::Error) -> RunError {
    if error.is_timeout() {
        RunError::DeadlineExceeded
    } else {
        RunError::NetworkRequestFailed
    }
}

pub(crate) fn safe_url(url: &Url) -> String {
    let mut safe = url.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    let _ = safe.set_username("");
    let _ = safe.set_password(None);
    safe.to_string()
}

fn safe_redirect_location(base: &Url, location: &str) -> Option<String> {
    let destination = base.join(location).ok()?;
    if !matches!(destination.scheme(), "http" | "https") {
        return None;
    }
    Some(safe_url(&destination))
}

#[cfg(windows)]
fn ping_command(target: &PingTarget) -> Command {
    let mut command = Command::new("ping");
    command.args(ping_arguments(target));
    command.kill_on_drop(true).stdin(Stdio::null());
    command
}

#[cfg(windows)]
fn ping_arguments(target: &PingTarget) -> Vec<OsString> {
    vec![
        OsString::from("/n"),
        OsString::from(target.count.to_string()),
        OsString::from("/w"),
        OsString::from((target.timeout_seconds * 1000).to_string()),
        OsString::from(target.host.as_str()),
    ]
}

#[cfg(target_os = "linux")]
fn ping_command(target: &PingTarget) -> Command {
    let mut command = Command::new("ping");
    command.args(ping_arguments(target));
    command.kill_on_drop(true).stdin(Stdio::null());
    command
}

#[cfg(target_os = "linux")]
fn ping_arguments(target: &PingTarget) -> Vec<OsString> {
    vec![
        OsString::from("-c"),
        OsString::from(target.count.to_string()),
        OsString::from("-W"),
        OsString::from(target.timeout_seconds.to_string()),
        OsString::from("--"),
        OsString::from(target.host.as_str()),
    ]
}

#[cfg(target_os = "linux")]
fn ping_reachable(
    exit_code: Option<i32>,
    stderr_empty: bool,
    _stdout: &str,
) -> Result<bool, RunError> {
    if !stderr_empty {
        return Err(RunError::PingFailed);
    }
    match exit_code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(RunError::PingFailed),
    }
}

#[cfg(windows)]
fn ping_reachable(
    exit_code: Option<i32>,
    stderr_empty: bool,
    stdout: &str,
) -> Result<bool, RunError> {
    if !stderr_empty || exit_code.is_none() {
        return Err(RunError::PingFailed);
    }
    if exit_code == Some(0) {
        return Ok(true);
    }
    let output = stdout.to_ascii_lowercase();
    if [
        "request timed out",
        "destination host unreachable",
        "destination net unreachable",
    ]
    .iter()
    .any(|message| output.contains(message))
    {
        Ok(false)
    } else {
        Err(RunError::PingFailed)
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn ping_reachable(
    _exit_code: Option<i32>,
    _stderr_empty: bool,
    _stdout: &str,
) -> Result<bool, RunError> {
    Err(RunError::DependencyUnavailable)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn ping_command(_target: &PingTarget) -> Command {
    Command::new("nyra-unsupported-ping-platform")
}

fn parse_ping_latency(output: &str) -> Option<f64> {
    let marker = output.find("time=").or_else(|| output.find("time<"))?;
    let value = output[marker + 5..]
        .split("ms")
        .next()?
        .trim()
        .trim_start_matches('<')
        .trim();
    value.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct RejectingVerifier;

    impl ServerCertVerifier for RejectingVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, TlsError> {
            Err(TlsError::General("test certificate is not trusted".into()))
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, TlsError> {
            Err(TlsError::General("test signature rejected".into()))
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, TlsError> {
            Err(TlsError::General("test signature rejected".into()))
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            Vec::new()
        }
    }

    #[test]
    fn accepts_only_the_advertised_dns_record_types() {
        assert_eq!(parse_record_type("mx"), Some(RecordType::MX));
        assert_eq!(parse_record_type("TXT"), Some(RecordType::TXT));
        assert_eq!(parse_record_type("SRV"), None);
    }

    #[test]
    fn extracts_ping_latency_when_the_platform_reports_it() {
        assert_eq!(
            parse_ping_latency("64 bytes from 127.0.0.1: time=0.12 ms"),
            Some(0.12)
        );
        assert_eq!(parse_ping_latency("Request timed out"), None);
    }

    #[test]
    fn redirect_destinations_are_resolved_and_sensitive_url_parts_are_removed() {
        let base = Url::parse("https://example.com/start?token=hidden").unwrap();
        assert_eq!(
            safe_redirect_location(&base, "/next?session=secret#section").as_deref(),
            Some("https://example.com/next")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_ping_adapter_builds_bounded_argument_list() {
        let target = PingTarget::new("example.com".into(), 4, 5).unwrap();
        assert_eq!(
            ping_arguments(&target),
            ["/n", "4", "/w", "5000", "example.com"]
                .map(OsString::from)
                .to_vec()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_ping_adapter_builds_bounded_argument_list() {
        let target = PingTarget::new("example.com".into(), 4, 5).unwrap();
        assert_eq!(
            ping_arguments(&target),
            ["-c", "4", "-W", "5", "--", "example.com"]
                .map(OsString::from)
                .to_vec()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_ping_no_reply_is_distinct_from_utility_failure() {
        assert_eq!(ping_reachable(Some(1), true, "0 received"), Ok(false));
        assert_eq!(
            ping_reachable(Some(2), false, "name or service not known"),
            Err(RunError::PingFailed)
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_ping_requires_process_output_for_a_negative_finding() {
        assert_eq!(
            ping_reachable(Some(1), true, "Request timed out."),
            Ok(false)
        );
        assert_eq!(
            ping_reachable(Some(1), true, "Ping request could not find host"),
            Err(RunError::PingFailed)
        );
        assert_eq!(
            ping_reachable(Some(1), false, "Request timed out."),
            Err(RunError::PingFailed)
        );
        assert_eq!(ping_reachable(None, true, ""), Err(RunError::PingFailed));
    }

    #[test]
    fn inspection_records_failed_validation_while_finishing_the_read_only_handshake() {
        let validation = Arc::new(Mutex::new(None));
        let verifier = InspectingVerifier {
            validator: Arc::new(RejectingVerifier),
            validation: Arc::clone(&validation),
        };
        let certificate = CertificateDer::from(vec![0_u8]);
        let server_name = ServerName::try_from("example.com").unwrap();
        let result =
            verifier.verify_server_cert(&certificate, &[], &server_name, &[], UnixTime::now());
        assert!(result.is_ok());
        assert_eq!(*validation.lock().unwrap(), Some(false));
    }
}
