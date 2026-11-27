#![cfg(feature = "format-probe-hooks")]

use core::sync::atomic::{AtomicBool, Ordering};

static FAIL_NEXT_HANDSHAKE: AtomicBool = AtomicBool::new(false);

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

pub fn snapshot_in_flight_async_requests_for_probe(out: &mut [usize]) -> usize {
    let mut count = 0;
    for address in super::ASYNC_IN_FLIGHT
        .iter()
        .map(|slot| slot.load(Ordering::Acquire))
        .filter(|address| *address != 0)
    {
        if let Some(slot) = out.get_mut(count) {
            *slot = address;
        }
        count += 1;
    }
    count
}

pub fn async_callback_reservations_for_probe() -> u32 {
    super::ASYNC_CALLBACKS_PENDING.load(Ordering::Acquire)
}
