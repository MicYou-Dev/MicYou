/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/MicYou-Dev/MicYou>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version, with the MicYou Plugin Exception.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 */

//! IPv6-aware socket binding helpers.
//!
//! This module is purely additive: the legacy IPv4 behaviour is preserved
//! byte-for-byte. For IPv4 literals and hostnames the helpers produce the
//! exact same `"host:port"` string the servers used to build inline, and the
//! IPv4 socket options (2 MiB UDP receive buffer, non-blocking mode) are
//! unchanged. Only textual IPv6 literals take the new code paths
//! (`[addr]:port` normalization, `AF_INET6` sockets, best-effort dual-stack
//! when binding to `::`).
//!
//! Kept free of any crate-internal dependencies so it can be compiled and
//! unit-tested in isolation from the Tauri application.

use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

/// Same backlog `tokio::net::TcpListener::bind` uses internally, so IPv6
/// listeners behave like the legacy IPv4 ones.
const TCP_BACKLOG: i32 = 1024;

/// Strips one layer of `[...]` brackets from an address literal.
/// IPv4 addresses and hostnames pass through unchanged.
pub fn strip_brackets(bind_address: &str) -> &str {
    bind_address
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(bind_address)
}

/// Returns true when `bind_address` is a textual IPv6 literal (bracketed or
/// not, with or without a `%zone` suffix). IPv4 literals and hostnames can
/// never contain `':'`, which makes this a safe discriminator.
pub fn is_ipv6_literal(bind_address: &str) -> bool {
    strip_brackets(bind_address).contains(':')
}

/// Returns true when `bind_address` is the unspecified IPv6 address
/// (`::` or `[::]`), i.e. "listen on every IPv6 interface".
pub fn is_unspecified_v6(bind_address: &str) -> bool {
    strip_brackets(bind_address)
        .parse::<Ipv6Addr>()
        .map(|ip| ip.is_unspecified())
        .unwrap_or(false)
}

/// Builds the `"host:port"` string for `ToSocketAddr` consumers.
///
/// IPv4 literals and hostnames are formatted exactly like the legacy
/// `format!("{}:{}", bind_address, port)`; IPv6 literals get the mandatory
/// brackets (`[fd00::1]:9123`). Already-bracketed input is preserved.
pub fn normalize_socket_addr(bind_address: &str, port: u16) -> String {
    if is_ipv6_literal(bind_address) {
        format!("[{}]:{}", strip_brackets(bind_address), port)
    } else {
        format!("{}:{}", bind_address, port)
    }
}

/// Binds the TCP control server.
///
/// IPv4 literals and hostnames go through `tokio::net::TcpListener::bind`
/// with the same string as before. IPv6 literals are bound through socket2
/// so that binding to `::` can request dual-stack (IPv4-mapped) operation on
/// a best-effort basis.
pub async fn bind_tcp_listener(
    bind_address: &str,
    port: u16,
) -> io::Result<tokio::net::TcpListener> {
    let normalized = normalize_socket_addr(bind_address, port);
    match normalized.parse::<SocketAddr>() {
        Ok(addr) if addr.ip().is_ipv6() => {
            let dual_stack = addr.ip().is_unspecified();
            tcp_listener_v6(addr, dual_stack)
        }
        // Legacy resolution/bind path for IPv4 literals and hostnames.
        _ => tokio::net::TcpListener::bind(normalized).await,
    }
}

/// Binds `[::]:port` as an IPv6-only listener (never accepts IPv4-mapped
/// connections). Used by the web server to *add* IPv6 reachability next to
/// its untouched `0.0.0.0` listener; `only_v6(true)` guarantees the two
/// sockets cannot conflict on dual-stack systems.
pub fn bind_tcp_listener_v6only(port: u16) -> io::Result<tokio::net::TcpListener> {
    let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port);
    // only_v6(true) MUST be applied before the bind: a socket that is still
    // dual-stack at bind time collides with an existing 0.0.0.0 listener on
    // the same port (EADDRINUSE on Linux), while a v6-only bind occupies a
    // disjoint address space and always coexists.
    let socket = new_v6_stream_socket(addr, Some(true))?;
    finish_tcp_listener(socket)
}

