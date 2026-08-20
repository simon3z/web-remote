//! Host auto-detection for the QR/URL (R28).
//!
//! Picks the IPv4 the phone can most likely reach: the local address of the
//! socket the OS would use for an outbound connection (the default-route
//! egress), falling back to `127.0.0.1`. Overridable with `--host`.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

/// Detect the host IP to print in the QR (R28).
pub fn detect_host_ip() -> Ipv4Addr {
    if let Some(ip) = socket_bound_ip() {
        return ip;
    }
    Ipv4Addr::LOCALHOST
}

/// The local IPv4 the kernel picks for an outbound UDP connection to a public
/// address. No packet is actually sent.
fn socket_bound_ip() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    match socket.local_addr().ok()? {
        SocketAddr::V4(v4) => Some(*v4.ip()),
        _ => None,
    }
}

/// Convenience: build a `SocketAddr` for the listener from host string + port.
pub fn bind_addr(host: &str, port: u16) -> SocketAddr {
    format!("{host}:{port}").parse().expect("valid socket addr")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_a_v4() {
        let ip = detect_host_ip();
        assert!(ip.is_loopback() || !ip.is_unspecified());
    }

    #[test]
    fn bind_addr_parses() {
        let a = bind_addr("0.0.0.0", 40443);
        assert_eq!(a, "0.0.0.0:40443".parse().unwrap());
    }
}
