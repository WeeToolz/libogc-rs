use core::ffi::c_void;

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

#[repr(align(32))]
struct Align32<const N: usize>([u8; N]);

static mut ARAM_BLOCKS: [u32; 32] = [0; 32];

pub fn run(logger: &mut ProbeLogger) {
    logger.section("BUS PROBE");

    probe_cache(logger);
    probe_aram(logger);
    probe_dsp(logger);
    probe_exi_si(logger);
    probe_card_helpers(logger);
    probe_disc(logger);
    probe_usb(logger);
    probe_runtime_helpers(logger);
}

fn probe_cache(logger: &mut ProbeLogger) {
    logger.section("CACHE PROBE");
    let mut data = Align32([0x5au8; 128]);
    let ptr = data.0.as_mut_ptr().cast::<c_void>();

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
    let previous = dsp::register_callback(Some(dsp_callback));
    // DSP_Init installs libogc's default task callback. Registration must
    // return that callback so the caller can restore it.
    logger.record("dsp callback replaced default", previous.is_some());
    logger.note_u32("dsp mail to ready", dsp::check_mail_to());
    logger.note_u32("dsp mail from ready", dsp::check_mail_from());
    logger.note_u32("dsp dma busy", dsp::get_dma_status());
    let _ = dsp::register_callback(previous);
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

unsafe extern "C" fn dsp_callback() {}
