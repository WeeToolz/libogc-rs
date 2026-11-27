#[cfg(target_arch = "powerpc")]
use core::ffi::{CStr, c_void};
use core::fmt::Write;
#[cfg(target_arch = "powerpc")]
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_ipc::{
    HeapId, IOS_OpenAsync, IPC_EINVAL, IPC_ENOMEM, IPC_OPEN_NONE, iosAlloc, iosCreateHeap, iosFree,
    poll as ipc_poll, set_debug_hook,
};
#[cfg(target_arch = "powerpc")]
use ogc_system as system;
use ogc_video as video;

#[cfg(target_arch = "powerpc")]
unsafe extern "C" {
    fn IOS_IoctlvFormatAsync(
        heap: i32,
        fd: i32,
        ioctl: i32,
        callback: ogc_ipc::Ipccallback,
        userdata: *mut c_void,
        format: *const u8,
        ...
    ) -> i32;
}

use crate::probe_support::ProbeLogger;

const IPC_ASYNC_PENDING: i32 = -20000;
const IPC_ASYNC_UNSET: i32 = -9999;
#[cfg(target_arch = "powerpc")]
const WII_TIMER_CLOCK_HZ: u64 = 60_750_000;
const AUTO_WINDOW_USEC: u64 = 1_000_000;
#[cfg(feature = "dolphin-automation")]
const AUTOMATION_FRAMES: u32 = 600;

#[cfg(target_arch = "powerpc")]
static RESULT_FS: AtomicI32 = AtomicI32::new(IPC_ASYNC_UNSET);
#[cfg(target_arch = "powerpc")]
static COUNT_FS: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_STAGE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_A: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_B: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_C: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static REINIT_MSR: AtomicU32 = AtomicU32::new(u32::MAX);
#[cfg(target_arch = "powerpc")]
static SENT_DEBUG_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static RESET_QUEUE_RESULTS: [AtomicI32; 3] = [const { AtomicI32::new(IPC_ASYNC_UNSET) }; 3];
#[cfg(target_arch = "powerpc")]
static RESET_QUEUE_CALLS: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];
#[cfg(target_arch = "powerpc")]
static RESET_QUEUE_CALLBACK_ORDER: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];
#[cfg(target_arch = "powerpc")]
static RESET_QUEUE_CALLBACK_SEQUENCE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static RESET_FORMAT_RESULT: AtomicI32 = AtomicI32::new(IPC_ASYNC_UNSET);
#[cfg(target_arch = "powerpc")]
static RESET_FORMAT_CALLS: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static RESET_FORMAT_CALLBACK_ORDER: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static RESET_FORMAT_USERDATA: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static RESET_FORMAT_OUTPUT: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);
#[cfg(target_arch = "powerpc")]
static RESET_FORMAT_OUTPUT_UNTOUCHED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

