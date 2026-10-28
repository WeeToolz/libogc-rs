//! Development-only process wrapper. Enabled by `xtask build-dol --dev`.
use super::{cache, lwp};
use crate::dev_protocol::{self as protocol, Identity, LOG_CAPACITY, LogBuffer};
use core::{
    cell::UnsafeCell,
    ffi::{CStr, c_void},
    fmt::{self, Write},
    sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering},
};
use ogc_network::{POLLERR, POLLHUP, POLLNVAL, POLLOUT, sys as net};
#[path = "dev_stub.rs"]
mod stub_evidence;

const RUNNING: i32 = i32::MIN;
const DEADLINE: i32 = i32::MIN + 1;
const STUB_BYTES: usize = 0x1800;
struct Shared<T>(UnsafeCell<T>);
// Access is protected by LOG_LOCK, or published once before thread creation.
unsafe impl<T: Send> Sync for Shared<T> {}
static LOG: Shared<LogBuffer> = Shared(UnsafeCell::new(LogBuffer::new()));
static LOG_LOCK: AtomicBool = AtomicBool::new(false);
static LOST_CONTENTION: AtomicUsize = AtomicUsize::new(0);
static IDENTITY: Shared<Option<Identity>> = Shared(UnsafeCell::new(None));
static STUB: Shared<[u8; STUB_BYTES]> = Shared(UnsafeCell::new([0; STUB_BYTES]));
static STUB_SAVED: AtomicBool = AtomicBool::new(false);
static STATUS: AtomicI32 = AtomicI32::new(RUNNING);
static FINISHING: AtomicBool = AtomicBool::new(false);
static REPORT_DONE: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);
#[unsafe(no_mangle)]
pub static OGC_DEV_STATE: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_DEV_REPORT_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
/// Last completed entry stage. This is separate from OGC_DEV_STATE, whose
/// existing values describe recovery arming and return, not application entry.
/// 0=not entered, 1=entered, 2=system init returned, 3=arguments accepted,
/// 4=watchdog created, 5=recovery armed, 6=reporter created/application entry.
#[unsafe(no_mangle)]
pub static OGC_DEV_ENTRY_STAGE: AtomicU32 = AtomicU32::new(0);
/// Original entry error, or zero only after all entry work completed.
#[unsafe(no_mangle)]
pub static OGC_DEV_ENTRY_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
/// Last reset event, its arguments, and retained DSP register observations.
/// A debugger can read these even after interrupts and reporting stop.
#[unsafe(no_mangle)]
pub static OGC_DEV_RESET_SEQUENCE: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_DEV_RESET_STAGE: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_DEV_RESET_PRIORITY: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_DEV_RESET_RESULT: AtomicI32 = AtomicI32::new(0);
#[unsafe(no_mangle)]
pub static OGC_DEV_RESET_DSP_CONTROL: AtomicU32 = AtomicU32::new(u32::MAX);
#[unsafe(no_mangle)]
pub static OGC_DEV_RESET_DSP_MAILBOX: AtomicU32 = AtomicU32::new(u32::MAX);
/// Zero means all 6 KiB in uncached memory match the saved caller stub at
/// the final handoff. -2 means no saved stub; -84 means a byte differs.
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_COMPARE_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_ENTRY_WORD: AtomicU32 = AtomicU32::new(0);
/// Raw words at 0x80002f08 and 0x80002f0c. Older HBC stubs store their
/// configured title here; these values do not validate a title or stub format.
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_CONFIG_HIGH: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_CONFIG_LOW: AtomicU32 = AtomicU32::new(0);
/// SHA-1 of all 6 KiB saved from the caller, before any observer patch.
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_SHA1: [AtomicU32; 5] = [const { AtomicU32::new(0) }; 5];
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_HASH_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
/// Raw address-load words and a decoded physical destination. -95 means
/// the sequence is unknown; a zero destination is not valid evidence then.
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_REAL_WORDS: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_REAL_TARGET: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_DEV_STUB_REAL_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

