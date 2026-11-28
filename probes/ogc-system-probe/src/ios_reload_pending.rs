//! Dolphin-only test of a caller-owned request when IOS is replaced.
use crate::probe_support::ProbeLogger;
use core::ffi::c_void;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use ogc_ipc as ipc;

#[path = "ios_reload_scope.rs"]
mod ios_reload_scope;

const EVENT_HOOK: i32 = 0x1000;
const RELEASE_HOOK: i32 = 0x3002;
const IPC_EEXIST: i32 = -2;
const UNSET: i32 = i32::MIN;
const TIMER_HZ: u64 = 60_750_000;
#[repr(C, align(32))]
struct Buffer([u8; 32]);
// Keep storage valid until cancellation or normal completion.
static mut OLD_IN: Buffer = Buffer([0; 32]);
static mut OLD_OUT: Buffer = Buffer([0xa5; 32]);
static mut NEW_IN: Buffer = Buffer([0; 32]);
static mut NEW_OUT: Buffer = Buffer([0xa5; 32]);
static OLD_COUNT: AtomicU32 = AtomicU32::new(0);
static OLD_RESULT: AtomicI32 = AtomicI32::new(UNSET);
static NEW_COUNT: AtomicU32 = AtomicU32::new(0);
static NEW_RESULT: AtomicI32 = AtomicI32::new(UNSET);
static SCOPE_CLOSE_FD: AtomicI32 = AtomicI32::new(-1);
static SCOPE_WORKER_DONE: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
static FORMAT_COUNT: AtomicU32 = AtomicU32::new(0);
static FORMAT_RESULT: AtomicI32 = AtomicI32::new(UNSET);
static mut FORMAT_OUT: Buffer = Buffer([0xa5; 32]);

unsafe extern "C" {
    fn IOS_IoctlvFormatAsync(
        heap: i32,
        fd: i32,
        ioctl: i32,
        callback: Option<unsafe extern "C" fn(i32, *mut c_void) -> i32>,
        user_data: *mut c_void,
        format: *const core::ffi::c_char,
        ...
    ) -> i32;
}

unsafe extern "C" fn format_callback(result: i32, _: *mut c_void) -> i32 {
    FORMAT_RESULT.store(result, Ordering::Relaxed);
    FORMAT_COUNT.fetch_add(1, Ordering::Release);
    result
}

// Idle Bluetooth ACL input is held by Dolphin's old USB controller.
// No HCI commands or ACL output are submitted, so there is no packet to return.
// Use a separate formatter heap to measure ownership independently of ES.
fn submit_pending_format(heap: i32) -> bool {
    unsafe {
        let fd = ipc::IOS_Open(c"/dev/usb/oh1/57e/305".as_ptr(), ipc::IPC_OPEN_NONE);
        if fd < 0 {
            return false;
        }
        core::ptr::addr_of_mut!(FORMAT_OUT).write(Buffer([0xa5; 32]));
        let result = IOS_IoctlvFormatAsync(
            heap,
            fd,
            1,
            Some(format_callback),
            core::ptr::null_mut(),
            c"bh:d".as_ptr(),
            0x82i32,
            13i32,
            core::ptr::addr_of_mut!(FORMAT_OUT).cast::<u8>().add(1),
            13u32,
        );
        // A fresh response proves the mailbox has processed the submission.
        let barrier = ipc::IOS_Open(c"/dev/fs".as_ptr(), ipc::IPC_OPEN_NONE);
        let barrier_ok = barrier >= 0 && ipc::IOS_Close(barrier) == 0;
        if result != 0 || !barrier_ok {
            let _ = ipc::IOS_Close(fd);
            return false;
        }
        true
    }
}
unsafe extern "C" fn old_callback(result: i32, _: *mut c_void) -> i32 {
    OLD_RESULT.store(result, Ordering::Relaxed);
    OLD_COUNT.fetch_add(1, Ordering::Release);
    result
}
unsafe extern "C" fn new_callback(result: i32, _: *mut c_void) -> i32 {
    NEW_RESULT.store(result, Ordering::Relaxed);
    NEW_COUNT.fetch_add(1, Ordering::Release);
    result
}
fn open_hook() -> i32 {
    unsafe { ipc::IOS_Open(c"/dev/stm/eventhook".as_ptr(), ipc::IPC_OPEN_NONE) }
}
fn control(fd: i32, command: i32) -> i32 {
    let mut input = Buffer([0; 32]);
    let mut output = Buffer([0; 32]);
    unsafe {
        ipc::IOS_Ioctl(
            fd,
            command,
            input.0.as_mut_ptr().cast(),
            32,
            output.0.as_mut_ptr().cast(),
            32,
        )
    }
}
fn release_hook() -> bool {
    let fd = unsafe { ipc::IOS_Open(c"/dev/stm/immediate".as_ptr(), ipc::IPC_OPEN_NONE) };
    if fd < 0 {
        return false;
    }
    let result = control(fd, RELEASE_HOOK);
    let close = ipc::IOS_Close(fd);
    result == 0 && close == 0
}

