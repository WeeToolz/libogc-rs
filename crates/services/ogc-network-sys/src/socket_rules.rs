//! Packet bounds and deadline engine used by the production std network service.
extern crate alloc;
use alloc::vec::Vec;
use core::time::Duration;
#[path = "socket_error.rs"]
mod error;
pub use error::NetError;

/// IOS may reject libogc's preferred receive-window size when network memory
/// is low. In that case, it keeps the socket's valid system-default window.
pub(crate) fn receive_window_setup(result: Result<(), NetError>) -> Result<(), NetError> {
    match result {
        Err(error) if error.domain == 1 && error.errno() == 12 => Ok(()),
        result => result,
    }
}

pub(crate) fn words(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect()
}

/// IOS getsockopt reads its request header from the output buffer.
pub(crate) fn boolean_option_request(fd: i32, level: u32, name: u32) -> Vec<u8> {
    let mut request = words(&[fd as u32, level, name, 4]);
    request.resize(36, 0);
    request
}

pub(crate) fn boolean_option_reply(
    fd: i32,
    level: u32,
    name: u32,
    reply: &[u8],
) -> Result<bool, NetError> {
    let header = words(&[fd as u32, level, name, 4]);
    if reply.len() != 36 || reply[..16] != header {
        return Err(NetError::local(77));
    }
    // C socket boolean options use zero/nonzero, not a required 0/1 value.
    // Testing all four value bytes preserves that meaning without guessing
    // an alternate integer byte order for numeric options such as TTL.
    Ok(reply[16..20].iter().any(|byte| *byte != 0))
}

pub(crate) fn address(ip: u32, port: u16) -> [u8; 8] {
    let p = port.to_be_bytes();
    let i = ip.to_be_bytes();
    [8, 2, p[0], p[1], i[0], i[1], i[2], i[3]]
}
pub(crate) fn read_address(bytes: &[u8]) -> Result<(u32, u16), NetError> {
    if bytes.len() != 8 || bytes[0] != 8 || bytes[1] != 2 {
        return Err(NetError::local(77));
    }
    Ok((
        u32::from_be_bytes(bytes[4..8].try_into().map_err(|_| NetError::local(77))?),
        u16::from_be_bytes([bytes[2], bytes[3]]),
    ))
}
pub(crate) fn addressed(fd: i32, destination: &[u8; 8], send: bool, flags: u32) -> Vec<u8> {
    let mut packet = if send {
        words(&[fd as u32, flags, 1])
    } else {
        words(&[fd as u32, 1])
    };
    packet.extend_from_slice(destination);
    packet.resize(if send { 40 } else { 36 }, 0);
    packet
}

/// Some stock IOS versions reject an explicit port-zero bind. Preserve native
/// support when present; otherwise reserve a real ephemeral port through IOS.
pub(crate) fn bind_port(
    requested: u16,
    mut next: impl FnMut() -> u16,
    mut bind: impl FnMut(u16) -> Result<(), NetError>,
) -> Result<(), NetError> {
    match bind(requested) {
        Err(NetError {
            domain: 1,
            code: -28,
        }) if requested == 0 => {}
        result => return result,
    }
    for _ in 0..16384 {
        let candidate = next();
        if candidate < 49152 {
            return Err(NetError::local(22));
        }
        match bind(candidate) {
            Err(NetError {
                domain: 1,
                code: -3,
            }) => continue,
            result => return result,
        }
    }
    Err(NetError {
        domain: 1,
        code: -3,
    })
}

/// Check limited and subnet-directed IPv4 broadcast addresses.
pub(crate) fn is_broadcast_address(destination: u32, local: u32, mask: u32) -> bool {
    destination == u32::MAX
        || (mask != 0
            && destination != local
            && destination & mask == local & mask
            && destination | mask == u32::MAX)
}

pub(crate) fn check_broadcast_send(
    destination: u32,
    local: u32,
    mask: u32,
    allowed: bool,
) -> Result<(), NetError> {
    if !allowed && is_broadcast_address(destination, local, mask) {
        Err(NetError::local(13))
    } else {
        Ok(())
    }
}