pub fn monitor(logger: &mut ProbeLogger) -> ! {
    logger.section("IPC ASYNC FS PROBE START");
    logger.note("no USB calls in this diagnostic");
    logger.note("manual IPC poll delayed until auto window ends");

    let mut state = IpcAsyncProbeState::new();

    #[cfg(target_arch = "powerpc")]
    {
        state.start_ticks = system::time();
        reset_probe();
        set_debug_hook(Some(ipc_debug_hook));
        let before = read_msr();
        ogc_ipc::reinitialize();
        let after = read_msr();
        logger.note_u32("IPC reinitialize MSR before", before);
        logger.note_u32(
            "IPC reinitialize MSR during",
            REINIT_MSR.load(Ordering::Acquire),
        );
        logger.note_u32("IPC reinitialize MSR after", after);
        logger.record(
            "IPC reinitialize masks and restores interrupts",
            REINIT_MSR.load(Ordering::Acquire) & 0x8000 == 0 && after & 0x8000 == before & 0x8000,
        );
        let saved = ogc_irq::irq_disable();
        REINIT_MSR.store(u32::MAX, Ordering::Release);
        ogc_ipc::reinitialize();
        let nested_after = read_msr();
        ogc_irq::irq_restore(saved);
        logger.record(
            "IPC reinitialize preserves disabled interrupts",
            REINIT_MSR.load(Ordering::Acquire) & 0x8000 == 0 && nested_after & 0x8000 == 0,
        );
        logger.record(
            "zero-sized IPC heap returns no-memory",
            HeapId::create(0) == Err(IPC_ENOMEM),
        );
        logger.record(
            "negative IPC heap size is rejected",
            HeapId::create(-1) == Err(IPC_EINVAL),
        );
        logger.record(
            "C IPC heap creation returns no-memory for zero size",
            iosCreateHeap(0) == IPC_ENOMEM,
        );
        let heap = iosCreateHeap(1024);
        let allocation = iosAlloc(heap, 32);
        let allocation_ok = heap >= 0 && !allocation.is_null();
        let contents_ok = if allocation_ok {
            // SAFETY: `iosAlloc` returned a live 32-byte allocation.
            unsafe {
                core::ptr::write_bytes(allocation.cast::<u8>(), 0x5a, 32);
                core::slice::from_raw_parts(allocation.cast::<u8>(), 32)
                    .iter()
                    .all(|byte| *byte == 0x5a)
            }
        } else {
            false
        };
        if allocation_ok {
            // SAFETY: This pointer was returned by `iosAlloc` for `heap`.
            unsafe { iosFree(heap, allocation) };
        }
        logger.record("C IPC heap allocation round trips bytes", contents_ok);
        logger.record(
            "C IPC heap rejects invalid heap IDs",
            iosAlloc(-1, 32).is_null(),
        );
        probe_loader_ipc_bounds(logger);
        probe_pending_reply_reinitialize(logger);
        probe_unsent_queue_reinitialize(logger);
        probe_ioctlv_reply(logger);
    }

    submit_fs_open(&mut state);

    loop {
        refresh_state(&mut state);
        service_ipc_after_auto_window(&mut state);
        refresh_state(&mut state);

        #[cfg(feature = "dolphin-automation")]
        if state.elapsed_frames >= AUTOMATION_FRAMES {
            logger.section("IPC ASYNC AUTOMATION RESULT");
            logger.note_i32("submit rc", state.submit_rc);
            logger.note_i32("callback result", state.callback_result);
            logger.note_u32("callback count", state.callback_count);
            logger.note_u32("manual poll calls", state.manual_poll_calls);
            logger.note_u32("manual poll hits", state.manual_poll_hits);
            logger.note_u32("IPC debug events", state.debug_count);
            logger.record("async open submission accepted", state.submit_rc >= 0);
            logger.record("async callback completed once", state.callback_count == 1);
            logger.record(
                "async open returned a descriptor",
                state.callback_result >= 0,
            );
            logger.record(
                "automatic IRQ or manual poll delivered completion",
                state.callback_count > 0
                    && (!state.manual_poll_started || state.manual_poll_calls > 0),
            );
            logger.summary();
            loop {
                logger.display().wait_vsync();
            }
        }

        draw_dashboard(&state);
        logger.display().wait_vsync();
    }
}

