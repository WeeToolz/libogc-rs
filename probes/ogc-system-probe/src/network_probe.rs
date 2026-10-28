use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::null_mut;

use crate::probe_support::ProbeLogger;

use ogc_network as network;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("NETWORK PROBE");

    probe_cleanup_before_init(logger);

    let init_rc = network::init();
    logger.record("network init", init_rc >= 0);
    logger.note_i32("network init rc", init_rc);

    let status = network::get_status();
    logger.record("network status matches init", status == init_rc);
    logger.note_i32("network status", status);

    let mut mac = [0u8; 6];
    let mac_rc = unsafe { network::get_mac_address(mac.as_mut_ptr().cast()) };
    logger.record(
        "network mac nonzero",
        mac_rc >= 0 && mac.iter().any(|byte| *byte != 0),
    );
    logger.note_i32("network mac rc", mac_rc);

    let host_ip = network::get_host_ip();
    logger.record("network host ip nonzero", host_ip != 0);
    logger.note_hex32("network host ip", host_ip);

    let mut local_ip = [0u8; 16];
    let mut netmask = [0u8; 16];
    let mut gateway = [0u8; 16];
    logger.record(
        "network dhcp if_config",
        unsafe {
            network::if_config(
                local_ip.as_mut_ptr().cast(),
                netmask.as_mut_ptr().cast(),
                gateway.as_mut_ptr().cast(),
                true,
                1,
            )
        } >= 0,
    );

    let socket = network::socket(network::PF_INET, network::SOCK_STREAM, network::IPPROTO_TCP);
    logger.record("network socket open", socket >= 0);
    if socket >= 0 {
        logger.record("network socket close", network::close(socket) == 0);
    }

    let udp = network::socket(network::PF_INET, network::SOCK_DGRAM, network::IPPROTO_UDP);
    logger.record("network udp socket open", udp >= 0);
    if udp >= 0 {
        logger.record("network shutdown accepted", network::shutdown(udp, 0) >= 0);
        logger.record("network udp socket close", network::close(udp) == 0);
    }

    probe_socket_edges(logger);
    probe_short_wii_sockaddr(logger);
    probe_bind_error_parity(logger);
    probe_cleanup_cycle(logger);

    network::deinit();
    network::wc24_cleanup();
}

fn probe_socket_edges(logger: &mut ProbeLogger) {
    let socket = network::socket(network::PF_INET, network::SOCK_STREAM, network::IPPROTO_TCP);
    logger.record("network edge socket open", socket >= 0);
    if socket < 0 {
        return;
    }

    let addr = network::SockaddrIn {
        sin_len: size_of::<network::SockaddrIn>() as u8,
        sin_family: network::AF_INET as u8,
        sin_port: 0,
        sin_addr: network::InAddr {
            s_addr: network::INADDR_ANY,
        },
        sin_zero: [0; 8],
    };
    logger.record(
        "network bind ephemeral",
        network::bind_addr(socket, &addr) == 0,
    );

    let mut local = network::SockaddrIn::default();
    logger.record(
        "network getsockname",
        network::getsockname_addr(socket, &mut local) == 0
            && local.sin_family == network::AF_INET as u8,
    );

    logger.record(
        "network SO_REUSEADDR roundtrip",
        roundtrip_i32_sockopt(socket, network::SOL_SOCKET, network::SO_REUSEADDR, 1),
    );
    logger.record(
        "network SO_RCVBUF roundtrip",
        roundtrip_i32_sockopt(socket, network::SOL_SOCKET, network::SO_RCVBUF, 16 * 1024),
    );
    logger.record(
        "network SO_RCVTIMEO roundtrip",
        roundtrip_timeval_sockopt(
            socket,
            network::SOL_SOCKET,
            network::SO_RCVTIMEO,
            network::Timeval {
                tv_sec: 2,
                tv_usec: 25_000,
            },
        ),
    );
    logger.record(
        "network IP_TTL roundtrip",
        roundtrip_i32_sockopt(socket, network::IPPROTO_IP, network::sys::IP_TTL, 64),
    );
    logger.record(
        "network TCP_NODELAY roundtrip",
        roundtrip_i32_sockopt(socket, network::IPPROTO_TCP, network::TCP_NODELAY, 1),
    );
    logger.record(
        "network TCP_KEEPALIVE roundtrip",
        roundtrip_i32_sockopt(socket, network::IPPROTO_TCP, network::TCP_KEEPALIVE, 30),
    );

    let mut pollfd = [network::PollFd {
        fd: socket,
        events: network::POLLIN | network::POLLOUT,
        revents: 0,
    }];
    logger.record(
        "network poll local socket",
        network::poll_slice(&mut pollfd, 0) >= 0,
    );

    let mut opt_len = size_of::<i32>() as network::SockLenT;
    logger.record(
        "network getsockopt null rejected",
        unsafe {
            network::getsockopt(
                socket,
                network::SOL_SOCKET,
                network::SO_RCVBUF,
                null_mut::<c_void>(),
                &mut opt_len,
            )
        } == network::SOCKET_ERROR,
    );
    let readonly = 1_i32;
    logger.record(
        "network readonly sockopt rejected",
        unsafe {
            network::setsockopt(
                socket,
                network::SOL_SOCKET,
                network::SO_TYPE,
                (&readonly as *const i32).cast(),
                size_of::<i32>() as network::SockLenT,
            )
        } == network::SOCKET_ERROR,
    );

    logger.record("network edge socket close", network::close(socket) == 0);
    logger.record(
        "network double close rejected",
        network::close(socket) == network::SOCKET_ERROR,
    );
}