/// Returns true when `bind_address` is the legacy IPv4 auto-bind wildcard.
/// Servers then *additionally* bind IPv6-only companion sockets on the same
/// ports so IPv6 clients work out of the box, while the IPv4 sockets and
/// their behaviour stay exactly as before. Explicit address selections
/// (v4 or v6) never get a companion: the user picked one family.
pub fn wants_v6_companion(bind_address: &str) -> bool {
    bind_address == "0.0.0.0"
}

/// Human-readable hint appended to companion-bind failures, mapping the
/// common real-world causes to actionable text. The error itself is always
/// printed alongside; this only adds context.
pub fn companion_failure_hint(error: &io::Error) -> &'static str {
    // EAFNOSUPPORT = "address family not supported", i.e. no usable IPv6
    // stack. The friendly ErrorKind variant is still unstable in std, so
    // match the raw OS code per platform.
    #[cfg(target_os = "linux")]
    const EAFNOSUPPORT: i32 = 97;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const EAFNOSUPPORT: i32 = 47;
    #[cfg(target_os = "windows")]
    const EAFNOSUPPORT: i32 = 10047; // WSAEAFNOSUPPORT
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "windows"
    )))]
    const EAFNOSUPPORT: i32 = i32::MIN;
    if error.raw_os_error() == Some(EAFNOSUPPORT) {
        return " (the system has no usable IPv6 stack; IPv6 is likely disabled)";
    }
    match error.kind() {
        // On Windows, WSAEACCES on bind usually means the port falls into an
        // excluded port range reserved by Hyper-V/WSL (they frequently cover
        // ports in the 5000-7000 area for both TCP and UDP).
        io::ErrorKind::PermissionDenied => " (on Windows this is usually an excluded port range reserved by Hyper-V/WSL: check `netsh interface ipv6 show excludedportrange protocol=tcp` / `... protocol=udp`, then pick a port outside it)",
        io::ErrorKind::AddrInUse => {
            " (another socket already occupies this port on the IPv6 side)"
        }
        _ => "",
    }
}

/// Binds `[::]:port` as an IPv6-only UDP socket — the companion of an
/// untouched `0.0.0.0` audio socket. `only_v6(true)` guarantees the two
/// sockets coexist on the same port on every platform (a dual-stack `::`
/// would collide with the existing IPv4 wildcard bind).
pub fn bind_udp_socket_v6only(port: u16) -> io::Result<std::net::UdpSocket> {
    let socket = socket2::Socket::new(socket2::Domain::IPV6, socket2::Type::DGRAM, None)?;
    if let Err(e) = socket.set_recv_buffer_size(2 * 1024 * 1024) {
        log::warn!("Failed to set UDP receive buffer size to 2MB: {}", e);
    }
    socket.set_only_v6(true)?;
    socket.bind(&socket2::SockAddr::from(SocketAddr::new(
        IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        port,
    )))?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

/// Receives one datagram from the primary socket or, when present, from the
/// IPv6 companion socket — whichever arrives first. Each socket writes into
/// its own buffer (a single shared buffer cannot be mutably borrowed by two
/// concurrent `select!` branches); the returned flag tells which one won.
/// `UdpSocket::recv_from` is cancellation-safe, so `select!` dropping the
/// losing branch cannot lose a datagram. With no companion this is exactly
/// `primary.recv_from(buf)`.
pub async fn recv_from_either(
    primary: &tokio::net::UdpSocket,
    companion: Option<&tokio::net::UdpSocket>,
    buf: &mut [u8],
    buf_companion: &mut [u8],
) -> io::Result<(usize, SocketAddr, bool)> {
    match companion {
        Some(companion) => tokio::select! {
            result = primary.recv_from(buf) => result.map(|(len, addr)| (len, addr, false)),
            result = companion.recv_from(buf_companion) => {
                result.map(|(len, addr)| (len, addr, true))
            }
        },
        None => primary
            .recv_from(buf)
            .await
            .map(|(len, addr)| (len, addr, false)),
    }
}

fn tcp_listener_v6(addr: SocketAddr, dual_stack: bool) -> io::Result<tokio::net::TcpListener> {
    // `::` asks for dual-stack (only_v6=false, best effort); a specific IPv6
    // address leaves the platform default (the flag is irrelevant there).
    let socket = new_v6_stream_socket(addr, dual_stack.then_some(false))?;
    finish_tcp_listener(socket)
}

/// Creates a bound IPv6 stream socket. `only_v6`, when set, is applied
/// BEFORE the bind — the flag changes which address space the bind occupies,
/// so applying it afterwards would be too late to avoid collisions with an
/// existing IPv4 wildcard listener.
fn new_v6_stream_socket(addr: SocketAddr, only_v6: Option<bool>) -> io::Result<socket2::Socket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    // Match tokio's IPv4 bind behaviour: SO_REUSEADDR everywhere except Windows.
    #[cfg(not(windows))]
    socket.set_reuse_address(true)?;
    match only_v6 {
        Some(true) => socket.set_only_v6(true)?,
        // Best effort dual-stack: accept IPv4-mapped connections on `::` when
        // the OS allows it. Failing (or staying v6-only) never breaks IPv6.
        Some(false) => {
            let _ = socket.set_only_v6(false);
        }
        None => {}
    }
    socket.set_nonblocking(true)?;
    socket.bind(&socket2::SockAddr::from(addr))?;
    Ok(socket)
}

