#![no_std]

//! Rust providers for the Wii standard library ABI.
//!
//! Link this crate in the application. The standard library calls these checked
//! entry points and shares the application's production scheduler and services.

#[cfg(not(target_arch = "powerpc"))]
compile_error!("ogc-std-runtime targets Wii PowerPC only");

#[cfg(all(
    target_arch = "powerpc",
    any(not(target_vendor = "nintendo"), not(target_os = "none"), gamecube)
))]
compile_error!("ogc-std-runtime requires the Wii PowerPC EABI target");

use core::ffi::c_void;
extern crate alloc;
mod errors;
use errors::{thread_error, tls_error};
mod stdio;
pub use stdio::{__wii_std_output, diagnostic_error};
mod fs;
pub use fs::{__wii_std_fs, FsRequest};
mod net;
pub use net::{__wii_std_net, NetRequest, network_initialize, network_snapshot};
use ogc_alloc::{ArenaAllocator, InitError};
use ogc_lwp_driver::TlsDestructor;

mod entry;
pub use entry::{EntryError, RuntimePhase, RuntimeSnapshot};

struct RuntimeState(core::cell::UnsafeCell<entry::EntryState>);
// SAFETY: All access is bounded and serialized with LWP interrupt masking.
unsafe impl Sync for RuntimeState {}
static RUNTIME: RuntimeState = RuntimeState(core::cell::UnsafeCell::new(entry::EntryState::new()));

fn with_entry<T>(operation: impl FnOnce(&mut entry::EntryState) -> T) -> T {
    let irq = ogc_lwp_driver::critical_enter();
    // SAFETY: Interrupt masking serializes this single-CPU state. Operations
    // passed here do not block, allocate, or call user code.
    let result = operation(unsafe { &mut *RUNTIME.0.get() });
    ogc_lwp_driver::critical_exit(irq);
    result
}

pub fn runtime_snapshot() -> RuntimeSnapshot {
    with_entry(|state| state.snapshot())
}

static SYSTEM: ArenaAllocator = ArenaAllocator::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupError {
    System(ogc_system::startup::InitError),
    Heap(InitError),
    UnsupportedHardware,
    ArgumentsUnavailable,
    Thread(ogc_lwp_driver::ThreadError),
    Entry(EntryError),
}

/// Initialize system services and reserve the System allocator's MEM2 heap.
/// The startup hook calls this before compiler-generated main can access TLS.
///
/// # Safety
/// Call once during startup before application allocation or concurrent arena
/// use. The loader must have supplied the checked-in Wii memory layout.
pub unsafe fn initialize() -> Result<(), StartupError> {
    ogc_system::init_checked().map_err(StartupError::System)?;
    if ogc_system::get_hollywood_revision() == 0 {
        return Err(StartupError::UnsupportedHardware);
    }
    let (argc, argv) = ogc_sys::startup_arguments().ok_or(StartupError::ArgumentsUnavailable)?;
    let thread = ogc_lwp_driver::current_thread_checked().map_err(StartupError::Thread)?;
    let mut prepared = entry::EntryState::new();
    prepared
        .prepare(thread, argc, argv as usize)
        .map_err(StartupError::Entry)?;
    if runtime_snapshot().phase != RuntimePhase::Cold {
        return Err(StartupError::Entry(EntryError::InvalidState));
    }
    let publish_arena2 = |new_low| {
        // SAFETY: initialize_wii publishes the end of its reserved region.
        unsafe { ogc_system::set_arena2_lo(new_low) };
    };
    // SAFETY: The caller ensures startup exclusivity. The system has initialized
    // arena2, and this allocator advances its low bound before application use.
    unsafe {
        ogc_alloc::initialize_wii(
            &SYSTEM,
            ogc_system::get_arena2_lo().cast(),
            ogc_system::get_arena2_hi().cast(),
            ogc_alloc::DEFAULT_MAX_MEM2_HEAP_BYTES,
            ogc_alloc::DEFAULT_MEM2_RESERVE_BYTES,
            publish_arena2,
        )
    }
    .map_err(StartupError::Heap)?;
    // SAFETY: Startup exclusivity prevents another caller from publishing state
    // between the validation and this final commit after heap initialization.
    with_entry(|state| *state = prepared);
    Ok(())
}