// Deliver one real IOS reply across the PPC client reset. This deliberately
// does not claim that an old IOS instance preserves requests across a reload.
#[cfg(target_arch = "powerpc")]
fn probe_pending_reply_reinitialize(logger: &mut ProbeLogger) {
    use ogc_ipc as ipc;
    static RESULT: AtomicI32 = AtomicI32::new(IPC_ASYNC_UNSET);
    static CALLS: AtomicU32 = AtomicU32::new(0);
    unsafe extern "C" fn complete(result: i32, _: *mut c_void) -> i32 {
        RESULT.store(result, Ordering::Relaxed);
        CALLS.fetch_add(1, Ordering::Release);
        result
    }
    let irq = ogc_irq::irq_disable();
    let submitted = unsafe {
        ipc::IOS_OpenAsync(
            c"/dev/fs".as_ptr(),
            IPC_OPEN_NONE,
            Some(complete),
            core::ptr::null_mut(),
        )
    };
    let start = system::time();
    let control = || unsafe { core::ptr::read_volatile((ipc::HW_IPC_PPCBASE + 4) as *const u32) };
    // With acknowledgements masked, IOS may require the request ACK to be
    // consumed before it posts its reply. Poll only the ACK via the normal
    // path while the reply bit is not set; then stop polling.
    let mut pending = false;
    if submitted == 0 {
        while system::time().wrapping_sub(start) < WII_TIMER_CLOCK_HZ {
            let flags = control();
            if flags & 0x14 == 0x14 {
                pending = true;
                break;
            }
            // The reply handler has priority if both bits become set between
            // the status check and poll. Do not claim a pending reply then.
            if flags & 0x22 == 0x22 {
                ipc_poll();
            }
            core::hint::spin_loop();
        }
    }
    let held = pending && CALLS.load(Ordering::Acquire) == 0;
    if held {
        ipc::reinitialize();
    }
    ogc_irq::irq_restore(irq);
    let start = system::time();
    while submitted == 0
        && CALLS.load(Ordering::Acquire) == 0
        && system::time().wrapping_sub(start) < WII_TIMER_CLOCK_HZ * 2
    {
        ipc_poll();
        ogc_lwp::sys::yield_thread();
    }
    let result = RESULT.load(Ordering::Acquire);
    let calls = CALLS.load(Ordering::Acquire);
    logger.record(
        "IPC reinitialize fixture held a real IOS reply",
        submitted == 0 && held,
    );
    logger.record(
        "IPC reinitialize delivers the held reply exactly once",
        held && calls == 1 && result >= 0,
    );
    let closed = calls == 1 && result >= 0 && ipc::IOS_Close(result) == 0;
    let recovered = ipc::open_file(c"/dev/fs", IPC_OPEN_NONE).is_ok_and(|fd| fd.close().is_ok());
    logger.record(
        "IPC reinitialize closes held descriptor and accepts new requests",
        closed && recovered && CALLS.load(Ordering::Acquire) == 1,
    );
}

