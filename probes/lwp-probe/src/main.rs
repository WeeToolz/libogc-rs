#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]
#![cfg_attr(feature = "wii-std-sync", feature(restricted_std))]

#[cfg(feature = "wii-std-sync")]
extern crate std;

#[cfg(target_arch = "powerpc")]
mod futex_probe;
#[cfg(all(target_arch = "powerpc", feature = "wii-std-sync"))]
mod std_sync_probe;

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
mod wii {
    use core::ffi::c_void;
    use core::ptr;
    use core::sync::atomic::{AtomicU32, Ordering};

    use ogc_ipc as ipc;
    use ogc_lwp::sys as lwp;
    use ogc_lwp::{MutexHandle, SemaphoreHandle};
    use ogc_lwp_driver as driver;
    use ogc_system_probe::probe_support::{self, ProbeLogger};
    use tuxedo_sys as tuxedo;

    const SWITCH_ITERATIONS: u32 = 20_000;
    const STACK_BYTES: usize = 16 * 1024;

    static TLS_KEY: AtomicU32 = AtomicU32::new(0);
    static TLS_PASSED: AtomicU32 = AtomicU32::new(0);
    static TLS_DESTRUCTORS: AtomicU32 = AtomicU32::new(0);
    static TLS_ERRORS: AtomicU32 = AtomicU32::new(0);
    static TLS_MARKER_A: u32 = 0x1357_2468;
    static TLS_MARKER_B: u32 = 0x9abc_def0;

    unsafe extern "C" fn tls_destructor(value: *mut c_void) {
        if value != ptr::addr_of!(TLS_MARKER_A).cast_mut().cast()
            && value != ptr::addr_of!(TLS_MARKER_B).cast_mut().cast()
        {
            TLS_ERRORS.fetch_add(1, Ordering::AcqRel);
        }
        TLS_DESTRUCTORS.fetch_add(1, Ordering::AcqRel);
    }

    unsafe extern "C" fn tls_worker(value: *mut c_void) -> *mut c_void {
        let key = TLS_KEY.load(Ordering::Acquire);
        let empty = driver::tls_get(key).is_ok_and(|value| value.is_null());
        let errno = if value == ptr::addr_of!(TLS_MARKER_A).cast_mut().cast() {
            13
        } else {
            28
        };
        // SAFETY: The value refers to a static marker accepted by the destructor.
        let stored = unsafe { driver::tls_set(key, value) }.is_ok();
        // SAFETY: The current thread owns its initialized errno field.
        unsafe { ogc_sysbase::set_reent_errno(ogc_sysbase::current_reent(), errno) };
        driver::yield_thread();
        if empty && stored && driver::tls_get(key) == Ok(value) && ogc_sysbase::errno() == errno {
            TLS_PASSED.fetch_add(1, Ordering::AcqRel);
        } else {
            TLS_ERRORS.fetch_add(1, Ordering::AcqRel);
        }
        value
    }

    fn run_std_foundation(logger: &mut ProbeLogger) {
        logger.section("STD FOUNDATION: TLS AND DETACH");
        logger.record(
            "std heap initialized before main",
            ogc_std_runtime::heap_capacity() > 0,
        );
        logger.record(
            "checked system initialization",
            ogc_system::init_checked().is_ok(),
        );
        let saved_errno = ogc_sysbase::errno();
        let key = match unsafe { driver::tls_create(Some(tls_destructor)) } {
            Ok(key) => key,
            Err(_) => {
                logger.record("TLS key created", false);
                return;
            }
        };
        TLS_KEY.store(key, Ordering::Release);
        let main_value = ptr::addr_of!(TLS_MARKER_B).cast_mut().cast();
        logger.record(
            "initial main TLS value is null",
            driver::tls_get(key) == Ok(ptr::null_mut()),
        );
        // SAFETY: Main stores a static marker; it deletes the key before exit.
        logger.record(
            "main TLS value stored",
            unsafe { driver::tls_set(key, main_value) }.is_ok(),
        );
        unsafe { ogc_sysbase::set_reent_errno(ogc_sysbase::current_reent(), 17) };
        for marker in [ptr::addr_of!(TLS_MARKER_A), ptr::addr_of!(TLS_MARKER_B)] {
            let mut thread = lwp::LWP_THREAD_NULL;
            let created = unsafe {
                driver::create_thread(
                    &mut thread,
                    Some(tls_worker),
                    marker.cast_mut().cast(),
                    ptr::null_mut(),
                    8192,
                    80,
                )
            };
            logger.record("TLS worker created", created == 0);
            if created != 0 {
                break;
            }
            let result = join(thread);
            logger.record(
                "TLS worker returned its real argument",
                result == Ok(marker as usize),
            );
        }
        logger.record(
            "main TLS survived worker cleanup",
            driver::tls_get(key) == Ok(main_value),
        );
        logger.record("errno values are isolated", ogc_sysbase::errno() == 17);
        logger.record(
            "joined TLS destructors ran",
            TLS_DESTRUCTORS.load(Ordering::Acquire) == 2,
        );

        let before = driver::thread_cleanup_counters();
        let mut completed = 0u32;
        for iteration in 0..64u32 {
            let mut thread = lwp::LWP_THREAD_NULL;
            // The 40 KiB stack exceeds the driver's static fallback capacity.
            // Repeated success therefore requires actual workspace reclamation.
            let created = unsafe {
                driver::create_thread(
                    &mut thread,
                    Some(tls_worker),
                    ptr::addr_of!(TLS_MARKER_A).cast_mut().cast(),
                    ptr::null_mut(),
                    40 * 1024,
                    80,
                )
            };
            if created != 0 {
                logger.record("detached dynamic-stack worker created", false);
                break;
            }
            let detached = driver::detach_thread(thread);
            if detached != 0 {
                logger.record("thread detached", false);
                logger.record(
                    "failed detach still permits cleanup by join",
                    join(thread).is_ok(),
                );
                break;
            }
            let started = driver::now_ticks();
            while driver::thread_cleanup_counters()
                .detached_reaped
                .wrapping_sub(before.detached_reaped)
                <= iteration
                && driver::now_ticks().wrapping_sub(started) < 2 * driver::TICKS_PER_SECOND
            {
                driver::sleep_ticks(driver::TICKS_PER_SECOND / 1000);
            }
            if driver::thread_cleanup_counters()
                .detached_reaped
                .wrapping_sub(before.detached_reaped)
                <= iteration
            {
                logger.record("detached cleanup completed before deadline", false);
                logger.note("TLS key retained because detached cleanup is incomplete");
                unsafe { ogc_sysbase::set_reent_errno(ogc_sysbase::current_reent(), saved_errno) };
                return;
            }
            completed += 1;
        }
        logger.record("64 detached dynamic stacks were reclaimed", completed == 64);
        logger.record(
            "all worker TLS values and errno were correct",
            TLS_PASSED.load(Ordering::Acquire) == 66 && TLS_ERRORS.load(Ordering::Acquire) == 0,
        );
        logger.record(
            "all TLS destructors completed",
            TLS_DESTRUCTORS.load(Ordering::Acquire) == 66,
        );
        logger.record(
            "no TLS cleanup failure",
            driver::thread_cleanup_counters().tls_failures == before.tls_failures,
        );
        logger.record("TLS key deleted", driver::tls_destroy(key).is_ok());
        logger.record(
            "deleted TLS key is rejected",
            driver::tls_get(key) == Err(driver::TlsError::InvalidKey),
        );
        unsafe { ogc_sysbase::set_reent_errno(ogc_sysbase::current_reent(), saved_errno) };

        logger.section("STD FOUNDATION: ALLOCATION");
        #[repr(align(64))]
        struct Heap([u8; 8192]);
        let mut heap = Heap([0xa5; 8192]);
        let allocator = ogc_alloc::ArenaAllocator::new();
        if unsafe { allocator.initialize_region(heap.0.as_mut_ptr(), heap.0.len()) }.is_err() {
            logger.record("allocator initialized", false);
            return;
        }
        let allocation = allocator.allocate_zeroed(8, 64);
        if allocation.is_null() {
            logger.record("zeroed allocation succeeded", false);
            return;
        }
        logger.record(
            "allocation alignment and zero bytes",
            (allocation as usize).is_multiple_of(64)
                && unsafe { core::slice::from_raw_parts(allocation, 8) } == [0; 8],
        );
        let expected = [0x13, 0x27, 0x39, 0x4b, 0x5d, 0x6f, 0x71, 0x83];
        unsafe { allocation.copy_from_nonoverlapping(expected.as_ptr(), 8) };
        let grown = unsafe { allocator.reallocate(allocation, 8, 64, 128) };
        if grown.is_null() {
            logger.record("resize succeeded", false);
            unsafe { allocator.deallocate(allocation, 8, 64) };
            return;
        }
        logger.record(
            "resize preserved bytes and alignment",
            (grown as usize).is_multiple_of(64)
                && unsafe { core::slice::from_raw_parts(grown, 8) } == expected,
        );
        let failed = unsafe { allocator.reallocate(grown, 128, 64, 16384) };
        logger.record(
            "failed resize preserved the old allocation",
            failed.is_null() && unsafe { core::slice::from_raw_parts(grown, 8) } == expected,
        );
        if !failed.is_null() {
            unsafe { allocator.deallocate(failed, 16384, 64) };
        } else {
            unsafe { allocator.deallocate(grown, 128, 64) };
        }
        logger.record("allocation storage released", allocator.used_bytes() == 0);

        logger.section("STD FOUNDATION: SYSTEM ABI");
        let used_before = ogc_std_runtime::heap_used_bytes();
        let allocation = ogc_std_runtime::__wii_std_alloc_zeroed(8, 64);
        if allocation.is_null() {
            logger.record("System ABI allocation succeeded", false);
            return;
        }
        logger.record(
            "System ABI returns aligned zero bytes",
            (allocation as usize).is_multiple_of(64)
                && unsafe { core::slice::from_raw_parts(allocation, 8) } == [0; 8],
        );
        unsafe { allocation.copy_from_nonoverlapping(expected.as_ptr(), 8) };
        let failed = unsafe { ogc_std_runtime::__wii_std_realloc(allocation, 8, 64, usize::MAX) };
        logger.record(
            "System ABI failed resize preserves bytes",
            failed.is_null() && unsafe { core::slice::from_raw_parts(allocation, 8) } == expected,
        );
        if !failed.is_null() {
            unsafe { ogc_std_runtime::__wii_std_dealloc(failed, usize::MAX, 64) };
        } else {
            unsafe { ogc_std_runtime::__wii_std_dealloc(allocation, 8, 64) };
        }
        logger.record(
            "System ABI releases allocation",
            ogc_std_runtime::heap_used_bytes() == used_before,
        );
        let mut output = main_value.cast::<u8>();
        logger.record(
            "TLS ABI rejects invalid key and preserves output",
            unsafe { ogc_std_runtime::__wii_std_tls_get(u32::MAX, &mut output) }
                == ogc_sysbase::EINVAL
                && output == main_value.cast::<u8>(),
        );
        logger.record(
            "TLS ABI rejects missing output",
            unsafe { ogc_std_runtime::__wii_std_tls_get(key, ptr::null_mut()) }
                == ogc_sysbase::EINVAL,
        );
    }

