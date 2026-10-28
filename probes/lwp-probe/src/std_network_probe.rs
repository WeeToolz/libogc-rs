//! Real std TCP, DNS, UDP, clone ownership, and deadline acceptance.
use ogc_std_runtime as runtime;
use ogc_system_probe::probe_support::ProbeLogger;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

fn network_detail(error: &std::io::Error) -> String {
    let snapshot = runtime::network_snapshot();
    format!(
        "{error}; ioctl={} rc={}",
        snapshot.last_failed_ioctl, snapshot.last_failed_ioctl_code
    )
}

fn wii_ipv4() -> std::io::Result<std::net::Ipv4Addr> {
    let raw = ogc_network::get_host_ip();
    let address = std::net::Ipv4Addr::new(
        ogc_network::ip4_addr1(raw) as u8,
        ogc_network::ip4_addr2(raw) as u8,
        ogc_network::ip4_addr3(raw) as u8,
        ogc_network::ip4_addr4(raw) as u8,
    );
    if address.is_unspecified() {
        Err(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "IOS returned an unspecified Wii IPv4 address",
        ))
    } else {
        Ok(address)
    }
}

fn parse_endpoint(logger: &mut ProbeLogger) -> Option<(String, u16)> {
    let endpoint = match crate::std_probe_config::value("network-endpoint") {
        Ok(Some(endpoint)) => endpoint,
        Ok(None) => std::env::args()
            .skip(1)
            .find(|arg| !arg.starts_with("--"))
            .unwrap_or_else(|| "example.com:80".to_string()),
        Err(error) => {
            logger.note(error);
            logger.record("network endpoint option is valid", false);
            return None;
        }
    };
    let Some((host, port)) = endpoint.rsplit_once(':') else {
        logger.record("endpoint uses host:port syntax", false);
        return None;
    };
    let Ok(port) = port.parse::<u16>() else {
        logger.record("endpoint has a valid port", false);
        return None;
    };
    if host.is_empty() || host.parse::<std::net::Ipv4Addr>().is_ok() {
        logger.record("endpoint uses a DNS name", false);
        return None;
    }
    logger.note(&format!("DNS and HTTP endpoint: {host}:{port}"));
    Some((host.to_string(), port))
}

fn parse_network_peer(logger: &mut ProbeLogger) -> Option<(std::net::Ipv4Addr, u16, u16)> {
    let value = match crate::std_probe_config::value("network-peer") {
        Ok(Some(value)) => value,
        Ok(None) => {
            logger.record("host TCP and UDP test peer is configured", false);
            return None;
        }
        Err(error) => {
            logger.note(error);
            logger.record("host TCP and UDP test peer is valid", false);
            return None;
        }
    };
    let parts = value.rsplitn(3, ':').collect::<Vec<_>>();
    if parts.len() != 3 {
        logger.record("host TCP and UDP test peer uses IPv4:tcp:udp syntax", false);
        return None;
    }
    let (Ok(udp), Ok(tcp), Ok(host)) = (
        parts[0].parse::<u16>(),
        parts[1].parse::<u16>(),
        parts[2].parse::<std::net::Ipv4Addr>(),
    ) else {
        logger.record("host TCP and UDP test peer has valid addresses", false);
        return None;
    };
    if tcp == 0 || udp == 0 || host.is_unspecified() {
        logger.record("host TCP and UDP test peer has usable endpoints", false);
        return None;
    }
    logger.note(&format!("host TCP and UDP peer: {host}:{tcp}/{udp}"));
    Some((host, tcp, udp))
}