fn probe_bind_error_parity(logger: &mut ProbeLogger) {
    let first = network::socket(network::PF_INET, network::SOCK_STREAM, network::IPPROTO_TCP);
    let second = network::socket(network::PF_INET, network::SOCK_STREAM, network::IPPROTO_TCP);
    let libdb_socket =
        network::socket(network::PF_INET, network::SOCK_STREAM, network::IPPROTO_TCP);
    logger.record(
        "network bind conflict sockets open",
        first >= 0 && second >= 0 && libdb_socket >= 0,
    );
    if first < 0 || second < 0 || libdb_socket < 0 {
        if first >= 0 {
            logger.record(
                "network bind conflict first socket close",
                network::close(first) == 0,
            );
        }
        if second >= 0 {
            logger.record(
                "network bind conflict second socket close",
                network::close(second) == 0,
            );
        }
        if libdb_socket >= 0 {
            logger.record(
                "libdb bind conflict socket close",
                network::close(libdb_socket) == 0,
            );
        }
        return;
    }

    let mut conflict_found = false;
    for port in 40_500u16..40_532 {
        let addr = network::SockaddrIn {
            sin_len: size_of::<network::SockaddrIn>() as u8,
            sin_family: network::AF_INET as u8,
            sin_port: port.to_be(),
            sin_addr: network::InAddr {
                s_addr: network::INADDR_ANY,
            },
            sin_zero: [0; 8],
        };
        if network::bind_addr(first, &addr) == 0 {
            let result = network::bind_addr(second, &addr);
            logger.note_i32("network bind conflict raw rc", result);
            logger.record("network bind preserves address-in-use error", result == -98);
            let mut libdb_addr = addr;
            let mut libdb_len = size_of::<network::SockaddrIn>() as u32;
            let libdb_result = unsafe {
                libdb_sys::tcpip_bind(
                    libdb_socket,
                    (&mut libdb_addr as *mut network::SockaddrIn).cast(),
                    &mut libdb_len,
                )
            };
            logger.note_i32("libdb bind conflict rc", libdb_result);
            logger.record(
                "libdb bind maps address conflict to ERR_USE",
                libdb_result == -10,
            );
            conflict_found = true;
            break;
        }
    }
    logger.record("network bind conflict port found", conflict_found);
    logger.record(
        "network bind conflict first socket close",
        network::close(first) == 0,
    );
    logger.record(
        "network bind conflict second socket close",
        network::close(second) == 0,
    );
    logger.record(
        "libdb bind conflict socket close",
        network::close(libdb_socket) == 0,
    );
}

#[repr(C, align(4))]
struct WiiSockaddr8([u8; 8]);