fn save_stub_identity(image: &[u8; STUB_BYTES]) -> Result<(), i32> {
    let mut digest = [0; 20];
    let initialized = ogc_sha::init();
    let hashed = if initialized == 0 {
        ogc_sha::calculate(image, &mut digest)
    } else {
        initialized
    };
    if hashed == 0 {
        for (output, bytes) in OGC_DEV_STUB_SHA1.iter().zip(digest.as_chunks::<4>().0) {
            output.store(u32::from_be_bytes(*bytes), Ordering::Relaxed);
        }
    }
    OGC_DEV_STUB_HASH_RESULT.store(hashed, Ordering::Release);
    for (output, bytes) in OGC_DEV_STUB_REAL_WORDS
        .iter()
        .zip(image[0x24..0x2c].as_chunks::<4>().0)
    {
        output.store(u32::from_be_bytes(*bytes), Ordering::Relaxed);
    }
    let decoded = stub_evidence::real_destination(image);
    if let Ok(target) = decoded {
        OGC_DEV_STUB_REAL_TARGET.store(target, Ordering::Relaxed);
    }
    OGC_DEV_STUB_REAL_RESULT.store(decoded.err().unwrap_or(0), Ordering::Release);
    if hashed != 0 {
        return Err(hashed);
    }
    Ok(())
}

pub(super) fn record_reset(stage: u32, priority: u32, result: i32) {
    // Single-core critical section: publish one complete event without a lock,
    // allocation, scheduler operation, or network request. Preserve the MSR.
    let interrupt_state = super::irq::irq_disable();
    if stage == super::SYS_RESET_TRACE_RELOAD_STUB {
        // Read the physical backing memory through the uncached MEM1 alias.
        // A cached read could hide a missing writeback before instruction fetch.
        let stub = 0xc0001800 as *const u8;
        let mut result = -2;
        if STUB_SAVED.load(Ordering::Acquire) {
            result = 0;
            for index in 0..STUB_BYTES {
                // SAFETY: both complete stub regions have process lifetime.
                let matches =
                    unsafe { core::ptr::read_volatile(stub.add(index)) == (*STUB.0.get())[index] };
                if !matches {
                    result = -84;
                    break;
                }
            }
        }
        OGC_DEV_STUB_COMPARE_RESULT.store(result, Ordering::Relaxed);
        // SAFETY: aligned words in the same documented low-memory region.
        unsafe {
            OGC_DEV_STUB_ENTRY_WORD.store(core::ptr::read_volatile(stub.cast()), Ordering::Relaxed);
            OGC_DEV_STUB_CONFIG_HIGH.store(
                core::ptr::read_volatile(0xc0002f08 as *const u32),
                Ordering::Relaxed,
            );
            OGC_DEV_STUB_CONFIG_LOW.store(
                core::ptr::read_volatile(0xc0002f0c as *const u32),
                Ordering::Relaxed,
            );
        }
    }
    OGC_DEV_RESET_STAGE.store(stage, Ordering::Relaxed);
    OGC_DEV_RESET_PRIORITY.store(priority, Ordering::Relaxed);
    OGC_DEV_RESET_RESULT.store(result, Ordering::Relaxed);
    if stage == super::SYS_RESET_TRACE_DSP {
        OGC_DEV_RESET_DSP_CONTROL.store(result as u32, Ordering::Relaxed);
    } else if stage == super::SYS_RESET_TRACE_DSP_MAILBOX {
        OGC_DEV_RESET_DSP_MAILBOX.store(result as u32, Ordering::Relaxed);
    }
    OGC_DEV_RESET_SEQUENCE.fetch_add(1, Ordering::Release);
    super::irq::irq_restore(interrupt_state);
}
#[repr(align(32))]
struct Stack([u8; 32 * 1024]);
static mut WATCHDOG_STACK: Stack = Stack([0; 32 * 1024]);
static mut REPORT_STACK: Stack = Stack([0; 32 * 1024]);