fn dns(logger: &mut ProbeLogger, host: &str, port: u16) -> Option<Vec<std::net::SocketAddr>> {
    logger.section("STD DNS OWNERSHIP AND CONCURRENCY");
    let addresses = match (host, port).to_socket_addrs() {
        Ok(addresses) => addresses.collect::<Vec<_>>(),
        Err(error) => {
            logger.note(&format!("DNS lookup failed: {error}"));
            logger.record("DNS returns at least one IPv4 address", false);
            return None;
        }
    };
    let ipv4_count = addresses.iter().filter(|address| address.is_ipv4()).count();
    logger.record("DNS returns at least one IPv4 address", ipv4_count > 0);
    let name = host.to_string();
    let gate = Arc::new(Barrier::new(2));
    let worker_gate = gate.clone();
    let worker = match thread::Builder::new().spawn(move || {
        worker_gate.wait();
        (name.as_str(), port)
            .to_socket_addrs()
            .map(|addresses| addresses.collect::<Vec<_>>())
    }) {
        Ok(worker) => worker,
        Err(error) => {
            logger.note(&format!("DNS worker creation failed: {error}"));
            logger.record("concurrent DNS lookup completes", false);
            return (ipv4_count > 0).then_some(addresses);
        }
    };
    gate.wait();
    let simultaneous = (host, port).to_socket_addrs();
    logger.record(
        "main DNS lookup overlaps the worker lookup",
        simultaneous.is_ok_and(|mut answers| answers.any(|address| address.is_ipv4())),
    );
    let concurrent = worker.join().is_ok_and(|result| {
        result.is_ok_and(|answers| answers.iter().any(std::net::SocketAddr::is_ipv4))
    });
    logger.record(
        "concurrent DNS lookup completes with an IPv4 answer",
        concurrent,
    );
    (ipv4_count > 0).then_some(addresses)
}

fn native_http(logger: &mut ProbeLogger, address: std::net::SocketAddr, request: &[u8]) {
    use ogc_network as net;
    let std::net::SocketAddr::V4(address) = address else {
        return;
    };
    let socket = net::socket(net::PF_INET, net::SOCK_STREAM, 0);
    if socket < 0 {
        logger.record_with_detail(
            "native HTTP diagnostic socket opens",
            false,
            &format!("{socket}"),
        );
        return;
    }
    let result = (|| -> Result<[u8; 5], String> {
        let destination = net::SockaddrIn {
            sin_len: core::mem::size_of::<net::SockaddrIn>() as u8,
            sin_family: net::AF_INET as u8,
            sin_port: address.port().to_be(),
            sin_addr: net::InAddr {
                s_addr: u32::from_be_bytes(address.ip().octets()),
            },
            sin_zero: [0; 8],
        };
        let rc = net::connect_addr(socket, &destination);
        if rc < 0 {
            return Err(format!("connect rc={rc}"));
        }
        let mut pending = request;
        while !pending.is_empty() {
            let count = net::write_slice(socket, pending);
            if count <= 0 || count as usize > pending.len() {
                return Err(format!("write rc={count}"));
            }
            pending = &pending[count as usize..];
        }
        let flags = net::fcntl(socket, 4, net::O_NONBLOCK);
        if flags < 0 {
            return Err(format!("fcntl rc={flags}"));
        }
        let mut prefix = [0u8; 32];
        let mut offset = 0;
        let until = Instant::now() + Duration::from_secs(3);
        while offset < prefix.len() {
            let count = net::read_slice(socket, &mut prefix[offset..]);
            if count == -6 && Instant::now() < until {
                thread::sleep(Duration::from_millis(1));
                continue;
            }
            if count <= 0 || count as usize > prefix.len() - offset {
                return Err(format!("read rc={count}"));
            }
            offset += count as usize;
        }
        logger.note(&format!("native HTTP 32-byte receive: {prefix:?}"));
        Ok(prefix[..5].try_into().expect("five-byte HTTP prefix"))
    })();
    let closed = net::close(socket);
    logger.note(&format!(
        "native HTTP diagnostic: {result:?}; close={closed}"
    ));
    logger.record_with_detail(
        "native HTTP diagnostic returns exact prefix",
        result.as_ref().is_ok_and(|bytes| bytes == b"HTTP/"),
        &format!("{result:?}"),
    );
    logger.record_with_detail(
        "native HTTP diagnostic socket closes",
        closed == 0,
        &format!("rc={closed}"),
    );
}