pub(crate) fn dns(response: &[u8]) -> Result<Vec<u32>, NetError> {
    let invalid = || NetError::local(77);
    if response.len() != 0x460 || response[8..12] != [0, 2, 0, 4] {
        return Err(invalid());
    }
    let word = |at: usize| -> Result<u32, NetError> {
        Ok(u32::from_be_bytes(
            response
                .get(at..at + 4)
                .ok_or_else(invalid)?
                .try_into()
                .map_err(|_| invalid())?,
        ))
    };
    let name = word(0)?;
    if name == 0 {
        return Err(invalid());
    }
    // C moves every IOS pointer by the same delta so h_name lands at byte 16.
    // Subtracting h_name's physical address minus 16 gives those relocated
    // offsets without trusting an address outside the returned buffer.
    let origin = (name & 0x3fff_ffff).checked_sub(16).ok_or_else(invalid)?;
    let offset = |pointer: u32, length: usize| -> Result<usize, NetError> {
        if pointer == 0 {
            return Err(invalid());
        }
        let at = ((pointer & 0x3fff_ffff)
            .checked_sub(origin)
            .ok_or_else(invalid)?) as usize;
        if at < 16
            || at
                .checked_add(length)
                .is_none_or(|end| end > response.len())
        {
            return Err(invalid());
        }
        Ok(at)
    };
    if offset(name, 1)? != 16 {
        return Err(invalid());
    }
    if !response[16..0x110].contains(&0) {
        return Err(invalid());
    }
    let pointers = |pointer: u32| -> Result<Vec<usize>, NetError> {
        let mut at = offset(pointer, 4)?;
        if at % 4 != 0 {
            return Err(invalid());
        }
        let mut result = Vec::new();
        loop {
            let value = word(at)?;
            if value == 0 {
                return Ok(result);
            }
            result.push(offset(value, 1)?);
            at = at.checked_add(4).ok_or_else(invalid)?;
        }
    };
    if word(4)? != 0 {
        for alias in pointers(word(4)?)? {
            if !response[alias..].contains(&0) {
                return Err(invalid());
            }
        }
    }
    let mut result = Vec::new();
    for at in pointers(word(12)?)? {
        result.push(word(at)?);
    }
    if result.is_empty() {
        return Err(NetError::local(2));
    }
    Ok(result)
}