fn finish_tcp_listener(socket: socket2::Socket) -> io::Result<tokio::net::TcpListener> {
    socket.listen(TCP_BACKLOG)?;
    let std_listener: std::net::TcpListener = socket.into();
    tokio::net::TcpListener::from_std(std_listener)
}

/// Binds the UDP audio socket.
///
/// Mirrors the legacy inline logic (2 MiB receive buffer, non-blocking,
/// `UdpSocket::from_std` at the call site) and only diverges for IPv6:
/// the socket domain follows the bind address, and `::` requests dual-stack
/// on a best-effort basis.
pub fn bind_udp_socket(bind_address: &str, port: u16) -> io::Result<std::net::UdpSocket> {
    let normalized = normalize_socket_addr(bind_address, port);
    let addr: SocketAddr = normalized.parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid bind address '{}': {}", normalized, e),
        )
    })?;
    let domain = if addr.ip().is_ipv6() {
        socket2::Domain::IPV6
    } else {
        socket2::Domain::IPV4
    };
    let socket = socket2::Socket::new(domain, socket2::Type::DGRAM, None)?;
    if let Err(e) = socket.set_recv_buffer_size(2 * 1024 * 1024) {
        log::warn!("Failed to set UDP receive buffer size to 2MB: {}", e);
    }
    if addr.ip().is_ipv6() && addr.ip().is_unspecified() {
        // Best effort dual-stack for `::`; harmless when unsupported.
        let _ = socket.set_only_v6(false);
    }
    socket.bind(&socket2::SockAddr::from(addr))?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

/// Returns true when an IPv6 address is usable as a LAN bind target:
/// global unicast or unique-local, excluding loopback, link-local
/// (`fe80::/10` — needs a zone id to bind/reach), multicast, unspecified
/// and IPv4-mapped addresses.
pub fn is_bindable_v6(ip: &Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_multicast() || ip.is_unspecified() {
        return false;
    }
    if ip.to_ipv4_mapped().is_some() {
        return false;
    }
    let seg0 = ip.segments()[0];
    // fe80::/10 link-local
    if seg0 & 0xffc0 == 0xfe80 {
        return false;
    }
    true
}

/// Ranks IPv6 addresses for LAN streaming, mirroring the IPv4 `score_ip`
/// preference for private ranges: unique-local (fc00::/7, the LAN analogue
/// of 192.168/10.x) first, then global unicast (2000::/3).
pub fn score_ip_v6(ip: &Ipv6Addr) -> i32 {
    let seg0 = ip.segments()[0];
    if seg0 & 0xfe00 == 0xfc00 {
        100 // ULA fc00::/7
    } else if seg0 & 0xe000 == 0x2000 {
        60 // GUA 2000::/3
    } else {
        0
    }
}