// Measure capacity without submitting any request to IOS. Release every
// record and reservation acquired by this diagnostic before returning.
fn measure_capacity(logger: &mut ProbeLogger, phase: &str) -> (usize, usize) {
    ipc::poll();
    ogc_lwp::sys::yield_thread();
    let (records, error) = ipc::hold_request_records_until_exhausted();
    let released = ipc::release_held_request_records();
    logger.note_u32(phase, records as u32);
    logger.record(
        "pending reload request capacity measurement releases all records",
        error == ipc::IPC_ENOMEM && records == released,
    );
    let mut slots = [0; 64];
    let mut count = 0;
    while count < slots.len() {
        let Some(token) = ogc_lwp::sys::deferred_reserve_interrupt_slot() else {
            break;
        };
        slots[count] = token;
        count += 1;
    }
    let mut cancelled = true;
    for token in slots[..count].iter().copied() {
        cancelled &=
            ogc_lwp::sys::deferred_cancel_interrupt_slot(token) == ogc_lwp::sys::LWP_SUCCESSFUL;
    }
    logger.note_u32("pending reload available deferred slots", count as u32);
    logger.record(
        "pending reload deferred capacity measurement releases all slots",
        count < slots.len() && cancelled,
    );
    (records, count)
}

// Run real submissions from an LWP other than the replacement owner.
// Return a bit mask so the parent can inspect the results after joining.
unsafe extern "C" fn competing_scope_worker(_: *mut c_void) -> *mut c_void {
    let sync_rejected = match ipc::open_file(c"/dev/fs", ipc::IPC_OPEN_NONE) {
        Err(error) => error == ipc::IPC_EQUEUEFULL,
        Ok(fd) => {
            let _ = fd.close();
            false
        }
    };
    let async_rejected = ipc::open_file_async(
        c"/dev/fs",
        ipc::IPC_OPEN_NONE,
        Some(new_callback),
        core::ptr::null_mut(),
    ) == ipc::IPC_EQUEUEFULL;
    let replacement_rejected = matches!(ipc::IosReplacement::begin(), Err(ipc::IPC_EQUEUEFULL));
    let es_close_rejected = ogc_es_sys::close() == ipc::IPC_EQUEUEFULL;
    let descriptor_close_rejected = match SCOPE_CLOSE_FD.swap(-1, Ordering::AcqRel) {
        raw if raw >= 0 => {
            let descriptor = ipc::FileDescriptor::new(raw);
            match descriptor.close_with_status() {
                Err(ipc::FileCloseError::Submission(descriptor, error)) => {
                    SCOPE_CLOSE_FD.store(descriptor.raw(), Ordering::Release);
                    error == ipc::IPC_EQUEUEFULL
                }
                Ok(()) | Err(ipc::FileCloseError::Reply(_)) => false,
            }
        }
        _ => false,
    };
    let result = usize::from(sync_rejected)
        | (usize::from(async_rejected) << 1)
        | (usize::from(replacement_rejected) << 2)
        | (usize::from(es_close_rejected) << 3)
        | (usize::from(descriptor_close_rejected) << 4);
    SCOPE_WORKER_DONE.store(true, Ordering::Release);
    result as *mut c_void
}

