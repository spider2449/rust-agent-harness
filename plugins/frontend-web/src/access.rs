//! Access control for the loopback server: a per-start secret token, the
//! `Host` header (against DNS rebinding) and the `Origin` header (against
//! cross-site requests).

use std::net::{IpAddr, SocketAddr};

/// A random 64-hex-digit secret (two UUIDv4, 244 random bits).
pub fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Compares in time independent of where the inputs differ.
pub fn token_matches(expected: &str, given: &str) -> bool {
    expected.len() == given.len()
        && expected
            .bytes()
            .zip(given.bytes())
            .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// The `Host` values a browser sends for this server.
pub fn allowed_hosts(address: SocketAddr) -> Vec<String> {
    let port = address.port();
    let mut hosts = vec![format!("localhost:{port}")];
    match address.ip() {
        IpAddr::V4(ip) => hosts.push(format!("{ip}:{port}")),
        IpAddr::V6(ip) => hosts.push(format!("[{ip}]:{port}")),
    }
    hosts
}

pub fn host_allowed(hosts: &[String], host: Option<&str>) -> bool {
    host.is_some_and(|host| {
        hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
    })
}

/// A missing `Origin` (non-browser clients) is allowed; a present one must
/// be this server.
pub fn origin_allowed(hosts: &[String], origin: Option<&str>) -> bool {
    origin.is_none_or(|origin| {
        hosts
            .iter()
            .any(|host| origin.eq_ignore_ascii_case(&format!("http://{host}")))
    })
}

/// The bind address must be a loopback address.
pub fn loopback(address: &str) -> Result<SocketAddr, String> {
    let parsed: SocketAddr = match address.strip_prefix("localhost:") {
        Some(port) => format!("127.0.0.1:{port}").parse(),
        None => address.parse(),
    }
    .map_err(|_| format!("`{address}` is not an address such as 127.0.0.1:8765"))?;
    if !parsed.ip().is_loopback() {
        return Err(format!(
            "`{address}` is not a loopback address; the web frontend only listens on this machine"
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_and_compared_exactly() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(token_matches(&a, &a.clone()));
        assert!(!token_matches(&a, &b));
        assert!(!token_matches(&a, &a[..63]));
        assert!(!token_matches(&a, ""));
    }

    #[test]
    fn only_this_server_is_an_allowed_host_or_origin() {
        let hosts = allowed_hosts("127.0.0.1:8765".parse().unwrap());
        for host in ["127.0.0.1:8765", "localhost:8765", "LOCALHOST:8765"] {
            assert!(host_allowed(&hosts, Some(host)), "{host}");
        }
        for host in [
            None,
            Some("evil.example:8765"),
            Some("127.0.0.1:9999"),
            Some("127.0.0.1"),
        ] {
            assert!(!host_allowed(&hosts, host), "{host:?}");
        }
        assert!(origin_allowed(&hosts, None));
        assert!(origin_allowed(&hosts, Some("http://localhost:8765")));
        for origin in ["http://evil.example", "https://127.0.0.1:8765", "null"] {
            assert!(!origin_allowed(&hosts, Some(origin)), "{origin}");
        }
        let v6 = allowed_hosts("[::1]:80".parse().unwrap());
        assert!(host_allowed(&v6, Some("[::1]:80")));
    }

    #[test]
    fn only_loopback_addresses_are_accepted() {
        for address in ["127.0.0.1:0", "[::1]:8765", "localhost:8765", "127.0.0.2:1"] {
            assert!(loopback(address).is_ok(), "{address}");
        }
        for address in [
            "0.0.0.0:8765",
            "192.168.1.2:80",
            "[::]:80",
            "example.com:80",
            "8765",
        ] {
            assert!(loopback(address).is_err(), "{address}");
        }
    }
}