fn tcp(logger: &mut ProbeLogger, address: std::net::SocketAddr, host: &str) {
    logger.section("STD TCP AND SHARED SOCKET OWNERSHIP");
    let connect_timeout = TcpStream::connect_timeout(&address, Duration::from_millis(50));
    logger.record(
        "connect_timeout reports the unsupported completion query",
        connect_timeout
            .as_ref()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::Unsupported),
    );
    drop(connect_timeout);

    let before = runtime::network_snapshot();
    let mut stream = match TcpStream::connect(address) {
        Ok(stream) => stream,
        Err(error) => {
            logger.note(&format!("TCP connect failed: {error}"));
            logger.record("TCP connects to the resolved peer", false);
            return;
        }
    };
    let mut clone = match stream.try_clone() {
        Ok(clone) => clone,
        Err(error) => {
            logger.note(&format!("TCP clone failed: {error}"));
            logger.record("TCP clone shares one live socket", false);
            return;
        }
    };
    let cloned = runtime::network_snapshot();
    logger.record(
        "TCP clone shares one live socket",
        cloned.live_sockets == before.live_sockets + 1,
    );
    let timeout = Duration::from_secs(3);
    let configured = stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .and_then(|()| {
            if stream.read_timeout()? == Some(timeout) && stream.write_timeout()? == Some(timeout) {
                Ok(())
            } else {
                Err(std::io::Error::other("socket timeout value changed"))
            }
        });
    logger.record(
        "TCP stores and returns read/write timeouts",
        configured.is_ok(),
    );
    if let Err(error) = configured {
        logger.note(&format!("TCP timeout query failed: {error}"));
        drop(stream);
        drop(clone);
        logger.record(
            "failed TCP timeout setup still closes the shared socket",
            runtime::network_snapshot().live_sockets == before.live_sockets,
        );
        return;
    }

    let request = format!("HEAD / HTTP/1.0\r\nHost: {host}\r\n\r\n");
    let response = stream.write_all(request.as_bytes()).and_then(|()| {
        let mut prefix = [0; 5];
        stream.read_exact(&mut prefix).map(|()| prefix)
    });
    let (http_ok, http_detail) = match &response {
        Ok(prefix) if prefix == b"HTTP/" => (true, String::new()),
        Ok(prefix) => (false, format!("unexpected response prefix: {prefix:?}")),
        Err(error) => (false, error.to_string()),
    };
    if !http_ok {
        logger.note(&http_detail);
        native_http(logger, address, request.as_bytes());
    }
    logger.record_with_detail("TCP exchanges real HTTP bytes", http_ok, &http_detail);
    if http_ok {
        let mut version = [0; 4];
        let result = clone.read_exact(&mut version);
        logger.record_with_detail(
            "TCP clone reads subsequent bytes from the shared stream",
            result.is_ok() && (&version == b"1.0 " || &version == b"1.1 "),
            &format!("read={result:?}, bytes={version:?}"),
        );
    }

    drop(stream);
    let after_first_drop = runtime::network_snapshot();
    logger.record(
        "dropping one clone keeps its socket open",
        after_first_drop.live_sockets == before.live_sockets + 1,
    );
    drop(clone);
    let after_final_drop = runtime::network_snapshot();
    logger.record(
        "dropping the final clone closes its socket",
        after_final_drop.live_sockets == before.live_sockets,
    );
    logger.record(
        "socket cleanup reports no close failure",
        after_final_drop.close_failures == before.close_failures,
    );
}

fn udp_deadline(logger: &mut ProbeLogger) {
    logger.section("STD UDP ABSOLUTE READ DEADLINE");
    let socket = match UdpSocket::bind(("0.0.0.0", 0)) {
        Ok(socket) => socket,
        Err(error) => {
            logger.note(&format!("UDP bind failed: {}", network_detail(&error)));
            logger.record_with_detail(
                "UDP bind reaches the real IOS socket service",
                false,
                &network_detail(&error),
            );
            return;
        }
    };
    let timeout = Duration::from_millis(75);
    if let Err(error) = socket.set_read_timeout(Some(timeout)) {
        logger.note(&format!("UDP timeout setup failed: {error}"));
        logger.record("UDP timeout setter reaches the real socket policy", false);
        return;
    }
    let start = ogc_lwp_driver::now_ticks();
    let wall_start = Instant::now();
    let result = socket.recv_from(&mut [0u8; 1]);
    let elapsed_ticks = ogc_lwp_driver::now_ticks().wrapping_sub(start);
    let elapsed = wall_start.elapsed();
    logger.record(
        "idle UDP receive returns the timeout error",
        result.is_err_and(|error| error.kind() == std::io::ErrorKind::TimedOut),
    );
    logger.note(&format!(
        "UDP timeout elapsed: {} ms, {} raw ticks",
        elapsed.as_millis(),
        elapsed_ticks
    ));
    logger.record(
        "UDP receive uses one bounded absolute deadline",
        (4_556_250..15_187_500).contains(&elapsed_ticks)
            && (timeout..Duration::from_millis(250)).contains(&elapsed),
    );
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("STD NETWORK INITIALIZATION");
    let initialized = runtime::network_initialize(false);
    logger.record("IOS network stack initializes", initialized.is_ok());
    if let Err(error) = initialized {
        logger.note(&format!("network initialization failed: {error:?}"));
        return;
    }
    if let Some(peer) = parse_network_peer(logger) {
        tcp_loopback(logger, peer);
        udp_edges(logger, peer);
    }
    udp_deadline(logger);
    let Some((host, port)) = parse_endpoint(logger) else {
        return;
    };
    let Some(addresses) = dns(logger, &host, port) else {
        return;
    };
    if let Some(address) = addresses
        .iter()
        .copied()
        .find(std::net::SocketAddr::is_ipv4)
    {
        tcp(logger, address, &host);
    }
}