pub fn heap_region() -> Option<core::ops::Range<usize>> {
    SYSTEM.region()
}

pub fn heap_capacity() -> usize {
    SYSTEM.capacity()
}
pub fn heap_used_bytes() -> usize {
    SYSTEM.used_bytes()
}

/// # Safety
/// The same startup requirements as initialize apply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_initialize() -> i32 {
    // SAFETY: ogc-sys calls this once before main on the application stack.
    match unsafe { initialize() } {
        Ok(()) => 0,
        Err(StartupError::System(ogc_system::startup::InitError::NotCompleted)) => -1,
        Err(StartupError::System(ogc_system::startup::InitError::FailedStage(stage))) => {
            -(0x1000 + stage as i32)
        }
        Err(StartupError::Heap(InitError::AlreadyInitialized)) => -2,
        Err(StartupError::Heap(InitError::TooSmall)) => -3,
        Err(StartupError::Heap(InitError::AddressOverflow)) => -4,
        Err(StartupError::Heap(InitError::NotEnoughArena)) => -5,
        Err(StartupError::UnsupportedHardware) => -6,
        Err(StartupError::ArgumentsUnavailable) => -7,
        Err(StartupError::Thread(error)) => -(0x2000 + thread_error(error)),
        Err(StartupError::Entry(error)) => -(0x3000 + entry_error(error)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __wii_std_alloc(size: usize, alignment: usize) -> *mut u8 {
    SYSTEM.allocate(size, alignment)
}

#[unsafe(no_mangle)]
pub extern "C" fn __wii_std_alloc_zeroed(size: usize, alignment: usize) -> *mut u8 {
    SYSTEM.allocate_zeroed(size, alignment)
}

/// # Safety
/// pointer must be a live System allocation with the supplied size/alignment.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_dealloc(pointer: *mut u8, size: usize, alignment: usize) {
    // SAFETY: The ABI caller supplies the original allocation and layout.
    unsafe { SYSTEM.deallocate(pointer, size, alignment) };
}

/// # Safety
/// pointer must be a live System allocation with the supplied size/alignment.
/// new_size must be nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_realloc(
    pointer: *mut u8,
    size: usize,
    alignment: usize,
    new_size: usize,
) -> *mut u8 {
    // SAFETY: The ABI caller supplies the live allocation and its layout.
    unsafe { SYSTEM.reallocate(pointer, size, alignment, new_size) }
}

#[unsafe(no_mangle)]
pub extern "C" fn __wii_std_errno() -> i32 {
    ogc_sysbase::errno()
}

/// # Safety
/// key must be null or writable/aligned for a u32. The destructor must remain
/// valid and accept all values stored under this key through thread cleanup.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_tls_create(
    destructor: Option<unsafe extern "C" fn(*mut u8)>,
    key: *mut u32,
) -> i32 {
    if key.is_null() {
        return ogc_sysbase::EINVAL;
    }
    // SAFETY: The C ABI represents both pointer argument types identically.
    let destructor = unsafe {
        core::mem::transmute::<Option<unsafe extern "C" fn(*mut u8)>, Option<TlsDestructor>>(
            destructor,
        )
    };
    // SAFETY: The caller supplies the callback/value lifetime contract.
    match unsafe { ogc_lwp_driver::tls_create(destructor) } {
        Ok(created) => {
            // SAFETY: key is writable/aligned under the caller's contract.
            unsafe { key.write(created) };
            0
        }
        Err(error) => tls_error(error),
    }
}

/// # Safety
/// output must be null or writable/aligned for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_tls_get(key: u32, output: *mut *mut u8) -> i32 {
    if output.is_null() {
        return ogc_sysbase::EINVAL;
    }
    match ogc_lwp_driver::tls_get(key) {
        Ok(value) => {
            // SAFETY: output is writable/aligned under the caller's contract.
            unsafe { output.write(value.cast()) };
            0
        }
        Err(error) => tls_error(error),
    }
}