fn probe_short_wii_sockaddr(logger: &mut ProbeLogger) {
    let bind_socket = network::socket(network::PF_INET, network::SOCK_STREAM, network::IPPROTO_TCP);
    let connect_socket =
        network::socket(network::PF_INET, network::SOCK_STREAM, network::IPPROTO_TCP);
    logger.record(
        "network short sockaddr sockets open",
        bind_socket >= 0 && connect_socket >= 0,
    );
    if bind_socket < 0 || connect_socket < 0 {
        if bind_socket >= 0 {
            logger.record(
                "network short sockaddr bind socket close",
                network::close(bind_socket) == 0,
            );
        }
        if connect_socket >= 0 {
            logger.record(
                "network short sockaddr connect socket close",
                network::close(connect_socket) == 0,
            );
        }
        return;
    }

    let mut bind_addr = WiiSockaddr8([0, network::AF_INET as u8, 0, 0, 0, 0, 0, 0]);
    let bind_result = unsafe {
        network::bind(
            bind_socket,
            bind_addr.0.as_mut_ptr().cast(),
            bind_addr.0.len() as network::SockLenT,
        )
    };
    logger.record(
        "network bind accepts eight-byte Wii sockaddr",
        bind_result == 0 && bind_addr.0[0] == 8,
    );
    logger.note_i32("network short sockaddr bind rc", bind_result);

    let mut connect_addr = WiiSockaddr8([0, network::AF_INET as u8, 0, 0, 0, 0, 0, 0]);
    let connect_result = unsafe {
        network::connect(
            connect_socket,
            connect_addr.0.as_mut_ptr().cast(),
            connect_addr.0.len() as network::SockLenT,
        )
    };
    logger.record(
        "network connect reaches Wii port-zero validation",
        connect_result == -22 && connect_addr.0[0] == 8,
    );
    logger.note_i32("network short sockaddr connect rc", connect_result);

    logger.record(
        "network short sockaddr bind socket close",
        network::close(bind_socket) == 0,
    );
    logger.record(
        "network short sockaddr connect socket close",
        network::close(connect_socket) == 0,
    );
}

fn probe_cleanup_before_init(logger: &mut ProbeLogger) {
    network::deinit();
    network::wc24_cleanup();
    logger.record("network cleanup before init", network::get_status() == -100);
}

fn probe_cleanup_cycle(logger: &mut ProbeLogger) {
    let socket = network::socket(network::PF_INET, network::SOCK_STREAM, network::IPPROTO_TCP);
    logger.record("network cleanup socket open", socket >= 0);
    if socket < 0 {
        return;
    }

    network::wc24_cleanup();
    network::deinit();
    logger.record(
        "network cleanup invalidates open socket",
        network::close(socket) == network::SOCKET_ERROR,
    );

    network::wc24_cleanup();
    let reinit_rc = network::init();
    logger.record("network reinit after cleanup", reinit_rc >= 0);
    logger.note_i32("network reinit rc", reinit_rc);
    if reinit_rc >= 0 {
        let reopened = network::socket(network::PF_INET, network::SOCK_DGRAM, network::IPPROTO_UDP);
        logger.record("network cleanup reinit socket open", reopened >= 0);
        if reopened >= 0 {
            logger.record(
                "network cleanup reinit socket close",
                network::close(reopened) == 0,
            );
        }
        network::deinit();
        network::wc24_cleanup();
    }
}

fn roundtrip_i32_sockopt(socket: i32, level: u32, optname: u32, value: i32) -> bool {
    let mut out = i32::MIN;
    let mut out_len = size_of::<i32>() as network::SockLenT;
    unsafe {
        network::setsockopt(
            socket,
            level,
            optname,
            (&value as *const i32).cast::<c_void>(),
            size_of::<i32>() as network::SockLenT,
        ) == 0
            && network::getsockopt(
                socket,
                level,
                optname,
                (&mut out as *mut i32).cast::<c_void>(),
                &mut out_len,
            ) == 0
            && out == value
            && out_len == size_of::<i32>() as network::SockLenT
    }
}

fn roundtrip_timeval_sockopt(
    socket: i32,
    level: u32,
    optname: u32,
    value: network::Timeval,
) -> bool {
    let mut out = network::Timeval::default();
    let mut out_len = size_of::<network::Timeval>() as network::SockLenT;
    unsafe {
        network::setsockopt(
            socket,
            level,
            optname,
            (&value as *const network::Timeval).cast::<c_void>(),
            size_of::<network::Timeval>() as network::SockLenT,
        ) == 0
            && network::getsockopt(
                socket,
                level,
                optname,
                (&mut out as *mut network::Timeval).cast::<c_void>(),
                &mut out_len,
            ) == 0
            && out == value
            && out_len == size_of::<network::Timeval>() as network::SockLenT
    }
}