    #[repr(C, align(32))]
    struct Stack([u8; STACK_BYTES]);

    static mut STACK_A: Stack = Stack([0; STACK_BYTES]);
    static mut STACK_B: Stack = Stack([0; STACK_BYTES]);
    static mut STACK_SYNC: Stack = Stack([0; STACK_BYTES]);
    static mut STACK_SEM: Stack = Stack([0; STACK_BYTES]);
    static mut STACK_MQ: Stack = Stack([0; STACK_BYTES]);
    static mut STACK_TUXEDO: Stack = Stack([0; STACK_BYTES]);
    static mut STACK_IPC_LOW: Stack = Stack([0; STACK_BYTES]);
    static mut STACK_IPC_HIGH: Stack = Stack([0; STACK_BYTES]);
    static mut TUXEDO_THREAD: tuxedo::KThread = tuxedo::KThread::empty();
    static mut TUXEDO_QUEUE: tuxedo::KThrQueue = tuxedo::KThrQueue {
        next: ptr::null_mut(),
        prev: ptr::null_mut(),
    };
    static mut TUXEDO_TICK_TASK: tuxedo::KTickTask = tuxedo::KTickTask {
        next: ptr::null_mut(),
        fn_: None,
        target: 0,
        period: 0,
    };

    static SWITCH_A: AtomicU32 = AtomicU32::new(0);
    static SWITCH_B: AtomicU32 = AtomicU32::new(0);
    static PRIORITY_ORDER: AtomicU32 = AtomicU32::new(0);
    static PRIORITY_POSITION: AtomicU32 = AtomicU32::new(0);
    static CHILD_REENT: AtomicU32 = AtomicU32::new(0);
    static EXTERNAL_SETUP_RAN: AtomicU32 = AtomicU32::new(0);
    static EXTERNAL_WAIT_READY: AtomicU32 = AtomicU32::new(0);
    static EXTERNAL_WATCHDOG: AtomicU32 = AtomicU32::new(0);
    const EXTERNAL_KEY: usize = 0x4558_5457;
    const EXTERNAL_TIMER: usize = 0x4558_5454;
    static ROTATION_ABI: AtomicU32 = AtomicU32::new(0);
    static SHARED_VALUE: AtomicU32 = AtomicU32::new(0);
    static SEM_RELEASED: AtomicU32 = AtomicU32::new(0);
    static SEM_WAIT_RC: AtomicU32 = AtomicU32::new(u32::MAX);
    static SYNC_MUTEX_LOCK_RC: AtomicU32 = AtomicU32::new(u32::MAX);
    static SYNC_COND_SIGNAL_RC: AtomicU32 = AtomicU32::new(u32::MAX);
    static SYNC_MUTEX_UNLOCK_RC: AtomicU32 = AtomicU32::new(u32::MAX);
    static DEFERRED_RAN: AtomicU32 = AtomicU32::new(0);
    static IRQ_RESERVATION: AtomicU32 = AtomicU32::new(u32::MAX);
    static IRQ_DEFERRED_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);
    static IRQ_RESERVED_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);
    static IRQ_DEFERRED_RAN: AtomicU32 = AtomicU32::new(0);
    static IRQ_CONTEXT: AtomicU32 = AtomicU32::new(0);
    #[unsafe(export_name = "lwp_probe_debug_hook_count")]
    static DEBUG_HOOK_COUNT: AtomicU32 = AtomicU32::new(0);
    #[unsafe(export_name = "lwp_probe_debug_last_event")]
    static DEBUG_LAST_EVENT: AtomicU32 = AtomicU32::new(0);
    static TUXEDO_TICK_RAN: AtomicU32 = AtomicU32::new(0);
    static DECREMENTER_CALLBACK_RAN: AtomicU32 = AtomicU32::new(0);
    static ASYNC_IPC_DONE: AtomicU32 = AtomicU32::new(0);
    static ASYNC_IPC_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);
    static ASYNC_IPC_THREAD: AtomicU32 = AtomicU32::new(lwp::LWP_THREAD_NULL);
    static ASYNC_IPC_CLOSE_OK: AtomicU32 = AtomicU32::new(0);
    static IPC_LOW_GATE: AtomicU32 = AtomicU32::new(lwp::LWP_SEM_NULL);
    static IPC_HIGH_GATE: AtomicU32 = AtomicU32::new(lwp::LWP_SEM_NULL);
    static IPC_LOW_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);
    static IPC_HIGH_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);
    static MUTEX: AtomicU32 = AtomicU32::new(lwp::LWP_MUTEX_NULL);
    static COND: AtomicU32 = AtomicU32::new(lwp::LWP_COND_NULL);
    static SEM: AtomicU32 = AtomicU32::new(lwp::LWP_SEM_NULL);
    static MQ: AtomicU32 = AtomicU32::new(lwp::MQ_BOX_NULL);

    unsafe extern "C" {
        fn __getreent() -> *mut c_void;
    }

    unsafe extern "C" fn external_setup_worker(storage: *mut c_void) -> *mut c_void {
        let matches = !storage.is_null() && driver::current_reent() == storage;
        EXTERNAL_SETUP_RAN.store(u32::from(matches), Ordering::Release);
        driver::thread_exit(if matches {
            0x5a17usize as *mut c_void
        } else {
            ptr::null_mut()
        })
    }

    unsafe extern "C" fn external_wait_worker(_: *mut c_void) -> *mut c_void {
        EXTERNAL_WAIT_READY.store(1, Ordering::Release);
        driver::block_current_external(EXTERNAL_KEY, 0x15) as usize as *mut c_void
    }

    unsafe extern "C" fn external_wait_watchdog(_: u32, _: *mut c_void) {
        EXTERNAL_WATCHDOG.store(1, Ordering::Release);
        driver::broadcast_external(EXTERNAL_KEY);
    }

    unsafe extern "C" fn reent_worker(_: *mut c_void) -> *mut c_void {
        let local = driver::current_reent();
        // SAFETY: The newlib hook has no arguments and returns the current thread's storage.
        let abi = unsafe { __getreent() };
        CHILD_REENT.store(local as u32, Ordering::Release);
        usize::from(!local.is_null() && local == abi) as *mut c_void
    }

    unsafe extern "C" fn rotation_worker(argument: *mut c_void) -> *mut c_void {
        let id = argument as u32;
        let position = PRIORITY_POSITION.fetch_add(1, Ordering::AcqRel);
        PRIORITY_ORDER.fetch_or(id << (position * 4), Ordering::AcqRel);
        if ROTATION_ABI.load(Ordering::Acquire) != 0 {
            lwp::lwp_reschedule_abi(20);
        } else {
            lwp::reschedule(20);
        }
        let position = PRIORITY_POSITION.fetch_add(1, Ordering::AcqRel);
        PRIORITY_ORDER.fetch_or(id << (position * 4), Ordering::AcqRel);
        argument
    }

    unsafe extern "C" fn priority_worker(argument: *mut c_void) -> *mut c_void {
        let position = PRIORITY_POSITION.fetch_add(1, Ordering::AcqRel);
        PRIORITY_ORDER.fetch_or((argument as u32) << (position * 4), Ordering::AcqRel);
        argument
    }

    unsafe extern "C" fn switch_worker(argument: *mut c_void) -> *mut c_void {
        let counter = if argument as usize == 0 {
            &SWITCH_A
        } else {
            &SWITCH_B
        };
        for _ in 0..SWITCH_ITERATIONS {
            counter.fetch_add(1, Ordering::Relaxed);
            lwp::yield_thread();
        }
        (0x100 + argument as usize) as *mut c_void
    }

    unsafe extern "C" fn sync_worker(_: *mut c_void) -> *mut c_void {
        let mutex = MUTEX.load(Ordering::Acquire);
        let cond = COND.load(Ordering::Acquire);
        let lock_rc = lwp::mutex_lock(mutex);
        SYNC_MUTEX_LOCK_RC.store(lock_rc as u32, Ordering::Release);
        if lock_rc != 0 {
            return ptr::null_mut();
        }
        SHARED_VALUE.store(0x51a7, Ordering::Release);
        let signal_rc = lwp::cond_signal(cond);
        SYNC_COND_SIGNAL_RC.store(signal_rc as u32, Ordering::Release);
        let unlock_rc = lwp::mutex_unlock(mutex);
        SYNC_MUTEX_UNLOCK_RC.store(unlock_rc as u32, Ordering::Release);
        if signal_rc == 0 && unlock_rc == 0 {
            0x51a7usize as *mut c_void
        } else {
            ptr::null_mut()
        }
    }

    unsafe extern "C" fn semaphore_worker(_: *mut c_void) -> *mut c_void {
        let wait_result = SemaphoreHandle::new(SEM.load(Ordering::Acquire)).wait();
        let wait_rc = wait_result.err().map_or(0, |error| error as u32);
        SEM_WAIT_RC.store(wait_rc, Ordering::Release);
        if wait_result.is_ok() {
            SEM_RELEASED.store(1, Ordering::Release);
            0x5eusize as *mut c_void
        } else {
            ptr::null_mut()
        }
    }

    unsafe extern "C" fn message_worker(_: *mut c_void) -> *mut c_void {
        unsafe {
            let mut message = ptr::null_mut();
            if lwp::mq_receive(MQ.load(Ordering::Acquire), &mut message, lwp::MQ_MSG_BLOCK) != 0 {
                message
            } else {
                ptr::null_mut()
            }
        }
    }

    unsafe fn deferred_callback(_: *mut c_void) {
        DEFERRED_RAN.store(1, Ordering::Release);
    }

    unsafe fn irq_deferred_callback(_: *mut c_void) {
        IRQ_DEFERRED_RAN.fetch_or(1, Ordering::AcqRel);
    }

    unsafe fn irq_reserved_callback(_: *mut c_void) {
        IRQ_DEFERRED_RAN.fetch_or(2, Ordering::AcqRel);
    }

    unsafe extern "C" fn irq_deferred_submit(_: u32, _: *mut c_void) {
        let msr: u32;
        unsafe { core::arch::asm!("mfmsr {value}", value = out(reg) msr, options(nomem, nostack)) };
        IRQ_CONTEXT.store(u32::from(msr & 0x8000 == 0), Ordering::Release);
        let reservation = IRQ_RESERVATION.load(Ordering::Acquire);
        // SAFETY: The callbacks ignore their null arguments and the reservation
        // is owned by this one-shot interrupt until it is consumed here.
        let ordinary = unsafe {
            lwp::deferred_call_from_interrupt(Some(irq_deferred_callback), ptr::null_mut())
        };
        let reserved = unsafe {
            lwp::deferred_call_reserved_from_interrupt(
                reservation,
                Some(irq_reserved_callback),
                ptr::null_mut(),
            )
        };
        IRQ_DEFERRED_RESULT.store(ordinary as u32, Ordering::Release);
        IRQ_RESERVED_RESULT.store(reserved as u32, Ordering::Release);
    }

    fn probe_interrupt_deferred(logger: &mut ProbeLogger) {
        let Some(cancelled) = lwp::deferred_reserve_interrupt_slot() else {
            logger.record("LWP interrupt reservation cancels once", false);
            return;
        };
        logger.record(
            "LWP interrupt reservation cancels once",
            lwp::deferred_cancel_interrupt_slot(cancelled) == 0
                && lwp::deferred_cancel_interrupt_slot(cancelled) == lwp::LWP_CLOSED,
        );
        let Some(reservation) = lwp::deferred_reserve_interrupt_slot() else {
            logger.record("LWP interrupt deferred submission completes", false);
            return;
        };
        IRQ_RESERVATION.store(reservation, Ordering::Release);
        IRQ_DEFERRED_RESULT.store(u32::MAX, Ordering::Release);
        IRQ_RESERVED_RESULT.store(u32::MAX, Ordering::Release);
        IRQ_DEFERRED_RAN.store(0, Ordering::Release);
        IRQ_CONTEXT.store(0, Ordering::Release);
        let started = driver::now_ticks();
        // SAFETY: This one-shot timer uses only static state in its callback.
        let rc = unsafe {
            driver::timer_start_with_key(
                0x4952_5144,
                Some(irq_deferred_submit),
                ptr::null_mut(),
                driver::TICKS_PER_SECOND / 100,
                0,
            )
        };
        while rc == 0
            && IRQ_DEFERRED_RAN.load(Ordering::Acquire) != 3
            && driver::now_ticks().wrapping_sub(started) < driver::TICKS_PER_SECOND
        {
            lwp::yield_thread();
        }
        driver::timer_cancel(0x4952_5144);
        if IRQ_RESERVED_RESULT.load(Ordering::Acquire) != 0 {
            let _ = lwp::deferred_cancel_interrupt_slot(reservation);
        }
        logger.record(
            "LWP interrupt deferred submission completes",
            rc == 0
                && IRQ_CONTEXT.load(Ordering::Acquire) == 1
                && IRQ_DEFERRED_RESULT.load(Ordering::Acquire) == 0
                && IRQ_RESERVED_RESULT.load(Ordering::Acquire) == 0
                && IRQ_DEFERRED_RAN.load(Ordering::Acquire) == 3,
        );
    }

    unsafe extern "C" fn tuxedo_queue_worker(_: *mut c_void) -> isize {
        let result = unsafe {
            tuxedo::k_thr_queue_block(core::ptr::addr_of_mut!(TUXEDO_QUEUE), 0x55) as isize
        };
        let owner = tuxedo::k_thread_get_self();
        unsafe {
            core::arch::asm!(
                "mtspr 275, {owner}",
                owner = in(reg) owner as usize,
                options(nomem, nostack, preserves_flags)
            );
        }
        result
    }

    fn fpu_owner_register() -> usize {
        let owner: usize;
        unsafe {
            core::arch::asm!(
                "mfspr {owner}, 275",
                owner = out(reg) owner,
                options(nomem, nostack, preserves_flags)
            );
        }
        owner
    }

    unsafe extern "C" fn tuxedo_tick_callback(_: *mut tuxedo::KTickTask) {
        TUXEDO_TICK_RAN.fetch_add(1, Ordering::AcqRel);
    }

    unsafe extern "C" fn decrementer_callback(_: u32, _: *mut c_void) {
        DECREMENTER_CALLBACK_RAN.store(1, Ordering::Release);
    }

    unsafe fn debug_hook(event: driver::DebugEvent) {
        DEBUG_HOOK_COUNT.fetch_add(1, Ordering::Relaxed);
        DEBUG_LAST_EVENT.store(event.event as u32, Ordering::Relaxed);
    }

    unsafe extern "C" fn async_ipc_callback(result: i32, _: *mut c_void) -> i32 {
        ASYNC_IPC_THREAD.store(lwp::get_self(), Ordering::Release);
        ASYNC_IPC_RESULT.store(result as u32, Ordering::Release);
        if result >= 0 && ipc::FileDescriptor::new(result).close().is_ok() {
            ASYNC_IPC_CLOSE_OK.store(1, Ordering::Release);
        }
        ASYNC_IPC_DONE.store(1, Ordering::Release);
        ipc::IPC_OK
    }

    unsafe extern "C" fn ipc_contention_worker(argument: *mut c_void) -> *mut c_void {
        let high = argument as usize != 0;
        let gate = if high {
            IPC_HIGH_GATE.load(Ordering::Acquire)
        } else {
            IPC_LOW_GATE.load(Ordering::Acquire)
        };
        if lwp::sem_wait(gate) != lwp::LWP_SUCCESSFUL {
            return ptr::null_mut();
        }
        let result = match ipc::open_file(c"/dev/stm/immediate", ipc::IPC_OPEN_NONE) {
            Ok(fd) => fd
                .close()
                .map(|_| ipc::IPC_OK)
                .unwrap_or_else(|error| error),
            Err(error) => error,
        };
        if high {
            IPC_HIGH_RESULT.store(result as u32, Ordering::Release);
            0x1a11usize as *mut c_void
        } else {
            IPC_LOW_RESULT.store(result as u32, Ordering::Release);
            0x1010usize as *mut c_void
        }
    }

    unsafe fn stack_base(stack: *mut Stack) -> *mut c_void {
        unsafe { core::ptr::addr_of_mut!((*stack).0).cast::<u8>().cast() }
    }

    fn create_with_stack(
        thread: &mut lwp::LwpThread,
        entry: lwp::ThreadEntry,
        argument: *mut c_void,
        stack: *mut Stack,
        priority: u8,
    ) -> i32 {
        unsafe {
            lwp::create_thread(
                thread,
                entry,
                argument,
                stack_base(stack),
                STACK_BYTES as u32,
                priority,
            )
        }
    }

    pub(crate) fn join(thread: lwp::LwpThread) -> Result<usize, i32> {
        let mut value = ptr::null_mut();
        // SAFETY: `value` is a valid unique output for the synchronous join.
        let rc = unsafe { lwp::join_thread(thread, &mut value) };
        if rc == lwp::LWP_SUCCESSFUL {
            Ok(value as usize)
        } else {
            Err(rc)
        }
    }

    fn probe_external_thread_setup(logger: &mut ProbeLogger) {
        let mut storage = [u64::MAX; 64];
        let storage_ptr = storage.as_mut_ptr().cast::<c_void>();
        // SAFETY: The aligned storage has 512 writable bytes and remains live until join.
        let initialized = unsafe { driver::initialize_external_reent(storage_ptr) };
        let errno = unsafe { storage_ptr.cast::<u32>().read() };
        logger.record(
            "LWP external reentrancy storage initializes",
            initialized && errno == 0,
        );
        if !initialized {
            return;
        }

        EXTERNAL_SETUP_RAN.store(0, Ordering::Release);
        let mut thread = lwp::LWP_THREAD_NULL;
        // SAFETY: The aligned static stack is unique to this worker until join;
        // storage and the handle output remain live for the worker's lifetime.
        let stack_top = unsafe {
            core::ptr::addr_of_mut!(STACK_A.0)
                .cast::<u8>()
                .add(STACK_BYTES)
                .cast()
        };
        let created = unsafe {
            driver::create_thread_at_top(
                &mut thread,
                Some(external_setup_worker),
                storage_ptr,
                stack_top,
                20,
                true,
            )
        };
        if created != 0 {
            logger.record("LWP stack-top worker starts suspended", false);
            return;
        }
        logger.record(
            "LWP stack-top worker starts suspended",
            lwp::thread_is_suspended(thread) != 0
                && EXTERNAL_SETUP_RAN.load(Ordering::Acquire) == 0,
        );
        // SAFETY: The initialized storage remains live until the worker is joined.
        let attached = unsafe { driver::set_thread_reent_storage(thread, storage_ptr) };
        let Some(mut mirror) = driver::thread_context_snapshot(thread) else {
            let _ = lwp::resume_thread(thread);
            let _ = join(thread);
            logger.record("LWP suspended thread context mirror is writable", false);
            return;
        };
        mirror.gpr[0] = 0x1357_2468;
        // SAFETY: The copied context has the required layout and outlives the worker.
        let mirrored = unsafe {
            driver::set_thread_context_mirror(
                thread,
                (&mut mirror as *mut driver::LwpContext).cast(),
            )
        };
        let mirrored_value =
            driver::thread_context_snapshot(thread).is_some_and(|ctx| ctx.gpr[0] == 0x1357_2468);
        logger.record(
            "LWP suspended thread context mirror is writable",
            mirrored && mirrored_value,
        );
        let resumed = lwp::resume_thread(thread) == 0;
        let result = join(thread);
        logger.record(
            "LWP external reentrancy reaches worker",
            attached && resumed && EXTERNAL_SETUP_RAN.load(Ordering::Acquire) == 1,
        );
        logger.record(
            "LWP explicit thread exit returns value",
            result == Ok(0x5a17),
        );
    }

    fn probe_external_wait_paths(logger: &mut ProbeLogger) {
        for scenario in 0..5 {
            EXTERNAL_WAIT_READY.store(0, Ordering::Release);
            EXTERNAL_WATCHDOG.store(0, Ordering::Release);
            let mut thread = lwp::LWP_THREAD_NULL;
            let created = create_with_stack(
                &mut thread,
                Some(external_wait_worker),
                ptr::null_mut(),
                core::ptr::addr_of_mut!(STACK_A),
                80,
            );
            if created != 0 {
                logger.record("LWP external wait worker starts", false);
                return;
            }
            // SAFETY: The watchdog uses only static state and is cancelled after the join.
            let timer = unsafe {
                driver::timer_start_with_key(
                    EXTERNAL_TIMER,
                    Some(external_wait_watchdog),
                    ptr::null_mut(),
                    driver::TICKS_PER_SECOND,
                    0,
                )
            };
            lwp::yield_thread();
            let ready = EXTERNAL_WAIT_READY.load(Ordering::Acquire) == 1;
            let (label, expected, submitted) = match scenario {
                0 => (
                    "LWP external direct wake returns token",
                    0x55,
                    driver::wake_thread_with_token(thread, 0x55),
                ),
                1 => {
                    driver::wake_external_by_value(EXTERNAL_KEY, 0x15, false);
                    ("LWP external value wake returns signal", 1, true)
                }
                2 => {
                    driver::wake_external_by_mask(EXTERNAL_KEY, 0x04, false);
                    ("LWP external mask wake returns matched bits", 4, true)
                }
                3 => {
                    driver::broadcast_external(EXTERNAL_KEY);
                    ("LWP external broadcast wakes waiter", 1, true)
                }
                _ => {
                    driver::cancel_external_wait(EXTERNAL_KEY, thread);
                    ("LWP external cancellation returns zero", 0, true)
                }
            };
            if timer != 0 {
                let _ = driver::wake_thread_with_token(thread, 0);
            }
            let result = join(thread);
            driver::timer_cancel(EXTERNAL_TIMER);
            logger.note_u32("external wait scenario", scenario);
            logger.note_u32("external wait worker ready", u32::from(ready));
            logger.note_i32("external wait timer result", timer);
            logger.note_u32("external wait submitted", u32::from(submitted));
            logger.note_i32(
                "external wait join result",
                result.map_or_else(|error| error, |value| value as i32),
            );
            logger.note_u32(
                "external wait watchdog ran",
                EXTERNAL_WATCHDOG.load(Ordering::Acquire),
            );
            logger.record(
                label,
                ready
                    && timer == 0
                    && submitted
                    && result == Ok(expected)
                    && EXTERNAL_WATCHDOG.load(Ordering::Acquire) == 0,
            );
        }
    }

    fn probe_scheduler_critical_sections(logger: &mut ProbeLogger) {
        fn msr() -> u32 {
            let value: u32;
            unsafe {
                core::arch::asm!("mfmsr {value}", value = out(reg) value, options(nomem, nostack))
            };
            value
        }
        let before = msr();
        let outer = driver::critical_enter();
        let first = msr();
        let inner = driver::critical_enter();
        let second = msr();
        driver::critical_exit(inner);
        let after_inner = msr();
        driver::critical_exit(outer);
        let after = msr();
        logger.record(
            "LWP nested critical sections restore interrupt state",
            before & 0x8000 != 0
                && first & 0x8000 == 0
                && second & 0x8000 == 0
                && after_inner & 0x8000 == 0
                && after & 0x8000 == before & 0x8000,
        );
    }

    fn probe_reentrancy_storage(logger: &mut ProbeLogger) {
        let parent = driver::current_reent();
        // SAFETY: The newlib hook returns the current thread's storage.
        let parent_abi = unsafe { __getreent() };
        CHILD_REENT.store(0, Ordering::Release);
        let mut child = lwp::LWP_THREAD_NULL;
        let rc = create_with_stack(
            &mut child,
            Some(reent_worker),
            ptr::null_mut(),
            core::ptr::addr_of_mut!(STACK_A),
            20,
        );
        let result = if rc == 0 { join(child) } else { Err(rc) };
        logger.record(
            "LWP newlib storage is stable and thread local",
            !parent.is_null()
                && parent == parent_abi
                && result == Ok(1)
                && CHILD_REENT.load(Ordering::Acquire) != 0
                && CHILD_REENT.load(Ordering::Acquire) != parent as u32
                && driver::current_reent() == parent,
        );
    }

    fn probe_reschedule_rotation(logger: &mut ProbeLogger) {
        for abi in [false, true] {
            PRIORITY_ORDER.store(0, Ordering::Release);
            PRIORITY_POSITION.store(0, Ordering::Release);
            ROTATION_ABI.store(u32::from(abi), Ordering::Release);
            let mut a = lwp::LWP_THREAD_NULL;
            let mut b = lwp::LWP_THREAD_NULL;
            let a_rc = create_with_stack(
                &mut a,
                Some(rotation_worker),
                1usize as *mut c_void,
                core::ptr::addr_of_mut!(STACK_A),
                20,
            );
            let b_rc = create_with_stack(
                &mut b,
                Some(rotation_worker),
                2usize as *mut c_void,
                core::ptr::addr_of_mut!(STACK_B),
                20,
            );
            let a_result = if a_rc == 0 { join(a) } else { Err(a_rc) };
            let b_result = if b_rc == 0 { join(b) } else { Err(b_rc) };
            let order = PRIORITY_ORDER.load(Ordering::Acquire);
            logger.note_hex32("LWP reschedule worker order", order);
            logger.record(
                if abi {
                    "LWP ABI reschedule rotates equal priority workers"
                } else {
                    "LWP reschedule rotates equal priority workers"
                },
                a_result == Ok(1)
                    && b_result == Ok(2)
                    && PRIORITY_POSITION.load(Ordering::Acquire) == 4
                    && (order == 0x2121 || order == 0x1212),
            );
        }
    }

    fn probe_priority_order(logger: &mut ProbeLogger) {
        for abi in [false, true] {
            PRIORITY_ORDER.store(0, Ordering::Release);
            PRIORITY_POSITION.store(0, Ordering::Release);
            let mut a = lwp::LWP_THREAD_NULL;
            let mut b = lwp::LWP_THREAD_NULL;
            let a_rc = create_with_stack(
                &mut a,
                Some(priority_worker),
                1usize as *mut c_void,
                core::ptr::addr_of_mut!(STACK_A),
                if abi { 30 } else { 20 },
            );
            let b_rc = create_with_stack(
                &mut b,
                Some(priority_worker),
                2usize as *mut c_void,
                core::ptr::addr_of_mut!(STACK_B),
                if abi { 20 } else { 30 },
            );
            if abi {
                lwp::lwp_set_thread_priority_abi(b, 50);
            } else {
                lwp::set_thread_priority(a, 40);
            }
            let a_result = if a_rc == 0 { join(a) } else { Err(a_rc) };
            let b_result = if b_rc == 0 { join(b) } else { Err(b_rc) };
            logger.record(
                if abi {
                    "LWP ABI priority change selects higher worker"
                } else {
                    "LWP priority change selects higher worker"
                },
                a_result == Ok(1)
                    && b_result == Ok(2)
                    && PRIORITY_POSITION.load(Ordering::Acquire) == 2
                    && PRIORITY_ORDER.load(Ordering::Acquire) == if abi { 0x12 } else { 0x21 },
            );
        }
    }

    fn run_switch_and_join(logger: &mut ProbeLogger) {
        logger.section("LWP SWITCH/JOIN STRESS");
        let before = lwp::debug_counters();
        let mut a = lwp::LWP_THREAD_NULL;
        let mut b = lwp::LWP_THREAD_NULL;
        let a_rc = create_with_stack(
            &mut a,
            Some(switch_worker),
            ptr::null_mut(),
            core::ptr::addr_of_mut!(STACK_A),
            80,
        );
        let b_rc = create_with_stack(
            &mut b,
            Some(switch_worker),
            core::ptr::dangling_mut::<c_void>(),
            core::ptr::addr_of_mut!(STACK_B),
            80,
        );
        logger.record("created two equal-priority workers", a_rc == 0 && b_rc == 0);
        let a_result = join(a);
        let b_result = join(b);
        logger.record("worker A joined with return value", a_result == Ok(0x100));
        logger.record("worker B joined with return value", b_result == Ok(0x101));
        logger.record(
            "worker A completed all switches",
            SWITCH_A.load(Ordering::Acquire) == SWITCH_ITERATIONS,
        );
        logger.record(
            "worker B completed all switches",
            SWITCH_B.load(Ordering::Acquire) == SWITCH_ITERATIONS,
        );
        let after = lwp::debug_counters();
        logger.note_u64(
            "context switches delta",
            after
                .context_switches
                .saturating_sub(before.context_switches),
        );
        logger.record(
            "context switch stress threshold",
            after
                .context_switches
                .saturating_sub(before.context_switches)
                >= (SWITCH_ITERATIONS as u64),
        );
    }

    fn run_synchronization(logger: &mut ProbeLogger) {
        logger.section("LWP SYNCHRONIZATION");
        let mut mutex = lwp::LWP_MUTEX_NULL;
        // SAFETY: `mutex` is a valid unique handle output.
        let mutex_init_rc = unsafe { lwp::mutex_init(&mut mutex, 0) };
        logger.record("mutex init", mutex_init_rc == 0);
        if mutex_init_rc != 0 {
            return;
        }
        let mutex_handle = MutexHandle::new(mutex);
        let mut cond = lwp::LWP_COND_NULL;
        let mut sem = lwp::LWP_SEM_NULL;
        // SAFETY: each local is a valid unique handle output.
        logger.record("condition init", unsafe { lwp::cond_init(&mut cond) } == 0);
        logger.record(
            "semaphore init",
            unsafe { lwp::sem_init(&mut sem, 0, 0) } == 0,
        );
        let sem_handle = SemaphoreHandle::new(sem);
        logger.record(
            "typed semaphore starts with zero permits",
            sem_handle.get_value() == Ok(0),
        );
        logger.record("typed semaphore post succeeds", sem_handle.post().is_ok());
        logger.record(
            "typed semaphore reports the posted permit",
            sem_handle.get_value() == Ok(1),
        );
        logger.record(
            "raw semaphore wait consumes permit",
            lwp::sem_wait(sem) == 0,
        );
        logger.record(
            "typed semaphore value returns to zero",
            sem_handle.get_value() == Ok(0),
        );
        let first_try_lock = mutex_handle.try_lock();
        logger.record(
            "mutex try-lock acquires free mutex",
            first_try_lock == Ok(true),
        );
        let held_try_lock = mutex_handle.try_lock();
        logger.record(
            "nonrecursive mutex try-lock reports busy",
            held_try_lock == Ok(false),
        );
        let try_lock_release = mutex_handle.unlock();
        logger.record(
            "mutex try-lock acquisition can be released",
            try_lock_release.is_ok(),
        );
        MUTEX.store(mutex, Ordering::Release);
        COND.store(cond, Ordering::Release);
        SEM.store(sem, Ordering::Release);

        SYNC_MUTEX_LOCK_RC.store(u32::MAX, Ordering::Release);
        SYNC_COND_SIGNAL_RC.store(u32::MAX, Ordering::Release);
        SYNC_MUTEX_UNLOCK_RC.store(u32::MAX, Ordering::Release);
        let main_lock_result = mutex_handle.lock();
        logger.record(
            "mutex lock acquires available mutex",
            main_lock_result.is_ok(),
        );
        let mut worker = lwp::LWP_THREAD_NULL;
        let create = create_with_stack(
            &mut worker,
            Some(sync_worker),
            ptr::null_mut(),
            core::ptr::addr_of_mut!(STACK_SYNC),
            90,
        );
        logger.record("condition worker created", create == 0);
        let timeout = lwp::TimeSpec {
            tv_sec: 2,
            tv_nsec: 0,
        };
        // SAFETY: `timeout` remains valid for the synchronous wait call.
        let wait_rc = unsafe { lwp::cond_timed_wait(cond, mutex, &timeout) };
        logger.record("condition wait completes before timeout", wait_rc == 0);
        logger.record(
            "condition worker acquires mutex",
            SYNC_MUTEX_LOCK_RC.load(Ordering::Acquire) == 0,
        );
        logger.record(
            "condition signal succeeds",
            SYNC_COND_SIGNAL_RC.load(Ordering::Acquire) == 0,
        );
        logger.record(
            "condition worker releases mutex",
            SYNC_MUTEX_UNLOCK_RC.load(Ordering::Acquire) == 0,
        );
        let main_unlock_result = mutex_handle.unlock();
        logger.record(
            "condition waiter releases mutex",
            main_unlock_result.is_ok(),
        );
        logger.record(
            "condition wait observed worker update",
            wait_rc == 0 && SHARED_VALUE.load(Ordering::Acquire) == 0x51a7,
        );
        logger.record("condition worker joined", join(worker) == Ok(0x51a7));

        let timeout_lock = mutex_handle.lock();
        logger.record(
            "driver condition timeout mutex acquired",
            timeout_lock.is_ok(),
        );
        let timeout_rc = driver::cond_wait(cond, mutex, Some(driver::TICKS_PER_SECOND / 20));
        let timeout_unlock = mutex_handle.unlock();
        logger.record("driver condition wait times out", timeout_rc == -116);
        logger.record(
            "driver condition timeout leaves mutex unlocked",
            timeout_unlock.is_ok(),
        );

        let mut sem_worker = lwp::LWP_THREAD_NULL;
        let create = create_with_stack(
            &mut sem_worker,
            Some(semaphore_worker),
            ptr::null_mut(),
            core::ptr::addr_of_mut!(STACK_SEM),
            90,
        );
        logger.record("semaphore worker created", create == 0);
        lwp::yield_thread();
        logger.record(
            "zero-count semaphore keeps worker blocked",
            create == 0
                && SEM_WAIT_RC.load(Ordering::Acquire) == u32::MAX
                && SEM_RELEASED.load(Ordering::Acquire) == 0,
        );
        logger.record("semaphore post", lwp::sem_post(sem) == 0);
        logger.record("semaphore worker joined", join(sem_worker) == Ok(0x5e));
        logger.record(
            "semaphore wait completes after post",
            SEM_WAIT_RC.load(Ordering::Acquire) == 0 && SEM_RELEASED.load(Ordering::Acquire) == 1,
        );

        logger.record("semaphore destroy", lwp::sem_destroy(sem) == 0);
        logger.record("condition destroy", lwp::cond_destroy(cond) == 0);
        logger.record("mutex destroy", lwp::mutex_destroy(mutex) == 0);

        let mut reused_mutex = lwp::LWP_MUTEX_NULL;
        match MutexHandle::init(&mut reused_mutex, false) {
            Ok(reused_handle) => {
                logger.record("mutex init reuses destroyed slot", reused_mutex == mutex);
                let acquired = reused_handle.try_lock() == Ok(true);
                logger.record("reinitialized mutex can be locked", acquired);
                logger.record(
                    "reinitialized mutex unlock succeeds",
                    reused_handle.unlock().is_ok(),
                );
                logger.record(
                    "reinitialized mutex destroy succeeds",
                    reused_handle.destroy().is_ok(),
                );
            }
            Err(_) => {
                logger.record("mutex init reuses destroyed slot", false);
                logger.record("reinitialized mutex can be locked", false);
                logger.record("reinitialized mutex unlock succeeds", false);
                logger.record("reinitialized mutex destroy succeeds", false);
            }
        }
    }

    static BROADCAST_READY: AtomicU32 = AtomicU32::new(0);
    static BROADCAST_WOKEN: AtomicU32 = AtomicU32::new(0);
    static BROADCAST_MUTEX: AtomicU32 = AtomicU32::new(0);
    static BROADCAST_COND: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn broadcast_worker(arg: *mut c_void) -> *mut c_void {
        let mutex = BROADCAST_MUTEX.load(Ordering::Acquire);
        let cond = BROADCAST_COND.load(Ordering::Acquire);
        if lwp::mutex_lock(mutex) != 0 {
            return ptr::null_mut();
        }
        BROADCAST_READY.fetch_add(1, Ordering::Release);
        let rc = if arg as usize == 0 {
            lwp::cond_wait(cond, mutex)
        } else {
            lwp::lwp_cond_wait_abi(cond, mutex)
        };
        if rc == 0 {
            BROADCAST_WOKEN.fetch_add(1, Ordering::Release);
        }
        let unlocked = lwp::mutex_unlock(mutex) == 0;
        if rc == 0 && unlocked {
            0xc011usize as *mut c_void
        } else {
            ptr::null_mut()
        }
    }

    fn wait_for_counter(counter: &AtomicU32, count: u32) -> bool {
        let deadline = driver::now_ticks().wrapping_add(driver::TICKS_PER_SECOND);
        while counter.load(Ordering::Acquire) < count && driver::now_ticks() < deadline {
            driver::sleep_ticks(driver::TICKS_PER_SECOND / 100);
        }
        counter.load(Ordering::Acquire) >= count
    }

    fn run_condition_broadcast(logger: &mut ProbeLogger) {
        logger.section("LWP CONDITION SIGNAL AND BROADCAST");
        for raw_broadcast in [false, true] {
            BROADCAST_READY.store(0, Ordering::Release);
            BROADCAST_WOKEN.store(0, Ordering::Release);
            let mut mutex = lwp::LWP_MUTEX_NULL;
            let mut cond = lwp::LWP_COND_NULL;
            // SAFETY: Both outputs are valid writable handles.
            let mutex_rc = unsafe { lwp::mutex_init(&mut mutex, 0) };
            logger.record("Broadcast mutex initializes", mutex_rc == 0);
            if mutex_rc != 0 {
                return;
            }
            let cond_rc = unsafe { lwp::cond_init(&mut cond) };
            logger.record("Broadcast condition initializes", cond_rc == 0);
            if cond_rc != 0 {
                logger.record(
                    "Broadcast unused mutex destroys",
                    lwp::mutex_destroy(mutex) == 0,
                );
                return;
            }
            BROADCAST_MUTEX.store(mutex, Ordering::Release);
            BROADCAST_COND.store(cond, Ordering::Release);
            let mut a = lwp::LWP_THREAD_NULL;
            let mut b = lwp::LWP_THREAD_NULL;
            let ca = create_with_stack(
                &mut a,
                Some(broadcast_worker),
                ptr::null_mut(),
                core::ptr::addr_of_mut!(STACK_A),
                80,
            );
            let cb = create_with_stack(
                &mut b,
                Some(broadcast_worker),
                core::ptr::dangling_mut::<c_void>(),
                core::ptr::addr_of_mut!(STACK_B),
                80,
            );
            logger.record("Broadcast waiters create", ca == 0 && cb == 0);
            if ca != 0 || cb != 0 {
                return;
            }
            let ready = wait_for_counter(&BROADCAST_READY, 2);
            logger.record("Broadcast waiters reach condition", ready);
            if !ready {
                return;
            }
            // The worker increments ready under this mutex immediately before it waits.
            let locked = lwp::mutex_lock(mutex) == 0;
            logger.record("Broadcast notifier acquires mutex", locked);
            if !locked {
                return;
            }
            if raw_broadcast {
                logger.record("Raw condition broadcasts", lwp::cond_broadcast(cond) == 0);
            } else {
                logger.record("ABI condition signals", lwp::lwp_cond_signal_abi(cond) == 0);
            }
            logger.record("Broadcast notifier unlocks", lwp::mutex_unlock(mutex) == 0);
            if raw_broadcast {
                logger.record(
                    "Raw broadcast wakes both waiters",
                    wait_for_counter(&BROADCAST_WOKEN, 2),
                );
            } else {
                let first = wait_for_counter(&BROADCAST_WOKEN, 1);
                logger.record(
                    "ABI signal wakes exactly one waiter",
                    first && BROADCAST_WOKEN.load(Ordering::Acquire) == 1,
                );
                let locked = lwp::mutex_lock(mutex) == 0;
                logger.record("ABI broadcast notifier locks", locked);
                if locked {
                    logger.record(
                        "ABI condition broadcasts",
                        lwp::lwp_cond_broadcast_abi(cond) == 0,
                    );
                    logger.record(
                        "ABI broadcast notifier unlocks",
                        lwp::mutex_unlock(mutex) == 0,
                    );
                }
                logger.record(
                    "ABI broadcast wakes remaining waiter",
                    wait_for_counter(&BROADCAST_WOKEN, 2),
                );
            }
            logger.record("Broadcast first waiter joins", join(a) == Ok(0xc011));
            logger.record("Broadcast second waiter joins", join(b) == Ok(0xc011));
            logger.record("Broadcast condition destroys", lwp::cond_destroy(cond) == 0);
            logger.record("Broadcast mutex destroys", lwp::mutex_destroy(mutex) == 0);
        }
    }

    static ABI_WORKER_RUN: AtomicU32 = AtomicU32::new(0);
    static ABI_WORKER_COUNT: AtomicU32 = AtomicU32::new(0);
    static ABI_WORKER_SELF: AtomicU32 = AtomicU32::new(0);
    static ABI_WORKER_GATE: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn abi_worker(_: *mut c_void) -> *mut c_void {
        ABI_WORKER_SELF.store(lwp::lwp_get_self_abi(), Ordering::Release);
        while lwp::sem_wait(ABI_WORKER_GATE.load(Ordering::Acquire)) == 0 {
            if ABI_WORKER_RUN.load(Ordering::Acquire) == 0 {
                break;
            }
            ABI_WORKER_COUNT.fetch_add(1, Ordering::Release);
            lwp::lwp_yield_thread_abi();
        }
        0x7155usize as *mut c_void
    }

    fn run_legacy_thread_abi(logger: &mut ProbeLogger) {
        logger.section("LWP LEGACY THREAD ABI");
        ABI_WORKER_COUNT.store(0, Ordering::Relaxed);
        ABI_WORKER_SELF.store(0, Ordering::Relaxed);
        ABI_WORKER_RUN.store(1, Ordering::Release);
        let mut gate = lwp::LWP_SEM_NULL;
        // SAFETY: The gate is valid writable handle storage.
        let gate_rc = unsafe { lwp::sem_init(&mut gate, 0, 1) };
        logger.record("ABI worker gate initializes", gate_rc == 0);
        if gate_rc != 0 {
            return;
        }
        ABI_WORKER_GATE.store(gate, Ordering::Release);
        let mut thread = lwp::LWP_THREAD_NULL;
        // SAFETY: This stack is no longer used by the joined switch workers.
        let created = unsafe {
            lwp::lwp_create_thread_abi(
                &mut thread,
                Some(abi_worker),
                ptr::null_mut(),
                stack_base(core::ptr::addr_of_mut!(STACK_A)),
                STACK_BYTES as u32,
                80,
            )
        };
        logger.record("ABI thread creates", created == 0);
        if created != 0 {
            logger.record(
                "unused ABI worker gate destroys",
                lwp::sem_destroy(gate) == 0,
            );
            return;
        }
        logger.record("ABI worker starts", lwp::sem_post(gate) == 0);
        let deadline = driver::now_ticks().wrapping_add(driver::TICKS_PER_SECOND);
        while ABI_WORKER_COUNT.load(Ordering::Acquire) == 0 && driver::now_ticks() < deadline {
            driver::sleep_ticks(driver::TICKS_PER_SECOND / 100);
        }
        logger.record(
            "ABI worker runs and reports its handle",
            ABI_WORKER_COUNT.load(Ordering::Acquire) > 0
                && ABI_WORKER_SELF.load(Ordering::Acquire) == thread,
        );
        let suspended = lwp::lwp_suspend_thread_abi(thread) == 0;
        logger.record(
            "ABI thread suspends",
            suspended
                && lwp::lwp_thread_is_suspended_abi(thread) != 0
                && lwp::thread_is_suspended(thread) != 0,
        );
        if suspended {
            let before = ABI_WORKER_COUNT.load(Ordering::Acquire);
            logger.record(
                "ABI suspended worker receives permit",
                lwp::sem_post(gate) == 0,
            );
            driver::sleep_ticks(driver::TICKS_PER_SECOND / 50);
            logger.record(
                "ABI suspended worker stops",
                ABI_WORKER_COUNT.load(Ordering::Acquire) == before,
            );
            let resumed = lwp::lwp_resume_thread_abi(thread) == 0;
            driver::sleep_ticks(driver::TICKS_PER_SECOND / 50);
            logger.record(
                "ABI resumed worker runs",
                resumed
                    && lwp::lwp_thread_is_suspended_abi(thread) == 0
                    && ABI_WORKER_COUNT.load(Ordering::Acquire) > before,
            );
        }
        let suspended = lwp::suspend_thread(thread) == 0;
        logger.record(
            "Raw thread suspends",
            suspended && lwp::thread_is_suspended(thread) != 0,
        );
        if suspended {
            let before = ABI_WORKER_COUNT.load(Ordering::Acquire);
            logger.record(
                "Raw suspended worker receives permit",
                lwp::sem_post(gate) == 0,
            );
            driver::sleep_ticks(driver::TICKS_PER_SECOND / 50);
            let stopped = ABI_WORKER_COUNT.load(Ordering::Acquire) == before;
            let resumed = lwp::resume_thread(thread) == 0;
            driver::sleep_ticks(driver::TICKS_PER_SECOND / 50);
            logger.record(
                "Raw thread resumes",
                stopped
                    && resumed
                    && lwp::thread_is_suspended(thread) == 0
                    && ABI_WORKER_COUNT.load(Ordering::Acquire) > before,
            );
        }
        ABI_WORKER_RUN.store(0, Ordering::Release);
        logger.record("ABI worker exit permit posts", lwp::sem_post(gate) == 0);
        let mut result = ptr::null_mut();
        // SAFETY: `result` is valid writable storage for the join.
        let joined = unsafe { lwp::lwp_join_thread_abi(thread, &mut result) };
        logger.record(
            "ABI thread joins with result",
            joined == 0 && result as usize == 0x7155,
        );
        logger.record("ABI worker gate destroys", lwp::sem_destroy(gate) == 0);
    }

    fn run_legacy_sync_abi(logger: &mut ProbeLogger) {
        logger.section("LWP LEGACY SYNC ABI");
        let mut mutex = lwp::LWP_MUTEX_NULL;
        // SAFETY: All output handles and values are valid local storage.
        let initialized = unsafe { lwp::lwp_mutex_init_abi(&mut mutex, 0) };
        logger.record("ABI mutex initializes", initialized == 0);
        if initialized == 0 {
            let locked = lwp::lwp_mutex_lock_abi(mutex) == 0;
            logger.record("ABI mutex locks", locked);
            if locked {
                let busy = lwp::lwp_mutex_try_lock_abi(mutex);
                logger.note_i32("ABI mutex held try-lock result", busy);
                logger.record("ABI mutex held try-lock fails", busy != 0);
                logger.record("ABI mutex unlocks", lwp::lwp_mutex_unlock_abi(mutex) == 0);
            }
            let acquired = lwp::lwp_mutex_try_lock_abi(mutex) == 0;
            logger.record("ABI mutex try-lock acquires free mutex", acquired);
            if acquired {
                logger.record(
                    "ABI try-lock releases",
                    lwp::lwp_mutex_unlock_abi(mutex) == 0,
                );
            }
            let mut cond = lwp::LWP_COND_NULL;
            let cond_init = unsafe { lwp::lwp_cond_init_abi(&mut cond) };
            logger.record("ABI condition initializes", cond_init == 0);
            if cond_init == 0 {
                let locked = lwp::lwp_mutex_lock_abi(mutex) == 0;
                logger.record("ABI timed wait mutex locks", locked);
                if locked {
                    let timeout = lwp::TimeSpec {
                        tv_sec: 0,
                        tv_nsec: 25_000_000,
                    };
                    let start = driver::now_ticks();
                    let rc = unsafe { lwp::lwp_cond_timed_wait_abi(cond, mutex, &timeout) };
                    let elapsed = driver::now_ticks().wrapping_sub(start);
                    logger.record(
                        "ABI timed wait returns timeout after delay",
                        rc == 116 && elapsed >= driver::TICKS_PER_SECOND / 50,
                    );
                    logger.record(
                        "ABI timed wait returns mutex",
                        lwp::lwp_mutex_unlock_abi(mutex) == 0,
                    );
                }
                logger.record(
                    "ABI condition destroys",
                    lwp::lwp_cond_destroy_abi(cond) == 0,
                );
            }
            logger.record("ABI mutex destroys", lwp::lwp_mutex_destroy_abi(mutex) == 0);
        }
        let mut sem = lwp::LWP_SEM_NULL;
        let initialized = unsafe { lwp::lwp_sem_init_abi(&mut sem, 1, 2) };
        logger.record("ABI semaphore initializes", initialized == 0);
        if initialized == 0 {
            let mut value = u32::MAX;
            logger.record(
                "ABI semaphore starts at one",
                unsafe { lwp::lwp_sem_get_value_abi(sem, &mut value) } == 0 && value == 1,
            );
            logger.record(
                "ABI semaphore wait consumes permit",
                lwp::lwp_sem_wait_abi(sem) == 0,
            );
            logger.record(
                "ABI semaphore reads zero",
                unsafe { lwp::lwp_sem_get_value_abi(sem, &mut value) } == 0 && value == 0,
            );
            logger.record("ABI semaphore posts", lwp::lwp_sem_post_abi(sem) == 0);
            logger.record(
                "ABI semaphore reads posted permit",
                unsafe { lwp::lwp_sem_get_value_abi(sem, &mut value) } == 0 && value == 1,
            );
            logger.record("ABI semaphore destroys", lwp::lwp_sem_destroy_abi(sem) == 0);
        }
        let mut queue = lwp::MQ_BOX_NULL;
        let initialized = unsafe { lwp::mq_init_abi(&mut queue, 3) };
        logger.record("ABI message queue initializes", initialized == 0);
        if initialized == 0 {
            let first = 0x1234usize as *mut c_void;
            let second = 0x5678usize as *mut c_void;
            let third = 0x9abcusize as *mut c_void;
            let sent = lwp::mq_send_abi(queue, first, lwp::MQ_MSG_NOBLOCK) != 0;
            let jammed = lwp::mq_jam_abi(queue, second, lwp::MQ_MSG_NOBLOCK) != 0;
            let raw_jammed = lwp::mq_jam(queue, third, lwp::MQ_MSG_NOBLOCK) != 0;
            logger.record(
                "ABI queue sends and jams messages",
                sent && jammed && raw_jammed,
            );
            let mut out = ptr::null_mut();
            let a = unsafe { lwp::mq_receive_abi(queue, &mut out, lwp::MQ_MSG_NOBLOCK) } != 0
                && out == third;
            let b = unsafe { lwp::mq_receive_abi(queue, &mut out, lwp::MQ_MSG_NOBLOCK) } != 0
                && out == second;
            let c = unsafe { lwp::mq_receive_abi(queue, &mut out, lwp::MQ_MSG_NOBLOCK) } != 0
                && out == first;
            let empty = unsafe { lwp::mq_receive_abi(queue, &mut out, lwp::MQ_MSG_NOBLOCK) } == 0;
            logger.record(
                "ABI queue returns jammed messages first",
                a && b && c && empty,
            );
            lwp::mq_close_abi(queue);
            logger.record(
                "ABI queue closes",
                lwp::mq_send_abi(queue, first, lwp::MQ_MSG_NOBLOCK) == 0,
            );
        }
    }

    fn run_timed_wait(logger: &mut ProbeLogger) {
        logger.section("LWP TIMED WAIT");
        let mut mutex = lwp::LWP_MUTEX_NULL;
        let mut cond = lwp::LWP_COND_NULL;
        // SAFETY: both locals are valid unique handle outputs.
        let initialized = unsafe { lwp::mutex_init(&mut mutex, 0) };
        logger.record("timed wait mutex initialized", initialized == 0);
        if initialized != 0 {
            return;
        }
        let initialized = unsafe { lwp::cond_init(&mut cond) };
        logger.record("timed wait condition initialized", initialized == 0);
        if initialized != 0 {
            logger.record(
                "failed condition setup released mutex",
                lwp::mutex_destroy(mutex) == 0,
            );
            return;
        }
        let timeout = lwp::TimeSpec {
            tv_sec: 0,
            tv_nsec: 25_000_000,
        };
        let locked = lwp::mutex_lock(mutex);
        logger.record("timed wait mutex acquired", locked == 0);
        if locked != 0 {
            logger.record("unused condition released", lwp::cond_destroy(cond) == 0);
            logger.record("unused mutex released", lwp::mutex_destroy(mutex) == 0);
            return;
        }
        let start = driver::now_ticks();
        // SAFETY: `timeout` remains valid for the synchronous wait setup.
        let rc = unsafe { lwp::cond_timed_wait(cond, mutex, &timeout) };
        let elapsed = driver::now_ticks().wrapping_sub(start);
        logger.record("timed wait mutex released", lwp::mutex_unlock(mutex) == 0);
        logger.note_i32("timed wait rc", rc);
        logger.note_u64("timed wait ticks", elapsed);
        logger.record("timed wait reports Newlib ETIMEDOUT", rc == 116);
        logger.record(
            "timed wait elapsed",
            elapsed >= driver::TICKS_PER_SECOND / 50,
        );
        logger.record(
            "timed wait condition destroyed",
            lwp::cond_destroy(cond) == 0,
        );
        logger.record("timed wait mutex destroyed", lwp::mutex_destroy(mutex) == 0);
    }

    fn run_message_and_deferred(logger: &mut ProbeLogger) {
        logger.section("LWP QUEUE/DEFERRED");
        let mut mq = lwp::MQ_BOX_NULL;
        // SAFETY: `mq` is a valid unique handle output.
        logger.record(
            "message queue init",
            unsafe { lwp::mq_init(&mut mq, 2) } == 0,
        );
        MQ.store(mq, Ordering::Release);
        let mut worker = lwp::LWP_THREAD_NULL;
        logger.record(
            "message receiver created",
            create_with_stack(
                &mut worker,
                Some(message_worker),
                ptr::null_mut(),
                core::ptr::addr_of_mut!(STACK_MQ),
                85,
            ) == 0,
        );
        let message = 0xcafeusize as *mut c_void;
        logger.record(
            "blocking message send",
            lwp::mq_send(mq, message, lwp::MQ_MSG_BLOCK) != 0,
        );
        logger.record("message receiver joined", join(worker) == Ok(0xcafe));
        lwp::mq_close(mq);

        logger.record(
            "deferred callback queued",
            // SAFETY: this callback ignores its null argument.
            unsafe { lwp::deferred_call(Some(deferred_callback), ptr::null_mut()) } == 0,
        );
        while DEFERRED_RAN.load(Ordering::Acquire) == 0 {
            lwp::yield_thread();
        }
        logger.record(
            "deferred callback executed",
            DEFERRED_RAN.load(Ordering::Acquire) == 1,
        );
    }

    fn run_tuxedo_compatibility(logger: &mut ProbeLogger) {
        logger.section("TUXEDO OVER LWP");
        unsafe {
            let stack_top = core::ptr::addr_of_mut!(STACK_TUXEDO.0)
                .cast::<u8>()
                .add(STACK_BYTES)
                .cast();
            tuxedo::k_thread_prepare(
                core::ptr::addr_of_mut!(TUXEDO_THREAD),
                Some(tuxedo_queue_worker),
                ptr::null_mut(),
                stack_top,
                0x30,
            );
            logger.record(
                "Tuxedo prepare created suspended LWP thread",
                TUXEDO_THREAD.state == tuxedo::KTHR_STATE_WAITING && TUXEDO_THREAD.suspend == 1,
            );
            tuxedo::k_thread_resume(core::ptr::addr_of_mut!(TUXEDO_THREAD));
        }
        lwp::yield_thread();
        let blocked = unsafe {
            TUXEDO_THREAD.state == tuxedo::KTHR_STATE_WAITING && !TUXEDO_QUEUE.next.is_null()
        };
        logger.record("Tuxedo queue blocked through LWP", blocked);
        // SAFETY: both statics remain live for the complete Tuxedo probe.
        unsafe {
            tuxedo::k_thr_queue_unblock_one_by_value(core::ptr::addr_of_mut!(TUXEDO_QUEUE), 0x55)
        };
        // SAFETY: the static thread object is live and joinable here.
        let result = unsafe { tuxedo::k_thread_join(core::ptr::addr_of_mut!(TUXEDO_THREAD)) };
        logger.note_i32("Tuxedo join result", result as i32);
        logger.record("Tuxedo queue wake token propagated", result == 1);
        logger.record("Tuxedo thread finished on LWP scheduler", unsafe {
            TUXEDO_THREAD.state == tuxedo::KTHR_STATE_FINISHED
        });
        logger.record(
            "Tuxedo exit clears its FPU owner register",
            fpu_owner_register() == 0,
        );
        // SAFETY: the static task remains live until it is stopped below.
        unsafe {
            tuxedo::k_tick_task_start(
                core::ptr::addr_of_mut!(TUXEDO_TICK_TASK),
                Some(tuxedo_tick_callback),
                driver::TICKS_PER_SECOND / 100,
                driver::TICKS_PER_SECOND / 100,
            )
        };
        let deadline = driver::now_ticks().wrapping_add(driver::TICKS_PER_SECOND);
        while TUXEDO_TICK_RAN.load(Ordering::Acquire) < 3
            && driver::now_ticks().wrapping_sub(deadline) >= (1u64 << 63)
        {
            lwp::yield_thread();
        }
        logger.record(
            "Tuxedo periodic tick task used LWP decrementer timer",
            TUXEDO_TICK_RAN.load(Ordering::Acquire) >= 3,
        );
        // SAFETY: this is the same live static task started above.
        unsafe { tuxedo::k_tick_task_stop(core::ptr::addr_of_mut!(TUXEDO_TICK_TASK)) };

        logger.section("DECREMENTER TIMER CALLBACK");
        DECREMENTER_CALLBACK_RAN.store(0, Ordering::Release);
        let started = driver::now_ticks();
        let result = unsafe {
            driver::timer_start_with_key(
                0x4c57_5043,
                Some(decrementer_callback),
                ptr::null_mut(),
                driver::TICKS_PER_SECOND / 100,
                0,
            )
        };
        let timer_started = result == lwp::LWP_SUCCESSFUL;
        while timer_started
            && DECREMENTER_CALLBACK_RAN.load(Ordering::Acquire) == 0
            && driver::now_ticks().wrapping_sub(started) < driver::TICKS_PER_SECOND
        {
            core::hint::spin_loop();
        }
        driver::timer_cancel(0x4c57_5043);
        logger.record(
            "timer callback ran without a scheduler yield",
            timer_started && DECREMENTER_CALLBACK_RAN.load(Ordering::Acquire) == 1,
        );
    }

    fn run_ipc_wakeup(logger: &mut ProbeLogger) {
        logger.section("IPC INTERRUPT WAKE");
        let before = lwp::debug_counters().ipc_wakeups;
        let open = ipc::open_file(c"/dev/stm/immediate", ipc::IPC_OPEN_NONE);
        let opened = open.is_ok();
        logger.record("IOS open completed", opened);
        if let Ok(fd) = open {
            logger.record("IOS close completed", fd.close().is_ok());
        }
        let after = lwp::debug_counters().ipc_wakeups;
        logger.note_u64("ipc wakeups delta", after.saturating_sub(before));
        logger.record("reply interrupt woke LWP queue", after > before);

        let mut low_gate = lwp::LWP_SEM_NULL;
        let mut high_gate = lwp::LWP_SEM_NULL;
        logger.record(
            "IPC low-priority gate init",
            // SAFETY: `low_gate` is a valid unique handle output.
            unsafe { lwp::sem_init(&mut low_gate, 0, 1) } == 0,
        );
        logger.record(
            "IPC high-priority gate init",
            // SAFETY: `high_gate` is a valid unique handle output.
            unsafe { lwp::sem_init(&mut high_gate, 0, 1) } == 0,
        );
        IPC_LOW_GATE.store(low_gate, Ordering::Release);
        IPC_HIGH_GATE.store(high_gate, Ordering::Release);
        let mut low = lwp::LWP_THREAD_NULL;
        let mut high = lwp::LWP_THREAD_NULL;
        logger.record(
            "IPC low-priority caller created",
            create_with_stack(
                &mut low,
                Some(ipc_contention_worker),
                ptr::null_mut(),
                core::ptr::addr_of_mut!(STACK_IPC_LOW),
                70,
            ) == 0,
        );
        logger.record(
            "IPC high-priority caller created",
            create_with_stack(
                &mut high,
                Some(ipc_contention_worker),
                core::ptr::dangling_mut::<c_void>(),
                core::ptr::addr_of_mut!(STACK_IPC_HIGH),
                100,
            ) == 0,
        );
        let _ = lwp::sem_post(low_gate);
        lwp::yield_thread();
        let _ = lwp::sem_post(high_gate);
        logger.record("high-priority IPC caller joined", join(high) == Ok(0x1a11));
        logger.record("low-priority IPC caller joined", join(low) == Ok(0x1010));
        logger.record(
            "scheduler-aware IPC serialization completed both callers",
            IPC_LOW_RESULT.load(Ordering::Acquire) == ipc::IPC_OK as u32
                && IPC_HIGH_RESULT.load(Ordering::Acquire) == ipc::IPC_OK as u32,
        );
        let _ = lwp::sem_destroy(high_gate);
        let _ = lwp::sem_destroy(low_gate);

        let caller = lwp::get_self();
        let deferred_before = lwp::debug_counters().deferred_callbacks;
        let queued = ipc::open_file_async(
            c"/dev/stm/immediate",
            ipc::IPC_OPEN_NONE,
            Some(async_ipc_callback),
            ptr::null_mut(),
        );
        logger.note_i32("async IOS submit rc", queued);
        logger.record("async IOS request queued", queued == ipc::IPC_OK);
        if queued == ipc::IPC_OK {
            let deadline = driver::now_ticks().wrapping_add(driver::TICKS_PER_SECOND * 5);
            while ASYNC_IPC_DONE.load(Ordering::Acquire) == 0
                && driver::now_ticks().wrapping_sub(deadline) >= (1u64 << 63)
            {
                let _ = ipc::poll();
                lwp::yield_thread();
            }
        }
        let async_result = ASYNC_IPC_RESULT.load(Ordering::Acquire) as i32;
        logger.note_i32("async IOS callback result", async_result);
        logger.record(
            "async IOS callback completed",
            ASYNC_IPC_DONE.load(Ordering::Acquire) != 0,
        );
        logger.record("async IOS open succeeded", async_result >= 0);
        let callback_thread = ASYNC_IPC_THREAD.load(Ordering::Acquire);
        logger.record(
            "async IOS callback ran on deferred worker",
            ASYNC_IPC_DONE.load(Ordering::Acquire) != 0
                && callback_thread != lwp::LWP_THREAD_NULL
                && callback_thread != caller,
        );
        logger.record(
            "async IOS descriptor closed",
            ASYNC_IPC_CLOSE_OK.load(Ordering::Acquire) != 0,
        );
        logger.record(
            "async IOS completion advanced deferred counter",
            lwp::debug_counters().deferred_callbacks > deferred_before,
        );
    }

    fn dump_debug(logger: &mut ProbeLogger) {
        logger.section("LWP DEBUG HOOKS");
        let counters = lwp::debug_counters();
        logger.note_u64("switches", counters.context_switches);
        logger.note_u64("created", counters.threads_created);
        logger.note_u64("exited", counters.threads_exited);
        logger.note_u64("waits", counters.blocking_waits);
        logger.note_u64("wakeups", counters.wakeups);
        logger.note_u64("timeouts", counters.timeouts);
        logger.note_u64("deferred", counters.deferred_callbacks);
        logger.note_u64("ipc wakeups", counters.ipc_wakeups);
        logger.note_u64("stack guard failures", counters.stack_guard_failures);
        logger.record("stack guards intact", counters.stack_guard_failures == 0);
        logger.note_u32("debug hook count", DEBUG_HOOK_COUNT.load(Ordering::Acquire));
        logger.note_u32("last debug event", DEBUG_LAST_EVENT.load(Ordering::Acquire));
        logger.record(
            "debug hook received scheduler events",
            DEBUG_HOOK_COUNT.load(Ordering::Acquire) != 0,
        );
        let mut events = [driver::DebugEvent::default(); 32];
        let count = lwp::debug_drain(&mut events);
        logger.note_u32("debug events drained", count as u32);
        for event in events.iter().take(count.min(8)) {
            logger.note_u32("event", event.event as u32);
            logger.note_u64("event tick", event.tick);
        }
    }

    fn read_cpu_state() -> (u32, u32) {
        let msr: u32;
        let sp: u32;
        unsafe {
            core::arch::asm!(
                "mfmsr {msr}", "mr {sp}, 1",
                msr = out(reg) msr, sp = out(reg) sp,
                options(nostack, preserves_flags)
            );
        }
        (msr, sp)
    }

    fn run_interrupt_frame_integrity(logger: &mut ProbeLogger) {
        logger.section("INTERRUPT FRAME INTEGRITY");
        let mut dsi_vector = [0u32; 64];
        for (index, word) in dsi_vector.iter_mut().enumerate() {
            *word = unsafe { ptr::read_volatile((0x8000_0300 + index * 4) as *const u32) };
        }
        let (before_msr, before_sp) = read_cpu_state();
        let initial_retraces = ogc_video::retrace_count();
        let deadline_start = driver::now_ticks();
        while ogc_video::retrace_count().wrapping_sub(initial_retraces) < 8
            && driver::now_ticks().wrapping_sub(deadline_start) < 2 * driver::TICKS_PER_SECOND
        {
            core::hint::spin_loop();
        }
        let (after_msr, after_sp) = read_cpu_state();
        let retraces = ogc_video::retrace_count().wrapping_sub(initial_retraces);
        let vector_preserved = dsi_vector.iter().enumerate().all(|(index, word)| {
            *word == unsafe { ptr::read_volatile((0x8000_0300 + index * 4) as *const u32) }
        });
        logger.record("eight real VI interrupts returned", retraces >= 8);
        logger.record(
            "interrupt return preserved MSR",
            before_msr & 0xb032 == after_msr & 0xb032,
        );
        logger.record(
            "address translation remains enabled",
            after_msr & 0x30 == 0x30,
        );
        logger.record(
            "interrupt return preserved stack pointer",
            before_sp == after_sp,
        );
        logger.record("interrupt scratch preserved DSI vector", vector_preserved);
    }

    pub fn run() -> i32 {
        // Install the scheduler hook before sys_pre_main performs its first
        // synchronous IOS request. Early block/wake events therefore remain
        // inspectable even if initialization stalls.
        lwp::set_debug_hook(Some(debug_hook));
        let mut logger = ogc_system_probe::init_named_probe_logger("lwp-probe");
        logger.section("MILESTONE 1 LWP PROBE");
        logger.note("persistent log: /tmp/ogc-system-probe.log (or mounted app log when enabled)");
        logger.note("debug hook is IRQ-safe only when it performs bounded atomic writes");
        logger.record(
            "system initialization kept floating point enabled",
            ogc_system::runtime_fp_disabled_stage() == 0,
        );

        run_interrupt_frame_integrity(&mut logger);
        run_switch_and_join(&mut logger);
        probe_priority_order(&mut logger);
        probe_reschedule_rotation(&mut logger);
        probe_scheduler_critical_sections(&mut logger);
        probe_reentrancy_storage(&mut logger);
        probe_external_thread_setup(&mut logger);
        probe_external_wait_paths(&mut logger);
        run_synchronization(&mut logger);
        run_legacy_sync_abi(&mut logger);
        run_legacy_thread_abi(&mut logger);
        run_condition_broadcast(&mut logger);
        run_timed_wait(&mut logger);
        run_message_and_deferred(&mut logger);
        probe_interrupt_deferred(&mut logger);
        run_tuxedo_compatibility(&mut logger);
        run_ipc_wakeup(&mut logger);
        run_std_foundation(&mut logger);
        crate::futex_probe::run(&mut logger);
        #[cfg(feature = "wii-std-sync")]
        crate::std_sync_probe::run(&mut logger);
        dump_debug(&mut logger);
        logger.summary();

        loop {
            probe_support::service_power_shutdown();
            probe_support::wait_vsync();
        }
    }
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    wii::run()
}

#[cfg(all(target_arch = "powerpc", not(feature = "wii-std-sync")))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    ogc_system_probe::probe_support::panic_screen()
}