/// # Safety
/// value must meet the registered key destructor's lifetime and validity rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_tls_set(key: u32, value: *mut u8) -> i32 {
    // SAFETY: The caller maintains the stored value through replacement/cleanup.
    match unsafe { ogc_lwp_driver::tls_set(key, value.cast::<c_void>()) } {
        Ok(()) => 0,
        Err(error) => tls_error(error),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __wii_std_tls_destroy(key: u32) -> i32 {
    match ogc_lwp_driver::tls_destroy(key) {
        Ok(()) => 0,
        Err(error) => tls_error(error),
    }
}

/// Wait on a std atomic word: 0 means wake/value change, 1 means timeout;
/// negative results are errors. No failure is reported as a successful wait.
///
/// # Safety
/// word must be null or an aligned live AtomicU32 that remains at this address
/// for the complete wait. Access to its storage must use atomic operations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_futex_wait(
    word: *const core::sync::atomic::AtomicU32,
    expected: u32,
    seconds: u64,
    nanoseconds: u32,
    timed: bool,
) -> i32 {
    if word.is_null() || !word.is_aligned() || nanoseconds >= 1_000_000_000 {
        return -ogc_sysbase::EINVAL;
    }
    let timeout = timed.then(|| core::time::Duration::new(seconds, nanoseconds));
    // SAFETY: The ABI caller keeps the atomic word live and at this address.
    match ogc_lwp_driver::futex_wait_for(unsafe { &*word }, expected, timeout) {
        Ok(ogc_lwp_driver::FutexWait::ValueChanged | ogc_lwp_driver::FutexWait::Woken) => 0,
        Ok(ogc_lwp_driver::FutexWait::TimedOut) => 1,
        Err(ogc_lwp_driver::FutexError::ExceptionContext) => -ogc_sysbase::EACCES,
        Err(ogc_lwp_driver::FutexError::InterruptsDisabled) => -ogc_sysbase::EBUSY,
        Err(ogc_lwp_driver::FutexError::InvalidTimeout) => -ogc_sysbase::EINVAL,
        Err(ogc_lwp_driver::FutexError::Cancelled) => -ogc_sysbase::EIO,
    }
}

/// Return the real number of woken std waiters, or a negative validation error.
///
/// # Safety
/// word must be null or an aligned live AtomicU32. Waiters must keep its storage
/// live until their waits finish. all selects one waiter or every waiter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_futex_wake(
    word: *const core::sync::atomic::AtomicU32,
    all: bool,
) -> i32 {
    if word.is_null() || !word.is_aligned() {
        return -ogc_sysbase::EINVAL;
    }
    // SAFETY: The ABI caller maintains the word's lifetime and alignment.
    let count = ogc_lwp_driver::futex_wake(unsafe { &*word }, all);
    match i32::try_from(count) {
        Ok(count) => count,
        Err(_) => -ogc_sysbase::EOVERFLOW,
    }
}

fn entry_error(error: EntryError) -> i32 {
    match error {
        EntryError::InvalidState => ogc_sysbase::EBUSY,
        EntryError::WrongThread => ogc_sysbase::EACCES,
        EntryError::InvalidArguments => ogc_sysbase::EINVAL,
        EntryError::CleanupFailed(result) => result,
    }
}

/// Validate the compiler-generated runtime entry against the sheltered loader
/// vector and the initial LWP thread. No argv pointer is dereferenced here.
#[unsafe(no_mangle)]
pub extern "C" fn __wii_std_runtime_enter(argc: isize, argv: *const *const u8) -> i32 {
    let thread = match ogc_lwp_driver::current_thread_checked() {
        Ok(thread) => thread,
        Err(error) => return thread_error(error),
    };
    match with_entry(|state| state.enter(thread, argc, argv as usize)) {
        Ok(()) => 0,
        Err(error) => entry_error(error),
    }
}

/// Complete initial-thread TLS and runtime cleanup. Idempotent only after a
/// real successful completion; reentrant or failed cleanup returns an error.
///
/// # Safety
/// No user TLS borrow may remain and no subsequent user code may use this
/// thread's TLS. The compiler runtime calls this after main and stream cleanup.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_runtime_cleanup() -> i32 {
    let thread = match ogc_lwp_driver::current_thread_checked() {
        Ok(thread) => thread,
        Err(error) => return thread_error(error),
    };
    match with_entry(|state| state.begin_cleanup(thread)) {
        Ok(false) => return 0,
        Ok(true) => {}
        Err(error) => return entry_error(error),
    }
    let result = match unsafe { ogc_lwp_driver::cleanup_current_thread() } {
        Ok(()) => 0,
        Err(error) => thread_error(error),
    };
    match with_entry(|state| state.complete_cleanup(result)) {
        Ok(()) => 0,
        Err(error) => entry_error(error),
    }
}

