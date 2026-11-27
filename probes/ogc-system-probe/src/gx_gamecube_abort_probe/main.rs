#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
mod target {
    use core::ffi::c_void;
    use core::ptr::null_mut;
    use core::sync::atomic::{AtomicU32, Ordering};
    use ogc_gx as gx;
    use ogc_system_probe::probe_support::{ProbeDisplay, ProbeLogger, ProbeLoggerOptions};

    #[repr(align(32))]
    struct Fifo([u8; 65536]);
    static mut FIFO: Fifo = Fifo([0; 65536]);
    static BREAK_COUNT: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn breakpoint() {
        BREAK_COUNT.fetch_add(1, Ordering::Release);
    }

    fn cp_pair(low: usize) -> u32 {
        let cp = 0xcc00_0000usize as *const u16;
        unsafe {
            u32::from(core::ptr::read_volatile(cp.add(low)))
                | (u32::from(core::ptr::read_volatile(cp.add(low + 1))) << 16)
        }
    }

    fn run(logger: &mut ProbeLogger) {
        logger.section("GAMECUBE GX ABORT");
        logger.record("GameCube target configuration", cfg!(gamecube));
        let base = unsafe { core::ptr::addr_of_mut!(FIFO.0).cast::<c_void>() };
        let initialized = !unsafe { gx::init(base, 65536) }.is_null();
        logger.record("GameCube GX caller-owned FIFO initialization", initialized);
        if !initialized {
            return;
        }
        let drained = unsafe { gx::finish_frame_and_wait() };
        logger.record("GameCube initial PE completion", drained);
        if !drained {
            return;
        }
        let mut fifo = gx::GxFifoObj::default();
        let start = unsafe { base.cast::<u8>().add(256).cast::<c_void>() };
        let physical = start as u32 & 0x1fff_ffe0;
        unsafe {
            gx::set_gp_fifo(null_mut());
            gx::init_fifo_base(&mut fifo, base, 65536);
            gx::init_fifo_ptrs(&mut fifo, start, start);
            gx::set_cpu_fifo(&mut fifo);
            gx::set_gp_fifo(&mut fifo);
        }
        let old_callback = gx::set_break_pt_callback(Some(breakpoint));
        BREAK_COUNT.store(0, Ordering::Release);
        gx::enable_break_pt(start);
        for _ in 0..3 {
            unsafe { gx::flush() };
        }
        let started = ogc_lwp::sys::now_ticks();
        while BREAK_COUNT.load(Ordering::Acquire) == 0
            && ogc_lwp::sys::now_ticks().wrapping_sub(started) < 40_500_000
        {
            core::hint::spin_loop();
        }
        let pending = cp_pair(24);
        let stopped = BREAK_COUNT.load(Ordering::Acquire) == 1;
        // The GameCube C branch does not clear dirty state or flush the Wii
        // revision commands. The next explicit flush must emit this VAT.
        gx::set_vtx_attr_fmt(7, u32::from(gx::GX_VA_POS), gx::GX_POS_XYZ, gx::GX_F32, 0);
        gx::abort_frame();
        let pi = 0xcc00_3000usize as *const u32;
        let control = unsafe { core::ptr::read_volatile(0xcc00_0002usize as *const u16) };
        let write_after_abort = unsafe { core::ptr::read_volatile(pi.add(5)) } & 0x1fff_ffe0;
        let clean = cp_pair(24) == 0 && cp_pair(26) == physical && cp_pair(28) == physical;
        let callback_cleared = gx::set_break_pt_callback(None).is_none();
        unsafe { gx::flush() };
        let bytes = ogc_core::mem_k0_to_k1(start as usize) as *const u8;
        let vat = unsafe {
            core::ptr::read_volatile(bytes) == 0x08
                && core::ptr::read_volatile(bytes.add(1)) == 0x77
                && core::ptr::read_volatile(bytes.add(6)) == 0x08
                && core::ptr::read_volatile(bytes.add(7)) == 0x87
                && core::ptr::read_volatile(bytes.add(12)) == 0x08
                && core::ptr::read_volatile(bytes.add(13)) == 0x97
        };
        let recovered = unsafe { gx::finish_frame_and_wait() };
        gx::set_break_pt_callback(old_callback);
        logger.note_u32("GameCube pending FIFO bytes", pending);
        logger.note_hex32("GameCube CP control after abort", u32::from(control));
        logger.record(
            "GameCube abort begins with live pending FIFO commands",
            stopped && pending >= 96,
        );
        logger.record(
            "GameCube abort clears FIFO and restores link and interrupt mask",
            clean && control & 0x3f == 0x15 && callback_cleared,
        );
        logger.record(
            "GameCube abort emits no Wii revision commands",
            write_after_abort == physical,
        );
        logger.record("GameCube abort retains pending VAT for the next flush", vat);
        logger.record("GameCube accepts PE completion after abort", recovered);
    }

    pub fn execute() -> ! {
        let startup = ogc_system::init_checked();
        let mut logger =
            ProbeLogger::with_options(ProbeDisplay::init(), ProbeLoggerOptions::disabled());
        logger.record("GameCube system initialization", startup.is_ok());
        if startup.is_ok() {
            run(&mut logger);
        }
        logger.summary();
        loop {
            ogc_system_probe::probe_support::wait_vsync();
        }
    }
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// # Safety
/// The caller supplies the live loader argument vector.
pub unsafe extern "C" fn main(argc: i32, argv: *mut *mut u8) -> i32 {
    unsafe { ogc_system_probe::probe_support::set_agent_arguments(argc, argv) };
    target::execute()
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
