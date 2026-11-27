#![cfg(feature = "format-probe-hooks")]

use core::sync::atomic::{AtomicBool, Ordering};

static FAIL_NEXT_HANDSHAKE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AsyncRequestSnapshot {
    pub address: usize,
    pub command: u32,
    pub descriptor: i32,
    pub callback: usize,
    pub user_data: usize,
}

fn snapshot_request(address: usize, request: &super::IpcRequest) -> AsyncRequestSnapshot {
    AsyncRequestSnapshot {
        address,
        command: request.cmd,
        descriptor: request.fd,
        callback: request
            .cb
            .map_or(0, |callback| callback as *const () as usize),
        user_data: request.usrdata as usize,
    }
}

#[cfg(test)]
mod tests {
    use super::snapshot_request;
    use crate::IpcRequest;
    use core::ffi::c_void;

    unsafe extern "C" fn callback(_: i32, _: *mut c_void) -> i32 {
        0
    }

    #[test]
    fn request_snapshot_copies_command_descriptor_and_callback_identity() {
        let mut request = IpcRequest::zeroed();
        request.cmd = 6;
        request.fd = 9;
        request.cb = Some(callback);
        request.usrdata = 0x1234usize as *mut c_void;

        let snapshot = snapshot_request(0x5678, &request);

        assert_eq!(snapshot.address, 0x5678);
        assert_eq!(snapshot.command, 6);
        assert_eq!(snapshot.descriptor, 9);
        assert_eq!(snapshot.callback, callback as *const () as usize);
        assert_eq!(snapshot.user_data, 0x1234);
    }
}

/// Inject one failed fresh-handshake result for the Dolphin IPC replacement probe.
pub fn fail_next_replacement_handshake_for_probe() {
    FAIL_NEXT_HANDSHAKE.store(true, Ordering::Release);
}

pub(crate) fn take_handshake_failure() -> bool {
    FAIL_NEXT_HANDSHAKE.swap(false, Ordering::AcqRel)
}

pub fn in_flight_async_request_count_for_probe() -> usize {
    super::ASYNC_IN_FLIGHT
        .iter()
        .filter(|slot| slot.load(Ordering::Acquire) != 0)
        .count()
}

pub fn snapshot_in_flight_async_requests_for_probe(out: &mut [AsyncRequestSnapshot]) -> usize {
    let irq_state = unsafe { super::lock_by_msr() };
    let mut count = 0;
    for address in super::ASYNC_IN_FLIGHT
        .iter()
        .map(|slot| slot.load(Ordering::Acquire))
    {
        if address != 0 {
            let request = unsafe { &*(address as *const super::IpcRequest) };
            if let Some(slot) = out.get_mut(count) {
                *slot = snapshot_request(address, request);
            }
            count += 1;
        }
    }
    unsafe { super::unlock_by_msr(irq_state) };
    count
}

pub fn async_callback_reservations_for_probe() -> u32 {
    super::ASYNC_CALLBACKS_PENDING.load(Ordering::Acquire)
}