/// Finish both normal Rust entry and C entry applications after main returns.
/// _start_rust stops before its exit callback if this operation fails.
///
/// # Safety
/// Main must have returned and all user TLS borrows must have ended. This is
/// the final runtime boundary, before the process exit callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_runtime_return(status: i32) -> i32 {
    let result = unsafe { __wii_std_runtime_cleanup() };
    if result != 0 {
        return result;
    }
    let thread = match ogc_lwp_driver::current_thread_checked() {
        Ok(thread) => thread,
        Err(error) => return thread_error(error),
    };
    match with_entry(|state| state.returned(thread, status)) {
        Ok(()) => 0,
        Err(error) => entry_error(error),
    }
}

/// # Safety
/// argc and argv must point to aligned writable output slots. Returned loader
/// storage is immutable and remains live through application return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_arguments(argc: *mut usize, argv: *mut *const *const u8) -> i32 {
    if argc.is_null() || !argc.is_aligned() || argv.is_null() || !argv.is_aligned() {
        return ogc_sysbase::EINVAL;
    }
    let snapshot = runtime_snapshot();
    if matches!(snapshot.phase, RuntimePhase::Cold | RuntimePhase::Failed) {
        return ogc_sysbase::EIO;
    }
    // SAFETY: Preparation stores the actual sheltered vector, and the caller
    // supplies writable output slots. This function does not read its strings.
    unsafe {
        argc.write(snapshot.argc as usize);
        argv.write(snapshot.argv as *const *const u8);
    }
    0
}

/// # Safety
/// entry must consume data exactly once, return null after its work completes,
/// and remain executable until completion. output must be aligned/writable.
/// On failure data remains caller-owned and output remains unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_thread_create(
    entry: ogc_lwp_driver::ThreadEntry,
    data: *mut c_void,
    stack: usize,
    output: *mut u32,
) -> i32 {
    if output.is_null() || !output.is_aligned() || data.is_null() {
        return ogc_sysbase::EINVAL;
    }
    // SAFETY: The caller supplies the callback/argument contract. The driver
    // validates context and stack size before publishing the new worker.
    match unsafe { ogc_lwp_driver::spawn_thread(entry, data, stack) } {
        Ok(thread) => {
            unsafe { output.write(thread) };
            0
        }
        Err(error) => thread_error(error),
    }
}

/// # Safety
/// thread must be an exclusively owned native handle from thread_create, or
/// an invalid sentinel. A join consumes that ownership, including on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_thread_join(thread: u32) -> i32 {
    match unsafe { ogc_lwp_driver::join_thread_checked(thread) } {
        Ok(result) if result.is_null() => 0,
        Ok(_unexpected_result) => ogc_sysbase::EIO,
        Err(error) => thread_error(error),
    }
}

/// # Safety
/// thread must be exclusively owned from thread_create, or an invalid sentinel.
/// Detach consumes ownership; cleanup completion remains observable in LWP.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_thread_detach(thread: u32) -> i32 {
    match unsafe { ogc_lwp_driver::detach_thread_checked(thread) } {
        Ok(()) => 0,
        Err(error) => thread_error(error),
    }
}

/// # Safety
/// output must be null or an aligned writable u32 slot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_thread_current(output: *mut u32) -> i32 {
    if output.is_null() || !output.is_aligned() {
        return ogc_sysbase::EINVAL;
    }
    match ogc_lwp_driver::current_thread_checked() {
        Ok(thread) => {
            unsafe { output.write(thread) };
            0
        }
        Err(error) => thread_error(error),
    }
}

/// # Safety
/// cleanup must meet the driver's runtime cleanup callback contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_thread_cleanup_register(
    cleanup: Option<ogc_lwp_driver::ThreadCleanup>,
) -> i32 {
    let Some(cleanup) = cleanup else {
        return ogc_sysbase::EINVAL;
    };
    // SAFETY: The caller keeps this non-unwinding code live through thread exit.
    match unsafe { ogc_lwp_driver::register_thread_cleanup(cleanup) } {
        Ok(()) => 0,
        Err(error) => thread_error(error),
    }
}

