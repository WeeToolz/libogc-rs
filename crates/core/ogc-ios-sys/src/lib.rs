#![no_std]
#![cfg(not(gamecube))]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(target_arch = "powerpc")]
extern crate alloc;

#[cfg(target_arch = "powerpc")]
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
mod ios_logic;
use ios_logic::{
    MAX_STORED_TMD_SIZE, MAX_TICKET_VIEWS, initialization_result, preferred_version_from_title,
    stored_tmd_size_is_supported, ticket_view_count_is_supported, update_error,
};

#[cfg(all(test, target_arch = "powerpc"))]
extern crate std;

#[cfg(target_arch = "powerpc")]
use ogc_es_sys::{self as es, Tikview};
#[cfg(target_arch = "powerpc")]
use ogc_irq as irq;
#[cfg(target_arch = "powerpc")]
use ogc_lwp_driver::{TICKS_PER_SECOND, now_ticks};

pub use ogc_sys::Bool;

pub const IOS_EINVAL: i32 = -0x3004;
pub const IOS_EBADVERSION: i32 = -0x3100;
pub const IOS_ETOOMANYVIEWS: i32 = -0x3101;
pub const IOS_EMISMATCH: i32 = -0x3102;

const DEFAULT_VERSION: i32 = 58;
const DEFAULT_REVISION: i32 = 0x0100;
#[cfg(target_arch = "powerpc")]
const IOS_POLL_INTERVAL_TICKS: u64 = TICKS_PER_SECOND.div_ceil(1_000);
#[cfg(target_arch = "powerpc")]
const MAX_IPC_RETRIES: usize = 400;
#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct AlignedTmdBuffer([u8; MAX_STORED_TMD_SIZE as usize]);

static IOS_SUBSYSTEMS_INITIALIZED: AtomicBool = AtomicBool::new(false);
static IOS_CURRENT_VERSION: AtomicI32 = AtomicI32::new(DEFAULT_VERSION);
static IOS_CURRENT_REVISION: AtomicI32 = AtomicI32::new(DEFAULT_REVISION);
static IOS_PREFERRED_VERSION: AtomicI32 = AtomicI32::new(DEFAULT_VERSION);
static IOS_ES_INIT_RESULT: AtomicI32 = AtomicI32::new(0);
static IOS_STM_INIT_RESULT: AtomicI32 = AtomicI32::new(0);
static IOS_ES_CLOSE_RESULT: AtomicI32 = AtomicI32::new(0);
static IOS_STM_CLOSE_RESULT: AtomicI32 = AtomicI32::new(0);

#[cfg(target_arch = "powerpc")]
const IOS_VERSION_WORD_ADDR: usize = 0x8000_3140;