/// Enumerates bindable IPv6 addresses (ULA/GUA) across interfaces, excluding
/// interfaces whose name contains any of `exclude_keywords` (case-insensitive,
/// same convention as the IPv4 virtual-interface filter). Sorted best-first.
pub fn collect_ipv6_interfaces(exclude_keywords: &[&str]) -> Vec<(Ipv6Addr, String)> {
    match local_ip_address::list_afinet_netifas() {
        Ok(interfaces) => select_ipv6_interfaces(interfaces, exclude_keywords),
        Err(_) => Vec::new(),
    }
}

/// Pure selection/ordering core of [`collect_ipv6_interfaces`], split out so
/// it can be unit-tested with synthetic interface data (CI sandboxes often
/// have no ULA/GUA addresses at all).
pub fn select_ipv6_interfaces(
    interfaces: impl IntoIterator<Item = (String, IpAddr)>,
    exclude_keywords: &[&str],
) -> Vec<(Ipv6Addr, String)> {
    let mut candidates: Vec<(Ipv6Addr, String)> = Vec::new();
    for (name, ip) in interfaces {
        let IpAddr::V6(ip) = ip else { continue };
        if !is_bindable_v6(&ip) {
            continue;
        }
        let name_lower = name.to_lowercase();
        if exclude_keywords.iter().any(|kw| name_lower.contains(kw)) {
            continue;
        }
        candidates.push((ip, name));
    }
    candidates.sort_by(|a, b| {
        score_ip_v6(&b.0)
            .cmp(&score_ip_v6(&a.0))
            .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
            .then_with(|| a.1.cmp(&b.1))
    });
    candidates
}

