use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};

use ogc_aram as aram;
use ogc_cache as cache;
use ogc_card as card;
use ogc_di as di;
use ogc_dsp as dsp;
use ogc_dvd as dvd;
use ogc_exi as exi;
use ogc_irq as irq;
use ogc_lwp as lwp;
use ogc_si as si;
use ogc_usb as usb;
use ogc_usbgecko as usbgecko;

use crate::probe_support::ProbeLogger;

mod dsp_tasks;

#[repr(align(32))]
struct Align32<const N: usize>([u8; N]);

static mut ARAM_BLOCKS: [u32; 32] = [0; 32];
static DSP_CALLBACKS: AtomicU32 = AtomicU32::new(0);
static DSP_SHUTDOWN_STAGES: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" fn dsp_callback() {
    DSP_CALLBACKS.fetch_add(1, Ordering::AcqRel);
}

fn dsp_shutdown_stage(stage: u32, _control: u16, _mail: u16) {
    DSP_SHUTDOWN_STAGES.fetch_or(1 << stage, Ordering::AcqRel);
}

unsafe fn dsp_control() -> u16 {
    unsafe { core::ptr::read_volatile((0xcc00_5000 as *const u16).add(5)) }
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("BUS PROBE");

    probe_cache(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_cache_controls(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_cache_instruction_apis(logger);
    probe_aram(logger);
    probe_dsp(logger);
    probe_exi_si(logger);
    probe_card_metadata_bits(logger);
    probe_card_helpers(logger);
    probe_disc(logger);
    probe_usb(logger);
    probe_runtime_helpers(logger);
}

fn probe_card_metadata_bits(logger: &mut ProbeLogger) {
    let mut stat = card::CardStat {
        banner_fmt: 0xa4,
        icon_fmt: 0xaaaa,
        icon_speed: 0x5555,
        ..Default::default()
    };
    // SAFETY: `stat` is initialized and exclusively accessible. Null is
    // permitted by these metadata helpers and is tested explicitly.
    unsafe {
        card::sys::set_banner_fmt(&mut stat, 0xfe);
        let banner = stat.banner_fmt == 0xa6
            && card::sys::get_banner_fmt(&stat) == 2
            && card::sys::get_banner_fmt(core::ptr::null()) == 0;
        card::sys::set_banner_fmt(core::ptr::null_mut(), 1);
        logger.record("card banner masks and null", banner);

        card::sys::set_icon_fmt(&mut stat, 0, 1);
        card::sys::set_icon_fmt(&mut stat, 7, 0xffff);
        card::sys::set_icon_fmt(&mut stat, 8, 0);
        card::sys::set_icon_fmt(core::ptr::null_mut(), 0, 0);
        logger.record(
            "card icon formats preserve adjacent fields",
            stat.icon_fmt == 0xeaa9
                && card::sys::get_icon_fmt(&stat, 0) == 1
                && card::sys::get_icon_fmt(&stat, 7) == 3
                && card::sys::get_icon_fmt(&stat, 8) == 0
                && card::sys::get_icon_fmt(core::ptr::null(), 0) == 0,
        );

        card::sys::set_icon_speed(&mut stat, 0, 3);
        card::sys::set_icon_speed(&mut stat, 7, 0xfffe);
        card::sys::set_icon_speed(&mut stat, 8, 0);
        card::sys::set_icon_speed(core::ptr::null_mut(), 0, 0);
        logger.record(
            "card icon speeds preserve adjacent fields",
            stat.icon_speed == 0x9557
                && card::sys::get_icon_speed(&stat, 0) == 3
                && card::sys::get_icon_speed(&stat, 7) == 2
                && card::sys::get_icon_speed(&stat, 8) == 0
                && card::sys::get_icon_speed(core::ptr::null(), 0) == 0,
        );

        card::sys::set_icon_addr(&mut stat, 0x1234_5678);
        card::sys::set_comment_addr(&mut stat, 0xabcdef01);
        card::sys::set_icon_addr(core::ptr::null_mut(), 0);
        card::sys::set_comment_addr(core::ptr::null_mut(), 0);
        logger.record(
            "card metadata addresses remain distinct",
            stat.icon_addr == 0x1234_5678 && stat.comment_addr == 0xabcdef01,
        );
    }
}

fn probe_cache(logger: &mut ProbeLogger) {
    logger.section("CACHE PROBE");
    let mut data = Align32([0x5au8; 128]);
    let ptr = data.0.as_mut_ptr().cast::<c_void>();

    // SAFETY: `ptr` addresses the live, 32-byte-aligned `data` allocation and
    // every requested range stays within it.
    unsafe {
        cache::dc_flush_range(ptr, 32);
        cache::dc_invalidate_range(ptr, 32);
    }
    logger.record(
        "cache invalidate preserves flushed data",
        data.0[..32].iter().all(|byte| *byte == 0x5a),
    );

    // SAFETY: Both no-sync operations use aligned ranges inside `data`.
    unsafe {
        cache::dc_flush_range_no_sync(ptr, 64);
        cache::dc_store_range_no_sync(ptr.cast::<u8>().add(64).cast(), 64);
    }
    logger.record(
        "cache no-sync operations preserve data",
        data.0.iter().all(|byte| *byte == 0x5a),
    );

    // SAFETY: The pointers reference the live aligned allocation. A zero byte
    // count returns before either pointer is read or written.
    let load_zero = unsafe { cache::lc_load_data(ptr, ptr, 0) };
    // SAFETY: The pointers reference the live aligned allocation. A zero byte
    // count returns before either pointer is read or written.
    let store_zero = unsafe { cache::lc_store_data(ptr, ptr, 0) };
    logger.record("locked cache zero-length load returns zero", load_zero == 0);
    logger.record(
        "locked cache zero-length store returns zero",
        store_zero == 0,
    );

    // SAFETY: `ptr` is aligned and valid. A zero tag count requests no cache
    // lines, so the operation must not access the allocation.
    unsafe { cache::lc_alloc_tags(false, ptr, 0) };
    logger.record(
        "locked cache zero-tag allocation preserves data",
        data.0.iter().all(|byte| *byte == 0x5a),
    );

    let waited = cache::lc_queue_wait(15);
    logger.record(
        "locked cache wait accepts maximum queue depth",
        waited == 16 && cache::lc_queue_length() <= 15,
    );

    // SAFETY: `ptr` addresses the live, 32-byte-aligned `data` allocation and
    // every requested range stays within it.
    unsafe {
        cache::dc_flush_range(ptr, data.0.len() as u32);
        cache::dc_store_range(ptr, data.0.len() as u32);
        cache::dc_touch_range(ptr, data.0.len() as u32);
        cache::ic_invalidate_range(ptr, data.0.len() as u32);
        cache::dc_zero_range(ptr, 32);
    }
    cache::sync();

    logger.record(
        "cache range operations",
        data.0[..32].iter().all(|byte| *byte == 0),
    );
    logger.note_u32("locked cache queue", cache::lc_queue_length());
    logger.note_hex32("locked cache base", cache::lc_get_base() as usize as u32);
}

#[cfg(feature = "dolphin-automation")]
fn probe_cache_controls(logger: &mut ProbeLogger) {
    const ICE: u32 = 0x8000;
    const DCE: u32 = 0x4000;
    const ILOCK: u32 = 0x2000;
    const DLOCK: u32 = 0x1000;

    fn read_hid0() -> u32 {
        let value: u32;
        // SAFETY: The Wii probe runs in supervisor mode.
        unsafe {
            core::arch::asm!("mfspr {value}, 1008", value = out(reg) value, options(nostack));
        }
        value
    }

    fn write_hid0(value: u32) {
        // SAFETY: The caller only changes cache control bits or restores HID0.
        unsafe {
            core::arch::asm!("sync", "mtspr 1008, {value}", "isync", value = in(reg) value, options(nostack));
        }
    }

    fn read_hid2() -> u32 {
        let value: u32;
        // SAFETY: The Wii probe runs in supervisor mode.
        unsafe {
            core::arch::asm!("mfspr {value}, 920", value = out(reg) value, options(nostack));
        }
        value
    }

    fn write_hid2(value: u32) {
        // SAFETY: Restore the HID2 value captured by this probe.
        unsafe {
            core::arch::asm!("sync", "mtspr 920, {value}", "isync", value = in(reg) value, options(nostack));
        }
    }

    logger.section("CACHE CONTROLS");
    let irq_state = irq::irq_disable();
    let original = read_hid0();
    let baseline = (original | ICE | DCE) & !(ILOCK | DLOCK);
    write_hid0(baseline);
    let initial = read_hid0();

    cache::dc_disable();
    let dc_disabled = read_hid0();
    cache::dc_enable();
    let dc_enabled = read_hid0();
    cache::dc_freeze();
    let dc_frozen = read_hid0();
    cache::dc_unfreeze();
    let dc_unfrozen = read_hid0();
    cache::ic_disable();
    let ic_disabled = read_hid0();
    cache::ic_enable();
    let ic_enabled = read_hid0();
    cache::ic_freeze();
    let ic_frozen = read_hid0();
    cache::ic_unfreeze();
    let ic_unfrozen = read_hid0();

    const LOCKED_CACHE_ENABLE: u32 = 0x1000_0000;
    let original_hid2 = read_hid2();
    cache::lc_disable();
    let lc_disabled = read_hid2();
    cache::lc_enable();
    let lc_enabled = read_hid2();
    let queue_before_flush = cache::lc_queue_length();
    cache::lc_flush_queue();
    let queue_after_flush = cache::lc_queue_length();
    write_hid2(original_hid2);
    let hid2_restored = read_hid2() == original_hid2;

    write_hid0(original);
    let restored = read_hid0();
    irq::irq_restore(irq_state);

    logger.record(
        "HID0 cache baseline and restoration",
        initial == baseline && restored == original,
    );
    logger.record("dc_disable clears only DCE", dc_disabled == baseline & !DCE);
    logger.record(
        "dc_enable sets only DCE",
        dc_disabled & DCE == 0 && dc_enabled == baseline,
    );
    logger.record("dc_freeze sets only DLOCK", dc_frozen == baseline | DLOCK);
    logger.record(
        "dc_unfreeze clears only DLOCK",
        dc_frozen & DLOCK != 0 && dc_unfrozen == baseline,
    );
    logger.record("ic_disable clears only ICE", ic_disabled == baseline & !ICE);
    logger.record(
        "ic_enable sets only ICE",
        ic_disabled & ICE == 0 && ic_enabled == baseline,
    );
    logger.record("ic_freeze sets only ILOCK", ic_frozen == baseline | ILOCK);
    logger.record(
        "ic_unfreeze clears only ILOCK",
        ic_frozen & ILOCK != 0 && ic_unfrozen == baseline,
    );
    logger.record(
        "lc_disable clears only the locked-cache enable bit",
        lc_disabled == original_hid2 & !LOCKED_CACHE_ENABLE,
    );
    logger.record(
        "lc_enable restores locked-cache enable after disable",
        lc_disabled & LOCKED_CACHE_ENABLE == 0
            && lc_enabled & LOCKED_CACHE_ENABLE != 0
            && hid2_restored,
    );
    logger.record(
        "lc_flush_queue leaves an initially empty queue empty",
        queue_before_flush == 0 && queue_after_flush == 0,
    );
}

#[cfg(feature = "dolphin-automation")]
#[inline(never)]
fn cache_instruction_probe_value() -> u32 {
    0x51c0_ffee
}

#[cfg(feature = "dolphin-automation")]
fn probe_cache_instruction_apis(logger: &mut ProbeLogger) {
    fn read_hid0() -> u32 {
        let value: u32;
        // SAFETY: The Wii probe runs in supervisor mode.
        unsafe {
            core::arch::asm!("mfspr {value}, 1008", value = out(reg) value, options(nostack));
        }
        value
    }

    fn write_hid0(value: u32) {
        // SAFETY: Restore the HID0 value captured by this probe.
        unsafe {
            core::arch::asm!("sync", "mtspr 1008, {value}", "isync", value = in(reg) value, options(nostack));
        }
    }

    logger.section("CACHE INSTRUCTION APIs");
    let irq_state = irq::irq_disable();
    let original = read_hid0();
    let baseline = (original | 0x8000 | 0x4000) & !0x0c00;
    write_hid0(baseline);
    ogc_cache::dc_flash_invalidate();
    let dc_after = read_hid0();
    ogc_cache::ic_flash_invalidate();
    let ic_after = read_hid0();
    write_hid0(original);
    let restored = read_hid0() == original;
    irq::irq_restore(irq_state);

    logger.note_hex32("HID0 after DC flash invalidate request", dc_after);
    logger.note_hex32("HID0 after IC flash invalidate request", ic_after);
    logger.note("Dolphin flash-invalidate readback does not prove cache timing on hardware");
    logger.record(
        "dc_flash_invalidate preserves other HID0 bits and restores state",
        dc_after & !0x0400 == baseline & !0x0400 && restored,
    );
    logger.record(
        "ic_flash_invalidate preserves other HID0 bits and restores state",
        ic_after & !0x0800 == dc_after & !0x0800 && restored,
    );

    let expected = cache_instruction_probe_value();
    let code_line = (cache_instruction_probe_value as *const () as usize) & !31;
    // SAFETY: `code_line` is the aligned cache line for live executable code.
    unsafe {
        ogc_cache::ic_block_invalidate(code_line as *const c_void);
    }
    let after_invalidate = cache_instruction_probe_value();
    logger.record(
        "ic_block_invalidate preserves executable code behavior",
        expected == 0x51c0_ffee && after_invalidate == expected,
    );
    ogc_cache::ic_sync();
    logger.record(
        "ic_sync completes after instruction cache invalidation",
        cache_instruction_probe_value() == expected,
    );
}

fn probe_aram(logger: &mut ProbeLogger) {
    logger.section("ARAM PROBE");
    let blocks = core::ptr::addr_of_mut!(ARAM_BLOCKS).cast::<u32>();
    let base = aram::init(blocks, 32);
    logger.note_hex32("aram base", base);
    logger.record("aram initialized", aram::check_init() != 0);
    logger.note_u32("aram size", aram::get_size());
    logger.note_u32("aram internal size", aram::get_internal_size());
    logger.note_u32("aram dma busy", aram::get_dma_status());

    let alloc = aram::alloc(32);
    let mut freed_len = 0u32;
    let free_top = aram::free(Some(&mut freed_len));
    logger.note_hex32("aram alloc", alloc);
    logger.note_hex32("aram free top", free_top);
    logger.record("aram alloc/free length", freed_len == 32);
}

fn probe_dsp(logger: &mut ProbeLogger) {
    logger.section("DSP PROBE");
    dsp::init();
    ogc_dsp_sys::init();
    let previous = dsp::register_callback(Some(dsp_callback));
    // DSP_Init installs libogc's default task callback. Registration must
    // return that callback so the caller can restore it.
    logger.record("dsp callback replaced default", previous.is_some());
    let direct_previous = ogc_dsp_sys::register_callback(Some(dsp_callback));
    let direct_replace = direct_previous.is_some();
    let direct_restore = ogc_dsp_sys::register_callback(previous);
    let wrapper_restore = dsp::register_callback(previous);
    logger.record(
        "DSP direct and wrapper callback registration restore the prior handler",
        direct_replace && direct_restore.is_some() && wrapper_restore.is_some(),
    );

    let word = ogc_dsp_sys::VolatileU32::new(0x1234_5678);
    let initial_word = word.read();
    word.write(0x89ab_cdef);
    let final_word = word.read();
    logger.record(
        "DSP volatile task fields read and write caller memory",
        initial_word == 0x1234_5678 && final_word == 0x89ab_cdef,
    );

    dsp::halt();
    let wrapper_halted = unsafe { dsp_control() } & 0x0004 != 0;
    ogc_dsp_sys::halt();
    let direct_halted = unsafe { dsp_control() } & 0x0004 != 0;
    logger.record(
        "DSP direct and wrapper halt set the hardware halt bit",
        wrapper_halted && direct_halted,
    );
    dsp::assert_int();
    let wrapper_interrupt = unsafe { dsp_control() } & 0x0002 != 0;
    ogc_dsp_sys::assert_int();
    let direct_interrupt = unsafe { dsp_control() } & 0x0002 != 0;
    logger.record(
        "DSP direct and wrapper assert_int set the processor interrupt bit",
        wrapper_interrupt && direct_interrupt,
    );
    dsp::send_mail_to(0x1234_5678);
    let mail_ready = dsp::check_mail_to();
    let mail_roundtrip = dsp::read_cpu_to_dsp();
    ogc_dsp_sys::send_mail_to(0x89ab_cdef);
    let direct_mail_ready = ogc_dsp_sys::check_mail_to();
    let direct_mail = ogc_dsp_sys::read_cp_uto_dsp();
    logger.note(&alloc::format!(
        "DSP cpu mailbox ready={mail_ready} wrapper=0x{mail_roundtrip:08x} direct=0x{direct_mail:08x}"
    ));
    logger.record(
        "DSP direct and wrapper CPU mailbox APIs return the written message",
        mail_ready == 0
            && mail_roundtrip == 0x1234_5678
            && direct_mail_ready == 0
            && direct_mail == 0x09ab_cdef,
    );

    dsp::reset();
    ogc_dsp_sys::reset();
    dsp::unhalt();
    let wrapper_unhalted = unsafe { dsp_control() } & 0x0004 == 0;
    ogc_dsp_sys::unhalt();
    let direct_unhalted = unsafe { dsp_control() } & 0x0004 == 0;
    logger.record(
        "DSP direct and wrapper unhalt clear the hardware halt bit",
        wrapper_unhalted && direct_unhalted,
    );
    let mut dsp_mail_ready = false;
    for _ in 0..120 {
        if dsp::check_mail_from() != 0 {
            dsp_mail_ready = true;
            break;
        }
        logger.display().wait_vsync();
    }
    let incoming = dsp::read_mail_from();
    let incoming_direct = ogc_dsp_sys::read_mail_from();
    let incoming_pending = ogc_dsp_sys::check_mail_from();
    logger.note(&alloc::format!(
        "DSP incoming mailbox ready={dsp_mail_ready} wrapper=0x{incoming:08x} direct=0x{incoming_direct:08x} pending={incoming_pending}"
    ));
    logger.record(
        "DSP mailbox status and reads acknowledge incoming DSP mail",
        dsp_mail_ready
            && incoming == 0x8071_feed
            && incoming_direct == 0x0071_feed
            && incoming_pending == 0,
    );

    let dma_status = dsp::get_dma_status();
    let direct_dma_status = ogc_dsp_sys::get_dma_status();
    logger.record(
        "DSP direct and wrapper DMA status report idle",
        dma_status == 0 && direct_dma_status == 0,
    );

    dsp_tasks::probe_dsp_tasks(logger);

    DSP_SHUTDOWN_STAGES.store(0, Ordering::Release);
    ogc_dsp_sys::shutdown_with_progress(Some(dsp_shutdown_stage));
    let stages = DSP_SHUTDOWN_STAGES.load(Ordering::Acquire);
    logger.record(
        "DSP shutdown completes all four observed stages",
        stages & 0b1_1110 == 0b1_1110,
    );
    ogc_dsp_sys::shutdown();
    dsp::shutdown();
    logger.record(
        "DSP direct and wrapper shutdown leave DMA and mailboxes idle",
        ogc_dsp_sys::get_dma_status() == 0
            && dsp::get_dma_status() == 0
            && dsp::check_mail_to() == 0,
    );
}

fn probe_exi_si(logger: &mut ProbeLogger) {
    logger.section("EXI SI PROBE");
    exi::init();
    si::init();

    for chan in 0..3 {
        logger.note_i32_index("exi probe ch", chan, exi::probe(chan));
        logger.note_i32_index("exi probe ex ch", chan, exi::probe_ex(chan));
        logger.note_i32_index("exi state ch", chan, exi::get_state(chan));
    }

    for chan in 0..4 {
        logger.note_hex32_index("si probe ch", chan, si::probe(chan));
        logger.note_hex32_index("si type ch", chan, si::get_type(chan));
        logger.note_hex32_index("si status ch", chan, si::get_status(chan));
    }
}

fn probe_card_helpers(logger: &mut ProbeLogger) {
    logger.section("CARD HELPER PROBE");
    let mut stat = card::CardStat::default();
    // SAFETY: `stat` is initialized and uniquely borrowed for each helper call.
    unsafe {
        card::set_banner_fmt(&mut stat, card::CARD_BANNER_CI);
        card::set_icon_fmt(&mut stat, 0, card::CARD_ICON_RGB as u16);
        card::set_icon_speed(&mut stat, 0, card::CARD_SPEED_FAST as u16);
        card::set_icon_addr(&mut stat, 0x2000);
        card::set_comment_addr(&mut stat, 0x40);
    }

    logger.record(
        "card banner helper",
        // SAFETY: `stat` is initialized and remains valid for this read.
        unsafe { card::get_banner_fmt(&stat) } == card::CARD_BANNER_CI,
    );
    logger.record(
        "card icon fmt helper",
        // SAFETY: `stat` is initialized and remains valid for this read.
        unsafe { card::get_icon_fmt(&stat, 0) } == card::CARD_ICON_RGB as u16,
    );
    logger.record(
        "card icon speed helper",
        // SAFETY: `stat` is initialized and remains valid for this read.
        unsafe { card::get_icon_speed(&stat, 0) } == card::CARD_SPEED_FAST as u16,
    );

    for slot in [card::CARD_SLOTA, card::CARD_SLOTB] {
        let mut mem_size = 0;
        let mut sector_size = 0;
        logger.note_i32_index("card probe slot", slot, card::probe(slot));
        logger.note_i32_index(
            "card probe ex slot",
            slot,
            // SAFETY: both output references remain valid for the call.
            unsafe { card::probe_ex(slot, &mut mem_size, &mut sector_size) },
        );
        logger.note_i32_index("card mem size slot", slot, mem_size);
        logger.note_i32_index("card sector size slot", slot, sector_size);
    }
}

fn probe_disc(logger: &mut ProbeLogger) {
    logger.section("DISC PROBE");
    dvd::init();
    logger.note_i32("dvd drive status", dvd::get_drive_status());
    logger.note_hex32(
        "dvd current disk id ptr",
        dvd::get_current_disk_id() as usize as u32,
    );
    logger.note_hex32("dvd drive info ptr", dvd::get_drive_info() as usize as u32);

    let di_init = di::init();
    logger.note_i32("di init rc", di_init);
    logger.note_i32("di status", di::get_status());
    let mut cover = 0u32;
    logger.note_i32("di cover rc", di::get_cover_register(&mut cover));
    logger.note_hex32("di cover", cover);
    let mut error = 0u32;
    logger.note_i32("di error rc", di::get_error(&mut error));
    logger.note_hex32("di error", error);
    di::close();
}

fn probe_usb(logger: &mut ProbeLogger) {
    logger.section("USB CORE PROBE");
    let init = usb::initialize();
    logger.note_i32("usb init rc", init);
    let mut entries = [usb::UsbDeviceEntry::default(); 16];
    match usb::device_list(&mut entries, 0) {
        Ok(count) => {
            logger.note_u32("usb device count", count as u32);
            for (idx, entry) in entries.iter().take(count).enumerate() {
                logger.note_i32_index("usb id idx", idx as i32, entry.device_id);
                logger.note_hex32_index("usb vid idx", idx as i32, entry.vid as u32);
                logger.note_hex32_index("usb pid idx", idx as i32, entry.pid as u32);
            }
        }
        Err(err) => logger.note_i32("usb list rc", err.raw()),
    }
    if logger.uses_mounted_log() {
        logger.note("usb deinit skipped: mounted log active");
    } else {
        logger.note_i32("usb deinit rc", usb::deinitialize());
    }

    for chan in 0..3 {
        logger.note_i32_index("usbgecko alive ch", chan, usbgecko::is_gecko_alive(chan));
    }
}

fn probe_runtime_helpers(logger: &mut ProbeLogger) {
    logger.section("IRQ LWP PROBE");
    logger.note_u32(
        "irq vi handler present",
        irq::get_handler(irq::IRQ_PI_VI).is_some() as u32,
    );
    let thread = lwp::current_thread();
    logger.note_u32("current thread raw", thread.raw());
    lwp::yield_now();
    logger.record("lwp yield returned", true);
}