// Exercise the reservation gate without claiming that an IOS launch occurred.
// Dropping the scope on this path must not cancel any accepted request.
fn verify_submission_scope(logger: &mut ProbeLogger) -> bool {
    logger.section("IOS replacement submission scope");
    let es_init = ogc_es_sys::init();
    logger.note_i32("replacement scope ES initialization", es_init);
    logger.record("replacement scope initializes ES", es_init == 0);
    if es_init != 0 {
        return false;
    }
    let descriptor = match ipc::open_file(c"/dev/fs", ipc::IPC_OPEN_NONE) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            logger.note_i32("replacement scope descriptor open result", error);
            logger.record("replacement scope opens descriptor for close test", false);
            let _ = ogc_es_sys::close();
            return false;
        }
    };
    logger.record("replacement scope opens descriptor for close test", true);
    let raw_descriptor = descriptor.raw();
    // FileDescriptor has no Drop implementation; the worker now owns this raw
    // handle until it returns the handle through SCOPE_CLOSE_FD.
    SCOPE_CLOSE_FD.store(raw_descriptor, Ordering::Release);
    let before = measure_capacity(logger, "replacement scope initial request records");
    let scope = ipc::IosReplacement::begin();
    logger.record("replacement scope acquired", scope.is_ok());
    let Ok(scope) = scope else {
        let _ = ipc::FileDescriptor::new(SCOPE_CLOSE_FD.swap(-1, Ordering::AcqRel)).close();
        let _ = ogc_es_sys::close();
        return false;
    };
    let nested = matches!(ipc::IosReplacement::begin(), Err(ipc::IPC_EQUEUEFULL));
    logger.record("replacement scope rejects nested launch", nested);
    NEW_COUNT.store(0, Ordering::Release);
    NEW_RESULT.store(UNSET, Ordering::Release);
    let rejected = ipc::open_file_async(
        c"/dev/fs",
        ipc::IPC_OPEN_NONE,
        Some(new_callback),
        core::ptr::null_mut(),
    ) == ipc::IPC_EQUEUEFULL;
    logger.record(
        "replacement scope rejects asynchronous submission",
        rejected,
    );
    let sync_ok = ipc::open_file(c"/dev/fs", ipc::IPC_OPEN_NONE).is_ok_and(|fd| fd.close().is_ok());
    logger.record("replacement scope permits owner synchronous IPC", sync_ok);
    let mut thread = ogc_lwp::sys::LWP_THREAD_NULL;
    SCOPE_WORKER_DONE.store(false, Ordering::Release);
    // SAFETY: the worker takes no input pointer and uses a workspace stack.
    // Join it before releasing the replacement scope.
    let created = unsafe {
        ogc_lwp::sys::create_thread(
            &mut thread,
            Some(competing_scope_worker),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            40 * 1024,
            64,
        )
    };
    let mut worker_result = core::ptr::null_mut();
    let joined = created == ogc_lwp::sys::LWP_SUCCESSFUL
        && unsafe { ogc_lwp::sys::join_thread(thread, &mut worker_result) }
            == ogc_lwp::sys::LWP_SUCCESSFUL;
    let worker_finished = if created != ogc_lwp::sys::LWP_SUCCESSFUL || joined {
        true
    } else {
        let started = ogc_lwp::sys::now_ticks();
        while !SCOPE_WORKER_DONE.load(Ordering::Acquire)
            && ogc_lwp::sys::now_ticks().wrapping_sub(started) < TIMER_HZ * 5
        {
            ogc_lwp::sys::yield_thread();
        }
        SCOPE_WORKER_DONE.load(Ordering::Acquire)
    };
    let worker_quiescent = ios_reload_scope::worker_is_quiescent(joined, worker_finished);
    logger.record(
        "replacement scope worker stops IPC before scope release",
        worker_quiescent,
    );
    if !worker_quiescent {
        // Keep the gate active if a created worker may still submit IPC.
        core::mem::forget(scope);
        return false;
    }
    let competing = joined && worker_result as usize == 31;
    logger.record(
        "replacement scope rejects competing thread sync async nested replacement ES close and descriptor close",
        competing,
    );
    drop(scope);
    let recovered =
        ipc::open_file(c"/dev/fs", ipc::IPC_OPEN_NONE).is_ok_and(|fd| fd.close().is_ok());
    logger.record("replacement scope drop permits fresh request", recovered);
    let retained_descriptor = SCOPE_CLOSE_FD.swap(-1, Ordering::AcqRel);
    let descriptor_recovered = retained_descriptor >= 0
        && ipc::FileDescriptor::new(retained_descriptor)
            .close()
            .is_ok();
    logger.record(
        "rejected descriptor close retains ownership for retry",
        descriptor_recovered,
    );
    let mut consumption_count = 0xa5a5_a5a5;
    // SAFETY: The output points to a writable u32. This read-only query checks
    // that a rejected ES close kept the descriptor usable.
    let es_usable = unsafe {
        ogc_es_sys::get_consumption_count(u64::MAX, &mut consumption_count) >= 0
            && consumption_count == 0
    };
    logger.record("ES remains usable after rejected close", es_usable);
    let es_closed = ogc_es_sys::close() == 0;
    logger.record("ES closes after replacement scope ends", es_closed);
    let after = measure_capacity(logger, "replacement scope final request records");
    let clean = before == after && NEW_COUNT.load(Ordering::Acquire) == 0;
    logger.record(
        "replacement scope rejection releases resources without callback",
        clean,
    );
    nested && rejected && sync_ok && competing && recovered && es_usable && es_closed && clean
}