/// Keep one absolute end time across partial readiness and interrupted calls.
/// Poll slices are bounded so a pending socket cannot hold the IPC serializer
/// indefinitely and prevent other sockets from making progress.
pub(crate) fn retry<R>(
    timeout: Option<Duration>,
    nonblocking: bool,
    mut now: impl FnMut() -> Result<Duration, NetError>,
    mut operation: impl FnMut() -> Result<R, NetError>,
    mut poll: impl FnMut(i32) -> Result<bool, NetError>,
) -> Result<R, NetError> {
    let end = match timeout {
        Some(timeout) => Some(now()?.checked_add(timeout).ok_or(NetError::local(139))?),
        None => None,
    };
    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(error) if !nonblocking && error.retryable() => {}
            Err(error) => return Err(error),
        }
        loop {
            let milliseconds = if let Some(end) = end {
                let remaining = end
                    .checked_sub(now()?)
                    .filter(|d| !d.is_zero())
                    .ok_or(NetError::local(116))?;
                remaining.as_nanos().div_ceil(1_000_000).min(25) as i32
            } else {
                25
            };
            match poll(milliseconds) {
                Ok(true) => break,
                Ok(false) => {}
                Err(error) if error.retryable() => {}
                Err(error) => return Err(error),
            }
            // A poll does real blocking transport work; no software sleep
            // substitutes for its result. The next iteration uses the same end.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NetError;
    use super::receive_window_setup;

    #[test]
    fn receive_window_resource_limit_keeps_default_socket_window() {
        assert_eq!(
            receive_window_setup(Err(NetError {
                domain: 1,
                code: -49,
            })),
            Ok(())
        );
        assert_eq!(
            receive_window_setup(Err(NetError {
                domain: 1,
                code: -28,
            })),
            Err(NetError {
                domain: 1,
                code: -28,
            })
        );
        assert_eq!(receive_window_setup(Ok(())), Ok(()));
    }

    use super::*;
    #[test]
    fn port_zero_reservation_checks_real_bind_results_and_collisions() {
        let mut calls = Vec::new();
        let mut ports = [49152, 49153].into_iter();
        let result = bind_port(
            0,
            || ports.next().unwrap(),
            |port| {
                calls.push(port);
                match port {
                    0 => Err(NetError {
                        domain: 1,
                        code: -28,
                    }),
                    49152 => Err(NetError {
                        domain: 1,
                        code: -3,
                    }),
                    _ => Ok(()),
                }
            },
        );
        assert_eq!(result, Ok(()));
        assert_eq!(calls, [0, 49152, 49153]);
    }
    #[test]
    fn port_selection_preserves_native_support_and_unrelated_errors() {
        assert_eq!(
            bind_port(0, || panic!("native supported"), |_| Ok(())),
            Ok(())
        );
        let invalid = NetError {
            domain: 1,
            code: -28,
        };
        assert_eq!(
            bind_port(1234, || panic!("explicit port"), |_| Err(invalid)),
            Err(invalid)
        );
        let mut first = true;
        let result = bind_port(
            0,
            || 49152,
            |_| {
                if core::mem::replace(&mut first, false) {
                    Err(invalid)
                } else {
                    Err(NetError {
                        domain: 1,
                        code: -4,
                    })
                }
            },
        );
        assert_eq!(
            result,
            Err(NetError {
                domain: 1,
                code: -4
            })
        );
    }

    #[test]
    fn packets_have_independent_network_byte_order() {
        assert_eq!(
            address(0xc0000201, 0x1234),
            [8, 2, 0x12, 0x34, 192, 0, 2, 1]
        );
        assert_eq!(
            read_address(&[8, 2, 0x12, 0x34, 192, 0, 2, 1]),
            Ok((0xc0000201, 0x1234))
        );
        assert!(read_address(&[0; 8]).is_err());
        assert_eq!(
            &addressed(9, &address(0xc0000201, 0x1234), true, 4)[..20],
            &[
                0, 0, 0, 9, 0, 0, 0, 4, 0, 0, 0, 1, 8, 2, 0x12, 0x34, 192, 0, 2, 1
            ]
        );
        assert_eq!(
            &addressed(9, &address(0xc0000201, 0x1234), true, 0)[..20],
            &[
                0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 1, 8, 2, 0x12, 0x34, 192, 0, 2, 1
            ]
        );
    }
    #[test]
    fn dns_is_owned_and_rejects_out_of_bounds_and_unterminated_lists() {
        let mut wire = [0u8; 0x460];
        wire[..16].copy_from_slice(&[0, 0, 0x11, 0x10, 0, 0, 0, 0, 0, 2, 0, 4, 0, 0, 0x14, 0x40]);
        wire[16..20].copy_from_slice(b"dns\0");
        wire[0x110..0x114].copy_from_slice(&[192, 0, 2, 1]);
        wire[0x340..0x344].copy_from_slice(&[0, 0, 0x12, 0x10]);
        let result = dns(&wire).unwrap();
        wire.fill(0);
        assert_eq!(result, [0xc0000201]);
        assert!(dns(&wire).is_err());
        wire[..16].copy_from_slice(&[0, 0, 0x11, 0x10, 0, 0, 0, 0, 0, 2, 0, 4, 0, 0, 0x15, 0x5c]);
        wire[0x45c..].copy_from_slice(&[0, 0, 0x12, 0x10]);
        assert!(dns(&wire).is_err());
        wire[12..16].copy_from_slice(&[0, 0, 0x14, 0x60]);
        assert!(dns(&wire).is_err());
    }
    #[test]
    fn error_spaces_and_io_kinds_are_not_conflated() {
        let ios = NetError::reply(Ok(-6)).unwrap_err();
        let ipc = NetError::reply(Err(-6)).unwrap_err();
        assert!(ios.retryable());
        assert!(!ipc.retryable());
        assert_ne!(ios.encoded(), ipc.encoded());
        assert_eq!(NetError::decode(ios.encoded()), ios);
        assert_eq!(
            NetError {
                domain: 1,
                code: -3
            }
            .errno(),
            112
        );
        assert_eq!(
            NetError {
                domain: 1,
                code: -14
            }
            .errno(),
            111
        );
        assert_eq!(
            NetError {
                domain: 1,
                code: -76
            }
            .errno(),
            116
        );
    }
    #[test]
    fn readiness_and_interrupts_do_not_reset_deadline() {
        use core::cell::Cell;
        let time = Cell::new(0);
        let calls = Cell::new(0);
        let result = retry::<usize>(
            Some(Duration::from_millis(10)),
            false,
            || Ok(Duration::from_millis(time.get())),
            || {
                calls.set(calls.get() + 1);
                Err(NetError {
                    domain: 1,
                    code: -6,
                })
            },
            |ms| {
                assert!(ms <= 10);
                time.set(time.get() + 4);
                Ok(true)
            },
        );
        assert_eq!(result, Err(NetError::local(116)));
        assert_eq!(calls.get(), 4);
    }

    #[test]
    fn interrupted_operations_and_polls_retry_with_the_same_deadline() {
        use core::cell::Cell;

        let operation_calls = Cell::new(0);
        let poll_calls = Cell::new(0);
        let operation_interrupt = retry::<usize>(
            None,
            false,
            || panic!("no timeout does not read the clock"),
            || {
                operation_calls.set(operation_calls.get() + 1);
                if operation_calls.get() == 1 {
                    Err(NetError {
                        domain: 1,
                        code: -27,
                    })
                } else {
                    Ok(9)
                }
            },
            |_| {
                poll_calls.set(poll_calls.get() + 1);
                Ok(true)
            },
        );
        assert_eq!(operation_interrupt, Ok(9));
        assert_eq!(poll_calls.get(), 1);

        let now_ms = Cell::new(0u64);
        let operation_calls = Cell::new(0);
        let poll_calls = Cell::new(0);
        let poll_interrupt = retry::<usize>(
            Some(Duration::from_millis(10)),
            false,
            || Ok(Duration::from_millis(now_ms.get())),
            || {
                operation_calls.set(operation_calls.get() + 1);
                Err(NetError {
                    domain: 1,
                    code: -6,
                })
            },
            |_| {
                let call = poll_calls.get();
                poll_calls.set(call + 1);
                now_ms.set(now_ms.get() + 4);
                if call == 0 {
                    Err(NetError {
                        domain: 1,
                        code: -27,
                    })
                } else {
                    Ok(call == 1)
                }
            },
        );
        assert_eq!(poll_interrupt, Err(NetError::local(116)));
        assert_eq!(operation_calls.get(), 2);
        assert_eq!(poll_calls.get(), 3);
    }

    #[test]
    fn deadline_overflow_is_reported_before_socket_io() {
        let now = Duration::MAX - Duration::from_millis(1);
        let mut operation_called = false;
        let result = retry(
            Some(Duration::from_millis(2)),
            false,
            || Ok(now),
            || {
                operation_called = true;
                Ok(())
            },
            |_| panic!("overflow fails before polling"),
        );
        assert_eq!(result, Err(NetError::local(139)));
        assert!(!operation_called);
    }
    #[test]
    fn poll_errors_partial_io_eof_and_nonblocking_are_observable() {
        let raw = NetError {
            domain: 2,
            code: -22,
        };
        assert_eq!(
            retry::<usize>(
                None,
                false,
                || panic!(),
                || Err(NetError {
                    domain: 1,
                    code: -6
                }),
                |_| Err(raw)
            ),
            Err(raw)
        );
        assert_eq!(
            retry(None, false, || panic!(), || Ok(3usize), |_| panic!()),
            Ok(3)
        );
        assert_eq!(
            retry(None, false, || panic!(), || Ok(0usize), |_| panic!()),
            Ok(0)
        );
        assert_eq!(
            retry::<usize>(
                None,
                true,
                || panic!(),
                || Err(NetError {
                    domain: 1,
                    code: -6
                }),
                |_| panic!()
            ),
            Err(NetError {
                domain: 1,
                code: -6
            })
        );
    }
}