/// Best IPv6 address for mDNS advertisement / auto-selection, or `None`
/// when the host has no bindable IPv6 address.
pub fn best_ipv6(exclude_keywords: &[&str]) -> Option<String> {
    collect_ipv6_interfaces(exclude_keywords)
        .into_iter()
        .next()
        .map(|(ip, _)| ip.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_keeps_ipv4_and_hostnames_untouched() {
        // Identical to the legacy `format!("{}:{}", bind_address, port)`.
        assert_eq!(normalize_socket_addr("0.0.0.0", 9123), "0.0.0.0:9123");
        assert_eq!(normalize_socket_addr("127.0.0.1", 8554), "127.0.0.1:8554");
        assert_eq!(
            normalize_socket_addr("192.168.1.5", 9123),
            format!("{}:{}", "192.168.1.5", 9123)
        );
        assert_eq!(normalize_socket_addr("myhost", 9123), "myhost:9123");
    }

    #[test]
    fn normalize_brackets_ipv6_literals() {
        assert_eq!(normalize_socket_addr("::1", 9123), "[::1]:9123");
        assert_eq!(normalize_socket_addr("::", 9123), "[::]:9123");
        assert_eq!(
            normalize_socket_addr("fd12:3456::789a", 9123),
            "[fd12:3456::789a]:9123"
        );
        assert_eq!(
            normalize_socket_addr("[fd12:3456::789a]", 9123),
            "[fd12:3456::789a]:9123"
        );
        // The result must parse as a SocketAddr for the servers.
        assert!(normalize_socket_addr("fd00::1", 9123)
            .parse::<SocketAddr>()
            .is_ok());
    }

    #[test]
    fn detects_unspecified_v6() {
        assert!(is_unspecified_v6("::"));
        assert!(is_unspecified_v6("[::]"));
        assert!(!is_unspecified_v6("0.0.0.0"));
        assert!(!is_unspecified_v6("fd00::1"));
        assert!(!is_unspecified_v6("::1"));
    }

    #[test]
    fn strip_brackets_is_noop_for_v4() {
        assert_eq!(strip_brackets("192.168.1.5"), "192.168.1.5");
        assert_eq!(strip_brackets("[fd00::1]"), "fd00::1");
        assert_eq!(strip_brackets("fd00::1"), "fd00::1");
    }

    #[test]
    fn bindable_v6_classification() {
        assert!(is_bindable_v6(&"fd00::1".parse().unwrap()));
        assert!(is_bindable_v6(&"2408:8000::1".parse().unwrap()));
        assert!(!is_bindable_v6(&"::1".parse().unwrap()));
        assert!(!is_bindable_v6(&"fe80::1".parse().unwrap()));
        assert!(!is_bindable_v6(&"febf::1".parse().unwrap()));
        assert!(!is_bindable_v6(&"ff02::1".parse().unwrap()));
        assert!(!is_bindable_v6(&"::".parse().unwrap()));
        assert!(!is_bindable_v6(&"::ffff:192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn v6_scoring_prefers_ula_then_gua() {
        let ula: Ipv6Addr = "fd00::1".parse().unwrap();
        let gua: Ipv6Addr = "2408:8000::1".parse().unwrap();
        assert!(score_ip_v6(&ula) > score_ip_v6(&gua));
        assert!(score_ip_v6(&gua) > 0);
        // fc00::/7 boundary checks
        assert_eq!(score_ip_v6(&"fc00::1".parse().unwrap()), score_ip_v6(&ula));
        assert_eq!(score_ip_v6(&"fdff::1".parse().unwrap()), score_ip_v6(&ula));
        assert!(score_ip_v6(&"fe00::1".parse().unwrap()) < score_ip_v6(&ula));
        // 2000::/3 boundary checks
        assert_eq!(score_ip_v6(&"2000::1".parse().unwrap()), score_ip_v6(&gua));
        assert_eq!(score_ip_v6(&"3fff::1".parse().unwrap()), score_ip_v6(&gua));
        assert!(score_ip_v6(&"1fff::1".parse().unwrap()) < score_ip_v6(&gua));
    }

    #[test]
    fn collected_v6_interfaces_never_contain_link_local() {
        for (ip, _name) in collect_ipv6_interfaces(&[]) {
            assert!(is_bindable_v6(&ip));
        }
    }

    fn iface(name: &str, ip: &str) -> (String, IpAddr) {
        (name.to_string(), ip.parse().unwrap())
    }

    #[test]
    fn select_filters_unusable_and_keyword_interfaces() {
        let got = select_ipv6_interfaces(
            vec![
                iface("eth0", "fd00:1234::1"),      // ULA — kept
                iface("wlan0", "2001:db8::5"),      // GUA — kept
                iface("eth0", "fe80::1"),           // link-local — dropped
                iface("lo", "::1"),                 // loopback — dropped
                iface("eth0", "ff02::1"),           // multicast — dropped
                iface("eth0", "::"),                // unspecified — dropped
                iface("eth0", "::ffff:10.0.0.1"),   // v4-mapped — dropped
                iface("Tailscale", "fd00:9999::2"), // keyword — dropped
                iface("WSL", "fd00:8888::3"),       // keyword (case-insensitive) — dropped
                iface("eth0", "192.168.1.5"),       // IPv4 — skipped
            ],
            &["tailscale", "wsl"],
        );
        let addrs: Vec<String> = got.iter().map(|(ip, _)| ip.to_string()).collect();
        assert_eq!(addrs, vec!["fd00:1234::1", "2001:db8::5"]);
        assert_eq!(got[0].1, "eth0");
        assert_eq!(got[1].1, "wlan0");
    }

    #[test]
    fn select_orders_ula_before_gua_and_is_deterministic() {
        let got = select_ipv6_interfaces(
            vec![
                iface("eth0", "2001:db8::9"),
                iface("eth0", "fd12::2"),
                iface("eth1", "2001:db8::1"),
                iface("eth0", "fd12::1"),
            ],
            &[],
        );
        let addrs: Vec<String> = got.iter().map(|(ip, _)| ip.to_string()).collect();
        assert_eq!(
            addrs,
            vec!["fd12::1", "fd12::2", "2001:db8::1", "2001:db8::9"]
        );
    }

    #[test]
    fn select_tie_breaks_on_interface_name() {
        let got = select_ipv6_interfaces(
            vec![iface("wlan0", "fd12::1"), iface("eth0", "fd12::1")],
            &[],
        );
        assert_eq!(got[0].1, "eth0");
        assert_eq!(got[1].1, "wlan0");
    }

    #[tokio::test]
    async fn tcp_listener_v4_legacy_path_roundtrip() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = bind_tcp_listener("127.0.0.1", 0).await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert!(addr.ip().is_ipv4());
        let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (mut server, peer) = listener.accept().await.unwrap();
        assert!(peer.ip().is_ipv4());
        client.write_all(b"ping").await.unwrap();
        let mut buf = [0u8; 4];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"ping");
    }

    #[tokio::test]
    async fn tcp_listener_v6_loopback_roundtrip() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = match bind_tcp_listener("::1", 0).await {
            Ok(listener) => listener,
            Err(e) => {
                eprintln!("skipping: no IPv6 support in this environment ({e})");
                return;
            }
        };
        let addr = listener.local_addr().unwrap();
        assert!(addr.ip().is_ipv6());
        let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (mut server, peer) = listener.accept().await.unwrap();
        assert_eq!(peer.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
        client.write_all(b"hello6").await.unwrap();
        let mut buf = [0u8; 6];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello6");
    }

    #[tokio::test]
    async fn tcp_listener_v6only_rejects_ipv4() {
        if std::net::UdpSocket::bind("[::1]:0").is_err() {
            eprintln!("skipping: no IPv6 support in this environment");
            return;
        }
        let listener = bind_tcp_listener_v6only(0).expect("v6-only listener must bind");
        let addr = listener.local_addr().unwrap();
        assert!(addr.ip().is_ipv6() && addr.ip().is_unspecified());
        // IPv6 connects fine.
        let v6_target = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), addr.port());
        assert!(tokio::net::TcpStream::connect(v6_target).await.is_ok());
        // IPv4 to the same port must NOT be served by this socket: nothing
        // else listens on that ephemeral port, so connect must fail.
        let v4_target = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), addr.port());
        assert!(tokio::net::TcpStream::connect(v4_target).await.is_err());
    }

    #[test]
    fn udp_socket_v4_legacy_path_roundtrip() {
        let server = bind_udp_socket("127.0.0.1", 0).unwrap();
        assert!(server.local_addr().unwrap().ip().is_ipv4());
        let port = server.local_addr().unwrap().port();
        let client = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let target: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        client.send_to(b"audio4", target).unwrap();
        let mut buf = [0u8; 16];
        let (len, from) = server.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"audio4");
        assert!(from.ip().is_ipv4());
    }

    #[test]
    fn udp_socket_v6_roundtrip() {
        let server = match bind_udp_socket("::1", 0) {
            Ok(server) => server,
            Err(e) => {
                eprintln!("skipping: no IPv6 support in this environment ({e})");
                return;
            }
        };
        assert!(server.local_addr().unwrap().ip().is_ipv6());
        let port = server.local_addr().unwrap().port();
        let client = std::net::UdpSocket::bind("[::1]:0").unwrap();
        let target: SocketAddr = format!("[::1]:{}", port).parse().unwrap();
        client.send_to(b"audio6", target).unwrap();
        let mut buf = [0u8; 16];
        let (len, from) = server.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"audio6");
        assert_eq!(from.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
    }

    #[test]
    fn udp_socket_rejects_invalid_address() {
        assert!(bind_udp_socket("not an address", 9124).is_err());
    }

    #[test]
    fn wants_v6_companion_only_for_legacy_auto_bind() {
        assert!(wants_v6_companion("0.0.0.0"));
        assert!(!wants_v6_companion("127.0.0.1"));
        assert!(!wants_v6_companion("192.168.1.5"));
        assert!(!wants_v6_companion("::"));
        assert!(!wants_v6_companion("::1"));
        assert!(!wants_v6_companion("fd00::1"));
    }

    #[test]
    fn companion_failure_hints_cover_common_causes() {
        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(companion_failure_hint(&denied).contains("excludedportrange"));
        let in_use = io::Error::from(io::ErrorKind::AddrInUse);
        assert!(companion_failure_hint(&in_use).contains("IPv6 side"));
        // EAFNOSUPPORT raw codes per platform (see companion_failure_hint).
        #[cfg(target_os = "linux")]
        let no_v6 = io::Error::from_raw_os_error(97);
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let no_v6 = io::Error::from_raw_os_error(47);
        #[cfg(target_os = "windows")]
        let no_v6 = io::Error::from_raw_os_error(10047);
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "windows"
        )))]
        let no_v6 = io::Error::from(io::ErrorKind::Unsupported);
        assert!(companion_failure_hint(&no_v6).contains("IPv6 stack"));
        let other = io::Error::from(io::ErrorKind::UnexpectedEof);
        assert_eq!(companion_failure_hint(&other), "");
    }

    #[test]
    fn udp_v6only_companion_coexists_with_v4_wildcard() {
        if std::net::UdpSocket::bind("[::1]:0").is_err() {
            eprintln!("skipping: no IPv6 support in this environment");
            return;
        }
        let v4 = bind_udp_socket("0.0.0.0", 0).unwrap();
        let port = v4.local_addr().unwrap().port();
        // STRICT: must coexist with the v4 wildcard socket on the same port.
        let v6 = bind_udp_socket_v6only(port)
            .expect("v6-only UDP companion must bind next to the v4 wildcard");
        assert_eq!(v6.local_addr().unwrap().port(), port);
        assert!(v6.local_addr().unwrap().ip().is_ipv6());

        // A datagram to the IPv4 wildcard reaches only the v4 socket...
        let v4_client = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let v4_target = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port);
        v4_client.send_to(b"to-v4", v4_target).unwrap();
        let mut buf = [0u8; 16];
        let (len, from) = v4.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"to-v4");
        assert!(from.ip().is_ipv4());

        // ...and a datagram to the IPv6 wildcard reaches only the companion.
        let v6_client = std::net::UdpSocket::bind("[::1]:0").unwrap();
        let v6_target = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port);
        v6_client.send_to(b"to-v6", v6_target).unwrap();
        let (len, from) = v6.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"to-v6");
        assert_eq!(from.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
    }

    #[tokio::test]
    async fn recv_from_either_serves_both_sockets() {
        if std::net::UdpSocket::bind("[::1]:0").is_err() {
            eprintln!("skipping: no IPv6 support in this environment");
            return;
        }
        let v4 = bind_udp_socket("0.0.0.0", 0).unwrap();
        let port = v4.local_addr().unwrap().port();
        let v6 = bind_udp_socket_v6only(port).expect("v6-only companion must bind");
        let v4 = tokio::net::UdpSocket::from_std(v4).unwrap();
        let v6 = tokio::net::UdpSocket::from_std(v6).unwrap();

        let v4_client = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        v4_client
            .send_to(
                b"a4",
                SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port),
            )
            .unwrap();
        let v6_client = std::net::UdpSocket::bind("[::1]:0").unwrap();
        v6_client
            .send_to(
                b"a6",
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
            )
            .unwrap();

        let mut buf = [0u8; 16];
        let mut buf_v6 = [0u8; 16];
        let mut seen_v4 = false;
        let mut seen_v6 = false;
        for _ in 0..2 {
            let (len, from, from_v6) = recv_from_either(&v4, Some(&v6), &mut buf, &mut buf_v6)
                .await
                .unwrap();
            let active_buf = if from_v6 { &buf_v6 } else { &buf };
            match from.ip() {
                IpAddr::V4(_) => {
                    assert!(!from_v6);
                    assert_eq!(&active_buf[..len], b"a4");
                    seen_v4 = true;
                }
                IpAddr::V6(_) => {
                    assert!(from_v6);
                    assert_eq!(&active_buf[..len], b"a6");
                    seen_v6 = true;
                }
            }
        }
        assert!(seen_v4 && seen_v6);

        // Without a companion it degrades to plain primary.recv_from.
        let (len, from, from_v6) = {
            v4_client
                .send_to(
                    b"b4",
                    SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port),
                )
                .unwrap();
            recv_from_either(&v4, None, &mut buf, &mut buf_v6)
                .await
                .unwrap()
        };
        assert!(!from_v6);
        assert_eq!(&buf[..len], b"b4");
        assert!(from.ip().is_ipv4());
    }
}