fn tcp_loopback(logger: &mut ProbeLogger, peer: (std::net::Ipv4Addr, u16, u16)) {
    logger.section("STD TCP LISTENER AND OPTIONS");
    let before = runtime::network_snapshot();
    let result = (|| -> std::io::Result<()> {
        let listener = TcpListener::bind((wii_ipv4()?, 0))?;
        let address = listener.local_addr()?;
        logger.note(&format!("Wii TCP listener endpoint: {address}"));
        let clone = listener.try_clone()?;
        logger.record(
            "listener clone shares its bound address",
            clone.local_addr()? == address,
        );
        listener.set_nonblocking(true)?;
        logger.record(
            "empty listener returns WouldBlock",
            listener
                .accept()
                .is_err_and(|error| error.kind() == std::io::ErrorKind::WouldBlock),
        );
        // Keep the outbound connection on the main LWP so this listener probe
        // tests IOS accept without concurrent socket creation.
        let mut client = TcpStream::connect((peer.0, peer.1))?;
        client.set_read_timeout(Some(Duration::from_secs(12)))?;
        client.set_write_timeout(Some(Duration::from_secs(12)))?;
        writeln!(client, "{} {}", address.ip(), address.port())?;
        let until = Instant::now() + Duration::from_secs(10);
        let accepted = loop {
            match clone.accept() {
                Ok(value) => break Ok(value),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until =>
                {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(error) => break Err(error),
            }
        };
        let server = (|| -> std::io::Result<()> {
            let (mut socket, peer) = accepted?;
            socket.set_read_timeout(Some(Duration::from_secs(3)))?;
            socket.set_write_timeout(Some(Duration::from_secs(3)))?;
            logger.record(
                "accepted socket reports its actual peer",
                socket.peer_addr()? == peer,
            );
            socket.set_nodelay(true)?;
            logger.record("nodelay getter reads enabled state", socket.nodelay()?);
            socket.set_nodelay(false)?;
            logger.record("nodelay getter reads disabled state", !socket.nodelay()?);
            let mut request = [0; 16];
            socket.read_exact(&mut request)?;
            logger.record(
                "TCP server receives fixed peer bytes",
                &request == b"WII_STD_TCP_51A7",
            );
            socket.write_all(&[0x51, 0xa7, 0x20, 0x26])?;
            Ok(())
        })();
        if let Err(server_error) = server {
            logger.note(&format!("TCP listener side failed: {server_error}"));
            return Err(std::io::Error::other(format!(
                "TCP listener side failed: {server_error}"
            )));
        }
        let mut status = [0; 1];
        client.read_exact(&mut status)?;
        if status[0] != 0 {
            let mut detail = String::new();
            client.read_to_string(&mut detail)?;
            return Err(std::io::Error::other(format!(
                "host TCP peer failed: {detail}"
            )));
        }
        let mut reply = [0; 4];
        client.read_exact(&mut reply)?;
        logger.record(
            "TCP client receives fixed peer reply",
            reply == [0x51, 0xa7, 0x20, 0x26],
        );
        Ok(())
    })();
    match result {
        Ok(()) => logger.record("TCP listener exchanges bytes with its peer", true),
        Err(error) => {
            logger.note(&format!("TCP listener failed: {}", network_detail(&error)));
            logger.record_with_detail(
                "TCP listener exchanges bytes with its peer",
                false,
                &network_detail(&error),
            );
        }
    }
    let after = runtime::network_snapshot();
    logger.record(
        "TCP listener tests close all socket owners",
        after.live_sockets == before.live_sockets && after.close_failures == before.close_failures,
    );
}

fn udp_edges(logger: &mut ProbeLogger, peer: (std::net::Ipv4Addr, u16, u16)) {
    logger.section("STD UDP DATAGRAMS AND CLONE SETTINGS");
    let before = runtime::network_snapshot();
    let result = (|| -> std::io::Result<()> {
        let local_ip = wii_ipv4()?;
        let socket = UdpSocket::bind((local_ip, 0))?;
        let clone = socket.try_clone()?;
        let timeout = Duration::from_secs(2);
        socket.set_read_timeout(Some(timeout))?;
        logger.record(
            "UDP clone sees shared timeout",
            clone.read_timeout()? == Some(timeout),
        );
        logger.record(
            "zero timeout fails without changing settings",
            socket
                .set_read_timeout(Some(Duration::ZERO))
                .is_err_and(|e| e.kind() == std::io::ErrorKind::InvalidInput)
                && clone.read_timeout()? == Some(timeout),
        );
        let broadcast = (|| -> std::io::Result<()> {
            logger.record(
                "broadcast send is rejected while permission is disabled",
                socket
                    .send_to(b"blocked", ("255.255.255.255", 9))
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied),
            );
            socket.set_broadcast(true)?;
            logger.record("broadcast getter reads enabled state", socket.broadcast()?);
            socket.set_broadcast(false)?;
            logger.record(
                "broadcast getter reads disabled state",
                !socket.broadcast()?,
            );
            Ok(())
        })();
        logger.record_with_detail(
            "UDP broadcast option acceptance completes",
            broadcast.is_ok(),
            &broadcast
                .err()
                .map(|error| network_detail(&error))
                .unwrap_or_default(),
        );
        logger.record(
            "oversized TTL rejected before transport",
            socket
                .set_ttl(256)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::InvalidInput),
        );
        logger.record(
            "UDP sends complete fixed datagram to real peer",
            socket.send_to(b"51a7-data", (peer.0, peer.2))? == 9,
        );
        let mut bytes = [0xcc; 12];
        let (count, address) = clone.peek_from(&mut bytes)?;
        logger.record(
            "UDP peek preserves bytes and peer source",
            count == 9 && &bytes[..9] == b"51a7-data" && address == (peer.0, peer.2).into(),
        );
        let (count, address) = socket.recv_from(&mut bytes[..4])?;
        logger.record(
            "UDP truncates one datagram and preserves source",
            count == 4 && &bytes[..4] == b"51a7" && address == (peer.0, peer.2).into(),
        );
        socket.set_nonblocking(true)?;
        logger.record(
            "UDP truncation consumes the entire datagram",
            clone
                .recv_from(&mut bytes)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::WouldBlock),
        );
        clone.set_nonblocking(false)?;
        logger.record(
            "UDP sends a real empty datagram",
            socket.send_to(&[], (peer.0, peer.2))? == 0,
        );
        let (count, address) = socket.recv_from(&mut bytes)?;
        logger.record(
            "UDP receives empty datagram from real peer",
            count == 0 && address == (peer.0, peer.2).into(),
        );
        socket.connect((peer.0, peer.2))?;
        logger.record(
            "connected UDP reports actual peer",
            socket.peer_addr()? == (peer.0, peer.2).into(),
        );
        logger.record(
            "connected UDP sends full datagram",
            socket.send(b"next")? == 4,
        );
        let (count, address) = socket.recv_from(&mut bytes)?;
        logger.record(
            "connected UDP receives peer reply",
            count == 4 && &bytes[..4] == b"next" && address == (peer.0, peer.2).into(),
        );
        Ok(())
    })();
    match result {
        Ok(()) => logger.record("UDP edge acceptance completes", true),
        Err(error) => {
            logger.note(&format!("UDP edge failed: {}", network_detail(&error)));
            logger.record_with_detail(
                "UDP edge acceptance completes",
                false,
                &network_detail(&error),
            );
        }
    }
    let after = runtime::network_snapshot();
    logger.record(
        "UDP edge tests close all shared sockets",
        after.live_sockets == before.live_sockets && after.close_failures == before.close_failures,
    );
}