#[cfg(test)]
mod option_tests {
    use super::*;
    #[test]
    fn boolean_query_has_fixed_wire_header_and_checked_reply() {
        let mut request = boolean_option_request(7, 0xffff, 0x20);
        assert_eq!(
            &request[..20],
            &[
                0, 0, 0, 7, 0, 0, 255, 255, 0, 0, 0, 32, 0, 0, 0, 4, 0, 0, 0, 0
            ]
        );
        assert_eq!(boolean_option_reply(7, 0xffff, 0x20, &request), Ok(false));
        request[16..20].copy_from_slice(&[0, 0, 0, 1]);
        assert_eq!(boolean_option_reply(7, 0xffff, 0x20, &request), Ok(true));
        request[16..20].copy_from_slice(&[1, 0, 0, 0]);
        assert_eq!(boolean_option_reply(7, 0xffff, 0x20, &request), Ok(true));
        request[15] = 3;
        assert_eq!(
            boolean_option_reply(7, 0xffff, 0x20, &request),
            Err(NetError::local(77))
        );
        assert_eq!(
            boolean_option_reply(7, 0xffff, 0x20, &[]),
            Err(NetError::local(77))
        );
        assert_eq!(
            boolean_option_reply(8, 0xffff, 0x20, &boolean_option_request(7, 0xffff, 0x20)),
            Err(NetError::local(77))
        );
    }
}

#[cfg(test)]
mod broadcast_tests {
    #[test]
    fn identifies_limited_and_directed_broadcasts_only() {
        let local = u32::from_be_bytes([192, 168, 1, 42]);
        let mask = u32::from_be_bytes([255, 255, 255, 0]);
        assert!(super::is_broadcast_address(u32::MAX, local, mask));
        assert!(super::is_broadcast_address(
            u32::from_be_bytes([192, 168, 1, 255]),
            local,
            mask
        ));
        assert!(!super::is_broadcast_address(local, local, mask));
        assert!(!super::is_broadcast_address(
            u32::from_be_bytes([192, 168, 2, 255]),
            local,
            mask
        ));
        assert!(!super::is_broadcast_address(
            u32::from_be_bytes([192, 168, 1, 43]),
            local,
            mask
        ));
        assert_eq!(
            super::check_broadcast_send(u32::MAX, local, mask, false),
            Err(super::NetError::local(13))
        );
        assert_eq!(
            super::check_broadcast_send(u32::MAX, local, mask, true),
            Ok(())
        );
        assert_eq!(
            super::check_broadcast_send(local, local, mask, false),
            Ok(())
        );
    }
}