pub fn capture(args: fmt::Arguments<'_>) {
    if LOG_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        LOST_CONTENTION.fetch_add(1, Ordering::Relaxed);
        return;
    }
    // SAFETY: this call owns LOG_LOCK until formatting is complete.
    let result = unsafe { (&mut *LOG.0.get()).write_fmt(args) };
    if result.is_err() {
        LOST_CONTENTION.fetch_add(1, Ordering::Relaxed);
    }
    LOG_LOCK.store(false, Ordering::Release);
}
fn snapshot(out: &mut [u8; LOG_CAPACITY]) -> Option<(usize, usize)> {
    if LOG_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return None;
    }
    // SAFETY: this call owns LOG_LOCK; the buffer is copied before unlock.
    let log = unsafe { &*LOG.0.get() };
    out[..log.len].copy_from_slice(&log.bytes[..log.len]);
    let result = (log.len, log.lost);
    LOG_LOCK.store(false, Ordering::Release);
    Some(result)
}
fn now_ms() -> u64 {
    lwp::now_ticks() / (lwp::TICKS_PER_SECOND / 1000)
}
fn pause() {
    lwp::sleep_ticks(lwp::TICKS_PER_SECOND / 50);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __ogc_dev_begin(argc: i32, argv: *mut *mut u8) -> i32 {
    if STARTED.swap(true, Ordering::AcqRel) {
        return -16;
    }
    OGC_DEV_ENTRY_STAGE.store(1, Ordering::Release);
    // SAFETY: the runtime supplies the sheltered loader vector to this entry.
    let result = unsafe { begin(argc, argv) };
    OGC_DEV_ENTRY_RESULT.store(result, Ordering::Release);
    if result != 0 {
        capture(format_args!(
            "FAIL dev entry stage={} error={result}\n",
            OGC_DEV_ENTRY_STAGE.load(Ordering::Acquire)
        ));
    }
    result
}

unsafe fn begin(argc: i32, argv: *mut *mut u8) -> i32 {
    if ogc_sys::hbc_reload_stub_present() {
        // SAFETY: the loader owns the documented 6 KiB low-memory stub area.
        unsafe {
            core::ptr::copy_nonoverlapping(
                0x80001800 as *const u8,
                STUB.0.get().cast::<u8>(),
                STUB_BYTES,
            );
        }
        STUB_SAVED.store(true, Ordering::Release);
        // SAFETY: published once here, before workers can read the saved image.
        if let Err(error) = save_stub_identity(unsafe { &*STUB.0.get() }) {
            return error;
        }
    }
    // The normal system initializer is idempotent. Arm after its scheduler and
    // IPC setup, before entering application code. No Rust heap is required.
    unsafe {
        super::sys_init();
    }
    OGC_DEV_ENTRY_STAGE.store(2, Ordering::Release);
    capture(format_args!("INFO dev system init returned\n"));
    if !(1..=64).contains(&argc) || argv.is_null() {
        return -22;
    }
    let mut args = [""; 64];
    for (index, argument) in args[..argc as usize].iter_mut().enumerate() {
        // SAFETY: argv is the runtime's sheltered loader vector and has argc entries.
        let pointer = unsafe { *argv.add(index) };
        if pointer.is_null() {
            return -22;
        }
        let bytes = unsafe { CStr::from_ptr(pointer.cast()) }.to_bytes();
        let Ok(text) = core::str::from_utf8(bytes) else {
            return -22;
        };
        *argument = text;
    }
    let identity = match protocol::parse(&args[..argc as usize]) {
        Ok(identity) => identity,
        Err(reason) => {
            capture(format_args!("FAIL dev arguments: {reason}\n"));
            return -22;
        }
    };
    // SAFETY: identity is published before either worker is created.
    unsafe {
        *IDENTITY.0.get() = Some(identity);
    }
    OGC_DEV_ENTRY_STAGE.store(3, Ordering::Release);
    let mut watchdog = 0;
    let result = unsafe {
        lwp::create_thread(
            &mut watchdog,
            Some(watchdog_worker),
            core::ptr::null_mut(),
            core::ptr::addr_of_mut!(WATCHDOG_STACK.0).cast(),
            32 * 1024,
            127,
        )
    };
    if result != 0 || watchdog == 0 {
        return if result != 0 { result } else { -1 };
    }
    OGC_DEV_ENTRY_STAGE.store(4, Ordering::Release);
    let arm_started = now_ms();
    while OGC_DEV_STATE.load(Ordering::Acquire) != 1 {
        if now_ms().wrapping_sub(arm_started) >= 1000 {
            return -110;
        }
        pause();
    }
    OGC_DEV_ENTRY_STAGE.store(5, Ordering::Release);
    capture(format_args!(
        "INFO dev_recovery=armed deadline_ms={} loader_stub={}\n",
        identity.deadline_ms,
        STUB_SAVED.load(Ordering::Acquire)
    ));
    let mut reporter = 0;
    let result = unsafe {
        lwp::create_thread(
            &mut reporter,
            Some(report_worker),
            core::ptr::null_mut(),
            core::ptr::addr_of_mut!(REPORT_STACK.0).cast(),
            32 * 1024,
            126,
        )
    };
    if result != 0 || reporter == 0 {
        let error = if result != 0 { result } else { -1 };
        OGC_DEV_REPORT_RESULT.store(error, Ordering::Release);
        REPORT_DONE.store(true, Ordering::Release);
        return error;
    }
    OGC_DEV_ENTRY_STAGE.store(6, Ordering::Release);
    capture(format_args!("INFO dev application entry ready\n"));
    0
}

/// Complete one process result, give reporting a bounded window, and restore
/// the launching channel's stub before the existing runtime exit route uses it.
pub fn finish(status: i32) {
    if !STARTED.load(Ordering::Acquire) {
        return;
    }
    if FINISHING.swap(true, Ordering::AcqRel) {
        // A deadline can interrupt normal finish while reporting is stalled.
        // Its exit still needs the original caller stub, even when the first
        // finisher cannot make further progress.
        restore_loader_stub();
        return;
    }
    if let Err(previous) =
        STATUS.compare_exchange(RUNNING, status, Ordering::AcqRel, Ordering::Acquire)
    {
        capture(format_args!(
            "INFO dev retained earlier result={previous}\n"
        ));
    }
    capture(format_args!("INFO dev main exit status={status}\n"));
    let started = now_ms();
    while !REPORT_DONE.load(Ordering::Acquire) && now_ms().wrapping_sub(started) < 2000 {
        pause();
    }
    restore_loader_stub();
}

fn restore_loader_stub() {
    // This single-core critical section prevents two exit workers from writing
    // the stub concurrently. It contains no sleep, allocation, or IOS request.
    let interrupt_state = super::irq::irq_disable();
    if STUB_SAVED.load(Ordering::Acquire) {
        // SAFETY: the saved loader bytes have process lifetime; exit owns this
        // low-memory area and restores the complete original image.
        unsafe {
            core::ptr::copy_nonoverlapping(
                STUB.0.get().cast::<u8>(),
                0x80001800 as *mut u8,
                STUB_BYTES,
            );
            cache::dc_flush_range((0x80001800 as *const u8).cast(), STUB_BYTES as u32);
            cache::ic_invalidate_range((0x80001800 as *const u8).cast(), STUB_BYTES as u32);
            // ic_invalidate_range is the asynchronous icbi range primitive.
            // Complete invalidation and discard prefetched instructions before
            // advertising the restored executable image as ready for handoff.
            cache::sync();
            cache::ic_sync();
        }
    }
    OGC_DEV_STATE.store(3, Ordering::Release);
    super::irq::irq_restore(interrupt_state);
}
#[unsafe(no_mangle)]
pub extern "C" fn __ogc_dev_end(status: i32) {
    finish(status);
}

unsafe extern "C" fn watchdog_worker(_: *mut c_void) -> *mut c_void {
    let identity = unsafe { (*IDENTITY.0.get()).expect("identity published before workers") };
    let started = now_ms();
    OGC_DEV_STATE.store(1, Ordering::Release);
    while now_ms().wrapping_sub(started) < u64::from(identity.deadline_ms) {
        pause();
    }
    OGC_DEV_STATE.store(2, Ordering::Release);
    if STATUS
        .compare_exchange(RUNNING, DEADLINE, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        capture(format_args!("FAIL dev application deadline expired\n"));
    }
    super::sys_exit_to_loader(-124)
}

fn checked(result: i32) -> Result<(), i32> {
    if result < 0 { Err(result) } else { Ok(()) }
}
fn connect(identity: &Identity) -> Result<i32, i32> {
    checked(net::net_attach_active())?;
    let socket = net::net_socket(net::AF_INET, net::SOCK_STREAM, net::IPPROTO_TCP);
    if socket < 0 {
        return Err(socket);
    }
    let result = (|| {
        checked(net::net_fcntl(socket, net::F_SETFL, net::O_NONBLOCK))?;
        let mut address = net::SockaddrIn {
            sin_len: 16,
            sin_family: net::AF_INET as u8,
            sin_port: identity.port.to_be(),
            sin_addr: net::InAddr {
                s_addr: u32::from_be_bytes(identity.host).to_be(),
            },
            sin_zero: [0; 8],
        };
        let code =
            unsafe { net::net_connect(socket, (&mut address as *mut net::SockaddrIn).cast(), 16) };
        if code < 0 {
            let started = now_ms();
            loop {
                let mut descriptor = net::PollSd {
                    socket,
                    events: POLLOUT as u32,
                    revents: 0,
                };
                let count = unsafe { net::net_poll(&mut descriptor, 1, 0) };
                checked(count)?;
                if count > 0 {
                    let mut error = 0i32;
                    let mut length = 4;
                    checked(unsafe {
                        net::net_getsockopt(
                            socket,
                            net::SOL_SOCKET,
                            net::SO_ERROR,
                            (&mut error as *mut i32).cast(),
                            &mut length,
                        )
                    })?;
                    if length != 4 {
                        return Err(-22);
                    }
                    if error != 0 {
                        return Err(-error.abs());
                    }
                    if descriptor.revents & POLLOUT as u32 == 0 {
                        return Err(code);
                    }
                    break;
                }
                if now_ms().wrapping_sub(started) >= 3000 {
                    return Err(code);
                }
                pause();
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        let close = net::net_close(socket);
        if close < 0 {
            capture(format_args!("FAIL dev socket cleanup error={close}\n"));
        }
        return Err(error);
    }
    Ok(socket)
}
fn write_all(socket: i32, mut bytes: &[u8], started: u64) -> Result<(), i32> {
    while !bytes.is_empty() {
        let mut descriptor = net::PollSd {
            socket,
            events: POLLOUT as u32,
            revents: 0,
        };
        checked(unsafe { net::net_poll(&mut descriptor, 1, 0) })?;
        if descriptor.revents & (POLLERR | POLLHUP | POLLNVAL) as u32 != 0 {
            return Err(-32);
        }
        if descriptor.revents & POLLOUT as u32 != 0 {
            let count = unsafe {
                net::net_send(
                    socket,
                    bytes.as_ptr().cast(),
                    bytes.len() as i32,
                    net::MSG_DONTWAIT,
                )
            };
            if count < 0 {
                return Err(count);
            }
            if count == 0 {
                return Err(-32);
            }
            if count as usize > bytes.len() {
                return Err(-22);
            }
            bytes = &bytes[count as usize..];
        } else {
            pause();
        }
        if now_ms().wrapping_sub(started) >= 3000 {
            return Err(-110);
        }
    }
    Ok(())
}
unsafe extern "C" fn report_worker(_: *mut c_void) -> *mut c_void {
    let identity = unsafe { (*IDENTITY.0.get()).expect("identity published before workers") };
    let result = run_reporter(&identity);
    OGC_DEV_REPORT_RESULT.store(result.err().unwrap_or(0), Ordering::Release);
    REPORT_DONE.store(true, Ordering::Release);
    core::ptr::null_mut()
}
fn run_reporter(identity: &Identity) -> Result<(), i32> {
    let socket = connect(identity)?;
    let result = (|| {
        let mut bytes = [0u8; LOG_CAPACITY];
        let mut cursor = 0;
        let mut sequence = 1;
        let mut heartbeat = now_ms();
        loop {
            if let Some((length, lost)) = snapshot(&mut bytes) {
                let status = STATUS.load(Ordering::Acquire);
                let started = now_ms();
                if status != RUNNING {
                    protocol::write_final_report(
                        identity,
                        &bytes[..length],
                        lost,
                        LOST_CONTENTION.load(Ordering::Relaxed),
                        status,
                        |part| write_all(socket, part, started),
                    )?;
                    return Ok(());
                }
                if length > cursor || now_ms().wrapping_sub(heartbeat) >= 1000 {
                    let delta = if length > cursor {
                        &bytes[cursor..length]
                    } else {
                        b"INFO dev report worker alive; application progress not measured\n"
                            .as_slice()
                    };
                    let header = protocol::progress_header(identity, sequence, delta.len());
                    write_all(socket, &header, started)?;
                    write_all(socket, &identity.run[..identity.run_len], started)?;
                    write_all(socket, &identity.build, started)?;
                    write_all(socket, delta, started)?;
                    cursor = length;
                    sequence = sequence.wrapping_add(1);
                    heartbeat = now_ms();
                }
            }
            pause();
        }
    })();
    let close = net::net_close(socket);
    if close < 0 {
        capture(format_args!("FAIL dev socket cleanup error={close}\n"));
    }
    result.and(checked(close))
}
