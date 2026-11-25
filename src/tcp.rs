use crate::{Finding, InputError, RunError};
use std::{
    fmt,
    io::ErrorKind,
    net::{IpAddr, SocketAddr},
    time::Duration,
};
#[cfg(test)]
use std::{future::Future, io};
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
    pub fn new(host: String, port: u16, seconds: u64) -> Result<Self, InputError> {
        if host.parse::<IpAddr>().is_err() && !valid_hostname(&host) {
            return Err(InputError::new(
                0,
                "Host must be an IPv4 address, IPv6 address, or DNS hostname",
            ));
        }
        crate::validate_numeric_input("network.tcp", "port", port.into())
            .map_err(|message| InputError::new(1, message))?;
        crate::validate_numeric_input("network.tcp", "timeout", seconds)
            .map_err(|message| InputError::new(2, message))?;
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
    check_with_peer(target).await.map(|(finding, _)| finding)
}

pub(crate) async fn check_with_peer(
    target: &TcpTarget,
) -> Result<(Finding, Option<SocketAddr>), RunError> {
    let deadline = Instant::now() + target.timeout;
    let addresses = resolve_addresses(&target.host, target.port, deadline).await?;
    check_resolved_addresses(&addresses, deadline).await
}

pub(crate) async fn check_addresses_with_peer(
    addresses: &[SocketAddr],
    timeout: Duration,
) -> Result<(Finding, Option<SocketAddr>), RunError> {
    check_resolved_addresses(addresses, Instant::now() + timeout).await
}

async fn check_resolved_addresses(
    addresses: &[SocketAddr],
    deadline: Instant,
) -> Result<(Finding, Option<SocketAddr>), RunError> {
    let mut finding = None;
    let mut incomplete = addresses.len() > MAX_ADDRESSES;
    for address in addresses.iter().copied().take(MAX_ADDRESSES) {
        if Instant::now() >= deadline {
            incomplete = true;
            break;
        }
        let current = match timeout_at(deadline, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => {
                let peer = stream.peer_addr().unwrap_or(address);
                return Ok((Finding::Open, Some(peer)));
            }
            Err(_) => Finding::TimedOut,
            Ok(Err(error)) => match error.kind() {
                ErrorKind::ConnectionRefused => Finding::Refused,
                ErrorKind::TimedOut => Finding::TimedOut,
                ErrorKind::HostUnreachable | ErrorKind::NetworkUnreachable => Finding::Unreachable,
                _ => Finding::Inconclusive,
            },
        };
        if current == Finding::Inconclusive {
            incomplete = true;
            continue;
        }
        finding = Some(match finding {
            Some(previous) if previous != current => Finding::Inconclusive,
            _ => current,
        });
    }
    match finding {
        Some(_) if incomplete => Ok((Finding::Inconclusive, None)),
        Some(finding) => Ok((finding, None)),
        None if Instant::now() >= deadline => Err(RunError::DeadlineExceeded),
        None => Err(RunError::SocketFailed),
    }
}

pub(crate) async fn resolve_addresses(
    host: &str,
    port: u16,
    deadline: Instant,
) -> Result<Vec<SocketAddr>, RunError> {
    let mut addresses = Vec::new();
    if let Ok(ip) = host.parse::<IpAddr>() {
        addresses.push(SocketAddr::new(ip, port));
    } else {
        let resolved = timeout_at(deadline, lookup_host((host, port)))
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
    Ok(addresses)
}

#[cfg(test)]
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