pub(super) fn run(logger: &mut ProbeLogger, version: i32) {
    if !verify_submission_scope(logger) {
        return;
    }
    let heap = match ipc::HeapId::create(1024) {
        Ok(heap) => heap.raw(),
        Err(_) => {
            logger.record("pending reload creates formatter heap", false);
            return;
        }
    };
    logger.record("pending reload creates formatter heap", true);
    for iteration in 1..=3 {
        logger.section("Outstanding application IPC across IOS replacement");
        logger.note_u32("pending reload iteration", iteration);
        if !run_once(logger, version, heap, iteration == 1, iteration == 3) {
            break;
        }
    }
}

fn run_once(
    logger: &mut ProbeLogger,
    version: i32,
    heap: i32,
    probe_launch_failure: bool,
    inject_handshake_failure: bool,
) -> bool {
    OLD_COUNT.store(0, Ordering::Release);
    OLD_RESULT.store(UNSET, Ordering::Release);
    NEW_COUNT.store(0, Ordering::Release);
    NEW_RESULT.store(UNSET, Ordering::Release);
    FORMAT_COUNT.store(0, Ordering::Release);
    FORMAT_RESULT.store(UNSET, Ordering::Release);
    // Remove the library hook so reload shutdown cannot release our hook.
    let closed = ogc_stm::close();
    logger.record("pending reload closes library STM hook", closed == 0);
    if closed != 0 {
        return false;
    }
    let before = measure_capacity(logger, "pending reload initial available request records");
    // The capacity check processes earlier deferred replies. Use its settled
    // point as the baseline for the two requests under test.
    let async_baseline = ipc::in_flight_async_request_count_for_probe();
    let callback_baseline = ipc::async_callback_reservations_for_probe();
    logger.note_u32(
        "pending reload baseline in-flight trackers",
        async_baseline as u32,
    );
    logger.note_u32(
        "pending reload baseline callback reservations",
        callback_baseline,
    );
    let old_fd = open_hook();
    logger.record("pending reload opens independent STM hook", old_fd >= 0);
    if old_fd < 0 {
        let _ = ogc_stm::init();
        return false;
    }
    // SAFETY: Each request has separate, aligned, static buffers.
    let submitted = unsafe {
        ipc::IOS_IoctlAsync(
            old_fd,
            EVENT_HOOK,
            core::ptr::addr_of_mut!(OLD_IN).cast(),
            32,
            core::ptr::addr_of_mut!(OLD_OUT).cast(),
            32,
            Some(old_callback),
            core::ptr::null_mut(),
        )
    };
    // Dolphin returns EEXIST if the async hook reached IOS and is pending.
    // The runner has a timeout if this invariant fails.
    let duplicate = if submitted == 0 {
        control(old_fd, EVENT_HOOK)
    } else {
        UNSET
    };
    let pending =
        submitted == 0 && duplicate == IPC_EEXIST && OLD_COUNT.load(Ordering::Acquire) == 0;
    logger.note_i32("pending reload old submit", submitted);
    logger.note_i32("pending reload duplicate IOS reply", duplicate);
    logger.record(
        "pending reload old IOS holds real application request",
        pending,
    );
    if !pending {
        let _ = release_hook();
        let _ = ipc::IOS_Close(old_fd);
        let _ = ogc_stm::init();
        return false;
    }
    // A failed owner request is not proof that IOS was replaced. Dropping
    // the scope without finish must leave the old IOS request alone.
    let aborted_scope = if let Ok(scope) = ipc::IosReplacement::begin() {
        let rejected =
            unsafe { ipc::IOS_Open(c"/dev/libogc-nonexistent".as_ptr(), ipc::IPC_OPEN_NONE) };
        if rejected >= 0 {
            let _ = ipc::IOS_Close(rejected);
        }
        drop(scope);
        rejected < 0
            && control(old_fd, EVENT_HOOK) == IPC_EEXIST
            && OLD_COUNT.load(Ordering::Acquire) == 0
    } else {
        false
    };
    logger.record(
        "aborted IOS replacement scope preserves pending request",
        aborted_scope,
    );
    let format_before = ipc::FORMAT_PROBE_LIVE_ALLOCATIONS.load(Ordering::Acquire);
    ipc::reset_format_heap_ledger();
    let formatted = submit_pending_format(heap);
    let format_pending = formatted
        && FORMAT_COUNT.load(Ordering::Acquire) == 0
        && ipc::FORMAT_PROBE_LIVE_ALLOCATIONS.load(Ordering::Acquire) > format_before;
    logger.record(
        "pending reload old IOS holds formatted Bluetooth ACL receive",
        format_pending,
    );
    if !format_pending {
        let _ = release_hook();
        let _ = ipc::IOS_Close(old_fd);
        let _ = ogc_stm::init();
        return false;
    }
    let pending_tracker_count = ipc::in_flight_async_request_count_for_probe();
    let pending_callback_reservations = ipc::async_callback_reservations_for_probe();
    let mut pending_addresses = [ipc::AsyncRequestSnapshot::default(); 64];
    let pending_address_count =
        ipc::snapshot_in_flight_async_requests_for_probe(&mut pending_addresses);
    logger.note_u32(
        "pending reload pending in-flight trackers",
        pending_tracker_count as u32,
    );
    logger.note_u32(
        "pending reload pending callback reservations",
        pending_callback_reservations,
    );
    logger.note_u32(
        "pending reload pending raw callback count",
        OLD_COUNT.load(Ordering::Acquire),
    );
    logger.note_u32(
        "pending reload pending formatter callback count",
        FORMAT_COUNT.load(Ordering::Acquire),
    );
    logger.record(
        "pending reload raw and formatted requests occupy two owned tracker entries",
        pending_tracker_count == async_baseline + 2
            && pending_callback_reservations == callback_baseline + 2,
    );
    let mut events = [ipc::AllocationEvent::default(); ipc::FORMAT_PROBE_LEDGER_CAPACITY];
    let submitted_events = ipc::read_format_heap_ledger(&mut events);
    let in_flight = measure_capacity(logger, "pending reload in-flight available request records");
    logger.record(
        "pending reload old requests occupy two records and two deferred slots",
        before.0.checked_sub(2) == Some(in_flight.0)
            && before.1.checked_sub(2) == Some(in_flight.1),
    );
    if probe_launch_failure {
        const INJECTED_LAUNCH_ERROR: i32 = -0x3f01;
        let es_initialized = ogc_es_sys::init() == 0;
        logger.record(
            "pending reload initializes ES before injected launch failure",
            es_initialized,
        );
        if !es_initialized {
            return false;
        }
        let injection_accepted = ogc_ios::sys::fail_next_launch_for_probe(INJECTED_LAUNCH_ERROR);
        logger.record(
            "pending reload accepts one-shot ES launch failure",
            injection_accepted,
        );
        let launch_result = ogc_ios::launch_new_ios(version);
        logger.note_i32("pending reload injected ES launch result", launch_result);
        let error_returned = launch_result == INJECTED_LAUNCH_ERROR;
        logger.record(
            "failed IOS launch returns the injected ES error",
            injection_accepted && error_returned,
        );
        let version_invalidated = ogc_ios::get_version() == ogc_ios::IOS_EBADVERSION;
        logger.record(
            "failed IOS launch leaves the IOS version word invalid like C",
            version_invalidated,
        );
        let ipc_usable =
            ipc::open_file(c"/dev/fs", ipc::IPC_OPEN_NONE).is_ok_and(|fd| fd.close().is_ok());
        let old_request_still_pending = control(old_fd, EVENT_HOOK) == IPC_EEXIST
            && OLD_COUNT.load(Ordering::Acquire) == 0
            && FORMAT_COUNT.load(Ordering::Acquire) == 0
            && ipc::FORMAT_PROBE_LIVE_ALLOCATIONS.load(Ordering::Acquire) > format_before;
        let mut after_launch_error = [ipc::AsyncRequestSnapshot::default(); 64];
        let after_launch_error_count =
            ipc::snapshot_in_flight_async_requests_for_probe(&mut after_launch_error);
        let old_requests_preserved = after_launch_error_count == pending_address_count
            && pending_addresses[..pending_address_count]
                .iter()
                .all(|pending| {
                    after_launch_error[..after_launch_error_count]
                        .iter()
                        .any(|current| current.address == pending.address)
                });
        logger.record(
            "failed IOS launch releases its scope and preserves pending requests",
            ipc_usable && old_request_still_pending && old_requests_preserved,
        );
        if !error_returned
            || !version_invalidated
            || !ipc_usable
            || !old_request_still_pending
            || !old_requests_preserved
        {
            return false;
        }
    }
    // Leave the request pending. Same-version reload constructs a new IOS in Dolphin.
    if inject_handshake_failure {
        ipc::fail_next_replacement_handshake_for_probe();
    }
    let result = ogc_ios::reload_ios(version);
    logger.note_i32("pending reload result", result);
    logger.record(
        "pending reload replaces IOS successfully",
        result == 0 && ogc_ios::get_version() == version,
    );
    if result != 0 {
        return false;
    }
    if inject_handshake_failure {
        let failed_tracker_count = ipc::in_flight_async_request_count_for_probe();
        let failed_callback_reservations = ipc::async_callback_reservations_for_probe();
        let mut failed_addresses = [ipc::AsyncRequestSnapshot::default(); 64];
        let failed_address_count =
            ipc::snapshot_in_flight_async_requests_for_probe(&mut failed_addresses);
        logger.note_u32(
            "pending reload failed-handshake in-flight trackers",
            failed_tracker_count as u32,
        );
        logger.note_u32(
            "pending reload failed-handshake callback reservations",
            failed_callback_reservations,
        );
        logger.note_u32(
            "pending reload failed-handshake raw callback count",
            OLD_COUNT.load(Ordering::Acquire),
        );
        logger.note_u32(
            "pending reload failed-handshake formatter callback count",
            FORMAT_COUNT.load(Ordering::Acquire),
        );
        logger.note_u32(
            "pending reload failed-handshake formatter live allocations",
            ipc::FORMAT_PROBE_LIVE_ALLOCATIONS.load(Ordering::Acquire) as u32,
        );
        let pending_addresses_preserved = pending_address_count <= pending_addresses.len()
            && failed_address_count <= failed_addresses.len()
            && pending_addresses[..pending_address_count]
                .iter()
                .all(|pending| {
                    failed_addresses[..failed_address_count]
                        .iter()
                        .any(|failed| failed.address == pending.address)
                });
        let mut extra_tracker_count = 0;
        let mut extra_stm_event_hook = false;
        for (index, request) in failed_addresses[..failed_address_count].iter().enumerate() {
            if !pending_addresses[..pending_address_count]
                .iter()
                .any(|pending| pending.address == request.address)
            {
                extra_tracker_count += 1;
                extra_stm_event_hook |= request.command == 6
                    && request.descriptor >= 0
                    && request.callback != 0
                    && request.user_data == 0;
                logger.note_u32_index(
                    "pending reload extra tracker address",
                    index as i32,
                    request.address as u32,
                );
                logger.note_u32_index(
                    "pending reload extra tracker command",
                    index as i32,
                    request.command,
                );
                logger.note_i32_index(
                    "pending reload extra tracker descriptor",
                    index as i32,
                    request.descriptor,
                );
                logger.note_u32_index(
                    "pending reload extra tracker callback",
                    index as i32,
                    request.callback as u32,
                );
                logger.note_u32_index(
                    "pending reload extra tracker userdata",
                    index as i32,
                    request.user_data as u32,
                );
            }
        }
        let failed_handshake_preserved = pending_addresses_preserved
            && failed_tracker_count >= pending_tracker_count
            && failed_callback_reservations >= pending_callback_reservations
            && extra_tracker_count == 1
            && extra_stm_event_hook
            && OLD_COUNT.load(Ordering::Acquire) == 0
            && FORMAT_COUNT.load(Ordering::Acquire) == 0
            && ipc::FORMAT_PROBE_LIVE_ALLOCATIONS.load(Ordering::Acquire) > format_before;
        logger.record(
            "pending reload failed fresh handshake preserves requests without callbacks",
            failed_handshake_preserved,
        );
        let stm_closed_before_retry = ogc_stm::close() == 0;
        let mut remaining_addresses = [ipc::AsyncRequestSnapshot::default(); 64];
        let remaining_address_count =
            ipc::snapshot_in_flight_async_requests_for_probe(&mut remaining_addresses);
        let old_requests_remain_after_stm_close = remaining_address_count == pending_address_count
            && pending_addresses[..pending_address_count]
                .iter()
                .all(|pending| {
                    remaining_addresses[..remaining_address_count]
                        .iter()
                        .any(|remaining| remaining.address == pending.address)
                });
        logger.record(
            "pending reload closes the new IOS STM event hook before retry",
            stm_closed_before_retry && old_requests_remain_after_stm_close,
        );
        if !stm_closed_before_retry || !old_requests_remain_after_stm_close {
            return false;
        }
        let retry_retirement = if let Ok(scope) = ipc::IosReplacement::begin() {
            // SAFETY: The IOS replacement and mailbox reinitialization have
            // completed. `finish` performs a fresh `/dev/es` round trip before
            // it releases the outstanding request storage.
            unsafe { scope.finish() };
            true
        } else {
            false
        };
        let retry_tracker_count = ipc::in_flight_async_request_count_for_probe();
        logger.note_u32(
            "pending reload successful retry in-flight trackers",
            retry_tracker_count as u32,
        );
        logger.record(
            "pending reload successful handshake retry retires preserved requests",
            retry_retirement && retry_tracker_count == async_baseline,
        );
        if !failed_handshake_preserved || !retry_retirement || retry_tracker_count != async_baseline
        {
            return false;
        }
    }
    // The old numeric descriptor may refer to a different device now.
    let closed = if inject_handshake_failure {
        0
    } else {
        ogc_stm::close()
    };
    logger.record("pending reload new library STM hook closes", closed == 0);
    if closed != 0 {
        return false;
    }
    let start = ogc_system::time();
    while (OLD_COUNT.load(Ordering::Acquire) == 0 || FORMAT_COUNT.load(Ordering::Acquire) == 0)
        && ogc_system::time().wrapping_sub(start) < TIMER_HZ * 2
    {
        ipc::poll();
        ogc_lwp::sys::yield_thread();
    }
    let after = measure_capacity(
        logger,
        "pending reload post-replacement available request records",
    );
    logger.record(
        "pending reload cancellation restores request and callback capacity",
        after == before,
    );
    let count = ipc::read_format_heap_ledger(&mut events);
    // Later ES calls may reuse their own heap allocations. Check only the
    // records created by this submission in the separate formatter heap.
    let heap_balanced = submitted_events > 0
        && submitted_events < count
        && count <= events.len()
        && events[..submitted_events].iter().all(|allocation| {
            let mut pair = [*allocation, ipc::AllocationEvent::default()];
            let mut matches = 0;
            for event in &events[submitted_events..count] {
                if event.pointer == allocation.pointer && event.size == allocation.size {
                    pair[1] = *event;
                    matches += 1;
                }
            }
            matches == 1 && ipc::allocation_ledger_balanced(&pair)
        });
    let completed = FORMAT_COUNT.load(Ordering::Acquire) == 1
        && FORMAT_RESULT.load(Ordering::Acquire) == ipc::IPC_ENOENT;
    let format_after = ipc::FORMAT_PROBE_LIVE_ALLOCATIONS.load(Ordering::Acquire);
    logger.note_u32(
        "pending reload formatted callback count",
        FORMAT_COUNT.load(Ordering::Acquire),
    );
    logger.note_i32(
        "pending reload formatted callback result",
        FORMAT_RESULT.load(Ordering::Acquire),
    );
    logger.note_u32(
        "pending reload formatted live allocations before",
        format_before as u32,
    );
    logger.note_u32(
        "pending reload formatted live allocations after",
        format_after as u32,
    );
    let format_clean = completed && format_after == format_before && heap_balanced;
    logger.record(
        "pending reload formatted cancellation frees all owned buffers",
        format_clean,
    );
    // SAFETY: cancellation callback completed; IOS no longer owns these buffers.
    let format_output_untouched =
        completed && unsafe { core::ptr::addr_of!(FORMAT_OUT).read().0 == [0xa5; 32] };
    logger.record(
        "pending reload formatted cancellation does not copy staged output",
        format_output_untouched,
    );
    let new_fd = open_hook();
    logger.record("pending reload opens new IOS hook", new_fd >= 0);
    if new_fd < 0 {
        let _ = ogc_stm::init();
        return false;
    }
    let submitted = unsafe {
        ipc::IOS_IoctlAsync(
            new_fd,
            EVENT_HOOK,
            core::ptr::addr_of_mut!(NEW_IN).cast(),
            32,
            core::ptr::addr_of_mut!(NEW_OUT).cast(),
            32,
            Some(new_callback),
            core::ptr::null_mut(),
        )
    };
    let duplicate = if submitted == 0 {
        control(new_fd, EVENT_HOOK)
    } else {
        UNSET
    };
    logger.note_i32("pending reload new IOS submit result", submitted);
    logger.note_i32("pending reload new IOS duplicate result", duplicate);
    logger.record(
        "pending reload new IOS holds new request",
        submitted == 0 && duplicate == IPC_EEXIST,
    );
    let released = release_hook();
    logger.record("pending reload new IOS release command succeeds", released);
    let start = ogc_system::time();
    while NEW_COUNT.load(Ordering::Acquire) == 0
        && ogc_system::time().wrapping_sub(start) < TIMER_HZ * 2
    {
        ipc::poll();
        ogc_lwp::sys::yield_thread();
    }
    let closed = ipc::IOS_Close(new_fd);
    let recovered = measure_capacity(logger, "pending reload recovered available request records");
    logger.record(
        "pending reload fresh request releases its record and deferred slot",
        recovered == after,
    );
    let initialized = ogc_stm::init();
    logger.note_u32(
        "pending reload old callback count",
        OLD_COUNT.load(Ordering::Acquire),
    );
    logger.note_u32(
        "pending reload new callback count",
        NEW_COUNT.load(Ordering::Acquire),
    );
    logger.note_i32(
        "pending reload new callback result",
        NEW_RESULT.load(Ordering::Acquire),
    );
    logger.record(
        "pending reload new hook releases exactly once",
        released
            && NEW_COUNT.load(Ordering::Acquire) == 1
            && NEW_RESULT.load(Ordering::Acquire) == 0,
    );
    logger.record(
        "pending reload old callback receives cancellation exactly once",
        OLD_COUNT.load(Ordering::Acquire) == 1
            && OLD_RESULT.load(Ordering::Acquire) == ipc::IPC_ENOENT,
    );
    logger.record(
        "pending reload restores STM and accepts fresh IPC",
        closed == 0
            && initialized >= 0
            && ipc::open_file(c"/dev/fs", ipc::IPC_OPEN_NONE).is_ok_and(|fd| fd.close().is_ok()),
    );
    format_clean
        && format_output_untouched
        && after == before
        && recovered == before
        && OLD_COUNT.load(Ordering::Acquire) == 1
        && OLD_RESULT.load(Ordering::Acquire) == ipc::IPC_ENOENT
        && NEW_COUNT.load(Ordering::Acquire) == 1
        && NEW_RESULT.load(Ordering::Acquire) == 0
        && initialized >= 0
}
