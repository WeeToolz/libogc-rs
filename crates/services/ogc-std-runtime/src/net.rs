//! Application-owned resources behind the std network ABI.
use alloc::boxed::Box;
use alloc::vec::IntoIter;
use core::ffi::c_void;
use core::time::Duration;
use ogc_network_sys::std_net::{self, NetError, Socket};

pub use ogc_network_sys::std_net::NetRequest;

pub fn network_initialize(attach: bool) -> Result<(), NetError> {
    std_net::initialize(attach)
}
pub fn network_snapshot() -> std_net::NetworkSnapshot {
    std_net::snapshot()
}

fn clock() -> Result<Duration, NetError> {
    ogc_system::monotonic_time().map_err(|_| NetError::local(5))
}
fn owned(socket: Socket) -> i64 {
    Box::into_raw(Box::new(socket)) as usize as i64
}
unsafe fn socket<'a>(pointer: *mut c_void) -> Result<&'a Socket, NetError> {
    if pointer.is_null() || !(pointer as usize).is_multiple_of(core::mem::align_of::<Socket>()) {
        return Err(NetError::local(22));
    }
    Ok(unsafe { &*pointer.cast::<Socket>() })
}
unsafe fn input<'a>(r: &NetRequest) -> Result<&'a [u8], NetError> {
    if r.length > isize::MAX as usize || (r.data.is_null() && r.length != 0) {
        return Err(NetError::local(22));
    }
    if r.length == 0 {
        Ok(&[])
    } else {
        Ok(unsafe { core::slice::from_raw_parts(r.data, r.length) })
    }
}
unsafe fn dispatch(r: &mut NetRequest) -> Result<i64, NetError> {
    match r.operation {
        1 => Ok(owned(Socket::connect(r.ip, r.port, clock)?)),
        2 => {
            if r.flags > 1 {
                return Err(NetError::local(22));
            }
            Ok(owned(Socket::bind(r.ip, r.port, r.flags != 0, clock)?))
        }
        3 => Ok(owned(unsafe { socket(r.handle)? }.clone())),
        4 => {
            unsafe { socket(r.handle)? };
            let handle = unsafe { Box::from_raw(r.handle.cast::<Socket>()) };
            (*handle).release()?;
            Ok(0)
        }
        5 => {
            let (socket, ip, port) = unsafe { socket(r.handle)? }.accept()?;
            r.ip = ip;
            r.port = port;
            Ok(owned(socket))
        }
        6 => {
            if r.flags > 1 {
                return Err(NetError::local(22));
            }
            let (ip, port) = unsafe { socket(r.handle)? }.address(r.flags != 0)?;
            r.ip = ip;
            r.port = port;
            Ok(0)
        }
        7 => {
            if r.flags > 3 {
                return Err(NetError::local(22));
            }
            unsafe { input(r)? };
            let data = if r.length == 0 {
                core::ptr::NonNull::<u8>::dangling().as_ptr()
            } else {
                r.data
            };
            let buffer = unsafe { core::slice::from_raw_parts_mut(data, r.length) };
            let (count, source) =
                unsafe { socket(r.handle)? }.receive(buffer, r.flags & 1 != 0, r.flags & 2 != 0)?;
            if let Some((ip, port)) = source {
                r.ip = ip;
                r.port = port;
            }
            Ok(count as i64)
        }
        8 => {
            if r.flags > 1 {
                return Err(NetError::local(22));
            }
            Ok(unsafe { socket(r.handle)? }.send(
                unsafe { input(r)? },
                if r.flags == 1 {
                    Some((r.ip, r.port))
                } else {
                    None
                },
            )? as i64)
        }
        9 => {
            if r.flags > 1 {
                return Err(NetError::local(22));
            }
            unsafe { socket(r.handle)? }.set_nonblocking(r.flags != 0)?;
            Ok(0)
        }
        10 => {
            if r.flags > 3 || r.nanos >= 1_000_000_000 {
                return Err(NetError::local(22));
            }
            let value = if r.flags & 2 != 0 {
                Some(Duration::new(r.seconds, r.nanos))
            } else {
                None
            };
            unsafe { socket(r.handle)? }.set_timeout(r.flags & 1 != 0, value)?;
            Ok(0)
        }
        11 => {
            if r.flags > 1 {
                return Err(NetError::local(22));
            }
            match unsafe { socket(r.handle)? }.timeout(r.flags != 0)? {
                Some(value) => {
                    r.seconds = value.as_secs();
                    r.nanos = value.subsec_nanos();
                    Ok(1)
                }
                None => Ok(0),
            }
        }
        12 => {
            unsafe { socket(r.handle)? }.shutdown(r.flags.into())?;
            Ok(0)
        }
        13 => {
            unsafe { socket(r.handle)? }.set_option(r.ip, r.port.into(), r.nanos)?;
            Ok(0)
        }
        14 => {
            unsafe { socket(r.handle)? }.udp_connect(r.ip, r.port)?;
            Ok(0)
        }
        15 => Ok(
            Box::into_raw(Box::new(std_net::lookup(unsafe { input(r)? })?.into_iter())) as usize
                as i64,
        ),
        16 | 17 => {
            if r.handle.is_null()
                || !(r.handle as usize).is_multiple_of(core::mem::align_of::<IntoIter<u32>>())
            {
                return Err(NetError::local(22));
            }
            if r.operation == 17 {
                drop(unsafe { Box::from_raw(r.handle.cast::<IntoIter<u32>>()) });
                Ok(0)
            } else {
                Ok(unsafe { &mut *r.handle.cast::<IntoIter<u32>>() }
                    .next()
                    .map_or(1i64 << 32, i64::from))
            }
        }
        18 => Ok(i64::from(
            unsafe { socket(r.handle)? }.bool_option(r.ip, u32::from(r.port))?,
        )),
        _ => Err(NetError::local(134)),
    }
}

/// # Safety
/// request must be null or an aligned, uniquely writable NetRequest. Buffer
/// pointers must have the operation's read/write extent and cannot overlap the
/// request or resource handle. Handles must come from this ABI, stay live for
/// the call, and be released once. DNS next has one exclusive iterator caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_net(request: *mut NetRequest) -> i64 {
    if request.is_null() || !request.is_aligned() {
        return NetError::local(22).encoded();
    }
    match unsafe { dispatch(&mut *request) } {
        Ok(value) => value,
        Err(error) => error.encoded(),
    }
}