#[cfg(target_arch = "powerpc")]
fn probe_unsent_queue_reinitialize(logger: &mut ProbeLogger) {
    use ogc_ipc as ipc;

    unsafe extern "C" fn complete(result: i32, userdata: *mut c_void) -> i32 {
        let index = (userdata as usize).wrapping_sub(1);
        if let Some(result_slot) = RESET_QUEUE_RESULTS.get(index) {
            result_slot.store(result, Ordering::Release);
            RESET_QUEUE_CALLS[index].fetch_add(1, Ordering::AcqRel);
            RESET_QUEUE_CALLBACK_ORDER[index].store(
                RESET_QUEUE_CALLBACK_SEQUENCE.fetch_add(1, Ordering::AcqRel) + 1,
                Ordering::Release,
            );
        }
        result
    }

    unsafe extern "C" fn format_complete(result: i32, userdata: *mut c_void) -> i32 {
        RESET_FORMAT_RESULT.store(result, Ordering::Release);
        RESET_FORMAT_USERDATA.store(userdata as usize as u32, Ordering::Release);
        RESET_FORMAT_CALLBACK_ORDER.store(
            RESET_QUEUE_CALLBACK_SEQUENCE.fetch_add(1, Ordering::AcqRel) + 1,
            Ordering::Release,
        );
        let output = RESET_FORMAT_OUTPUT.load(Ordering::Acquire) as *const u8;
        let untouched = !output.is_null()
            // SAFETY: The probe keeps the four-byte output alive until this
            // callback returns and uses it only for this formatter request.
            && unsafe { core::slice::from_raw_parts(output, 4) == [0xa5; 4] };
        RESET_FORMAT_OUTPUT_UNTOUCHED.store(untouched, Ordering::Release);
        RESET_FORMAT_CALLS.fetch_add(1, Ordering::AcqRel);
        result
    }

    ipc::reinitialize();
    let mut queue_wrap_ready = true;
    for _ in 0..7 {
        queue_wrap_ready &=
            ipc::open_file(c"/dev/fs", IPC_OPEN_NONE).is_ok_and(|fd| fd.close().is_ok());
    }
    let fd = match ipc::open_file(c"/dev/es", IPC_OPEN_NONE) {
        Ok(fd) => fd,
        Err(error) => {
            logger.record("IPC reset formatter fixture opens ES", false);
            logger.note_i32("IPC reset formatter ES open result", error);
            return;
        }
    };
    for result in &RESET_QUEUE_RESULTS {
        result.store(IPC_ASYNC_UNSET, Ordering::Release);
    }
    for calls in &RESET_QUEUE_CALLS {
        calls.store(0, Ordering::Release);
    }
    for order in &RESET_QUEUE_CALLBACK_ORDER {
        order.store(0, Ordering::Release);
    }
    RESET_QUEUE_CALLBACK_SEQUENCE.store(0, Ordering::Release);
    RESET_FORMAT_RESULT.store(IPC_ASYNC_UNSET, Ordering::Release);
    RESET_FORMAT_CALLS.store(0, Ordering::Release);
    RESET_FORMAT_CALLBACK_ORDER.store(0, Ordering::Release);
    RESET_FORMAT_USERDATA.store(0, Ordering::Release);
    RESET_FORMAT_OUTPUT_UNTOUCHED.store(false, Ordering::Release);
    let mut output = [0xa5u8; 4];
    let title_id = 0x0000_0001_0000_0002u64.to_be_bytes();
    RESET_FORMAT_OUTPUT.store(output.as_mut_ptr() as usize, Ordering::Release);
    let userdata = 0x1357usize as *mut c_void;
    let sent_before = SENT_DEBUG_COUNT.load(Ordering::Acquire);
    let irq = ogc_irq::irq_disable();
    let mut submissions = [IPC_ASYNC_UNSET; 3];
    for (index, submitted) in submissions.iter_mut().enumerate() {
        *submitted = unsafe {
            ipc::IOS_OpenAsync(
                c"/dev/fs".as_ptr(),
                IPC_OPEN_NONE,
                Some(complete),
                (index + 1) as *mut c_void,
            )
        };
    }
    let formatted_submission = unsafe {
        IOS_IoctlvFormatAsync(
            ipc::IPC_HEAP,
            fd.raw(),
            0x10,
            Some(format_complete),
            userdata,
            c"d:d".as_ptr(),
            title_id.as_ptr(),
            title_id.len() as u32,
            output.as_mut_ptr(),
            output.len() as u32,
        )
    };
    let sent_before_reset = SENT_DEBUG_COUNT
        .load(Ordering::Acquire)
        .wrapping_sub(sent_before);
    ipc::reinitialize();
    ogc_irq::irq_restore(irq);

    let accepted = submissions.iter().all(|result| *result == 0) && formatted_submission == 0;
    let start = system::time();
    while accepted
        && RESET_QUEUE_CALLS
            .iter()
            .any(|calls| calls.load(Ordering::Acquire) == 0)
        && system::time().wrapping_sub(start) < WII_TIMER_CLOCK_HZ * 2
    {
        ipc_poll();
        ogc_lwp::sys::yield_thread();
    }

    let results: [i32; 3] =
        core::array::from_fn(|index| RESET_QUEUE_RESULTS[index].load(Ordering::Acquire));
    let calls: [u32; 3] =
        core::array::from_fn(|index| RESET_QUEUE_CALLS[index].load(Ordering::Acquire));
    let callback_order: [u32; 3] =
        core::array::from_fn(|index| RESET_QUEUE_CALLBACK_ORDER[index].load(Ordering::Acquire));
    let format_callback_order = RESET_FORMAT_CALLBACK_ORDER.load(Ordering::Acquire);
    let first_closed = results[0] >= 0 && ipc::IOS_Close(results[0]) == 0;
    let exact_outcomes =
        results[0] >= 0 && results[1] == ipc::IPC_ENOENT && results[2] == ipc::IPC_ENOENT;
    for (index, result) in results.iter().enumerate() {
        logger.note_i32("IPC reset queued callback result", *result);
        logger.note_u32("IPC reset queued callback count", calls[index]);
    }
    logger.record(
        "IPC reset fixture accepts one sent request and queued requests",
        accepted && sent_before_reset == 1,
    );
    logger.record(
        "IPC reset fixture places queued requests across ring wrap",
        queue_wrap_ready,
    );
    logger.record(
        "IPC reset completes sent request and cancels unsent open once",
        exact_outcomes && calls == [1, 1, 1],
    );
    logger.record(
        "IPC reset cancels queued formatter once without output copyback",
        RESET_FORMAT_CALLS.load(Ordering::Acquire) == 1
            && RESET_FORMAT_RESULT.load(Ordering::Acquire) == ipc::IPC_ENOENT
            && RESET_FORMAT_USERDATA.load(Ordering::Acquire) == userdata as usize as u32
            && RESET_FORMAT_OUTPUT_UNTOUCHED.load(Ordering::Acquire)
            && output == [0xa5; 4],
    );
    logger.record(
        "IPC reset cancellation callbacks preserve queue order",
        callback_order[1] != 0
            && callback_order[1] < callback_order[2]
            && callback_order[2] < format_callback_order,
    );
    logger.record("IPC reset closes the sent request descriptor", first_closed);
    logger.record("IPC reset formatter fixture closes ES", fd.close().is_ok());
    RESET_FORMAT_OUTPUT.store(0, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
fn probe_loader_ipc_bounds(logger: &mut ProbeLogger) {
    use ogc_ipc as ipc;
    unsafe extern "C" {
        static __ipcbufferLo: u8;
        static __ipcbufferHi: u8;
    }
    // Only the exact deliberately narrowed fixture values enable this check.
    let linker_lo = core::ptr::addr_of!(__ipcbufferLo) as usize;
    let linker_hi = core::ptr::addr_of!(__ipcbufferHi) as usize;
    let (low, high) = unsafe {
        (
            core::ptr::read_volatile(0x8000_3130 as *const u32) as usize,
            core::ptr::read_volatile(0x8000_3134 as *const u32) as usize,
        )
    };
    if (low != 0 && low != linker_lo + 0x1000)
        || (high != 0 && high != linker_hi - 0x1000)
        || (low == 0 && high == 0)
    {
        return;
    }
    let expected_lo = if low == 0 { linker_lo } else { low };
    let expected_hi = if high == 0 { linker_hi } else { high };
    logger.note_hex32("IPC loader low", low as u32);
    logger.note_hex32("IPC loader high", high as u32);
    // No allocation may occur while temporarily testing the base. Restore the
    // allocated low watermark before reenabling interrupts or logging.
    let irq = ogc_irq::irq_disable();
    let current_lo = ipc::get_buffer_lo();
    let current_hi = ipc::get_buffer_hi();
    unsafe { ipc::IPC_SetBufferLo((expected_lo - 32) as *mut c_void) };
    let rejects_below = ipc::get_buffer_lo() == current_lo;
    unsafe { ipc::IPC_SetBufferLo(expected_lo as *mut c_void) };
    let accepts_base = ipc::get_buffer_lo() as usize == expected_lo;
    unsafe { ipc::IPC_SetBufferLo(current_lo) };
    unsafe { ipc::IPC_SetBufferHi((expected_hi + 32) as *mut c_void) };
    let rejects_above = ipc::get_buffer_hi() == current_hi;
    unsafe { ipc::IPC_SetBufferHi(current_hi) };
    let restored = ipc::get_buffer_lo() == current_lo && ipc::get_buffer_hi() == current_hi;
    ogc_irq::irq_restore(irq);
    logger.record(
        "IPC loader lower bound selected",
        rejects_below && accepts_base && current_lo as usize >= expected_lo,
    );
    logger.record(
        "IPC loader upper bound selected",
        rejects_above && current_hi as usize == expected_hi,
    );
    logger.record("IPC loader allocated watermarks restored", restored);
}

struct IpcAsyncProbeState {
    phase: &'static str,
    submit_rc: i32,
    callback_result: i32,
    callback_count: u32,
    callback_count_before_poll: u32,
    manual_poll_started: bool,
    manual_poll_calls: u32,
    manual_poll_hits: u32,
    elapsed_frames: u32,
    elapsed_usec: u64,
    start_ticks: u64,
    debug_stage: u32,
    debug_a: u32,
    debug_b: u32,
    debug_c: u32,
    debug_count: u32,
}

impl IpcAsyncProbeState {
    const fn new() -> Self {
        Self {
            phase: "init",
            submit_rc: IPC_ASYNC_UNSET,
            callback_result: IPC_ASYNC_UNSET,
            callback_count: 0,
            callback_count_before_poll: 0,
            manual_poll_started: false,
            manual_poll_calls: 0,
            manual_poll_hits: 0,
            elapsed_frames: 0,
            elapsed_usec: 0,
            start_ticks: 0,
            debug_stage: 0,
            debug_a: 0,
            debug_b: 0,
            debug_c: 0,
            debug_count: 0,
        }
    }
}

fn submit_fs_open(state: &mut IpcAsyncProbeState) {
    state.phase = "submitted /dev/fs async open";
    state.submit_rc = IPC_ASYNC_PENDING;
    state.callback_result = IPC_ASYNC_PENDING;
    state.callback_count = 0;

    #[cfg(target_arch = "powerpc")]
    {
        RESULT_FS.store(IPC_ASYNC_PENDING, Ordering::Release);
        COUNT_FS.store(0, Ordering::Release);
        let rc = unsafe {
            IOS_OpenAsync(
                fs_path().as_ptr(),
                IPC_OPEN_NONE,
                Some(ipc_async_open_cb),
                core::ptr::null_mut(),
            )
        };
        state.submit_rc = rc;
        if rc < 0 {
            state.phase = "submit failed";
            state.callback_result = IPC_ASYNC_UNSET;
            RESULT_FS.store(IPC_ASYNC_UNSET, Ordering::Release);
        }
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        state.submit_rc = 0;
    }
}

fn service_ipc_after_auto_window(state: &mut IpcAsyncProbeState) {
    if state.submit_rc < 0 || state.callback_count > 0 {
        return;
    }

    if !state.manual_poll_started {
        if state.elapsed_usec < AUTO_WINDOW_USEC {
            state.phase = "auto IRQ window; not polling";
            return;
        }

        state.manual_poll_started = true;
        state.callback_count_before_poll = state.callback_count;
        state.phase = "manual poll started";
    }

    state.manual_poll_calls = state.manual_poll_calls.wrapping_add(1);

    #[cfg(target_arch = "powerpc")]
    {
        if ipc_poll() {
            state.manual_poll_hits = state.manual_poll_hits.wrapping_add(1);
        }
    }
}

// IOS replaces the request command with IPC_REPLY and writes the original
// command into the descriptor word. The callback must see restored CPU
// addresses and the output without doing its own cache maintenance.
#[cfg(target_arch = "powerpc")]
fn probe_ioctlv_reply(logger: &mut ProbeLogger) {
    use ogc_ipc as ipc;
    #[repr(align(32))]
    struct Aligned<T>(T);
    struct Reply {
        vector: *const ipc::Ioctlv,
        output: *const u8,
        expected: [u8; 4],
        count: AtomicU32,
        result: AtomicI32,
        restored: AtomicU32,
        output_matches: AtomicU32,
    }
    unsafe extern "C" fn complete(result: i32, userdata: *mut c_void) -> i32 {
        // SAFETY: The submitter keeps the context and DMA storage alive until
        // the completion callback finishes. Only this callback reads them.
        let reply = unsafe { &*userdata.cast::<Reply>() };
        let vector = unsafe { &*reply.vector };
        let bytes = unsafe { core::slice::from_raw_parts(reply.output, 32) };
        reply.result.store(result, Ordering::Relaxed);
        reply.restored.store(
            u32::from(vector.data.cast_const().cast::<u8>() == reply.output && vector.len == 4),
            Ordering::Relaxed,
        );
        reply.output_matches.store(
            u32::from(bytes[..4] == reply.expected && bytes[4..].iter().all(|byte| *byte == 0xa5)),
            Ordering::Relaxed,
        );
        reply.count.fetch_add(1, Ordering::Release);
        result
    }
    let Ok(fd) = ipc::open_file(c"/dev/es", ipc::IPC_OPEN_NONE) else {
        logger.record("IPC async reply opens ES", false);
        return;
    };
    let mut expected = [0u8; 4];
    let reference = fd.ioctlv(0x0e, 0, [ipc::IoctlvEntry::Out(&mut expected)]);
    logger.record(
        "IPC async reply reference title count",
        reference == 0 && expected != [0; 4],
    );
    let mut output = Aligned([0xa5u8; 32]);
    let mut vector = Aligned([ipc::Ioctlv {
        data: output.0.as_mut_ptr().cast(),
        len: 4,
    }]);
    let reply = Reply {
        vector: vector.0.as_ptr(),
        output: output.0.as_ptr(),
        expected,
        count: AtomicU32::new(0),
        result: AtomicI32::new(IPC_ASYNC_UNSET),
        restored: AtomicU32::new(0),
        output_matches: AtomicU32::new(0),
    };
    // SAFETY: Both aligned buffers own their complete cache lines and are
    // retained until the callback finishes. An accepted request never times
    // out by returning from this frame while IOS still owns those buffers.
    let submitted = unsafe {
        ipc::IOS_IoctlvAsync(
            fd.raw(),
            0x0e,
            0,
            1,
            vector.0.as_mut_ptr(),
            Some(complete),
            (&reply as *const Reply).cast_mut().cast(),
        )
    };
    logger.record("IPC async reply submits ES title count", submitted == 0);
    if submitted == 0 {
        while reply.count.load(Ordering::Acquire) == 0 {
            ipc_poll();
            ogc_lwp::sys::yield_thread();
        }
        logger.record(
            "IPC async reply restores vector address before callback",
            reply.restored.load(Ordering::Acquire) == 1,
        );
        logger.record(
            "IPC async reply output and canaries visible before callback",
            reply.result.load(Ordering::Acquire) == 0
                && reply.output_matches.load(Ordering::Acquire) == 1
                && reply.count.load(Ordering::Acquire) == 1,
        );
    } else {
        logger.record(
            "IPC async reply restores vector address before callback",
            false,
        );
        logger.record(
            "IPC async reply output and canaries visible before callback",
            false,
        );
    }
    logger.record("IPC async reply closes ES", fd.close().is_ok());
}

#[cfg(target_arch = "powerpc")]
fn reset_probe() {
    RESULT_FS.store(IPC_ASYNC_UNSET, Ordering::Release);
    COUNT_FS.store(0, Ordering::Release);
    DEBUG_STAGE.store(0, Ordering::Release);
    DEBUG_A.store(0, Ordering::Release);
    DEBUG_B.store(0, Ordering::Release);
    DEBUG_C.store(0, Ordering::Release);
    DEBUG_COUNT.store(0, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
fn fs_path() -> &'static CStr {
    c"/dev/fs"
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn ipc_async_open_cb(result: i32, _userdata: *mut c_void) -> i32 {
    RESULT_FS.store(result, Ordering::Release);
    let _ = COUNT_FS.fetch_add(1, Ordering::AcqRel);
    result
}

#[cfg(target_arch = "powerpc")]
unsafe fn ipc_debug_hook(stage: u32, a: u32, b: u32, c: u32) {
    if stage == 5 {
        SENT_DEBUG_COUNT.fetch_add(1, Ordering::AcqRel);
    }
    if stage == 70 {
        REINIT_MSR.store(read_msr(), Ordering::Release);
    }
    DEBUG_STAGE.store(stage, Ordering::Release);
    DEBUG_A.store(a, Ordering::Release);
    DEBUG_B.store(b, Ordering::Release);
    DEBUG_C.store(c, Ordering::Release);
    let _ = DEBUG_COUNT.fetch_add(1, Ordering::AcqRel);
}

#[cfg(target_arch = "powerpc")]
fn read_msr() -> u32 {
    let value: u32;
    // SAFETY: Reading MSR has no side effects.
    unsafe { core::arch::asm!("mfmsr {value}", value = out(reg) value, options(nomem, nostack)) };
    value
}

#[cfg(target_arch = "powerpc")]
fn refresh_state(state: &mut IpcAsyncProbeState) {
    state.callback_result = RESULT_FS.load(Ordering::Acquire);
    state.callback_count = COUNT_FS.load(Ordering::Acquire);
    state.debug_stage = DEBUG_STAGE.load(Ordering::Acquire);
    state.debug_a = DEBUG_A.load(Ordering::Acquire);
    state.debug_b = DEBUG_B.load(Ordering::Acquire);
    state.debug_c = DEBUG_C.load(Ordering::Acquire);
    state.debug_count = DEBUG_COUNT.load(Ordering::Acquire);
    state.elapsed_frames = state.elapsed_frames.wrapping_add(1);

    let now = system::time();
    let elapsed_ticks = now.wrapping_sub(state.start_ticks);
    state.elapsed_usec = elapsed_ticks.saturating_mul(1_000_000) / WII_TIMER_CLOCK_HZ;
}

#[cfg(not(target_arch = "powerpc"))]
fn refresh_state(state: &mut IpcAsyncProbeState) {
    state.elapsed_frames = state.elapsed_frames.wrapping_add(1);
}

fn draw_dashboard(state: &IpcAsyncProbeState) {
    video::console_clear();

    draw_at(0, 0, "IPC ASYNC /dev/fs PROBE");
    draw_at(0, 1, "No USB backend calls");
    draw_at(0, 2, "Manual poll starts after 1000 ms");

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "phase={}", state.phase);
    draw_at(0, 4, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "request=/dev/fs async open");
    draw_at(0, 6, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "submit rc=");
    write_rc(&mut line, state.submit_rc);
    draw_at(0, 7, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "callback result=");
    write_rc(&mut line, state.callback_result);
    draw_at(0, 8, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "callback count={}", state.callback_count);
    draw_at(0, 9, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "manual poll active={} calls/hits={}/{}",
        state.manual_poll_started, state.manual_poll_calls, state.manual_poll_hits
    );
    draw_at(0, 11, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "callback count before poll={}",
        state.callback_count_before_poll
    );
    draw_at(0, 12, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "elapsed frames/usec={}/{}",
        state.elapsed_frames, state.elapsed_usec
    );
    draw_at(0, 14, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "debug stage/count={}/{}",
        state.debug_stage, state.debug_count
    );
    draw_at(0, 16, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "debug a/b/c=0x{:08x}/0x{:08x}/0x{:08x}",
        state.debug_a, state.debug_b, state.debug_c
    );
    draw_at(0, 17, line.as_str());

    draw_interpretation(state);
}

fn write_rc(line: &mut LineBuf, rc: i32) {
    if rc == IPC_ASYNC_PENDING {
        let _ = write!(line, "pending");
    } else {
        let _ = write!(line, "{}", rc);
    }
}

fn draw_interpretation(state: &IpcAsyncProbeState) {
    if state.callback_count > 0 && !state.manual_poll_started {
        draw_at(0, 20, "Result: callback fired before manual poll.");
    } else if state.callback_count > 0 {
        draw_at(0, 20, "Result: callback fired after manual poll.");
    } else if state.manual_poll_hits > 0 {
        draw_at(
            0,
            20,
            "Manual poll serviced IPC status; waiting for callback.",
        );
    } else if state.manual_poll_started {
        draw_at(0, 20, "Manual poll active; callback still pending.");
    } else {
        draw_at(0, 20, "Waiting for automatic IRQ delivery.");
    }
}

fn draw_at(col: i32, row: i32, text: &str) {
    let mut seq = LineBuf::new();
    let _ = write!(&mut seq, "\x1b[{};{}H", row + 1, col + 1);
    let _ = crate::probe_support::screen_write(seq.as_bytes());
    let _ = crate::probe_support::screen_write(text.as_bytes());
}

struct LineBuf {
    bytes: [u8; 256],
    len: usize,
}

impl LineBuf {
    const fn new() -> Self {
        Self {
            bytes: [0; 256],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("")
    }
}

impl Write for LineBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let remaining = self.bytes.len().saturating_sub(self.len);
        if s.len() > remaining {
            return Err(core::fmt::Error);
        }
        self.bytes[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}