/// # Safety
/// bytes must reference length readable bytes. A null pointer is permitted for
/// an empty name. Invalid UTF-8 or embedded NUL leaves the old name unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_thread_set_name(bytes: *const u8, length: usize) -> i32 {
    if (bytes.is_null() && length != 0) || length > isize::MAX as usize {
        return ogc_sysbase::EINVAL;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        // SAFETY: The caller supplies a readable byte buffer of this extent.
        unsafe { core::slice::from_raw_parts(bytes, length) }
    };
    let name = match core::str::from_utf8(bytes) {
        Ok(name) => name,
        Err(_) => return ogc_sysbase::EINVAL,
    };
    match ogc_lwp_driver::set_current_thread_name(name) {
        Ok(()) => 0,
        Err(error) => thread_error(error),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __wii_std_thread_sleep(seconds: u64, nanoseconds: u32) -> i32 {
    if nanoseconds >= 1_000_000_000 {
        return ogc_sysbase::EINVAL;
    }
    match ogc_lwp_driver::sleep_for(core::time::Duration::new(seconds, nanoseconds)) {
        Ok(()) => 0,
        Err(error) => thread_error(error),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __wii_std_thread_yield() -> i32 {
    if let Err(error) = ogc_lwp_driver::current_thread_checked() {
        return thread_error(error);
    }
    ogc_lwp_driver::yield_thread();
    0
}

/// # Safety
/// output must be null or an aligned writable usize slot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_thread_parallelism(output: *mut usize) -> i32 {
    if output.is_null() || !output.is_aligned() {
        return ogc_sysbase::EINVAL;
    }
    if ogc_sys::std_startup_result() != Some(0) || ogc_system::get_hollywood_revision() == 0 {
        return ogc_sysbase::ENODEV;
    }
    // A supported Wii has one Broadway CPU with one hardware thread. There is
    // no affinity, quota, or hotplug service that can expose another CPU.
    unsafe { output.write(1) };
    0
}

fn clock_error(error: ogc_system::ClockError) -> i32 {
    use ogc_system::ClockError;
    match error {
        ClockError::NotInitialized | ClockError::UnsupportedHardware => ogc_sysbase::ENODEV,
        ClockError::UtcOffsetMissing => ogc_sysbase::ENOTSUP,
        ClockError::InvalidUtcOffset => ogc_sysbase::EINVAL,
        ClockError::ExceptionContext => ogc_sysbase::EACCES,
        ClockError::InterruptsDisabled | ClockError::Busy => ogc_sysbase::EBUSY,
        ClockError::Arithmetic => ogc_sysbase::EOVERFLOW,
        ClockError::TimebaseReversed
        | ClockError::RtcFailure { .. }
        | ClockError::RtcUnstable
        | ClockError::CounterBias(_) => ogc_sysbase::EIO,
    }
}

/// Read a real clock and publish normalized seconds/nanoseconds only on success.
/// Detailed hardware and configuration errors remain in the system snapshot.
///
/// # Safety
/// seconds and nanos must be null or aligned writable output slots. wall selects
/// UTC wall time; false selects elapsed monotonic time. Failed outputs are unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_clock_now(
    wall: bool,
    seconds: *mut u64,
    nanos: *mut u32,
) -> i32 {
    if seconds.is_null() || !seconds.is_aligned() || nanos.is_null() || !nanos.is_aligned() {
        return ogc_sysbase::EINVAL;
    }
    let seconds_address = seconds as usize;
    let nanos_address = nanos as usize;
    if (nanos_address >= seconds_address
        && nanos_address - seconds_address < core::mem::size_of::<u64>())
        || (seconds_address >= nanos_address
            && seconds_address - nanos_address < core::mem::size_of::<u32>())
    {
        return ogc_sysbase::EINVAL;
    }
    let value = if wall {
        ogc_system::wall_time()
    } else {
        ogc_system::monotonic_time()
    };
    match value {
        Ok(value) => {
            unsafe {
                seconds.write(value.as_secs());
                nanos.write(value.subsec_nanos());
            }
            0
        }
        Err(error) => clock_error(error),
    }
}