#[cfg(target_arch = "powerpc")]
#[inline(always)]
fn ios_read_version_word() -> u32 {
    unsafe {
        core::arch::asm!(
            "dcbi 0, {addr}",
            addr = in(reg) IOS_VERSION_WORD_ADDR,
            options(nostack)
        );
        core::ptr::read_volatile(IOS_VERSION_WORD_ADDR as *const u32)
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
fn ios_read_version_word() -> u32 {
    0
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
fn ipc_control_ready() -> bool {
    unsafe {
        let control =
            core::ptr::read_volatile((ogc_ipc::HW_IPC_PPCBASE as usize as *const u32).add(1));
        (control & ogc_ipc::HW_IPC_PPC_CTRL_REGS) == ogc_ipc::HW_IPC_PPC_CTRL_REGS
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
fn poll_interval_elapsed(start: u64, now: u64) -> bool {
    now.wrapping_sub(start) >= IOS_POLL_INTERVAL_TICKS
}

fn restore_irq_before_reinitialize_and_query(
    restore_irq: impl FnOnce(),
    reinitialize_ipc: impl FnOnce(),
    query_version: impl FnOnce() -> i32,
) -> i32 {
    restore_irq();
    reinitialize_ipc();
    query_version()
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
fn wait_one_millisecond() {
    let start = now_ticks();
    while !poll_interval_elapsed(start, now_ticks()) {
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "powerpc")]
fn launch_ios_from_es(version: i32) -> i32 {
    if !(3..=0xff).contains(&version) {
        return IOS_EBADVERSION;
    }

    let title_id = 0x0000_0001_0000_0000u64 | version as u64;
    let mut num_views = 0u32;
    let rc = unsafe { es::get_num_ticket_views(title_id, &mut num_views) };
    if rc < 0 {
        return rc;
    }
    // BUG FIX: C accepts zero ticket views and passes the aligned empty array to ES.
    if !ticket_view_count_is_supported(num_views) {
        return IOS_ETOOMANYVIEWS;
    }

    let mut views = [Tikview::default(); MAX_TICKET_VIEWS];
    let rc = unsafe { es::get_ticket_views(title_id, views.as_mut_ptr(), num_views) };
    if rc < 0 {
        return rc;
    }

    unsafe {
        core::ptr::write_volatile(IOS_VERSION_WORD_ADDR as *mut u32, 0);
    }

    let rc = unsafe { es::launch_title_background(title_id, views.as_ptr()) };
    if rc < 0 {
        return rc;
    }

    es::reset();
    // BUG FIX: C masks and frees the ACR handler only after the ES reset.
    irq::mask(irq::IM_PI_ACR);
    let irq_handler = irq::free(irq::IRQ_PI_ACR);

    while (ios_read_version_word() >> 16) == 0 {
        wait_one_millisecond();
    }

    for _ in 0..=MAX_IPC_RETRIES {
        wait_one_millisecond();
        if ipc_control_ready() {
            break;
        }
    }

    // C restores/unmasks ACR before IPC reinitialization and the version query.
    let new_version = restore_irq_before_reinitialize_and_query(
        || {
            let _ = unsafe { irq::request(irq::IRQ_PI_ACR, irq_handler, core::ptr::null_mut()) };
            irq::unmask(irq::IM_PI_ACR);
        },
        ogc_ipc::reinitialize,
        get_version,
    );
    if new_version != version {
        return IOS_EMISMATCH;
    }

    IOS_CURRENT_VERSION.store(new_version, Ordering::Release);
    IOS_CURRENT_REVISION.store(get_revision(), Ordering::Release);
    version
}

#[cfg(target_arch = "powerpc")]
fn stm_init_powerpc() -> i32 {
    ogc_stm::init()
}

#[cfg(target_arch = "powerpc")]
fn stm_close_powerpc() -> i32 {
    ogc_stm::close()
}

#[cfg(target_arch = "powerpc")]
fn preferred_version_powerpc() -> i32 {
    let mut count = 0u32;
    let rc = unsafe { es::get_num_titles(&mut count) };
    if rc < 0 {
        return rc;
    }

    let mut titles = Vec::<u64>::new();
    if titles.try_reserve_exact(count as usize).is_err() {
        return -1;
    }
    titles.resize(count as usize, 0);

    let rc = unsafe { es::get_titles(titles.as_mut_ptr(), count) };
    if rc < 0 {
        return rc;
    }

    let mut tmd_buffer = AlignedTmdBuffer([0; 4096]);
    let mut preferred = IOS_EBADVERSION;
    for title in titles {
        // BUG FIX: C checks the complete title word and supports IOS versions 28 through 61.
        let Some(version) = preferred_version_from_title(title) else {
            continue;
        };

        let mut tmd_size = 0u32;
        let rc = unsafe { es::get_stored_tmd_size(title, &mut tmd_size) };
        if rc < 0 || !stored_tmd_size_is_supported(tmd_size) {
            continue;
        }

        let rc = unsafe {
            es::get_stored_tmd(
                title,
                tmd_buffer.0.as_mut_ptr().cast::<es::SignedBlob>(),
                tmd_size,
            )
        };
        if rc < 0 {
            continue;
        }

        let words = unsafe {
            core::slice::from_raw_parts(tmd_buffer.0.as_ptr().cast::<u32>(), tmd_buffer.0.len() / 4)
        };
        if words.get(1).copied().unwrap_or(0) == 0 && words.get(2).copied().unwrap_or(0) == 0 {
            continue;
        }

        if (version > preferred && preferred != 58) || version == 58 {
            preferred = version;
        }
    }

    preferred
}

#[cfg(all(test, target_arch = "powerpc"))]
fn set_subsystem_results(es_init: i32, stm_init: i32, stm_close: i32, es_close: i32) {
    IOS_ES_INIT_RESULT.store(es_init, Ordering::Release);
    IOS_STM_INIT_RESULT.store(stm_init, Ordering::Release);
    IOS_STM_CLOSE_RESULT.store(stm_close, Ordering::Release);
    IOS_ES_CLOSE_RESULT.store(es_close, Ordering::Release);
}

#[cfg(all(test, target_arch = "powerpc"))]
fn reset_ios_state() {
    IOS_SUBSYSTEMS_INITIALIZED.store(false, Ordering::Release);
    IOS_CURRENT_VERSION.store(DEFAULT_VERSION, Ordering::Release);
    IOS_CURRENT_REVISION.store(DEFAULT_REVISION, Ordering::Release);
    IOS_PREFERRED_VERSION.store(DEFAULT_VERSION, Ordering::Release);
    IOS_ES_INIT_RESULT.store(0, Ordering::Release);
    IOS_STM_INIT_RESULT.store(0, Ordering::Release);
    IOS_ES_CLOSE_RESULT.store(0, Ordering::Release);
    IOS_STM_CLOSE_RESULT.store(0, Ordering::Release);
}

pub fn _ios_initialize_subsystems() -> i32 {
    if cfg!(target_arch = "powerpc") {
        let es_init = es::init();
        // BUG FIX: C always attempts STM and does not roll ES back on STM failure.
        let stm_init = stm_init_powerpc();
        let ret = initialization_result(es_init, stm_init);
        IOS_SUBSYSTEMS_INITIALIZED.store(ret >= 0, Ordering::Release);
        return ret;
    }

    let es_init = IOS_ES_INIT_RESULT.load(Ordering::Acquire);
    let stm_init = IOS_STM_INIT_RESULT.load(Ordering::Acquire);
    let ret = initialization_result(es_init, stm_init);
    IOS_SUBSYSTEMS_INITIALIZED.store(ret >= 0, Ordering::Release);
    ret
}

pub fn _ios_shutdown_subsystems() -> i32 {
    if cfg!(target_arch = "powerpc") {
        IOS_SUBSYSTEMS_INITIALIZED.store(false, Ordering::Release);
        let mut ret = 0;
        let stm = stm_close_powerpc();
        if stm < 0 {
            ret = stm;
        }
        let es = es::close();
        if es < 0 {
            ret = es;
        }
        return ret;
    }

    IOS_SUBSYSTEMS_INITIALIZED.store(false, Ordering::Release);
    let mut ret = 0;
    let stm = IOS_STM_CLOSE_RESULT.load(Ordering::Acquire);
    if stm < 0 {
        ret = stm;
    }
    let es = IOS_ES_CLOSE_RESULT.load(Ordering::Acquire);
    if es < 0 {
        ret = es;
    }
    ret
}

pub fn _ios_load_startup_ios() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return 0;
    }

    0
}

pub fn _ios_launch_new_ios(version: i32) -> i32 {
    #[cfg(target_arch = "powerpc")]
    {
        launch_ios_from_es(version)
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        if !(3..=0xff).contains(&version) {
            return IOS_EBADVERSION;
        }

        IOS_CURRENT_VERSION.store(version, Ordering::Release);
        IOS_CURRENT_REVISION.store(DEFAULT_REVISION, Ordering::Release);
        version
    }
}

pub fn get_preferred_version() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return preferred_version_powerpc();
    }

    IOS_PREFERRED_VERSION.load(Ordering::Acquire)
}

pub fn reload_ios(version: i32) -> i32 {
    #[cfg(target_arch = "powerpc")]
    {
        // BUG FIX: C lets each later failure replace an earlier reload error.
        let mut ret = 0;
        let shutdown = _ios_shutdown_subsystems();
        ret = update_error(ret, shutdown);

        let es_init = es::init();
        ret = update_error(ret, es_init);
        if es_init >= 0 {
            let launch = _ios_launch_new_ios(version);
            if launch < 0 {
                ret = update_error(ret, launch);
                // C closes ES here but preserves the launch result if close fails.
                let _ = es::close();
            }
        }

        let init = _ios_initialize_subsystems();
        ret = update_error(ret, init);
        ret
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let mut ret = 0;

        let shutdown = _ios_shutdown_subsystems();
        ret = update_error(ret, shutdown);

        let es_init = IOS_ES_INIT_RESULT.load(Ordering::Acquire);
        ret = update_error(ret, es_init);
        if es_init >= 0 {
            let launch = _ios_launch_new_ios(version);
            if launch < 0 {
                ret = update_error(ret, launch);
            }
        }

        let init = _ios_initialize_subsystems();
        ret = update_error(ret, init);

        ret
    }
}

pub fn get_version() -> i32 {
    if cfg!(target_arch = "powerpc") {
        let version = (ios_read_version_word() >> 16) as i32;
        if version == 0 || version > 0xff {
            return IOS_EBADVERSION;
        }
        return version;
    }

    IOS_CURRENT_VERSION.load(Ordering::Acquire)
}

pub fn get_revision() -> i32 {
    if cfg!(target_arch = "powerpc") {
        let vercode = ios_read_version_word();
        let revision = (vercode & 0xffff) as i32;
        if vercode == 0 || revision == 0 {
            return IOS_EBADVERSION;
        }
        return revision;
    }

    IOS_CURRENT_REVISION.load(Ordering::Acquire)
}

pub fn get_revision_major() -> i32 {
    let rev = get_revision();
    if rev < 0 {
        return rev;
    }
    (rev >> 8) & 0xff
}

pub fn get_revision_minor() -> i32 {
    let rev = get_revision();
    if rev < 0 {
        return rev;
    }
    rev & 0xff
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn launch_restores_irq_before_reinitialize_and_version_query() {
        let step = core::cell::Cell::new(0);
        let version = restore_irq_before_reinitialize_and_query(
            || assert_eq!(step.replace(1), 0),
            || assert_eq!(step.replace(2), 1),
            || {
                assert_eq!(step.replace(3), 2);
                58
            },
        );
        assert_eq!(version, 58);
        assert_eq!(step.get(), 3);
    }

    #[test]
    fn launch_poll_interval_requires_one_elapsed_millisecond() {
        assert_eq!(IOS_POLL_INTERVAL_TICKS, 60_750);
        assert!(!poll_interval_elapsed(
            100,
            100 + IOS_POLL_INTERVAL_TICKS - 1
        ));
        assert!(poll_interval_elapsed(100, 100 + IOS_POLL_INTERVAL_TICKS));
        assert!(poll_interval_elapsed(
            u64::MAX - 10,
            (u64::MAX - 10).wrapping_add(IOS_POLL_INTERVAL_TICKS),
        ));
    }

    #[test]
    fn subsystem_lifecycle_and_version_queries_work() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        reset_ios_state();
        set_subsystem_results(0, 0, 0, 0);
        assert_eq!(_ios_initialize_subsystems(), 0);
        assert_eq!(get_preferred_version(), DEFAULT_VERSION);
        assert_eq!(get_version(), DEFAULT_VERSION);
        assert_eq!(get_revision(), DEFAULT_REVISION);
        assert_eq!(get_revision_major(), 1);
        assert_eq!(get_revision_minor(), 0);
        assert_eq!(_ios_load_startup_ios(), 0);
        assert_eq!(_ios_launch_new_ios(59), 59);
        assert_eq!(get_version(), 59);
        assert_eq!(reload_ios(60), 0);
        assert_eq!(get_version(), 60);
        assert_eq!(_ios_launch_new_ios(2), IOS_EBADVERSION);
        assert_eq!(_ios_shutdown_subsystems(), 0);
    }

    #[test]
    fn subsystem_helpers_propagate_mocked_errors() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        reset_ios_state();
        set_subsystem_results(-11, -12, -13, -14);
        assert_eq!(_ios_initialize_subsystems(), -11);
        assert!(!IOS_SUBSYSTEMS_INITIALIZED.load(Ordering::Acquire));
        assert_eq!(_ios_shutdown_subsystems(), -14);
        assert!(!IOS_SUBSYSTEMS_INITIALIZED.load(Ordering::Acquire));
        set_subsystem_results(-12, 0, 0, 0);
        assert_eq!(reload_ios(61), -12);
        assert!(!IOS_SUBSYSTEMS_INITIALIZED.load(Ordering::Acquire));
        set_subsystem_results(0, 0, 0, 0);
    }
}
