use crate::{Finding, RunError};
use std::{
    fmt,
    future::Future,
    io::{self, ErrorKind},
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::{
    net::{TcpStream, lookup_host},
    time::{Instant, timeout_at},
};

const MAX_ADDRESSES: usize = 8;

pub struct TcpTarget {
    host: String,
    port: u16,
    timeout: Duration,
}

impl TcpTarget {
    pub fn new(host: String, port: u16, seconds: u64) -> Result<Self, &'static str> {
        if host.parse::<IpAddr>().is_err() && !valid_hostname(&host) {
            return Err("Host must be an IPv4 address, IPv6 address, or DNS hostname");
        }
        crate::validate_numeric_input("network.tcp", "port", port.into())?;
        crate::validate_numeric_input("network.tcp", "timeout", seconds)?;
        Ok(Self {
            host,
            port,
            timeout: Duration::from_secs(seconds),
        })
    }
}

impl fmt::Display for TcpTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.contains(':') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

pub(crate) fn valid_hostname(host: &str) -> bool {
    let name = host.strip_suffix('.').unwrap_or(host);
    name.len() <= 253
        && name.bytes().any(|byte| byte.is_ascii_alphabetic())
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

pub(crate) async fn check(target: &TcpTarget) -> Result<Finding, RunError> {
    let deadline = Instant::now() + target.timeout;
    let mut addresses = Vec::new();
    if let Ok(ip) = target.host.parse::<IpAddr>() {
        addresses.push(SocketAddr::new(ip, target.port));
    } else {
        let resolved = timeout_at(deadline, lookup_host((target.host.as_str(), target.port)))
            .await
            .map_err(|_| RunError::DeadlineExceeded)?
            .map_err(|_| RunError::ResolutionFailed)?;
        for address in resolved {
            if !addresses.contains(&address) {
                addresses.push(address);
                if addresses.len() > MAX_ADDRESSES {
                    break;
                }
            }
        }
    }
    if addresses.is_empty() {
        return Err(RunError::ResolutionFailed);
    }
    let mut finding = None;
    let mut incomplete = addresses.len() > MAX_ADDRESSES;
    for address in addresses.into_iter().take(MAX_ADDRESSES) {
        if Instant::now() >= deadline {
            incomplete = true;
            break;
        }
        let current = match attempt(deadline, TcpStream::connect(address)).await {
            Some(Finding::Open) => return Ok(Finding::Open),
            Some(finding) => finding,
            None => {
                incomplete = true;
                continue;
            }
        };
        finding = Some(match finding {
            Some(previous) if previous != current => Finding::Inconclusive,
            _ => current,
        });
    }
    match finding {
        Some(_) if incomplete => Ok(Finding::Inconclusive),
        Some(finding) => Ok(finding),
        None if Instant::now() >= deadline => Err(RunError::DeadlineExceeded),
        None => Err(RunError::SocketFailed),
    }
}

async fn attempt<T>(
    deadline: Instant,
    connect: impl Future<Output = io::Result<T>>,
) -> Option<Finding> {
    match timeout_at(deadline, connect).await {
        Ok(Ok(_)) => Some(Finding::Open),
        Err(_) => Some(Finding::TimedOut),
        Ok(Err(error)) => match error.kind() {
            ErrorKind::ConnectionRefused => Some(Finding::Refused),
            ErrorKind::TimedOut => Some(Finding::TimedOut),
            ErrorKind::HostUnreachable | ErrorKind::NetworkUnreachable => {
                Some(Finding::Unreachable)
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn pending_connection_yields_a_timeout_finding() {
        let pending = std::future::pending::<io::Result<()>>();
        assert_eq!(
            attempt(Instant::now(), pending).await,
            Some(Finding::TimedOut)
        );
    }

    #[test]
    fn validates_host_port_and_timeout() {
        for host in ["127.0.0.1", "::1", "localhost", "web-1.example."] {
            assert!(TcpTarget::new(host.into(), 443, 5).is_ok(), "{host}");
        }
        for host in [
            "",
            "https://example.com",
            "name:443",
            "a..b",
            "999.0.0.1",
            "a\nb",
        ] {
            assert!(TcpTarget::new(host.into(), 443, 5).is_err(), "{host:?}");
        }
        assert!(TcpTarget::new("localhost".into(), 0, 5).is_err());
        assert!(TcpTarget::new("localhost".into(), 443, 0).is_err());
        assert!(TcpTarget::new("localhost".into(), 443, 61).is_err());
    }
}
