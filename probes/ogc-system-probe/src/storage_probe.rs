use alloc::alloc::{alloc_zeroed, dealloc};
use core::alloc::Layout;
use core::ffi::{CStr, c_char, c_void};
use core::fmt::{self, Write};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use ogc_card as card;
use ogc_di as di;
use ogc_dvd as dvd;
use ogc_fat as fat;
#[cfg(not(feature = "dolphin-automation"))]
use ogc_gcsd as gcsd;
use ogc_lwp::sys as lwp;
use ogc_sysbase as sysbase;
use ogc_system::sys;
use ogc_usb as usb_core;
use ogc_usbstorage as usb_storage;
use ogc_usbstorage::sys as usb_storage_sys;
use ogc_wiisd as wiisd;

use crate::probe_support::ProbeLogger;

#[repr(align(32))]
struct Workarea([u8; card::CARD_WORKAREA_SIZE as usize]);

struct CardIdentifierReset {
    gamecode: card::CardGameCode,
    company: card::CardCompanyCode,
}

impl Drop for CardIdentifierReset {
    fn drop(&mut self) {
        card::sys::set_gamecode(Some(self.gamecode));
        card::sys::set_company(Some(self.company));
    }
}

#[repr(align(32))]
struct SectorBuf([u8; RAW_USB_SECTOR_BUFFER_SIZE]);

#[repr(align(32))]
struct UsbInterfaceSector([u8; 512]);

#[repr(align(32))]
struct DvdSector([u8; 0x800]);

const USB_CLASS_MASS_STORAGE: u8 = 0x08;
const USB_CLASS_ANY: u8 = 0x00;
// USB mass-storage logical sectors are not universally 512 bytes. This
// matches the backend's maximum single transfer, so one raw sector read
// cannot overrun the diagnostic buffer on 4 KiB-sector media.
const RAW_USB_SECTOR_BUFFER_SIZE: usize = 16 * 1024;
const USB_PROBE_PATH: &CStr = c"usb:/libogc-usb-probe.txt";
const USB_CLOSE_PROBE_PATH: &CStr = c"usb:/libogc-usb-close-probe.txt";
const USB_MOUNT_NAME: &CStr = c"usb";
const USB_PROBE_MSG: &[u8] = b"libogc usb probe\n";
const USB_CLOSE_PROBE_MSG: &[u8] = b"libogc usb close persistence probe\n";
const SD_PROBE_PATH: &CStr = c"sd:/libogc-sd-probe.txt";
const SD_MOUNT_NAME: &CStr = c"sd";
const SD_PROBE_MSG: &[u8] = b"libogc sd probe\n";
const HOTPLUG_SECONDS: u32 = 60;

#[unsafe(no_mangle)]
pub static OGC_STORAGE_PROBE_STAGE: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_STORAGE_PROBE_LAST_RESULT: AtomicI32 = AtomicI32::new(0);
#[unsafe(no_mangle)]
pub static OGC_STORAGE_PROBE_LWP_EVENTS: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_STORAGE_PROBE_USB_V5_REMOVALS: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static OGC_STORAGE_PROBE_USB_V5_LAST_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
#[unsafe(no_mangle)]
pub static OGC_STORAGE_PROBE_USB_V5_LAST_USERDATA: AtomicU32 = AtomicU32::new(0);
static DI_MOUNT_CALLBACK_STATUS: AtomicU32 = AtomicU32::new(u32::MAX);
static DI_MOUNT_CALLBACK_ERROR: AtomicU32 = AtomicU32::new(u32::MAX);

static ASYNC_RETURNED: AtomicBool = AtomicBool::new(false);
static ASYNC_DONE: AtomicBool = AtomicBool::new(false);
static ASYNC_AFTER_RETURN: AtomicBool = AtomicBool::new(false);
static ASYNC_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
static CARD_DETACHED: AtomicBool = AtomicBool::new(false);
static USB_V5_CALLBACKS: AtomicU32 = AtomicU32::new(0);
static CARD_INVALID_CHANNEL_CALLBACKS: AtomicU32 = AtomicU32::new(0);

pub fn run(logger: &mut ProbeLogger, destructive_card_format: bool) {
    logger.section("STORAGE PROBE");
    lwp::set_debug_hook(Some(storage_debug_hook));

    OGC_STORAGE_PROBE_STAGE.store(1, Ordering::Release);
    probe_dvd(logger);
    OGC_STORAGE_PROBE_STAGE.store(2, Ordering::Release);
    probe_usb_storage(logger);
    OGC_STORAGE_PROBE_STAGE.store(3, Ordering::Release);
    probe_sd_interface(logger);
    OGC_STORAGE_PROBE_STAGE.store(4, Ordering::Release);
    probe_card_save_data(logger);
    probe_card_mount_lifecycles(logger);
    if destructive_card_format {
        probe_card_format(logger);
    } else {
        logger.note(
            "card format skipped: pass --destructive-card-format=ERASE-SLOT-A to erase slot A",
        );
    }
    OGC_STORAGE_PROBE_STAGE.store(5, Ordering::Release);
    probe_media_hotplug(logger);

    if !logger.uses_mounted_log() {
        log_fat_result(logger, "fat shutdown all", &fat::shutdown_all());
        logger.note_i32("card shutdown", card::sys::shutdown());
        let dvd_shutdown = dvd::sys::shutdown();
        logger.note_i32("dvd shutdown", dvd_shutdown);
        logger.record("dvd sys shutdown completes", dvd_shutdown == 0);
    }
    OGC_STORAGE_PROBE_STAGE.store(6, Ordering::Release);
    logger.note_u32(
        "storage lwp debug events",
        OGC_STORAGE_PROBE_LWP_EVENTS.load(Ordering::Acquire),
    );
}

/// Runs the CARD transfer and metadata checks without Wii IOS storage probes.
pub fn run_card_probe(logger: &mut ProbeLogger, destructive_card_format: bool) {
    logger.section("GAMECUBE CARD PROBE");
    probe_card_save_data(logger);
    probe_card_mount_lifecycles(logger);
    if destructive_card_format {
        probe_card_format(logger);
    } else {
        logger.note(
            "card format skipped: pass --destructive-card-format=ERASE-SLOT-A to erase slot A",
        );
    }
    logger.note_i32("card shutdown", card::sys::shutdown());
}

unsafe fn storage_debug_hook(event: lwp::DebugEvent) {
    OGC_STORAGE_PROBE_LWP_EVENTS.fetch_add(1, Ordering::AcqRel);
    OGC_STORAGE_PROBE_LAST_RESULT.store(event.event as i32, Ordering::Release);
}

fn prepare_async_probe() {
    ASYNC_RETURNED.store(false, Ordering::Release);
    ASYNC_DONE.store(false, Ordering::Release);
    ASYNC_AFTER_RETURN.store(false, Ordering::Release);
    ASYNC_RESULT.store(i32::MIN, Ordering::Release);
}

fn mark_async_returned() {
    ASYNC_RETURNED.store(true, Ordering::Release);
}

fn wait_async_probe(timeout_seconds: u32) -> bool {
    let sleep = (lwp::TICKS_PER_SECOND / 100).max(1);
    for _ in 0..timeout_seconds.saturating_mul(100) {
        if ASYNC_DONE.load(Ordering::Acquire) {
            return true;
        }
        lwp::sleep_ticks(sleep);
    }
    false
}

unsafe extern "C" fn dvd_async_callback(result: i32, _block: *mut dvd::DvdCmdBlk) {
    ASYNC_AFTER_RETURN.store(ASYNC_RETURNED.load(Ordering::Acquire), Ordering::Release);
    ASYNC_RESULT.store(result, Ordering::Release);
    OGC_STORAGE_PROBE_LAST_RESULT.store(result, Ordering::Release);
    ASYNC_DONE.store(true, Ordering::Release);
}

unsafe extern "C" fn di_mount_callback(status: u32, error: u32) -> i32 {
    DI_MOUNT_CALLBACK_STATUS.store(status, Ordering::Release);
    DI_MOUNT_CALLBACK_ERROR.store(error, Ordering::Release);
    0
}

unsafe extern "C" fn card_detach_callback(_chn: i32, result: i32) {
    CARD_DETACHED.store(true, Ordering::Release);
    OGC_STORAGE_PROBE_LAST_RESULT.store(result, Ordering::Release);
}

unsafe extern "C" fn card_invalid_channel_callback(_chn: i32, _result: i32) {
    CARD_INVALID_CHANNEL_CALLBACKS.fetch_add(1, Ordering::AcqRel);
}

unsafe extern "C" fn usb_v5_removal_callback(result: i32, userdata: *mut c_void) -> i32 {
    OGC_STORAGE_PROBE_USB_V5_LAST_RESULT.store(result, Ordering::Release);
    OGC_STORAGE_PROBE_USB_V5_LAST_USERDATA.store(userdata as usize as u32, Ordering::Release);
    USB_V5_CALLBACKS.fetch_add(1, Ordering::Release);
    0
}

fn record_async(
    logger: &mut ProbeLogger,
    label: &str,
    submit_result: i32,
    success_result: i32,
    timeout_seconds: u32,
) -> bool {
    mark_async_returned();
    let completed = wait_async_probe(timeout_seconds);
    let after_return = ASYNC_AFTER_RETURN.load(Ordering::Acquire);
    let result = ASYNC_RESULT.load(Ordering::Acquire);
    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "{} submit={} complete={} result={} after_return={}",
        label, submit_result, completed as u32, result, after_return as u32
    );
    logger.note(line.as_str());
    logger.record(&alloc::format!("{} accepted", label), submit_result >= 0);
    logger.record(&alloc::format!("{} callback", label), completed);
    logger.record(
        &alloc::format!("{} callback after return", label),
        after_return,
    );
    logger.record(
        &alloc::format!("{} result", label),
        completed && result == success_result,
    );
    completed && after_return && result == success_result
}

pub fn probe_dvd(logger: &mut ProbeLogger) -> bool {
    logger.section("DVD /DEV/DI");
    dvd::init();
    let initial_status = dvd::get_drive_status();
    let cover_status = dvd::get_cover_status();
    let disc_present = dvd::disk_present();
    logger.note_i32("dvd initial status", initial_status);
    logger.note_i32("dvd cover status", cover_status);
    logger.note_i32("dvd disc present", disc_present as i32);

    if disc_present == 0 {
        logger.note("dvd tests skipped: insert a readable Wii/GameCube disc");
        // libogc reports DVD_STATE_NO_DISK as 4. Check the literal value so
        // this probe can detect a wrong public constant or state mapping.
        logger.record(
            "dvd init and drive status report no disc",
            initial_status == 4,
        );
        logger.record("dvd cover status reports no disc", cover_status == 4);
        logger.record("dvd presence reports no disc", disc_present == 0);
        logger.record("dvd no fake success without media", dvd::mount() < 0);
        probe_dvd_no_media_helpers(logger);
        probe_dvd_no_media_async(logger);
        probe_dvd_sys_no_media(logger);
        return true;
    }

    let mount_result = dvd::mount();
    logger.note_i32("dvd mount", mount_result);
    logger.record("dvd mount", mount_result == 0);

    let mut inquiry_block = dvd::DvdCmdBlk::default();
    let mut info = dvd::DvdDrvInfo::default();
    // SAFETY: command and output structures remain live until the synchronous
    // inquiry completes.
    let inquiry = unsafe { dvd::inquiry(&mut inquiry_block, &mut info) };
    logger.note_i32("dvd inquiry", inquiry);
    logger.note_u32("dvd revision", info.rev_level as u32);
    logger.note_u32("dvd device code", info.dev_code as u32);
    logger.record("dvd inquiry", inquiry >= 0);

    let mut id_block = dvd::DvdCmdBlk::default();
    let mut id = dvd::DvdDiskId::default();
    prepare_async_probe();
    // SAFETY: both structures remain live until `record_async` observes callback
    // completion below.
    let submit = unsafe { dvd::read_disk_id(&mut id_block, &mut id, Some(dvd_async_callback)) };
    let id_ok = record_async(logger, "dvd disk id async", submit, 0x20, 10);
    logger.record("dvd disk id transferred", id_ok && id_block.txdsize == 0x20);
    if id_ok {
        let game = [
            id.gamename[0] as u8,
            id.gamename[1] as u8,
            id.gamename[2] as u8,
            id.gamename[3] as u8,
        ];
        let mut line = LineBuf::new();
        let _ = write!(
            &mut line,
            "dvd id={}",
            core::str::from_utf8(&game).unwrap_or("????")
        );
        logger.note(line.as_str());
    }
    logger.record(
        "dvd disk id matches synthetic disc header",
        id_ok
            && id.gamename.map(|byte| byte as u8) == *b"TST1"
            && id.company.map(|byte| byte as u8) == *b"TS",
    );

    let mut sector = DvdSector([0; 0x800]);
    let mut read_block = dvd::DvdCmdBlk::default();
    // SAFETY: the command block and complete sector buffer remain live for this
    // synchronous operation.
    let sync_read = unsafe {
        dvd::read_prio(
            &mut read_block,
            sector.0.as_mut_ptr().cast(),
            sector.0.len() as u32,
            0,
            2,
        )
    };
    logger.note_i32("dvd sector0 sync bytes", sync_read);
    logger.record("dvd sector0 sync read", sync_read == sector.0.len() as i32);
    logger.record(
        "dvd sector0 sync matches synthetic disc header",
        sync_read == sector.0.len() as i32 && sector.0[..8] == *b"TST1TS\0\0",
    );

    sector.0.fill(0);
    let mut async_read_block = dvd::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: the command block and complete sector buffer remain live until
    // callback completion is observed below.
    let submit = unsafe {
        dvd::read_abs_async_prio(
            &mut async_read_block,
            sector.0.as_mut_ptr().cast(),
            sector.0.len() as u32,
            0,
            Some(dvd_async_callback),
            2,
        )
    };
    let read_ok = record_async(
        logger,
        "dvd sector0 async",
        submit,
        sector.0.len() as i32,
        10,
    );
    logger.record(
        "dvd async byte count",
        read_ok && async_read_block.txdsize == sector.0.len() as u32,
    );
    logger.record(
        "dvd sector0 async matches synthetic disc header",
        read_ok && sector.0[..8] == *b"TST1TS\0\0",
    );
    sector.0.fill(0);
    let mut offset_read_block = dvd::DvdCmdBlk::default();
    // SAFETY: the command block and full aligned sector buffer remain live
    // during this synchronous read from the fixed synthetic test image.
    let offset_read = unsafe {
        dvd::read_prio(
            &mut offset_read_block,
            sector.0.as_mut_ptr().cast(),
            sector.0.len() as u32,
            0x2000,
            2,
        )
    };
    logger.note_i32("dvd offset sector sync bytes", offset_read);
    logger.record(
        "dvd offset sector sync matches synthetic payload",
        offset_read == sector.0.len() as i32
            && sector.0[..4] == [11, 48, 85, 122]
            && sector.0[sector.0.len() - 1] == 230,
    );
    logger.note_u32("dvd last drive error", dvd::get_last_error());
    false
}

fn probe_dvd_no_media_async(logger: &mut ProbeLogger) {
    let mut block = dvd::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: `block` remains live until the callback completes.
    let submit = unsafe { dvd::mount_async(&mut block, Some(dvd_async_callback)) };
    record_async(
        logger,
        "dvd mount async without media",
        submit,
        dvd::DVD_ERROR_COVER_CLOSED,
        10,
    );

    let mut inquiry_block = dvd::DvdCmdBlk::default();
    let mut info = dvd::DvdDrvInfo::default();
    prepare_async_probe();
    // SAFETY: both values remain live until the callback completes.
    let submit =
        unsafe { dvd::inquiry_async(&mut inquiry_block, &mut info, Some(dvd_async_callback)) };
    record_async(
        logger,
        "dvd inquiry async without media",
        submit,
        dvd::DVD_ERROR_OK,
        10,
    );

    let mut bs_block = dvd::DvdCmdBlk::default();
    let mut sector = DvdSector([0; 0x800]);
    prepare_async_probe();
    // SAFETY: the aligned sector and command block remain live until callback.
    let submit = unsafe {
        dvd::read_abs_async_for_bs(
            &mut bs_block,
            sector.0.as_mut_ptr().cast(),
            sector.0.len() as u32,
            0,
            Some(dvd_async_callback),
        )
    };
    record_async(
        logger,
        "dvd bs read async without media",
        submit,
        dvd::DVD_ERROR_COVER_CLOSED,
        10,
    );

    let mut control_block = dvd::DvdCmdBlk::default();
    prepare_async_probe();
    // CHECKDISK only reads the current cover state in this implementation.
    // SAFETY: `control_block` remains live until the callback completes.
    let submit = unsafe {
        dvd::control_drive_async(
            &mut control_block,
            dvd::DVD_SPINMOTOR_CHECKDISK,
            Some(dvd_async_callback),
        )
    };
    record_async(
        logger,
        "dvd control check async without media",
        submit,
        dvd::DVD_ERROR_COVER_CLOSED,
        10,
    );

    let mut offset_block = dvd::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: `offset_block` remains live until the callback completes.
    let submit =
        unsafe { dvd::set_gcm_offset_async(&mut offset_block, 0, Some(dvd_async_callback)) };
    record_async(
        logger,
        "dvd set zero gcm offset async",
        submit,
        dvd::DVD_ERROR_OK,
        10,
    );

    let mut sync_offset_block = dvd::DvdCmdBlk::default();
    // SAFETY: the command block remains exclusively accessible for this call.
    let sync_offset = unsafe { dvd::set_gcm_offset(&mut sync_offset_block, 0) };
    logger.note_i32("dvd set zero gcm offset sync result", sync_offset);
    logger.record("dvd set zero gcm offset sync", sync_offset == 1);

    let mut sync_control_block = dvd::DvdCmdBlk::default();
    // CHECKDISK only reads the current cover state in this implementation.
    // SAFETY: the command block remains exclusively accessible for this call.
    let sync_control =
        unsafe { dvd::control_drive(&mut sync_control_block, dvd::DVD_SPINMOTOR_CHECKDISK) };
    logger.note_i32("dvd control check sync result", sync_control);
    logger.record(
        "dvd control check sync reports no media",
        sync_control == -1,
    );

    let mut invalid_seek_block = dvd::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: `invalid_seek_block` remains live until the callback completes.
    let submit = unsafe {
        dvd::seek_abs_async_prio(&mut invalid_seek_block, 0, Some(dvd_async_callback), 2)
    };
    record_async(
        logger,
        "dvd zero offset seek rejects invalid offset",
        submit,
        dvd::DVD_ERROR_COVER_CLOSED,
        10,
    );

    let mut sync_seek_block = dvd::DvdCmdBlk::default();
    // SAFETY: this offset is invalid, so the call completes without a disc IO.
    let sync_seek = unsafe { dvd::seek_prio(&mut sync_seek_block, 0, 2) };
    logger.note_i32("dvd zero offset sync seek result", sync_seek);
    logger.record(
        "dvd zero offset sync seek reports invalid state",
        sync_seek == -1,
    );

    let mut stream_block = dvd::DvdCmdBlk::default();
    prepare_async_probe();
    // This command stops DVD audio streaming; it does not move the drive motor.
    // SAFETY: `stream_block` remains live until the callback completes.
    let submit =
        unsafe { dvd::stop_stream_at_end_async(&mut stream_block, Some(dvd_async_callback)) };
    record_async(
        logger,
        "dvd stop stream async without media",
        submit,
        dvd::DVD_ERROR_COVER_CLOSED,
        10,
    );

    let mut sync_stream_block = dvd::DvdCmdBlk::default();
    // SAFETY: the command block remains exclusively accessible for this call.
    let sync_stream = unsafe { dvd::stop_stream_at_end(&mut sync_stream_block) };
    logger.note_i32("dvd stop stream sync result", sync_stream);
    logger.record("dvd stop stream sync without media", sync_stream == -1);

    let mut spin_block = dvd::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: `spin_block` remains live until the callback completes.
    let submit = unsafe { dvd::spin_up_drive_async(&mut spin_block, Some(dvd_async_callback)) };
    record_async(
        logger,
        "dvd spin up async without media",
        submit,
        dvd::DVD_ERROR_OK,
        10,
    );

    let mut sync_spin_block = dvd::DvdCmdBlk::default();
    // SAFETY: the command block remains exclusively accessible for this call.
    let sync_spin = unsafe { dvd::spin_up_drive(&mut sync_spin_block) };
    logger.note_i32("dvd spin up sync result", sync_spin);
    logger.record("dvd spin up sync without media", sync_spin == 1);

    let stop_motor = dvd::sys::stop_motor();
    logger.note_i32("dvd stop motor without media result", stop_motor);
    logger.record("dvd stop motor call completes", stop_motor == 0);
}

fn probe_dvd_sys_no_media(logger: &mut ProbeLogger) {
    logger.section("DVD SYS DIRECT NO MEDIA");
    dvd::sys::init();
    let status = dvd::sys::get_drive_status();
    let cover = dvd::sys::get_cover_status();
    let present = dvd::sys::disk_present();
    logger.record(
        "dvd sys status APIs report no disc",
        status == dvd::sys::DVD_STATE_NO_DISK
            && cover == dvd::sys::DVD_STATE_NO_DISK
            && present == 0,
    );

    let disk_id = dvd::sys::get_current_disk_id();
    let drive_info = dvd::sys::get_drive_info();
    logger.record(
        "dvd sys static output pointers remain stable",
        !disk_id.is_null()
            && !drive_info.is_null()
            && core::ptr::eq(disk_id, dvd::sys::get_current_disk_id())
            && core::ptr::eq(drive_info, dvd::sys::get_drive_info()),
    );

    let mut idle = dvd::sys::DvdCmdBlk::default();
    // SAFETY: the idle command block is live and is not owned by a request.
    let idle_state = unsafe { dvd::sys::get_cmd_block_status(&mut idle) };
    logger.record("dvd sys idle command status is readable", idle_state == 0);

    let mut mount_block = dvd::sys::DvdCmdBlk::default();
    // SAFETY: the command block stays live until its callback completes.
    let sync_mount = dvd::sys::mount();
    logger.record("dvd sys sync mount rejects no disc", sync_mount < 0);
    prepare_async_probe();
    let mount_submit = unsafe { dvd::sys::mount_async(&mut mount_block, Some(dvd_async_callback)) };
    let mount_done = record_async(
        logger,
        "dvd sys mount async without media",
        mount_submit,
        dvd::sys::DVD_ERROR_COVER_CLOSED,
        10,
    );
    // SAFETY: the callback has completed, so the command block is available.
    let mount_state = unsafe { dvd::sys::get_cmd_block_status(&mut mount_block) };
    logger.record(
        "dvd sys mount async updates its command block",
        mount_done && mount_state == dvd::sys::DVD_STATE_NO_DISK && mount_block.txdsize == 0,
    );

    let mut control_block = dvd::sys::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: the command block stays live until its callback completes.
    let control_submit = unsafe {
        dvd::sys::control_drive_async(
            &mut control_block,
            dvd::sys::DVD_SPINMOTOR_CHECKDISK,
            Some(dvd_async_callback),
        )
    };
    let control_done = record_async(
        logger,
        "dvd sys control async without media",
        control_submit,
        dvd::sys::DVD_ERROR_COVER_CLOSED,
        10,
    );
    // SAFETY: this command block is exclusively available to the sync call.
    let sync_control =
        unsafe { dvd::sys::control_drive(&mut control_block, dvd::sys::DVD_SPINMOTOR_CHECKDISK) };
    logger.record(
        "dvd sys control sync reports no disc",
        control_done && sync_control < 0,
    );

    let mut offset_block = dvd::sys::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: the command block stays live until its callback completes.
    let offset_submit =
        unsafe { dvd::sys::set_gcm_offset_async(&mut offset_block, 0, Some(dvd_async_callback)) };
    let offset_done = record_async(
        logger,
        "dvd sys zero offset async",
        offset_submit,
        dvd::sys::DVD_ERROR_OK,
        10,
    );
    // SAFETY: the command block is exclusively available to the sync call.
    let sync_offset = unsafe { dvd::sys::set_gcm_offset(&mut offset_block, 0) };
    logger.record(
        "dvd sys zero offset sync completes",
        offset_done && sync_offset == 1,
    );

    let mut spin_block = dvd::sys::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: the command block stays live until its callback completes.
    let spin_submit =
        unsafe { dvd::sys::spin_up_drive_async(&mut spin_block, Some(dvd_async_callback)) };
    let spin_done = record_async(
        logger,
        "dvd sys spin up async without media",
        spin_submit,
        dvd::sys::DVD_ERROR_OK,
        10,
    );
    // SAFETY: the command block is exclusively available to the sync call.
    let sync_spin = unsafe { dvd::sys::spin_up_drive(&mut spin_block) };
    logger.record(
        "dvd sys spin up sync completes without media",
        spin_done && sync_spin == 1,
    );

    let mut inquiry_block = dvd::sys::DvdCmdBlk::default();
    let mut info = dvd::sys::DvdDrvInfo::default();
    prepare_async_probe();
    // SAFETY: both values stay live until the callback completes.
    let inquiry_submit =
        unsafe { dvd::sys::inquiry_async(&mut inquiry_block, &mut info, Some(dvd_async_callback)) };
    let inquiry_done = record_async(
        logger,
        "dvd sys inquiry async without media",
        inquiry_submit,
        dvd::sys::DVD_ERROR_OK,
        10,
    );
    // SAFETY: both values are distinct and exclusively available to the call.
    let sync_inquiry = unsafe { dvd::sys::inquiry(&mut inquiry_block, &mut info) };
    logger.record(
        "dvd sys inquiry sync returns drive information",
        inquiry_done && sync_inquiry >= 0,
    );

    let mut id_block = dvd::sys::DvdCmdBlk::default();
    let mut id = dvd::sys::DvdDiskId::default();
    prepare_async_probe();
    // SAFETY: both values stay live until the callback completes.
    let id_submit =
        unsafe { dvd::sys::read_disk_id(&mut id_block, &mut id, Some(dvd_async_callback)) };
    let id_done = record_async(
        logger,
        "dvd sys read disk ID async without media",
        id_submit,
        dvd::sys::DVD_ERROR_COVER_CLOSED,
        10,
    );
    logger.record(
        "dvd sys disk ID failure preserves transfer count",
        id_done && id_block.state == dvd::sys::DVD_STATE_NO_DISK && id_block.txdsize == 0,
    );

    let mut read_block = dvd::sys::DvdCmdBlk::default();
    let mut sector = DvdSector([0xa5; 0x800]);
    prepare_async_probe();
    // SAFETY: the block and aligned sector stay live until callback completion.
    let read_submit = unsafe {
        dvd::sys::read_abs_async_prio(
            &mut read_block,
            sector.0.as_mut_ptr().cast(),
            sector.0.len() as u32,
            0,
            Some(dvd_async_callback),
            2,
        )
    };
    let read_done = record_async(
        logger,
        "dvd sys absolute read async without media",
        read_submit,
        dvd::sys::DVD_ERROR_COVER_CLOSED,
        10,
    );
    let read_preserved = sector.0.iter().all(|byte| *byte == 0xa5);
    // SAFETY: the aligned output buffer and command block are live and distinct.
    let sync_read = unsafe {
        dvd::sys::read_prio(
            &mut read_block,
            sector.0.as_mut_ptr().cast(),
            sector.0.len() as u32,
            0,
            2,
        )
    };
    logger.record(
        "dvd sys reads preserve output and report no disc",
        read_done
            && read_preserved
            && sync_read < 0
            && read_block.state == dvd::sys::DVD_STATE_NO_DISK
            && read_block.txdsize == 0
            && sector.0.iter().all(|byte| *byte == 0xa5),
    );

    let mut bs_block = dvd::sys::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: the aligned sector and block stay live until callback completion.
    let bs_submit = unsafe {
        dvd::sys::read_abs_async_for_bs(
            &mut bs_block,
            sector.0.as_mut_ptr().cast(),
            sector.0.len() as u32,
            0,
            Some(dvd_async_callback),
        )
    };
    let bs_done = record_async(
        logger,
        "dvd sys BS read async without media",
        bs_submit,
        dvd::sys::DVD_ERROR_COVER_CLOSED,
        10,
    );
    logger.record(
        "dvd sys BS read preserves output and byte count",
        bs_done && bs_block.txdsize == 0 && sector.0.iter().all(|byte| *byte == 0xa5),
    );

    let mut seek_block = dvd::sys::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: the command block stays live until callback completion.
    let seek_submit =
        unsafe { dvd::sys::seek_abs_async_prio(&mut seek_block, 0, Some(dvd_async_callback), 2) };
    let seek_done = record_async(
        logger,
        "dvd sys seek async without media",
        seek_submit,
        dvd::sys::DVD_ERROR_COVER_CLOSED,
        10,
    );
    // SAFETY: the command block is exclusively available to the sync call.
    let sync_seek = unsafe { dvd::sys::seek_prio(&mut seek_block, 0, 2) };
    logger.record(
        "dvd sys seek sync reports no disc",
        seek_done && sync_seek < 0,
    );

    let mut stream_block = dvd::sys::DvdCmdBlk::default();
    prepare_async_probe();
    // SAFETY: the command block stays live until callback completion.
    let stream_submit =
        unsafe { dvd::sys::stop_stream_at_end_async(&mut stream_block, Some(dvd_async_callback)) };
    let stream_done = record_async(
        logger,
        "dvd sys stop stream async without media",
        stream_submit,
        dvd::sys::DVD_ERROR_COVER_CLOSED,
        10,
    );
    // SAFETY: the command block is exclusively available to the sync call.
    let sync_stream = unsafe { dvd::sys::stop_stream_at_end(&mut stream_block) };
    logger.record(
        "dvd sys stop stream sync reports no disc",
        stream_done && sync_stream < 0,
    );

    let old_auto_invalidation = dvd::sys::set_auto_invalidation(0);
    let changed_auto_invalidation = dvd::sys::set_auto_invalidation(1);
    let restored_auto_invalidation = dvd::sys::set_auto_invalidation(old_auto_invalidation);
    logger.record(
        "dvd sys auto invalidation changes and restores state",
        old_auto_invalidation == 1
            && changed_auto_invalidation == 0
            && restored_auto_invalidation == 1,
    );

    dvd::sys::pause();
    let paused_status = dvd::sys::get_drive_status();
    let cancel_result = dvd::sys::cancel_all_async(None);
    let cancelled_status = dvd::sys::get_drive_status();
    dvd::sys::reset(dvd::sys::DVD_RESETNONE);
    let reset_status = dvd::sys::get_drive_status();
    logger.record(
        "dvd sys pause cancel and reset update drive state",
        paused_status == dvd::sys::DVD_STATE_IGNORED
            && cancel_result == 1
            && cancelled_status == dvd::sys::DVD_STATE_IGNORED
            && reset_status == dvd::sys::DVD_STATE_NO_DISK,
    );

    let last_error = dvd::sys::get_last_error();
    let repeated_last_error = dvd::sys::get_last_error();
    logger.note_u32("dvd sys last error after no-disc calls", last_error);
    logger.record(
        "dvd sys last error read is stable",
        last_error == repeated_last_error,
    );
    let stop_motor = dvd::sys::stop_motor();
    logger.record("dvd sys stop motor completes", stop_motor == 0);
}

fn probe_dvd_no_media_helpers(logger: &mut ProbeLogger) {
    let mut block = dvd::DvdCmdBlk::default();
    // SAFETY: `block` is live and no request uses it.
    logger.record(
        "dvd idle command block status",
        unsafe { dvd::get_cmd_block_status(&mut block) } == 0,
    );

    let previous_invalidation = dvd::set_auto_invalidation(0);
    let disabled_invalidation = dvd::set_auto_invalidation(1);
    logger.record(
        "dvd auto invalidation update and restore",
        previous_invalidation == 1 && disabled_invalidation == 0,
    );

    dvd::pause();
    let paused_status = dvd::get_drive_status();
    logger.record("dvd pause updates drive state", paused_status == 8);

    let cancel_result = dvd::cancel_all_async(None);
    let cancelled_status = dvd::get_drive_status();
    logger.record(
        "dvd cancel all pauses without pending requests",
        cancel_result == 1 && cancelled_status == 8,
    );

    // DVD_RESETNONE clears the pause state without issuing a drive reset.
    dvd::reset(2);
    let reset_status = dvd::get_drive_status();
    logger.record("dvd reset none restores no-disc state", reset_status == 4);

    let disk_id = dvd::get_current_disk_id();
    let drive_info = dvd::get_drive_info();
    logger.record(
        "dvd static information pointers remain stable",
        !disk_id.is_null()
            && !drive_info.is_null()
            && core::ptr::eq(disk_id, dvd::get_current_disk_id())
            && core::ptr::eq(drive_info, dvd::get_drive_info()),
    );
}

pub fn probe_di_no_disc(logger: &mut ProbeLogger) {
    logger.section("OGC DI DIRECT NO-DISC");
    let init_result = di::init();
    logger.note_i32("di init result", init_result);
    logger.record("di init opens transport", init_result == 0);
    logger.record(
        "di init state has no disc",
        di::get_status() == (di::DVD_INIT | di::DVD_NO_DISC) as i32,
    );

    DI_MOUNT_CALLBACK_STATUS.store(u32::MAX, Ordering::Release);
    DI_MOUNT_CALLBACK_ERROR.store(u32::MAX, Ordering::Release);
    di::set_mount_callback(Some(di_mount_callback));
    di::mount();
    di::set_mount_callback(None);
    logger.record(
        "di no-disc mount callback",
        DI_MOUNT_CALLBACK_STATUS.load(Ordering::Acquire) == di::DVD_NO_DISC
            && DI_MOUNT_CALLBACK_ERROR.load(Ordering::Acquire) == 0,
    );
    logger.record(
        "di mount leaves no-disc state",
        di::get_status() == di::DVD_NO_DISC as i32,
    );
    DI_MOUNT_CALLBACK_STATUS.store(u32::MAX, Ordering::Release);
    DI_MOUNT_CALLBACK_ERROR.store(u32::MAX, Ordering::Release);
    di::set_mount_callback(Some(di_mount_callback));
    di::DI_Mount();
    di::set_mount_callback(None);
    logger.record(
        "DI_Mount delivers no-disc callback",
        DI_MOUNT_CALLBACK_STATUS.load(Ordering::Acquire) == di::DVD_NO_DISC
            && DI_MOUNT_CALLBACK_ERROR.load(Ordering::Acquire) == 0,
    );

    let mut drive_id = di::DiDriveId {
        rev: 0x1234,
        dev_code: 0x5678,
        rel_date: 0x1020_3040,
    };
    let id_result = di::identify(&mut drive_id);
    let mut c_drive_id = di::DiDriveId {
        rev: 0x1234,
        dev_code: 0x5678,
        rel_date: 0x1020_3040,
    };
    // SAFETY: `c_drive_id` is aligned and writable for one drive identifier.
    let c_id_result = unsafe { di::DI_Identify(&mut c_drive_id) };
    logger.note_i32("di identify result", id_result);
    logger.note_u32("di drive revision", u32::from(drive_id.rev));
    logger.note_u32("di drive code", u32::from(drive_id.dev_code));
    logger.note_u32("di drive release date", drive_id.rel_date);
    logger.record("di identify returns a drive record", id_result == 0);
    logger.record(
        "C DI identify matches Rust output",
        c_id_result == id_result
            && c_drive_id.rev == drive_id.rev
            && c_drive_id.dev_code == drive_id.dev_code
            && c_drive_id.rel_date == drive_id.rel_date,
    );
    let dvd_support = di::check_dvd_support();
    let c_dvd_support = di::DI_CheckDVDSupport();
    logger.note_i32("di dvd support", dvd_support);
    logger.record("di dvd support reports Dolphin drive", dvd_support == 1);
    logger.record(
        "C DI support matches Rust result",
        c_dvd_support == dvd_support,
    );

    let mut disc_id = 0x1234_5678_9ABC_DEF0;
    let id_result = di::read_disc_id(&mut disc_id);
    let mut c_disc_id = 0x1234_5678_9ABC_DEF0;
    // SAFETY: `c_disc_id` is aligned and writable for one identifier.
    let c_disc_id_result = unsafe { di::DI_ReadDiscID(&mut c_disc_id) };
    logger.record(
        "di disc id errors preserve output",
        id_result < 0 && disc_id == 0x1234_5678_9ABC_DEF0,
    );
    logger.record(
        "C DI disc ID matches Rust result and output",
        c_disc_id_result == id_result && c_disc_id == disc_id,
    );
    let mut disc_id_bytes = [0xA5; 0x20];
    let bytes_result = di::read_disc_id_bytes(&mut disc_id_bytes);
    logger.record(
        "di disc id bytes errors preserve output",
        bytes_result < 0 && disc_id_bytes == [0xA5; 0x20],
    );

    let mut error = 0xA5A5_5A5A;
    let error_result = di::get_error(&mut error);
    let mut c_error = 0xA5A5_5A5A;
    // SAFETY: `c_error` is aligned and writable for one error word.
    let c_error_result = unsafe { di::DI_GetError(&mut c_error) };
    logger.note_i32("di get-error result", error_result);
    logger.note_u32("di get-error output", error);
    logger.note_i32("C DI get-error result", c_error_result);
    logger.note_u32("C DI get-error output", c_error);
    logger.record(
        "di error output is valid or preserved",
        error_result == 0 || (error_result < 0 && error == 0xA5A5_5A5A),
    );
    logger.record(
        "Rust and C DI error calls return data and preserve success",
        error_result == 0 && c_error_result == 0 && error != 0xA5A5_5A5A && c_error != 0xA5A5_5A5A,
    );
    let mut cover = 0xA5A5_5A5A;
    let cover_result = di::get_cover_register(&mut cover);
    let mut c_cover = 0xA5A5_5A5A;
    // SAFETY: `c_cover` is aligned and writable for one cover-status word.
    let c_cover_result = unsafe { di::DI_GetCoverRegister(&mut c_cover) };
    logger.record(
        "di cover result reports no inserted disc",
        (cover_result == 0 && cover & di::DVD_COVER_DISC_INSERTED == 0)
            || (cover_result < 0 && cover == 0xA5A5_5A5A),
    );
    logger.record(
        "C DI cover status matches Rust result and output",
        c_cover_result == cover_result && c_cover == cover,
    );

    let mut config = 0xA5A5_5A5A;
    let config_result = di::read_dvd_config(&mut config, 0);
    let mut c_config = 0xA5A5_5A5A;
    // SAFETY: `c_config` is aligned and writable for one configuration word.
    let c_config_result = unsafe { di::DI_ReadDVDConfig(&mut c_config, 0) };
    logger.record(
        "di config output is valid or preserved",
        config_result == 0 || (config_result < 0 && config == 0xA5A5_5A5A),
    );
    logger.record(
        "C DI configuration matches Rust result and output",
        c_config_result == config_result && c_config == config,
    );
    let mut copyright = 0xA5A5_5A5A;
    let copyright_result = di::read_dvd_copyright(&mut copyright);
    let mut c_copyright = 0xA5A5_5A5A;
    // SAFETY: `c_copyright` is aligned and writable for one copyright word.
    let c_copyright_result = unsafe { di::DI_ReadDVDCopyright(&mut c_copyright) };
    logger.record(
        "di copyright output is valid or preserved",
        copyright_result == 0 || (copyright_result < 0 && copyright == 0xA5A5_5A5A),
    );
    logger.record(
        "C DI copyright matches Rust result and output",
        c_copyright_result == copyright_result && c_copyright == copyright,
    );

    let mut aligned = DvdSector([0; 0x800]);
    // SAFETY: `aligned` is a writable, 32-byte aligned buffer of one sector.
    let read_result = unsafe { di::read_dvd(aligned.0.as_mut_ptr().cast(), 1, 0) };
    logger.note_i32("di sector read result", read_result);
    logger.note_u32(
        "di sector read first word",
        u32::from_be_bytes(aligned.0[..4].try_into().unwrap()),
    );
    logger.record(
        "di direct sector read reports no-media error",
        read_result == 1,
    );
    // SAFETY: The null pointer deliberately checks the documented argument
    // error. The open DI descriptor takes precedence over pointer validation.
    logger.record(
        "di raw read rejects null buffer",
        unsafe { di::read(core::ptr::null_mut(), 0x20, 0) } == -22,
    );
    let mut misaligned = DvdSector([0xa5; 0x800]);
    // SAFETY: The pointer is live and has space for 0x20 bytes, but is
    // deliberately unaligned so the call must reject it before I/O.
    logger.record(
        "di raw read rejects unaligned buffer and preserves bytes",
        unsafe { di::read(misaligned.0.as_mut_ptr().add(1).cast(), 0x20, 0) } == -14
            && misaligned.0.iter().all(|byte| *byte == 0xa5),
    );
    // SAFETY: The null pointer deliberately checks the documented argument
    // error while the DI descriptor is open.
    logger.record(
        "di unencrypted read rejects null buffer",
        unsafe { di::unencrypted_read(core::ptr::null_mut(), 0x20, 0) } == -22,
    );

    logger.section("OGC DI C ABI ARGUMENTS");
    logger.record(
        "DI_GetStatus reports no-disc state",
        di::DI_GetStatus() == di::DVD_NO_DISC as i32,
    );
    // SAFETY: Null pointers deliberately exercise the C wrapper checks.
    logger.record(
        "DI_Identify rejects null output",
        unsafe { di::DI_Identify(core::ptr::null_mut()) } == -22,
    );
    logger.record(
        "DI_CheckDVDSupport reports Dolphin drive",
        di::DI_CheckDVDSupport() == 1,
    );
    // SAFETY: Null pointers deliberately exercise the C wrapper checks.
    logger.record(
        "DI_ReadDiscID rejects null output",
        unsafe { di::DI_ReadDiscID(core::ptr::null_mut()) } == -22,
    );
    logger.record(
        "DI_GetError rejects null output",
        unsafe { di::DI_GetError(core::ptr::null_mut()) } == -22,
    );
    logger.record(
        "DI_GetCoverRegister rejects null output",
        unsafe { di::DI_GetCoverRegister(core::ptr::null_mut()) } == -22,
    );
    logger.record("DI_Reset completes", di::DI_Reset() == 0);
    logger.record("DI_StopMotor completes", di::DI_StopMotor() == 0);
    logger.record("DI_Eject completes", di::DI_Eject() == 0);
    logger.record("DI_KillDrive completes", di::DI_KillDrive() == 0);
    // SAFETY: `aligned` is a writable, 32-byte aligned buffer of one sector.
    logger.record(
        "DI_ReadDVD reports no-media error",
        unsafe { di::DI_ReadDVD(aligned.0.as_mut_ptr().cast(), 1, 0) } == 1,
    );
    // SAFETY: The no-disc state rejects this request before any buffer access.
    logger.record(
        "DI_ReadDVDAsync reports no mounted media",
        unsafe { di::DI_ReadDVDAsync(core::ptr::null_mut(), 1, 0, None) } < 0,
    );
    // SAFETY: Null pointers deliberately exercise the C wrapper checks.
    logger.record(
        "DI_Read rejects null buffer",
        unsafe { di::DI_Read(core::ptr::null_mut(), 0x20, 0) } == -22,
    );
    logger.record(
        "DI_UnencryptedRead rejects null buffer",
        unsafe { di::DI_UnencryptedRead(core::ptr::null_mut(), 0x20, 0) } == -22,
    );
    logger.record(
        "DI_ReadDVDConfig rejects null output",
        unsafe { di::DI_ReadDVDConfig(core::ptr::null_mut(), 0) } == -22,
    );
    logger.record(
        "DI_ReadDVDCopyright rejects null output",
        unsafe { di::DI_ReadDVDCopyright(core::ptr::null_mut()) } == -22,
    );
    logger.record(
        "DI_ReadDVDDiscKey rejects null buffer",
        unsafe { di::DI_ReadDVDDiscKey(core::ptr::null_mut()) } == -22,
    );
    logger.record(
        "DI_ReadDVDPhysical rejects null buffer",
        unsafe { di::DI_ReadDVDPhysical(core::ptr::null_mut()) } == -22,
    );
    logger.record(
        "DI_Read_BCA rejects null buffer",
        unsafe { di::DI_Read_BCA(core::ptr::null_mut()) } == -22,
    );
    logger.record(
        "DI_ReportKey rejects null buffer",
        unsafe { di::DI_ReportKey(0, 0, core::ptr::null_mut()) } == -22,
    );

    let mut rust_key = DvdSector([0xA5; 0x800]);
    let mut c_key = DvdSector([0xA5; 0x800]);
    // SAFETY: both buffers are writable, 32-byte aligned, and 0x800 bytes.
    let key_result = unsafe { di::read_dvd_disc_key(rust_key.0.as_mut_ptr().cast()) };
    let key_c_result = unsafe { di::DI_ReadDVDDiscKey(c_key.0.as_mut_ptr().cast()) };
    let mut rust_physical = DvdSector([0xA5; 0x800]);
    let mut c_physical = DvdSector([0xA5; 0x800]);
    // SAFETY: both buffers are writable, 32-byte aligned, and 0x800 bytes.
    let physical_result = unsafe { di::read_dvd_physical(rust_physical.0.as_mut_ptr().cast()) };
    let physical_c_result = unsafe { di::DI_ReadDVDPhysical(c_physical.0.as_mut_ptr().cast()) };
    logger.note_i32("di disc key result", key_result);
    logger.note_i32("DI_ReadDVDDiscKey result", key_c_result);
    logger.note_i32("di physical data result", physical_result);
    logger.note_i32("DI_ReadDVDPhysical result", physical_c_result);
    logger.record(
        "Rust and C DI disc key results and bytes match",
        key_result <= 0 && key_result == key_c_result && rust_key.0 == c_key.0,
    );
    logger.record(
        "Rust and C DI physical data results and bytes match",
        physical_result <= 0
            && physical_result == physical_c_result
            && rust_physical.0 == c_physical.0,
    );

    let mut rust_bca = DvdSector([0xA5; 0x800]);
    let mut c_bca = DvdSector([0xA5; 0x800]);
    // SAFETY: both buffers are writable and 32-byte aligned. The APIs use the
    // first 64 bytes of each buffer.
    let bca_result = unsafe { di::read_bca(rust_bca.0.as_mut_ptr().cast()) };
    let bca_c_result = unsafe { di::DI_Read_BCA(c_bca.0.as_mut_ptr().cast()) };
    let mut rust_report = DvdSector([0xA5; 0x800]);
    let mut c_report = DvdSector([0xA5; 0x800]);
    // SAFETY: both buffers are writable and 32-byte aligned. The APIs use the
    // first 32 bytes of each buffer.
    let report_result = unsafe { di::report_key(0x5A, 0x1234, rust_report.0.as_mut_ptr().cast()) };
    let report_c_result = unsafe { di::DI_ReportKey(0x5A, 0x1234, c_report.0.as_mut_ptr().cast()) };
    logger.note_i32("di BCA result", bca_result);
    logger.note_i32("DI_Read_BCA result", bca_c_result);
    logger.note_i32("di report key result", report_result);
    logger.note_i32("DI_ReportKey result", report_c_result);
    logger.record(
        "Rust and C DI BCA results and bytes match",
        bca_result <= 0 && bca_result == bca_c_result && rust_bca.0[..64] == c_bca.0[..64],
    );
    logger.record(
        "Rust and C DI report-key results and bytes match",
        report_result <= 0
            && report_result == report_c_result
            && rust_report.0[..32] == c_report.0[..32],
    );

    let config_status_before = di::get_status();
    di::load_dvdx(true);
    di::load_dvdx(false);
    di::use_cache(true);
    di::use_cache(false);
    logger.record(
        "Rust DI configuration setters preserve drive status",
        di::get_status() == config_status_before,
    );
    di::DI_LoadDVDX(true);
    di::DI_LoadDVDX(false);
    di::DI_UseCache(true);
    di::DI_UseCache(false);
    logger.record(
        "C DI configuration setters preserve drive status",
        di::get_status() == config_status_before,
    );

    DI_MOUNT_CALLBACK_STATUS.store(u32::MAX, Ordering::Release);
    DI_MOUNT_CALLBACK_ERROR.store(u32::MAX, Ordering::Release);
    di::set_init_callback(Some(di_mount_callback));
    di::mount();
    let rust_init_callback_untouched = DI_MOUNT_CALLBACK_STATUS.load(Ordering::Acquire) == u32::MAX
        && DI_MOUNT_CALLBACK_ERROR.load(Ordering::Acquire) == u32::MAX;
    di::set_init_callback(None);
    logger.record(
        "Rust DI init callback is not called for no-disc mount",
        rust_init_callback_untouched,
    );

    DI_MOUNT_CALLBACK_STATUS.store(u32::MAX, Ordering::Release);
    DI_MOUNT_CALLBACK_ERROR.store(u32::MAX, Ordering::Release);
    di::DI_SetInitCallback(Some(di_mount_callback));
    di::DI_Mount();
    let c_init_callback_untouched = DI_MOUNT_CALLBACK_STATUS.load(Ordering::Acquire) == u32::MAX
        && DI_MOUNT_CALLBACK_ERROR.load(Ordering::Acquire) == u32::MAX;
    di::DI_SetInitCallback(None);
    logger.record(
        "C DI init callback is not called for no-disc mount",
        c_init_callback_untouched,
    );
    let reset_result = di::reset();
    logger.note_i32("di reset result", reset_result);
    logger.record("di reset completes in Dolphin", reset_result == 0);
    let stop_result = di::stop_motor();
    let eject_result = di::eject();
    let kill_result = di::kill_drive();
    logger.note_i32("di stop motor result", stop_result);
    logger.note_i32("di eject result", eject_result);
    logger.note_i32("di kill drive result", kill_result);
    logger.record("di stop motor command completes", stop_result == 0);
    logger.record("di eject command completes", eject_result == 0);
    logger.record("di kill drive command completes", kill_result == 0);
    logger.record(
        "di audio stream without media reports an error",
        di::audio_stream(0, 0, 0) < 0,
    );

    let close_result = di::close();
    logger.note_i32("di close result", close_result);
    logger.record("di close succeeds", close_result == 0);
    // SAFETY: The closed descriptor is checked before the null buffer.
    logger.record(
        "di raw read after close rejects missing descriptor",
        unsafe { di::read(core::ptr::null_mut(), 0x20, 0) } == -6,
    );
    logger.record(
        "di close restores initial status",
        di::get_status() == (di::DVD_INIT | di::DVD_NO_DISC) as i32,
    );
    // SAFETY: The null pointer is rejected by the closed-descriptor state
    // before any buffer access.
    logger.record(
        "di closed async read rejects missing descriptor",
        unsafe { di::read_dvd_async(core::ptr::null_mut(), 1, 0, None) } == -6,
    );
    let c_reopen_result = di::DI_Init();
    logger.note_i32("DI_Init after close result", c_reopen_result);
    logger.record("DI_Init reopens transport", c_reopen_result == 0);
    di::DI_Close();
    logger.record(
        "DI_Close restores initial status",
        di::DI_GetStatus() == (di::DVD_INIT | di::DVD_NO_DISC) as i32,
    );
    let reopen_result = di::init();
    logger.note_i32("di reopen result", reopen_result);
    logger.record("di reinitializes after close", reopen_result == 0);
    let final_close_result = di::close();
    logger.note_i32("di final close result", final_close_result);
    logger.record("di final close succeeds", final_close_result == 0);
    let rust_open_partition_closed = di::open_partition(0) == -6;
    let rust_close_partition_closed = di::close_partition() == -6;
    logger.record(
        "Rust DI partition APIs reject a closed descriptor",
        rust_open_partition_closed && rust_close_partition_closed,
    );
    let c_open_partition_closed = di::DI_OpenPartition(0) == -6;
    let c_close_partition_closed = di::DI_ClosePartition() == -6;
    logger.record(
        "C DI partition APIs reject a closed descriptor",
        c_open_partition_closed && c_close_partition_closed,
    );
}

pub fn probe_di_media(logger: &mut ProbeLogger) {
    logger.section("OGC DI INSERTED MEDIA");
    let init_result = di::init();
    logger.note_i32("di media init", init_result);
    logger.record("di media init opens transport", init_result == 0);
    logger.record(
        "di media state reports inserted disc",
        di::get_status() & di::DVD_NO_DISC as i32 == 0,
    );

    let mut id_bytes = [0u8; 0x20];
    let id_result = di::read_disc_id_bytes(&mut id_bytes);
    logger.note_i32("di media disc id bytes result", id_result);
    logger.record(
        "di media disc id bytes match fixture",
        id_result == 0 && id_bytes[..6] == *b"TST1TS",
    );

    let mut rust_sector = DvdSector([0; 0x800]);
    // SAFETY: the buffer is writable, 32-byte aligned, and one complete DI sector.
    let rust_read = unsafe { di::read_dvd(rust_sector.0.as_mut_ptr().cast(), 1, 0) };
    logger.note_i32("di media Rust read result", rust_read);
    logger.record(
        "di media Rust sector read matches fixture",
        rust_read == 0 && rust_sector.0[..8] == *b"TST1TS\0\0",
    );

    let mut c_sector = DvdSector([0; 0x800]);
    // SAFETY: the buffer is writable, 32-byte aligned, and one complete DI sector.
    let c_read = unsafe { di::DI_ReadDVD(c_sector.0.as_mut_ptr().cast(), 1, 0) };
    logger.note_i32("di media C read result", c_read);
    logger.record(
        "di media C sector read matches fixture",
        c_read == rust_read && c_sector.0 == rust_sector.0,
    );
}

fn probe_usb_storage(logger: &mut ProbeLogger) {
    logger.section("USB ARGV");
    probe_usb_argv(logger);

    logger.section("RAW USB STORAGE");
    let init_rc = usb_storage::initialize();
    let startup = unsafe {
        usb_storage_sys::USB_STORAGE_INTERFACE
            .startup
            .is_some_and(|callback| callback())
    };
    let inserted = unsafe {
        usb_storage_sys::USB_STORAGE_INTERFACE
            .is_inserted
            .is_some_and(|callback| callback())
    };
    logger.note_i32("usb initialize rc", init_rc);
    logger.note_i32("usb startup", startup as i32);
    logger.note_i32("usb inserted", inserted as i32);
    logger.record("usb raw startup", startup);
    logger.record(
        "usb storage null handle rejected",
        // SAFETY: null deliberately exercises the API's invalid-handle path.
        unsafe { usb_storage::get_max_lun(null_mut()) } < 0,
    );

    let mut entries = [usb_core::UsbDeviceEntry::default(); 16];
    let (device_count, list_rc) = collect_usb_storage_candidates(&mut entries);
    logger.note_i32("usb candidate count", device_count as i32);
    if list_rc < 0 {
        logger.note_i32("usb storage list rc", list_rc);
    }
    let no_usb_device = !inserted && device_count == 0 && list_rc == 0;
    logger.record(
        "usb insertion state matches device enumeration",
        (inserted && device_count > 0) || no_usb_device,
    );

    if no_usb_device {
        logger.section("USB INTERFACE WITHOUT A DEVICE");
        probe_usb_interface_without_media(
            logger,
            "USB slot A callbacks reject reads and writes and restart cleanly",
            &usb_storage_sys::USB_STORAGE_INTERFACE,
        );
        probe_usb_interface_without_media(
            logger,
            "USB slot B callbacks reject reads and writes and restart cleanly",
            &usb_storage_sys::USB_STORAGE2_INTERFACE,
        );
    } else {
        logger.note("USB interface callback checks skipped: storage media is present");
    }

    if inserted {
        let mut sector0 = SectorBuf([0; RAW_USB_SECTOR_BUFFER_SIZE]);
        let read_sector0 = unsafe {
            usb_storage_sys::USB_STORAGE_INTERFACE
                .read_sectors
                .is_some_and(|callback| callback(0, 1, sector0.0.as_mut_ptr().cast::<c_void>()))
        };
        logger.note_i32("usb read sector0", read_sector0 as i32);
        logger.record("usb raw read sector0", read_sector0);
        if read_sector0 {
            let mut line = LineBuf::new();
            let _ = write!(
                &mut line,
                "sector0 bytes={:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
                sector0.0[0],
                sector0.0[1],
                sector0.0[2],
                sector0.0[3],
                sector0.0[4],
                sector0.0[5],
                sector0.0[6],
                sector0.0[7]
            );
            logger.note(line.as_str());
        }
    } else {
        logger.note("usb raw sector read skipped: no USB storage device is present");
    }

    logger.section("FAT USB MOUNT");
    let mount_result = fat::mount_usb();
    let mounted = matches!(mount_result, Ok(()) | Err(fat::FatError::AlreadyMounted));
    let owned_mount = mount_result.is_ok();
    log_fat_result(logger, "fat mount usb", &mount_result);
    if inserted {
        logger.record("usb fat mount", mounted);
    } else if no_usb_device {
        logger.note("usb FAT file operations skipped: no USB storage device is present");
        logger.record(
            "usb FAT mount reports no device",
            matches!(mount_result, Err(fat::FatError::DeviceUnavailable)),
        );
    } else {
        logger.record("usb storage state is consistent", false);
    }

    if owned_mount {
        logger.section("USB FILE I/O");
        let file_io = probe_usb_file_io();
        logger.note_i32("usb fopen write", file_io.write_open as i32);
        logger.note_i32("usb write callback result", file_io.write_rc as i32);
        logger.note_i32("usb write callback errno", file_io.write_errno);
        logger.note_i32("usb fsync", file_io.write_sync_rc);
        logger.note_i32("usb fsync errno", file_io.write_sync_errno);
        logger.note_i32("usb fclose write", file_io.write_close_rc);
        logger.note_i32("usb fclose write errno", file_io.write_close_errno);
        logger.note_i32("usb fopen read", file_io.read_open as i32);
        logger.note_i32("usb read callback result", file_io.read_rc as i32);
        logger.note_i32("usb read callback errno", file_io.read_errno);
        logger.note_i32("usb fclose read", file_io.read_close_rc);
        logger.note_i32("usb fclose read errno", file_io.read_close_errno);
        logger.note_i32("usb remove", file_io.remove_rc);
        if file_io.read_rc > 0 {
            let mut line = LineBuf::new();
            let _ = write!(&mut line, "usb read data=\"{}\"", file_io.read_back());
            logger.note(line.as_str());
        }
        logger.record("usb file write open", file_io.write_open);
        logger.record(
            "usb file write body",
            file_io.write_rc == USB_PROBE_MSG.len() as isize,
        );
        logger.record("usb file fsync", file_io.write_sync_rc == 0);
        logger.record("usb file write close", file_io.write_close_rc == 0);
        logger.record("usb file read open", file_io.read_open);
        logger.record("usb file roundtrip", file_io.matches(USB_PROBE_MSG));
        logger.record("usb file read close", file_io.read_close_rc == 0);
        logger.record("usb file remove", file_io.remove_rc == 0);
    } else if mounted {
        logger.note("usb file io skipped: mount existed before probe");
    }

    if logger.uses_usb_or_unknown_mounted_log() {
        logger.note("usb unmount skipped: mounted log active");
        logger.note("usb storage deinit skipped: mounted log active");
        return;
    }
    if owned_mount {
        let persistence_written = write_usb_close_probe(logger);
        logger.record("usb close probe write fsync close", persistence_written);
        let unmount_result = fat::unmount(USB_MOUNT_NAME);
        log_fat_result(logger, "fat unmount usb", &unmount_result);
        logger.record("usb fat unmount", unmount_result.is_ok());
        if unmount_result.is_err() {
            logger.note("usb deinitialize skipped: retryable unmount state retained");
            return;
        }
        let deinitialize_result = usb_storage::deinitialize();
        logger.note_i32("usb storage deinitialize", deinitialize_result);
        logger.record("usb storage deinitialize", deinitialize_result >= 0);
        if persistence_written && deinitialize_result >= 0 {
            probe_usb_close_persistence(logger);
        }
    } else if mounted {
        logger.note("usb unmount skipped: mount existed before probe");
        logger.note("usb storage deinit skipped: mount existed before probe");
    } else {
        let deinitialize_result = usb_storage::deinitialize();
        logger.note_i32("usb storage deinitialize", deinitialize_result);
        logger.record("usb storage deinitialize", deinitialize_result >= 0);
    }
}

fn probe_usb_interface_without_media(
    logger: &mut ProbeLogger,
    label: &str,
    interface: &usb_storage_sys::DiscInterface,
) {
    let Some(startup) = interface.startup else {
        logger.record(label, false);
        return;
    };
    let Some(is_inserted) = interface.is_inserted else {
        logger.record(label, false);
        return;
    };
    let Some(read_sectors) = interface.read_sectors else {
        logger.record(label, false);
        return;
    };
    let Some(write_sectors) = interface.write_sectors else {
        logger.record(label, false);
        return;
    };
    let Some(clear_status) = interface.clear_status else {
        logger.record(label, false);
        return;
    };
    let Some(shutdown) = interface.shutdown else {
        logger.record(label, false);
        return;
    };

    // SAFETY: The interface callbacks are static and the sector buffers are
    // aligned, writable or readable, and large enough for one sector.
    let first_startup = unsafe { startup() };
    // SAFETY: The static callback has no pointer inputs.
    let inserted_before_io = unsafe { is_inserted() };
    let mut read_buffer = UsbInterfaceSector([0xa5; 512]);
    // SAFETY: The buffer is aligned and holds one complete sector.
    let read_result = unsafe {
        read_sectors(
            0,
            1,
            read_buffer.0.as_mut_ptr().cast::<c_void>(),
        )
    };
    let write_buffer = UsbInterfaceSector([0x5a; 512]);
    // SAFETY: The buffer is aligned and holds one complete sector.
    let write_result = unsafe {
        write_sectors(0, 1, write_buffer.0.as_ptr().cast::<c_void>())
    };
    // SAFETY: The static callback has no pointer inputs.
    let clear_result = unsafe { clear_status() };
    // SAFETY: The static callback has no pointer inputs.
    let shutdown_result = unsafe { shutdown() };
    // SAFETY: The static callback has no pointer inputs.
    let restarted = unsafe { startup() };
    // SAFETY: The static callback has no pointer inputs.
    let inserted_after_restart = unsafe { is_inserted() };
    // SAFETY: The static callback has no pointer inputs.
    let final_shutdown = unsafe { shutdown() };

    logger.record(
        label,
        first_startup
            && !inserted_before_io
            && !read_result
            && read_buffer.0 == [0xa5; 512]
            && !write_result
            && clear_result
            && shutdown_result
            && restarted
            && !inserted_after_restart
            && final_shutdown,
    );
}

fn write_usb_close_probe(logger: &mut ProbeLogger) -> bool {
    let Some(file) = ProbeDeviceFile::open(
        USB_CLOSE_PROBE_PATH,
        sysbase::O_WRONLY | sysbase::O_CREAT | sysbase::O_EXCL,
    ) else {
        return false;
    };
    let written = file.write(USB_CLOSE_PROBE_MSG);
    let sync_result = file.fsync();
    let close_result = file.close();
    let completed = written.0 == USB_CLOSE_PROBE_MSG.len() as isize
        && sync_result.0 == 0
        && close_result.0 == 0;
    if !completed {
        let cleanup = unlink_device_file(USB_CLOSE_PROBE_PATH);
        logger.record("usb close probe setup cleanup", cleanup == 0);
    }
    completed
}

fn probe_usb_close_persistence(logger: &mut ProbeLogger) {
    let remount_result = fat::mount_usb();
    log_fat_result(logger, "usb close probe remount", &remount_result);
    logger.record("usb close probe remount", remount_result.is_ok());
    if remount_result.is_err() {
        return;
    }

    let mut read_back = [0u8; 64];
    let (read, close_result) = match ProbeDeviceFile::open(USB_CLOSE_PROBE_PATH, sysbase::O_RDONLY)
    {
        Some(file) => {
            let read = file.read(&mut read_back);
            logger.record("usb read-only file sync", file.fsync().0 == 0);
            let close_result = file.close();
            (read, close_result)
        }
        None => ((0, 0), (-1, 0)),
    };
    logger.record(
        "usb close probe persisted after remount",
        read.0 == USB_CLOSE_PROBE_MSG.len() as isize
            && &read_back[..USB_CLOSE_PROBE_MSG.len()] == USB_CLOSE_PROBE_MSG
            && close_result.0 == 0,
    );
    logger.record(
        "usb close probe remove",
        unlink_device_file(USB_CLOSE_PROBE_PATH) == 0,
    );
    let unmount_result = fat::unmount(USB_MOUNT_NAME);
    log_fat_result(logger, "usb close probe final unmount", &unmount_result);
    logger.record("usb close probe final unmount", unmount_result.is_ok());
    if unmount_result.is_err() {
        logger.note("usb final deinitialize skipped: retryable unmount state retained");
        return;
    }
    let deinitialize_result = usb_storage::deinitialize();
    logger.note_i32("usb close probe final deinitialize", deinitialize_result);
    logger.record(
        "usb close probe final deinitialize",
        deinitialize_result >= 0,
    );
}

fn probe_usb_argv(logger: &mut ProbeLogger) {
    let Some(argv) = (unsafe { sys::current_system_argv().as_ref() }) else {
        logger.note("argv handoff skipped: this launcher supplied no argument block");
        return;
    };
    logger.note_i32("argc", argv.argc);
    if argv.argc == 0 {
        logger.note("argv handoff skipped: this launch supplied no arguments");
        return;
    }
    if argv.argc < 0 || argv.argv.is_null() {
        logger.record("argv handoff", false);
        logger.note("argv0 unavailable");
        return;
    }
    let arg0_ptr = unsafe { *argv.argv };
    if arg0_ptr.is_null() {
        logger.record("argv handoff", false);
        logger.note("argv0 unavailable");
        return;
    }
    let arg0 = unsafe { CStr::from_ptr(arg0_ptr.cast::<c_char>()) };
    let arg0_bytes = arg0.to_bytes();
    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "argv0={}",
        core::str::from_utf8(arg0_bytes).unwrap_or("<non-utf8>")
    );
    logger.note(line.as_str());
    logger.note(if arg0_bytes.starts_with(b"usb:") {
        "launched_from=usb"
    } else if arg0_bytes.starts_with(b"sd:") {
        "launched_from=sd"
    } else {
        "launched_from=unknown"
    });
    logger.record("argv handoff", true);
}

fn log_fat_result(logger: &mut ProbeLogger, label: &str, result: &Result<(), fat::FatError>) {
    let status = match result {
        Ok(()) => "ok",
        Err(fat::FatError::AlreadyMounted) => "already-mounted",
        Err(fat::FatError::AlreadyExists) => "already-exists",
        Err(fat::FatError::Busy) => "busy",
        Err(fat::FatError::DeviceUnavailable) => "device-unavailable",
        Err(fat::FatError::MediaRemoved) => "media-removed",
        Err(fat::FatError::InvalidBootSector) => "invalid-boot-sector",
        Err(fat::FatError::UnsupportedVariant) => "unsupported-variant",
        Err(fat::FatError::PathError) => "path-error",
        Err(fat::FatError::Io) => "io",
        Err(fat::FatError::NotFound) => "not-found",
        Err(fat::FatError::NoSpace) => "no-space",
        Err(fat::FatError::ReadOnly) => "read-only",
        Err(fat::FatError::IsDirectory) => "is-directory",
        Err(fat::FatError::NotDirectory) => "not-directory",
        Err(fat::FatError::DirectoryNotEmpty) => "directory-not-empty",
        Err(fat::FatError::NotSupported) => "not-supported",
        Err(fat::FatError::PermissionDenied) => "permission-denied",
        Err(fat::FatError::CrossDevice) => "cross-device",
        Err(fat::FatError::BadDescriptor) => "bad-descriptor",
        Err(fat::FatError::FileTooLarge) => "file-too-large",
    };
    let mut line = LineBuf::new();
    let _ = write!(&mut line, "{}={}", label, status);
    logger.note(line.as_str());
}

struct ProbeDeviceFile {
    devoptab: *const sysbase::Devoptab,
    file_struct: *mut c_void,
    layout: Layout,
}

impl ProbeDeviceFile {
    fn open(path: &CStr, flags: i32) -> Option<Self> {
        let devoptab = sysbase::device_op_tab(path)?;
        let open_r = unsafe { (*devoptab).open_r? };
        let layout = Layout::from_size_align(unsafe { (*devoptab).struct_size }.max(1), 8).ok()?;
        let file_struct = unsafe { alloc_zeroed(layout) }.cast::<c_void>();
        if file_struct.is_null() {
            return None;
        }
        let rc = unsafe {
            open_r(
                sysbase::current_reent(),
                file_struct,
                path.as_ptr(),
                flags,
                0o666,
            )
        };
        if rc < 0 {
            unsafe { dealloc(file_struct.cast::<u8>(), layout) };
            return None;
        }
        Some(Self {
            devoptab,
            file_struct,
            layout,
        })
    }

    fn write(&self, bytes: &[u8]) -> (isize, i32) {
        let Some(write_r) = (unsafe { (*self.devoptab).write_r }) else {
            return (-1, sysbase::ENOTSUP);
        };
        let reent = sysbase::current_reent();
        unsafe { sysbase::set_reent_errno(reent, 0) };
        let rc = unsafe {
            write_r(
                reent,
                self.file_struct,
                bytes.as_ptr().cast::<c_char>(),
                bytes.len(),
            )
        };
        (rc, unsafe { (*reent)._errno })
    }

    fn read(&self, out: &mut [u8]) -> (isize, i32) {
        let Some(read_r) = (unsafe { (*self.devoptab).read_r }) else {
            return (-1, sysbase::ENOTSUP);
        };
        let reent = sysbase::current_reent();
        unsafe { sysbase::set_reent_errno(reent, 0) };
        let rc = unsafe {
            read_r(
                reent,
                self.file_struct,
                out.as_mut_ptr().cast::<c_char>(),
                out.len(),
            )
        };
        (rc, unsafe { (*reent)._errno })
    }

    fn fsync(&self) -> (i32, i32) {
        let Some(fsync_r) = (unsafe { (*self.devoptab).fsync_r }) else {
            return (-1, sysbase::ENOTSUP);
        };
        let reent = sysbase::current_reent();
        unsafe { sysbase::set_reent_errno(reent, 0) };
        let rc = unsafe { fsync_r(reent, self.file_struct) };
        (rc, unsafe { (*reent)._errno })
    }

    fn read_raw(&self, out: &mut [u8]) -> isize {
        let Some(read_r) = (unsafe { (*self.devoptab).read_r }) else {
            return -1;
        };
        unsafe {
            read_r(
                sysbase::current_reent(),
                self.file_struct,
                out.as_mut_ptr().cast::<c_char>(),
                out.len(),
            )
        }
    }

    fn close(mut self) -> (i32, i32) {
        let Some(close_r) = (unsafe { (*self.devoptab).close_r }) else {
            return (-1, sysbase::ENOTSUP);
        };
        let reent = sysbase::current_reent();
        unsafe { sysbase::set_reent_errno(reent, 0) };
        let rc = unsafe { close_r(reent, self.file_struct) };
        unsafe { dealloc(self.file_struct.cast::<u8>(), self.layout) };
        self.file_struct = null_mut();
        (rc, unsafe { (*reent)._errno })
    }
}

impl Drop for ProbeDeviceFile {
    fn drop(&mut self) {
        if self.file_struct.is_null() {
            return;
        }
        if let Some(close_r) = unsafe { (*self.devoptab).close_r } {
            let _ = unsafe { close_r(sysbase::current_reent(), self.file_struct) };
        }
        unsafe { dealloc(self.file_struct.cast::<u8>(), self.layout) };
        self.file_struct = null_mut();
    }
}

struct UsbFileIoResult {
    write_open: bool,
    write_rc: isize,
    write_errno: i32,
    write_sync_rc: i32,
    write_sync_errno: i32,
    write_close_rc: i32,
    write_close_errno: i32,
    read_open: bool,
    read_rc: isize,
    read_errno: i32,
    read_close_rc: i32,
    read_close_errno: i32,
    remove_rc: i32,
    read_back: [u8; 64],
}

impl UsbFileIoResult {
    fn matches(&self, expected: &[u8]) -> bool {
        self.read_rc == expected.len() as isize && &self.read_back[..expected.len()] == expected
    }

    fn read_back(&self) -> &str {
        let len = usize::try_from(self.read_rc.max(0))
            .unwrap_or(0)
            .min(self.read_back.len());
        core::str::from_utf8(&self.read_back[..len]).unwrap_or("")
    }
}

fn probe_usb_file_io() -> UsbFileIoResult {
    probe_file_io(USB_PROBE_PATH, USB_PROBE_MSG)
}

fn probe_sd_file_io() -> UsbFileIoResult {
    probe_file_io(SD_PROBE_PATH, SD_PROBE_MSG)
}

fn probe_file_io(path: &CStr, message: &[u8]) -> UsbFileIoResult {
    let mut result = UsbFileIoResult {
        write_open: false,
        write_rc: -1,
        write_errno: 0,
        write_sync_rc: -1,
        write_sync_errno: 0,
        write_close_rc: -1,
        write_close_errno: 0,
        read_open: false,
        read_rc: -1,
        read_errno: 0,
        read_close_rc: -1,
        read_close_errno: 0,
        remove_rc: -1,
        read_back: [0; 64],
    };

    if let Some(file) =
        ProbeDeviceFile::open(path, sysbase::O_WRONLY | sysbase::O_CREAT | sysbase::O_EXCL)
    {
        result.write_open = true;
        (result.write_rc, result.write_errno) = file.write(message);
        (result.write_sync_rc, result.write_sync_errno) = file.fsync();
        (result.write_close_rc, result.write_close_errno) = file.close();
    }

    if result.write_open {
        if let Some(file) = ProbeDeviceFile::open(path, sysbase::O_RDWR) {
            result.read_open = true;
            (result.read_rc, result.read_errno) = file.read(&mut result.read_back);
            (result.read_close_rc, result.read_close_errno) = file.close();
        }
        result.remove_rc = unlink_device_file(path);
    }

    result
}

fn unlink_device_file(path: &CStr) -> i32 {
    let Some(devoptab) = sysbase::device_op_tab(path) else {
        return -1;
    };
    unsafe {
        (*devoptab).unlink_r.map_or(-1, |unlink_r| {
            unlink_r(sysbase::current_reent(), path.as_ptr())
        })
    }
}

pub(super) fn collect_usb_storage_candidates(
    entries: &mut [usb_core::UsbDeviceEntry],
) -> (usize, i32) {
    let mut count = 0usize;
    let mut first_error = 0;

    for interface_class in [USB_CLASS_MASS_STORAGE, USB_CLASS_ANY] {
        let mut scratch = [usb_core::UsbDeviceEntry::default(); 16];
        match usb_core::device_list(&mut scratch, interface_class) {
            Ok(found) => {
                for entry in scratch.iter().copied().take(found) {
                    push_usb_candidate(entries, &mut count, entry);
                }
            }
            Err(err) if first_error == 0 => {
                first_error = err.raw();
            }
            Err(_) => {}
        }
    }

    let list_rc = if count == 0 { first_error } else { 0 };
    (count, list_rc)
}

fn push_usb_candidate(
    entries: &mut [usb_core::UsbDeviceEntry],
    count: &mut usize,
    entry: usb_core::UsbDeviceEntry,
) {
    if entry.vid == 0 || entry.pid == 0 || *count >= entries.len() {
        return;
    }
    if entries
        .iter()
        .take(*count)
        .any(|seen| same_usb_device(*seen, entry))
    {
        return;
    }
    entries[*count] = entry;
    *count += 1;
}

fn same_usb_device(a: usb_core::UsbDeviceEntry, b: usb_core::UsbDeviceEntry) -> bool {
    a.device_id == b.device_id && a.vid == b.vid && a.pid == b.pid
}

fn probe_sd_interface(logger: &mut ProbeLogger) {
    probe_disc_interface(logger, "wii sd", &wiisd::WII_SD_INTERFACE);
    #[cfg(not(feature = "dolphin-automation"))]
    {
        probe_disc_interface(logger, "gcsd a", &gcsd::GAMECUBE_SD_A_INTERFACE);
        probe_disc_interface(logger, "gcsd b", &gcsd::GAMECUBE_SD_B_INTERFACE);
        probe_disc_interface(logger, "gcsd 2", &gcsd::GAMECUBE_SD_2_INTERFACE);
    }
    #[cfg(feature = "dolphin-automation")]
    logger.note("GameCube SD interfaces skipped: no emulated EXI SD adapter configured");

    logger.section("FAT SD MOUNT");
    let sd_presence = fat::device_is_inserted(fat::MountSource::Sd);
    let mount_result = fat::mount_sd();
    let mounted = matches!(mount_result, Ok(()) | Err(fat::FatError::AlreadyMounted));
    let owned_mount = mount_result.is_ok();
    log_fat_result(logger, "fat mount sd", &mount_result);
    if sd_presence == Ok(true) {
        logger.record("sd fat mount", mounted);
    } else {
        let no_media = matches!(
            sd_presence,
            Ok(false) | Err(fat::FatError::DeviceUnavailable)
        );
        logger.record("sd FAT presence reports no inserted media", no_media);
        logger.record(
            "sd FAT mount reports unavailable without media",
            matches!(mount_result, Err(fat::FatError::DeviceUnavailable)),
        );
        if no_media {
            logger.note("SD file operations skipped: no Wii SD media is present");
        }
    }

    if owned_mount {
        let file_io = probe_sd_file_io();
        logger.note_i32("sd fopen write", file_io.write_open as i32);
        logger.note_i32("sd write callback result", file_io.write_rc as i32);
        logger.note_i32("sd write callback errno", file_io.write_errno);
        logger.note_i32("sd fsync", file_io.write_sync_rc);
        logger.note_i32("sd fsync errno", file_io.write_sync_errno);
        logger.note_i32("sd fclose write", file_io.write_close_rc);
        logger.note_i32("sd fclose write errno", file_io.write_close_errno);
        logger.note_i32("sd fopen read", file_io.read_open as i32);
        logger.note_i32("sd read callback result", file_io.read_rc as i32);
        logger.note_i32("sd read callback errno", file_io.read_errno);
        logger.note_i32("sd fclose read", file_io.read_close_rc);
        logger.note_i32("sd fclose read errno", file_io.read_close_errno);
        logger.note_i32("sd remove", file_io.remove_rc);
        logger.record(
            "sd file write",
            file_io.write_rc == SD_PROBE_MSG.len() as isize,
        );
        logger.record("sd file fsync", file_io.write_sync_rc == 0);
        logger.record("sd file roundtrip", file_io.matches(SD_PROBE_MSG));
        logger.record("sd file remove", file_io.remove_rc == 0);
    }

    if owned_mount && !logger.uses_mounted_log() {
        let unmount_result = fat::unmount(SD_MOUNT_NAME);
        log_fat_result(logger, "fat unmount sd", &unmount_result);
        logger.record("sd fat unmount", unmount_result.is_ok());
    }
}

fn probe_disc_interface(logger: &mut ProbeLogger, label: &str, io: &ogc_wiisd::DiscInterface) {
    let startup = unsafe { io.startup.is_some_and(|callback| callback()) };
    let inserted = unsafe { io.is_inserted.is_some_and(|callback| callback()) };

    let mut line = [0u8; 512];
    let read_ok = if inserted {
        unsafe {
            io.read_sectors
                .is_some_and(|callback| callback(0, 1, line.as_mut_ptr().cast::<c_void>()))
        }
    } else {
        false
    };
    let clear_ok = unsafe { io.clear_status.is_some_and(|callback| callback()) };
    let shutdown_ok = unsafe { io.shutdown.is_some_and(|callback| callback()) };
    let restart_ok = unsafe { io.startup.is_some_and(|callback| callback()) };
    let inserted_after_restart = unsafe { io.is_inserted.is_some_and(|callback| callback()) };
    let shutdown_after_restart = unsafe { io.shutdown.is_some_and(|callback| callback()) };

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "{} startup={} inserted={} read={} clear={} shutdown={} restart={} inserted2={} shutdown2={}",
        label,
        startup as u32,
        inserted as u32,
        read_ok as u32,
        clear_ok as u32,
        shutdown_ok as u32,
        restart_ok as u32,
        inserted_after_restart as u32,
        shutdown_after_restart as u32,
    );
    logger.note(line.as_str());
}

fn wait_card_owned_status(
    mut status: impl FnMut() -> card::CardAsyncStatus,
    timeout_seconds: u32,
) -> Option<i32> {
    let sleep = (lwp::TICKS_PER_SECOND / 100).max(1);
    for _ in 0..timeout_seconds.saturating_mul(100) {
        if let card::CardAsyncStatus::Complete(result) = status() {
            return Some(result);
        }
        lwp::sleep_ticks(sleep);
    }
    None
}

fn record_owned_card_async(
    logger: &mut ProbeLogger,
    label: &str,
    submit_result: i32,
    result: Option<i32>,
    success_result: i32,
) -> bool {
    let completed = result.is_some();
    let result_value = result.unwrap_or(i32::MIN);
    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "{} submit={} complete={} result={}",
        label, submit_result, completed as u32, result_value
    );
    logger.note(line.as_str());
    logger.record(&alloc::format!("{} accepted", label), submit_result >= 0);
    logger.record(&alloc::format!("{} completion", label), completed);
    logger.record(
        &alloc::format!("{} result", label),
        completed && result_value == success_result,
    );
    completed && result_value == success_result
}

fn mount_card_owned(
    logger: &mut ProbeLogger,
    slot: card::CardSlot,
    timeout_seconds: u32,
) -> Option<card::MountedCard> {
    let request = card::mount_async_owned(slot).ok()?;
    let request_owns_slot = request.slot() == slot;
    logger.record("card mount request retains its slot", request_owns_slot);
    if !request_owns_slot {
        return None;
    }
    wait_card_owned_status(|| request.status(), timeout_seconds)?;
    match request.try_complete().ok()? {
        card::CardMountCompletion::Mounted(mounted) => Some(mounted),
        card::CardMountCompletion::Failed(_) => None,
    }
}

fn record_card_unmount(logger: &mut ProbeLogger, label: &str, mounted: card::MountedCard) {
    logger.record(label, mounted.unmount() == card::CARD_ERROR_READY);
}

fn probe_card_invalid_channel_apis(logger: &mut ProbeLogger) {
    logger.section("CARD INVALID CHANNEL APIs");
    let invalid_channel = 3;

    let mut memory_size = 0x1234_5678;
    let mut sector_size = 0x2345_6789;
    // SAFETY: both output pointers refer to live, aligned, writable values.
    let sys_probe =
        unsafe { card::sys::probe_ex(invalid_channel, &mut memory_size, &mut sector_size) };
    logger.record(
        "CARD sys probe_ex rejects an invalid channel and preserves outputs",
        sys_probe == card::CARD_ERROR_FATAL_ERROR
            && memory_size == 0x1234_5678
            && sector_size == 0x2345_6789,
    );

    memory_size = 0x3456_789a;
    sector_size = 0x4567_89ab;
    // SAFETY: both output pointers refer to live, aligned, writable values.
    let wrapper_probe =
        unsafe { card::probe_ex(invalid_channel, &mut memory_size, &mut sector_size) };
    logger.record(
        "CARD wrapper probe_ex forwards the invalid-channel error",
        wrapper_probe == card::CARD_ERROR_FATAL_ERROR
            && memory_size == 0x3456_789a
            && sector_size == 0x4567_89ab,
    );

    logger.record(
        "CARD sys get_error_code rejects an invalid channel",
        card::sys::get_error_code(invalid_channel) == card::CARD_ERROR_NOCARD,
    );
    logger.record(
        "CARD wrapper get_error_code forwards the invalid-channel result",
        card::get_error_code(invalid_channel) == card::CARD_ERROR_NOCARD,
    );

    let mut sys_sector_size = 0x5678_9abc;
    // SAFETY: the output pointer refers to a live, aligned, writable value.
    let sector_result =
        unsafe { card::sys::get_sector_size(invalid_channel, &mut sys_sector_size) };
    logger.record(
        "CARD sys get_sector_size preserves output on invalid channel",
        sector_result == card::CARD_ERROR_NOCARD && sys_sector_size == 0x5678_9abc,
    );

    let mut sys_created_file = card::CardFile {
        filenum: 0x3456,
        ..card::CardFile::default()
    };
    // SAFETY: the static filename and file output are valid. An invalid
    // channel must reject creation before it changes the output or card data.
    let create_result = unsafe {
        card::sys::create(
            invalid_channel,
            c"INVALID".as_ptr(),
            card::CARD_READSIZE,
            &mut sys_created_file,
        )
    };
    logger.record(
        "CARD sys create rejects invalid channel and preserves output",
        create_result == card::CARD_ERROR_FATAL_ERROR && sys_created_file.filenum == 0x3456,
    );
    // SAFETY: the static filename is a valid NUL-terminated string. The
    // invalid channel must reject both calls before they can affect a file.
    let sys_delete = unsafe { card::sys::delete(invalid_channel, c"INVALID".as_ptr()) };
    // SAFETY: same valid filename and invalid-channel precondition.
    let wrapper_delete = unsafe { card::delete(invalid_channel, c"INVALID".as_ptr().cast()) };
    logger.record(
        "CARD sys and wrapper delete reject an invalid channel",
        sys_delete == card::CARD_ERROR_FATAL_ERROR
            && wrapper_delete == card::CARD_ERROR_FATAL_ERROR,
    );

    let mut block_count = 0x1234_5678;
    // SAFETY: the pointer refers to a live, aligned, writable value.
    let sys_block_count = unsafe { card::sys::get_block_count(invalid_channel, &mut block_count) };
    logger.record(
        "CARD sys get_block_count preserves output on invalid channel",
        sys_block_count == card::CARD_ERROR_NOCARD && block_count == 0x1234_5678,
    );

    block_count = 0x2345_6789;
    // SAFETY: the pointer refers to a live, aligned, writable value.
    let wrapper_block_count = unsafe { card::get_block_count(invalid_channel, &mut block_count) };
    logger.record(
        "CARD wrapper get_block_count forwards error and preserves output",
        wrapper_block_count == card::CARD_ERROR_NOCARD && block_count == 0x2345_6789,
    );

    let mut free_blocks = 0x1234;
    // SAFETY: the pointer refers to a live, aligned, writable value.
    let sys_free_blocks = unsafe { card::sys::get_free_blocks(invalid_channel, &mut free_blocks) };
    logger.record(
        "CARD sys get_free_blocks preserves output on invalid channel",
        sys_free_blocks == card::CARD_ERROR_NOCARD && free_blocks == 0x1234,
    );

    free_blocks = 0x2345;
    // SAFETY: the pointer refers to a live, aligned, writable value.
    let wrapper_free_blocks = unsafe { card::get_free_blocks(invalid_channel, &mut free_blocks) };
    logger.record(
        "CARD wrapper get_free_blocks forwards error and preserves output",
        wrapper_free_blocks == card::CARD_ERROR_NOCARD && free_blocks == 0x2345,
    );

    let mut serial_first = 0x1234_5678;
    let mut serial_second = 0x2345_6789;
    // SAFETY: both pointers refer to live, aligned, writable values.
    let sys_serial =
        unsafe { card::sys::get_serial_no(invalid_channel, &mut serial_first, &mut serial_second) };
    logger.record(
        "CARD sys get_serial_no preserves outputs on invalid channel",
        sys_serial == card::CARD_ERROR_NOCARD
            && serial_first == 0x1234_5678
            && serial_second == 0x2345_6789,
    );

    serial_first = 0x3456_789a;
    serial_second = 0x4567_89ab;
    // SAFETY: both pointers refer to live, aligned, writable values.
    let wrapper_serial =
        unsafe { card::get_serial_no(invalid_channel, &mut serial_first, &mut serial_second) };
    logger.record(
        "CARD wrapper get_serial_no forwards error and preserves outputs",
        wrapper_serial == card::CARD_ERROR_NOCARD
            && serial_first == 0x3456_789a
            && serial_second == 0x4567_89ab,
    );

    let mut stats = card::CardStat {
        len: 0x1234_5678,
        ..card::CardStat::default()
    };
    // SAFETY: `stats` is a live, aligned, writable output.
    let status = unsafe { card::sys::get_status(invalid_channel, 0, &mut stats) };
    logger.record(
        "CARD sys get_status rejects an invalid channel and preserves output",
        status == card::CARD_ERROR_NOCARD && stats.len == 0x1234_5678,
    );
    stats.len = 0x2345_6789;
    // SAFETY: `stats` is a live, aligned, writable output.
    let wrapper_status = unsafe { card::get_status(invalid_channel, 0, &mut stats) };
    logger.record(
        "CARD wrapper get_status forwards the error and preserves output",
        wrapper_status == card::CARD_ERROR_NOCARD && stats.len == 0x2345_6789,
    );

    let mut entry = card::sys::CardDirEntry {
        last_modified: 0x1234_5678,
        ..card::sys::CardDirEntry::default()
    };
    // SAFETY: `entry` is a live, aligned, writable output.
    let status_ex = unsafe { card::sys::get_status_ex(invalid_channel, 0, &mut entry) };
    logger.record(
        "CARD sys get_status_ex rejects an invalid channel and preserves output",
        status_ex == card::CARD_ERROR_FATAL_ERROR && entry.last_modified == 0x1234_5678,
    );

    let mut attributes = 0xa5;
    // SAFETY: `attributes` is a live, aligned, writable output.
    let attribute_status =
        unsafe { card::sys::get_attributes(invalid_channel, 0, &mut attributes) };
    logger.record(
        "CARD sys get_attributes rejects an invalid channel and preserves output",
        attribute_status == card::CARD_ERROR_FATAL_ERROR && attributes == 0xa5,
    );

    // SAFETY: `stats` is initialized and remains live for this call.
    let set_status = unsafe { card::sys::set_status(invalid_channel, 0, &mut stats) };
    logger.record(
        "CARD sys set_status rejects an invalid channel without changing input",
        set_status == card::CARD_ERROR_NOCARD && stats.len == 0x2345_6789,
    );
    // SAFETY: `entry` is initialized and remains live for this call.
    let set_status_ex = unsafe { card::sys::set_status_ex(invalid_channel, 0, &mut entry) };
    logger.record(
        "CARD sys set_status_ex rejects an invalid channel without changing input",
        set_status_ex == card::CARD_ERROR_NOCARD && entry.last_modified == 0x1234_5678,
    );
    logger.record(
        "CARD sys set_attributes rejects an invalid channel",
        card::sys::set_attributes(invalid_channel, 0, 0) == card::CARD_ERROR_NOCARD,
    );

    let mut directories = [card::CardDir::default(); 1];
    directories[0].fileno = 0x1234;
    let mut directory_count = 0x5678;
    // SAFETY: `directories` and `directory_count` are live writable outputs.
    let directory_status = unsafe {
        card::sys::get_directory(
            invalid_channel,
            directories.as_mut_ptr(),
            &mut directory_count,
            0,
        )
    };
    logger.record(
        "CARD sys get_directory rejects an invalid channel and preserves outputs",
        directory_status == card::CARD_ERROR_NOCARD
            && directories[0].fileno == 0x1234
            && directory_count == 0x5678,
    );

    let mut search = card::CardDir {
        fileno: 4,
        ..card::CardDir::default()
    };
    // SAFETY: `search` is a live, aligned, writable directory cursor.
    let first_status = unsafe { card::sys::find_first(invalid_channel, &mut search, 0) };
    logger.record(
        "CARD sys find_first initializes its cursor before an invalid-channel error",
        first_status == card::CARD_ERROR_NOCARD
            && search.chn == invalid_channel
            && search.fileno == 0
            && search.showall == 0,
    );
    search.fileno = 4;
    // SAFETY: `search` contains an invalid channel and remains live.
    let next_status = unsafe { card::sys::find_next(&mut search) };
    logger.record(
        "CARD sys find_next advances its cursor before an invalid-channel error",
        next_status == card::CARD_ERROR_NOCARD && search.fileno == 5,
    );

    let mut workarea = Workarea([0; card::CARD_WORKAREA_SIZE as usize]);
    CARD_INVALID_CHANNEL_CALLBACKS.store(0, Ordering::Release);
    // SAFETY: the work area is aligned, writable, and remains live; invalid
    // channel validation must reject the request before it retains the pointer.
    let mount_async = unsafe {
        card::sys::mount_async(
            invalid_channel,
            workarea.0.as_mut_ptr().cast(),
            Some(card_invalid_channel_callback),
            Some(card_invalid_channel_callback),
        )
    };
    let wrapper_mount_async = unsafe {
        card::mount_async(
            invalid_channel,
            workarea.0.as_mut_ptr().cast(),
            Some(card_invalid_channel_callback),
            Some(card_invalid_channel_callback),
        )
    };
    logger.record(
        "CARD sys and wrapper mount_async reject invalid channels without callbacks",
        mount_async == card::CARD_ERROR_FATAL_ERROR
            && wrapper_mount_async == card::CARD_ERROR_FATAL_ERROR
            && CARD_INVALID_CHANNEL_CALLBACKS.load(Ordering::Acquire) == 0,
    );

    let mut output_file = card::CardFile {
        filenum: 0x1234,
        ..card::CardFile::default()
    };
    CARD_INVALID_CHANNEL_CALLBACKS.store(0, Ordering::Release);
    // SAFETY: the static filename and output file are valid for the call.
    let create_async = unsafe {
        card::sys::create_async(
            invalid_channel,
            c"INVALID".as_ptr(),
            0x2000,
            &mut output_file,
            Some(card_invalid_channel_callback),
        )
    };
    logger.record(
        "CARD sys create_async rejects an invalid channel without callback or output changes",
        create_async == card::CARD_ERROR_FATAL_ERROR
            && output_file.filenum == 0x1234
            && CARD_INVALID_CHANNEL_CALLBACKS.load(Ordering::Acquire) == 0,
    );

    let mut entry_file = card::CardFile {
        filenum: 0x2345,
        ..card::CardFile::default()
    };
    let mut create_entry = card::CardDir {
        filelen: 0x2000,
        ..card::CardDir::default()
    };
    create_entry.filename[..7].copy_from_slice(b"INVALID");
    // SAFETY: both objects are live and writable; invalid channel validation
    // must reject these calls before any CARD state changes.
    let create_entry_rc =
        unsafe { card::sys::create_entry(invalid_channel, &mut create_entry, &mut entry_file) };
    let delete_entry_rc = unsafe { card::sys::delete_entry(invalid_channel, &mut create_entry) };
    logger.record(
        "CARD sys create_entry and delete_entry reject invalid channels",
        create_entry_rc == card::CARD_ERROR_FATAL_ERROR
            && delete_entry_rc == card::CARD_ERROR_NOCARD
            && entry_file.filenum == 0x2345,
    );

    let mut open_entry_file = card::CardFile {
        filenum: 0x3456,
        ..card::CardFile::default()
    };
    // SAFETY: the directory entry and file output are valid and remain live.
    let open_entry_rc =
        unsafe { card::sys::open_entry(invalid_channel, &mut create_entry, &mut open_entry_file) };
    logger.record(
        "CARD sys open_entry rejects an invalid channel and preserves output",
        open_entry_rc == card::CARD_ERROR_NOCARD && open_entry_file.filenum == 0x3456,
    );

    let mut invalid_channel_file = card::CardFile {
        chn: invalid_channel,
        filenum: 0,
        ..card::CardFile::default()
    };
    let mut transfer_buffer = DvdSector([0; 0x800]);
    CARD_INVALID_CHANNEL_CALLBACKS.store(0, Ordering::Release);
    // SAFETY: the file and aligned transfer buffer remain live; its invalid
    // channel must reject the operations before retaining either pointer.
    let read_async_rc = unsafe {
        card::sys::read_async(
            &mut invalid_channel_file,
            transfer_buffer.0.as_mut_ptr().cast(),
            0x200,
            0,
            Some(card_invalid_channel_callback),
        )
    };
    let write_async_rc = unsafe {
        card::sys::write_async(
            &mut invalid_channel_file,
            transfer_buffer.0.as_mut_ptr().cast(),
            0x200,
            0,
            Some(card_invalid_channel_callback),
        )
    };
    logger.record(
        "CARD sys read_async and write_async reject invalid channels without callbacks",
        read_async_rc == card::CARD_ERROR_FATAL_ERROR
            && write_async_rc == card::CARD_ERROR_FATAL_ERROR
            && CARD_INVALID_CHANNEL_CALLBACKS.load(Ordering::Acquire) == 0
            && invalid_channel_file.chn == invalid_channel
            && invalid_channel_file.filenum == 0,
    );

    CARD_INVALID_CHANNEL_CALLBACKS.store(0, Ordering::Release);
    // SAFETY: all operation objects are valid; the invalid channel must stop
    // these submissions before any callback or payload ownership begins.
    let create_entry_async = unsafe {
        card::sys::create_entry_async(
            invalid_channel,
            &mut search,
            &mut output_file,
            Some(card_invalid_channel_callback),
        )
    };
    let delete_async = unsafe {
        card::sys::delete_async(
            invalid_channel,
            c"INVALID".as_ptr(),
            Some(card_invalid_channel_callback),
        )
    };
    let delete_entry_async = unsafe {
        card::sys::delete_entry_async(
            invalid_channel,
            &mut search,
            Some(card_invalid_channel_callback),
        )
    };
    let format_async =
        card::sys::format_async(invalid_channel, Some(card_invalid_channel_callback));
    let set_status_async = unsafe {
        card::sys::set_status_async(
            invalid_channel,
            0,
            &mut stats,
            Some(card_invalid_channel_callback),
        )
    };
    let set_attributes_async =
        card::sys::set_attributes_async(invalid_channel, 0, 0, Some(card_invalid_channel_callback));
    logger.record(
        "CARD sys metadata async submissions reject invalid channels without callbacks",
        create_entry_async == card::CARD_ERROR_FATAL_ERROR
            && delete_async == card::CARD_ERROR_FATAL_ERROR
            && delete_entry_async == card::CARD_ERROR_FATAL_ERROR
            && format_async == card::CARD_ERROR_NOCARD
            && set_status_async == card::CARD_ERROR_NOCARD
            && set_attributes_async == card::CARD_ERROR_FATAL_ERROR
            && output_file.filenum == 0x1234
            && stats.len == 0x2345_6789
            && CARD_INVALID_CHANNEL_CALLBACKS.load(Ordering::Acquire) == 0,
    );

    let slot = card::CardSlot::A;
    let exi_base = ogc_exi::hardware_exi_reg_base();
    // SAFETY: This reads the EXI channel 0 status register without modifying it.
    let exi_status = unsafe { core::ptr::read_volatile(exi_base as *const u32) };
    logger.note_hex32("CARD EXI channel 0 status before probe", exi_status);
    let probe_result = card::probe_slot(slot);
    let status_before_probe = card::sys::get_error_code(slot.channel());
    let info_result = card::probe_info(slot);
    let status_after_probe = card::sys::get_error_code(slot.channel());
    logger.note_i32("CARD raw slot probe result", probe_result);
    logger.note_i32(
        "CARD typed probe_info result",
        info_result
            .as_ref()
            .map_or_else(|error| *error, |_| card::CARD_ERROR_READY),
    );
    logger.record(
        "CARD probe returns an EXI presence value and rejects an invalid channel",
        (0..=1).contains(&probe_result) && card::probe(-1) == ogc_exi::probe(-1),
    );
    let info_valid = match info_result {
        Ok(info) => {
            info.memory_size > 0 && info.sector_size > 0 && info.memory_size % info.sector_size == 0
        }
        Err(error) => matches!(
            error,
            card::CARD_ERROR_BUSY | card::CARD_ERROR_NOCARD | card::CARD_ERROR_WRONGDEVICE
        ),
    };
    logger.record(
        "CARD probe_info returns valid geometry or a device error",
        info_valid,
    );
    logger.record(
        "CARD probe_info preserves the operation status",
        status_before_probe == status_after_probe,
    );
    logger.record(
        "CARD serial_number rejects an unmounted slot",
        card::serial_number(slot) == Err(card::CARD_ERROR_NOCARD),
    );
    let unavailable = matches!(
        info_result,
        Err(card::CARD_ERROR_BUSY | card::CARD_ERROR_NOCARD)
    );
    if unavailable {
        let sector_size_result = card::sector_size(slot);
        let block_count_result = card::block_count(slot);
        let free_blocks_result = card::free_blocks(slot);
        let file_status_result = card::file_status(slot, 0);
        record_card_no_media_query(
            logger,
            "CARD sector_size status",
            "CARD sector_size returns no-card for unavailable media",
            sector_size_result,
        );
        record_card_no_media_query(
            logger,
            "CARD block_count status",
            "CARD block_count returns no-card for unavailable media",
            block_count_result,
        );
        record_card_no_media_query(
            logger,
            "CARD free_blocks status",
            "CARD free_blocks returns no-card for unavailable media",
            free_blocks_result,
        );
        record_card_no_media_query(
            logger,
            "CARD file_status status",
            "CARD file_status returns no-card for unavailable media",
            file_status_result,
        );
    }
}

fn record_card_no_media_query<T>(
    logger: &mut ProbeLogger,
    status_label: &str,
    check_label: &str,
    result: Result<T, i32>,
) {
    let status = result.map_or_else(|error| error, |_| card::CARD_ERROR_READY);
    logger.note_i32(status_label, status);
    logger.record(check_label, status == card::CARD_ERROR_NOCARD);
}

fn probe_card_save_data(logger: &mut ProbeLogger) {
    logger.section("CARD ASYNC + DETACH");

    let gamecode = match card::CardGameCode::new(*b"RUST") {
        Ok(value) => value,
        Err(_) => {
            logger.record("card game identifier validation", false);
            return;
        }
    };
    let company = match card::CardCompanyCode::new(*b"RS") {
        Ok(value) => value,
        Err(_) => {
            logger.record("card company identifier validation", false);
            return;
        }
    };
    logger.record("card identifier validation", true);
    logger.record(
        "CARD slot conversion validates both slots and preserves invalid input",
        card::CardSlot::A.channel() == card::CARD_SLOTA
            && card::CardSlot::B.channel() == card::CARD_SLOTB
            && card::CardSlot::try_from(card::CARD_SLOTA) == Ok(card::CardSlot::A)
            && card::CardSlot::try_from(card::CARD_SLOTB) == Ok(card::CardSlot::B)
            && card::CardSlot::try_from(3) == Err(card::InvalidCardSlot(3)),
    );
    let init = card::sys::init(Some(gamecode), Some(company));
    logger.note_i32("card init", init);
    let slot = card::CardSlot::A;
    probe_card_invalid_channel_apis(logger);

    let mut invalid_file = card::CardFile::default();
    let mut empty_output = [];
    let empty_read = card::read_slice(&mut invalid_file, &mut empty_output, 0);
    let mut short_output = [0xa5; 1];
    let short_read = card::read_slice(&mut invalid_file, &mut short_output, 0);
    let transfer_len = card::CARD_READSIZE as usize;
    let mut transfer_output = alloc::vec![0xa5; transfer_len];
    let transfer_input = alloc::vec![0x5a; transfer_len];
    let misaligned_read = card::read_slice(&mut invalid_file, &mut transfer_output, 1);
    let overflowing_offset = u32::MAX - (u32::MAX % card::CARD_READSIZE);
    let overflowing_read =
        card::read_slice(&mut invalid_file, &mut transfer_output, overflowing_offset);
    let empty_write = card::write_slice(&mut invalid_file, &[], 0);
    let short_write = card::write_slice(&mut invalid_file, &[0x5a], 0);
    let misaligned_write = card::write_slice(&mut invalid_file, &transfer_input, 1);
    let overflowing_write =
        card::write_slice(&mut invalid_file, &transfer_input, overflowing_offset);
    logger.record(
        "CARD slice transfers reject empty, short, misaligned, and overflowing requests before I/O",
        empty_read == card::CARD_ERROR_FATAL_ERROR
            && short_read == card::CARD_ERROR_FATAL_ERROR
            && misaligned_read == card::CARD_ERROR_FATAL_ERROR
            && overflowing_read == card::CARD_ERROR_FATAL_ERROR
            && empty_write == card::CARD_ERROR_FATAL_ERROR
            && short_write == card::CARD_ERROR_FATAL_ERROR
            && misaligned_write == card::CARD_ERROR_FATAL_ERROR
            && overflowing_write == card::CARD_ERROR_FATAL_ERROR
            && short_output == [0xa5]
            && transfer_output.iter().all(|byte| *byte == 0xa5),
    );

    let invalid_owned_file = card::CardFile {
        chn: 3,
        filenum: 0x3456,
        ..card::CardFile::default()
    };
    let read_invalid_channel =
        match card::read_async_owned(invalid_owned_file, card::CARD_READSIZE as usize, 0) {
            Err(error) => {
                error.status() == card::CARD_ERROR_NOCARD
                    && error.file().chn == invalid_owned_file.chn
                    && error.file().filenum == invalid_owned_file.filenum
            }
            Ok(request) => {
                drop(request);
                false
            }
        };
    logger.record(
        "CARD read_async_owned rejects an invalid channel and returns its file",
        read_invalid_channel,
    );
    let invalid_write_bytes = alloc::vec![0xa5; card::CARD_READSIZE as usize];
    let write_invalid_channel =
        match card::write_async_owned(invalid_owned_file, invalid_write_bytes.clone(), 0) {
            Err(error) => {
                let (status, file, bytes) = error.into_parts();
                status == card::CARD_ERROR_NOCARD
                    && file.chn == invalid_owned_file.chn
                    && file.filenum == invalid_owned_file.filenum
                    && bytes == invalid_write_bytes
            }
            Ok(request) => {
                drop(request);
                false
            }
        };
    logger.record(
        "CARD write_async_owned rejects an invalid channel and returns file and bytes",
        write_invalid_channel,
    );

    let invalid_transfer_shapes = [
        (0usize, 0u32),
        (1usize, 0u32),
        (card::CARD_READSIZE as usize, 1u32),
        (card::CARD_READSIZE as usize, overflowing_offset),
    ];
    let mut read_rejections_preserve_files = true;
    let mut write_rejections_preserve_payloads = true;
    for (case, (len, offset)) in invalid_transfer_shapes.into_iter().enumerate() {
        let file = card::CardFile {
            chn: card::CARD_SLOTA,
            filenum: 0x4000 + case as i32,
            offset: 0x1234,
            len: 0x2345,
            iblock: 0x3456,
        };
        match card::read_async_owned(file, len, offset) {
            Err(error) => {
                read_rejections_preserve_files &= error.status() == card::CARD_ERROR_FATAL_ERROR
                    && error.file().chn == file.chn
                    && error.file().filenum == file.filenum
                    && error.file().offset == file.offset
                    && error.file().len == file.len
                    && error.file().iblock == file.iblock;
            }
            Ok(request) => {
                drop(request);
                read_rejections_preserve_files = false;
            }
        }

        let bytes = alloc::vec![0x5a; len];
        let expected_bytes = bytes.clone();
        match card::write_async_owned(file, bytes, offset) {
            Err(error) => {
                let (status, returned_file, returned_bytes) = error.into_parts();
                write_rejections_preserve_payloads &= status == card::CARD_ERROR_FATAL_ERROR
                    && returned_file.chn == file.chn
                    && returned_file.filenum == file.filenum
                    && returned_file.offset == file.offset
                    && returned_file.len == file.len
                    && returned_file.iblock == file.iblock
                    && returned_bytes == expected_bytes;
            }
            Ok(request) => {
                drop(request);
                write_rejections_preserve_payloads = false;
            }
        }
    }
    logger.record(
        "CARD read_async_owned rejects invalid transfer shapes and preserves files",
        read_rejections_preserve_files,
    );
    logger.record(
        "CARD write_async_owned rejects invalid transfer shapes and returns payloads",
        write_rejections_preserve_payloads,
    );

    let nul_filename = [b'A', 0, b'B'];
    let oversized_filename = [b'F'; card::CARD_FILENAMELEN + 1];
    logger.record(
        "CARD owned create and delete reject embedded NUL and oversized filenames",
        matches!(
            card::create_async_owned(slot, &nul_filename, card::CARD_READSIZE),
            Err(card::CARD_ERROR_FATAL_ERROR)
        ) && matches!(
            card::create_async_owned(slot, &oversized_filename, card::CARD_READSIZE),
            Err(card::CARD_ERROR_NAMETOOLONG)
        ) && matches!(
            card::delete_async_owned(slot, &nul_filename),
            Err(card::CARD_ERROR_FATAL_ERROR)
        ) && matches!(
            card::delete_async_owned(slot, &oversized_filename),
            Err(card::CARD_ERROR_NAMETOOLONG)
        ),
    );

    let mut invalid_entry = card::CardDir {
        filelen: 0x2345,
        fileno: 0x3456,
        ..card::CardDir::default()
    };
    invalid_entry.filename.fill(b'X');
    let create_entry_rejects_name = match card::create_entry_async_owned(slot, invalid_entry) {
        Err(error) => {
            error.status() == card::CARD_ERROR_NAMETOOLONG
                && error.value().filelen == invalid_entry.filelen
                && error.value().fileno == invalid_entry.fileno
                && error.value().filename == invalid_entry.filename
        }
        Ok(request) => {
            drop(request);
            false
        }
    };
    let delete_entry_rejects_name = match card::delete_entry_async_owned(slot, invalid_entry) {
        Err(error) => {
            error.status() == card::CARD_ERROR_NAMETOOLONG
                && error.value().filelen == invalid_entry.filelen
                && error.value().fileno == invalid_entry.fileno
                && error.value().filename == invalid_entry.filename
        }
        Ok(request) => {
            drop(request);
            false
        }
    };
    logger.record(
        "CARD owned entry operations reject unterminated names and return entries",
        create_entry_rejects_name && delete_entry_rejects_name,
    );

    let invalid_status = card::CardStat {
        len: 0x1234_5678,
        ..card::CardStat::default()
    };
    let negative_status_fileno = match card::set_status_async_owned(slot, -1, invalid_status) {
        Err(error) => {
            error.status() == card::CARD_ERROR_FATAL_ERROR && error.value().len == 0x1234_5678
        }
        Ok(request) => {
            drop(request);
            false
        }
    };
    let upper_status_fileno =
        match card::set_status_async_owned(slot, card::CARD_MAXFILES as i32, invalid_status) {
            Err(error) => {
                error.status() == card::CARD_ERROR_FATAL_ERROR && error.value().len == 0x1234_5678
            }
            Ok(request) => {
                drop(request);
                false
            }
        };
    logger.record(
        "CARD set_status_async_owned rejects out-of-range file numbers and returns stat",
        negative_status_fileno && upper_status_fileno,
    );

    let mut present = card::probe_slot(slot);
    for _ in 0..10 {
        if present == 1 {
            break;
        }
        lwp::sleep_ticks((lwp::TICKS_PER_SECOND / 100).max(1));
        present = card::probe_slot(slot);
    }
    logger.note_i32("CARD slot probe after bounded retries", present);
    if present != 1 {
        logger.note("card tests skipped: insert a memory card in slot A");
        return;
    }
    let geometry = card::probe_info(slot);
    logger.note_i32(
        "CARD probe_info after bounded retries",
        geometry
            .as_ref()
            .map_or_else(|error| *error, |_| card::CARD_ERROR_READY),
    );
    let probe_gamecode = match card::CardGameCode::new(*b"TST1") {
        Ok(value) => value,
        Err(_) => {
            logger.record("card probe game identifier validation", false);
            return;
        }
    };
    let probe_company = match card::CardCompanyCode::new(*b"TS") {
        Ok(value) => value,
        Err(_) => {
            logger.record("card probe company identifier validation", false);
            return;
        }
    };
    let _identifier_reset = CardIdentifierReset { gamecode, company };
    card::sys::set_gamecode(Some(probe_gamecode));
    card::sys::set_company(Some(probe_company));

    prepare_async_probe();
    let mount_request = card::mount_async_owned(slot);
    let request_slot = mount_request
        .as_ref()
        .ok()
        .map(card::CardMountRequest::slot);
    logger.record(
        "card mount request retains its slot",
        request_slot == Some(slot),
    );
    let mount_submit = mount_request
        .as_ref()
        .map_or_else(|status| *status, |_| card::CARD_ERROR_READY);
    mark_async_returned();
    let mount_result = mount_request
        .as_ref()
        .ok()
        .and_then(|request| wait_card_owned_status(|| request.status(), 10));
    let mounted = match (mount_request, mount_result) {
        (Ok(request), Some(_)) => match request.try_complete() {
            Ok(card::CardMountCompletion::Mounted(mounted)) => Some(mounted),
            Ok(card::CardMountCompletion::Failed(_)) | Err(_) => None,
        },
        _ => None,
    };
    if mounted.is_none() && mount_result == Some(card::CARD_ERROR_NOCARD) {
        let no_card_checks_passed = record_owned_card_async(
            logger,
            "card mount async owned no-card",
            mount_submit,
            mount_result,
            card::CARD_ERROR_NOCARD,
        );
        logger.note(if no_card_checks_passed {
            "CARD save operations skipped: asynchronous mount reports no card"
        } else {
            "CARD save operations not run: no-card completion checks failed"
        });
        return;
    }
    if !record_owned_card_async(
        logger,
        "card mount async owned",
        mount_submit,
        mount_result,
        card::CARD_ERROR_READY,
    ) || mounted.is_none()
    {
        return;
    }
    let mounted = mounted.unwrap();
    logger.record(
        "card mounted owner retains its slot",
        mounted.slot() == slot,
    );
    logger.record("card mounted owner reports attached", mounted.is_attached());
    logger.record(
        "card mounted owner has no detach result before removal",
        mounted.detach_result().is_none(),
    );
    logger.record(
        "CARD owned duplicate mount returns busy",
        matches!(card::mount_async_owned(slot), Err(card::CARD_ERROR_BUSY)),
    );
    logger.record(
        "CARD owned mount remains attached after duplicate request",
        mounted.is_attached(),
    );

    let (sector_rc, sector_size) = match card::sector_size(slot) {
        Ok(value) => (card::CARD_ERROR_READY, value),
        Err(status) => (status, 0),
    };
    logger.note_i32("card sector size rc", sector_rc);
    logger.note_u32("card sector size", sector_size);
    if sector_rc != card::CARD_ERROR_READY || sector_size < card::CARD_READSIZE {
        logger.record("card sector size supports transfers", false);
        logger.record(
            "card unmount after invalid sector size",
            mounted.unmount() == card::CARD_ERROR_READY,
        );
        return;
    }
    let mut existing_save = card::CardFile::default();
    // SAFETY: the filename is static and `existing_save` is a unique output.
    let existing_save_status =
        unsafe { card::open(slot.channel(), c"SAVE".as_ptr().cast(), &mut existing_save) };
    if existing_save_status == card::CARD_ERROR_READY {
        // SAFETY: `existing_save` is live and not used by another operation.
        let close = unsafe { card::sys::close(&mut existing_save) };
        logger.record(
            "card existing SAVE handle cleanup",
            close == card::CARD_ERROR_READY,
        );
        logger.note("card save checks skipped: SAVE already exists; existing data was preserved");
        let unmount = mounted.unmount();
        logger.record(
            "card unmount after existing SAVE",
            unmount == card::CARD_ERROR_READY,
        );
        return;
    }
    if existing_save_status != card::CARD_ERROR_NOFILE {
        logger.note_i32("card SAVE preflight status", existing_save_status);
        logger.note("card save checks skipped: SAVE preflight did not prove the name is free");
        let unmount = mounted.unmount();
        logger.record(
            "card unmount after SAVE preflight",
            unmount == card::CARD_ERROR_READY,
        );
        return;
    }
    logger.record("card SAVE preflight found no existing file", true);
    let (freeblocks_ok, freeblocks) = match card::free_blocks(slot) {
        Ok(value) => (true, value),
        Err(_) => (false, 0),
    };
    logger.record("card free blocks before create", freeblocks_ok);
    logger.note_u32("card free block count before create", freeblocks as u32);
    let create_request = card::create_async_owned(slot, b"SAVE", sector_size.max(0x2000));
    let create_request_slot = create_request.as_ref().ok().map(|request| request.slot());
    logger.record(
        "card create request retains its slot",
        create_request_slot == Some(slot),
    );
    let create_submit = create_request
        .as_ref()
        .map_or_else(|status| *status, |_| card::CARD_ERROR_READY);
    let create_result = create_request
        .as_ref()
        .ok()
        .and_then(|request| wait_card_owned_status(|| request.status(), 15));
    let create_completion = match (create_request, create_result) {
        (Ok(request), Some(_)) => request.try_complete().ok(),
        _ => None,
    };
    let create_ok = record_owned_card_async(
        logger,
        "card create async owned",
        create_submit,
        create_result,
        card::CARD_ERROR_READY,
    );
    let Some(mut file) = create_completion.and_then(|completion| completion.into_result().ok())
    else {
        record_card_unmount(
            logger,
            "card unmount after failed create completion",
            mounted,
        );
        return;
    };
    if !create_ok {
        // SAFETY: the completed create returned a live, uniquely owned file handle.
        let close = unsafe { card::sys::close(&mut file) };
        logger.record(
            "card close after failed create result",
            close == card::CARD_ERROR_READY,
        );
        record_card_unmount(logger, "card unmount after failed create result", mounted);
        return;
    }
    logger.note_i32("card SAVE file channel after create", file.chn);
    logger.note_i32("card SAVE file number after create", file.filenum);

    let status_marker = card::CARD_BANNER_RGB;
    let status_request = card::file_status(slot, file.filenum)
        .ok()
        .and_then(|mut stat| {
            stat.banner_fmt = status_marker;
            card::set_status_async_owned(slot, file.filenum, stat).ok()
        });
    let status_result = status_request
        .as_ref()
        .and_then(|request| wait_card_owned_status(|| request.status(), 10));
    let status_completion = match (status_request, status_result) {
        (Some(request), Some(_)) => request.try_complete().ok(),
        _ => None,
    };
    logger.record(
        "card set status async owned",
        status_result == Some(card::CARD_ERROR_READY)
            && status_completion
                .is_some_and(|completion| completion.status() == card::CARD_ERROR_READY),
    );

    let mut entry = card::CardDir {
        chn: slot.channel(),
        filelen: sector_size.max(0x2000),
        gamecode: *b"TST1",
        company: *b"TS",
        ..card::CardDir::default()
    };
    entry.filename[..5].copy_from_slice(b"ENTRY");

    let entry_create_request = card::create_entry_async_owned(slot, entry);
    let entry_create_result = entry_create_request
        .as_ref()
        .ok()
        .and_then(|request| wait_card_owned_status(|| request.status(), 15));
    let entry_create_completion = match (entry_create_request, entry_create_result) {
        (Ok(request), Some(_)) => request.try_complete().ok(),
        _ => None,
    };
    let entry_created = entry_create_result == Some(card::CARD_ERROR_READY)
        && entry_create_completion
            .as_ref()
            .is_some_and(|completion| completion.status() == card::CARD_ERROR_READY);
    logger.record("card create entry async owned", entry_created);

    if let Some(completion) = entry_create_completion {
        let (create_status, returned_entry, mut entry_file) = completion.into_parts();
        if create_status == card::CARD_ERROR_READY {
            // SAFETY: no request retains `entry_file`; it is uniquely borrowed.
            let close_status = unsafe { card::sys::close(&mut entry_file) };
            logger.record(
                "card close created entry",
                close_status == card::CARD_ERROR_READY,
            );
            let entry_delete_request = card::delete_entry_async_owned(slot, returned_entry);
            let entry_delete_result = entry_delete_request
                .as_ref()
                .ok()
                .and_then(|request| wait_card_owned_status(|| request.status(), 15));
            let entry_delete_completion = match (entry_delete_request, entry_delete_result) {
                (Ok(request), Some(_)) => request.try_complete().ok(),
                _ => None,
            };
            logger.note_i32(
                "card delete entry completion status",
                entry_delete_result.unwrap_or(i32::MIN),
            );
            logger.record(
                "card delete entry async owned",
                entry_delete_result == Some(card::CARD_ERROR_READY)
                    && entry_delete_completion
                        .is_some_and(|completion| completion.status() == card::CARD_ERROR_READY),
            );
        }
    }

    let mut write_bytes = alloc::vec![0x5a; sector_size as usize];
    write_bytes[1] = 0xa5;
    let write_request = card::write_async_owned(file, write_bytes, 0);
    let write_submit = write_request
        .as_ref()
        .map_or_else(|error| error.status(), |_| card::CARD_ERROR_READY);
    let write_result = write_request
        .as_ref()
        .ok()
        .and_then(|request| wait_card_owned_status(|| request.status(), 15));
    let write_completion = match (write_request, write_result) {
        (Ok(request), Some(_)) => request.try_complete().ok(),
        _ => None,
    };
    let write_ok = record_owned_card_async(
        logger,
        "card write async owned",
        write_submit,
        write_result,
        card::CARD_ERROR_READY,
    );
    let Some(write_completion) = write_completion else {
        record_card_unmount(logger, "card unmount after incomplete write", mounted);
        return;
    };
    let (_, returned_file, _) = write_completion.into_parts();
    file = returned_file;

    let read_request = card::read_async_owned(file, card::CARD_READSIZE as usize, 0);
    let read_submit = read_request
        .as_ref()
        .map_or_else(|error| error.status(), |_| card::CARD_ERROR_READY);
    let read_result = read_request
        .as_ref()
        .ok()
        .and_then(|request| wait_card_owned_status(|| request.status(), 10));
    let read_completion = match (read_request, read_result) {
        (Ok(request), Some(_)) => request.try_complete().ok(),
        _ => None,
    };
    let read_ok = record_owned_card_async(
        logger,
        "card read async owned",
        read_submit,
        read_result,
        card::CARD_ERROR_READY,
    );
    let Some(read_completion) = read_completion else {
        record_card_unmount(logger, "card unmount after incomplete read", mounted);
        return;
    };
    let (_, returned_file, mut bytes) = read_completion.into_parts();
    file = returned_file;
    logger.record(
        "card async owned roundtrip",
        write_ok
            && read_ok
            && bytes[0] == 0x5a
            && bytes[1] == 0xa5
            && bytes[2..].iter().all(|byte| *byte == 0x5a),
    );

    bytes.resize(sector_size as usize, 0);
    bytes.fill(0);
    bytes[0] = 0x3c;
    bytes[1] = 0xc3;
    let sync_write = card::write_slice(&mut file, &bytes, 0) == card::CARD_ERROR_READY;
    bytes.fill(0);
    let sync_read = card::read_slice(&mut file, &mut bytes, 0) == card::CARD_ERROR_READY;
    logger.record(
        "card safe sync roundtrip",
        sync_write
            && sync_read
            && bytes[0] == 0x3c
            && bytes[1] == 0xc3
            && bytes[2..].iter().all(|byte| *byte == 0),
    );

    #[repr(align(32))]
    #[derive(Clone)]
    struct CardLine([u8; 32]);
    let mut raw_buffer = alloc::vec![CardLine([0; 32]); (sector_size as usize).div_ceil(32)];
    raw_buffer[0].0[0] = 0x3c;
    raw_buffer[0].0[1] = 0xc3;
    let raw_ptr = raw_buffer.as_mut_ptr().cast::<core::ffi::c_void>();
    // SAFETY: The aligned allocation covers the sector and stays live for both
    // synchronous transfers. The file is uniquely borrowed by each call.
    let (raw_write, raw_read) = unsafe {
        ogc_cache::dc_flush_range(raw_ptr.cast_const(), sector_size);
        let write = card::sys::write(&mut file, raw_ptr, sector_size, 0);
        core::ptr::write_bytes(raw_ptr.cast::<u8>(), 0, sector_size as usize);
        ogc_cache::dc_flush_range(raw_ptr.cast_const(), sector_size);
        let read = card::sys::read(&mut file, raw_ptr, card::CARD_READSIZE, 0);
        ogc_cache::dc_invalidate_range(raw_ptr.cast_const(), card::CARD_READSIZE);
        (write, read)
    };
    logger.record(
        "card raw sync roundtrip",
        raw_write == card::CARD_ERROR_READY
            && raw_read == card::CARD_ERROR_READY
            && raw_buffer[0].0[..2] == [0x3c, 0xc3],
    );

    raw_buffer[0].0.fill(0);
    raw_buffer[0].0[0] = 0x96;
    raw_buffer[0].0[1] = 0x69;
    // SAFETY: the same aligned buffer and unique file remain live for both
    // synchronous transfers. The cache operations bracket each data transfer.
    let (wrapper_write, wrapper_read) = unsafe {
        ogc_cache::dc_flush_range(raw_ptr.cast_const(), sector_size);
        let write = card::write(&mut file, raw_ptr, sector_size, 0);
        core::ptr::write_bytes(raw_ptr.cast::<u8>(), 0, sector_size as usize);
        ogc_cache::dc_flush_range(raw_ptr.cast_const(), sector_size);
        let read = card::read(&mut file, raw_ptr, card::CARD_READSIZE, 0);
        ogc_cache::dc_invalidate_range(raw_ptr.cast_const(), card::CARD_READSIZE);
        (write, read)
    };
    logger.record(
        "card raw wrapper sync roundtrip",
        wrapper_write == card::CARD_ERROR_READY
            && wrapper_read == card::CARD_ERROR_READY
            && raw_buffer[0].0[..2] == [0x96, 0x69],
    );

    logger.record(
        "card close save",
        // SAFETY: `file` is a live, uniquely borrowed card handle and no
        // asynchronous request remains pending.
        unsafe { card::sys::close(&mut file) } == card::CARD_ERROR_READY,
    );

    let first_unmount = mounted.unmount();
    logger.record(
        "card unmount after create",
        first_unmount == card::CARD_ERROR_READY,
    );
    if first_unmount != card::CARD_ERROR_READY {
        return;
    }

    let Some(remounted) = mount_card_owned(logger, slot, 15) else {
        logger.record("card remount after create", false);
        return;
    };
    logger.record("card remount after create", true);
    let mut reopened = card::CardFile::default();
    // SAFETY: the static filename and unique file output remain valid for the call.
    let reopen = unsafe { card::sys::open(slot.channel(), c"SAVE".as_ptr().cast(), &mut reopened) };
    logger.record(
        "card create persisted after remount",
        reopen == card::CARD_ERROR_READY,
    );
    if reopen != card::CARD_ERROR_READY {
        logger.record(
            "card unmount after failed reopen",
            remounted.unmount() == card::CARD_ERROR_READY,
        );
        return;
    }
    let mut persisted = alloc::vec![0u8; card::CARD_READSIZE as usize];
    let persisted_read = card::read_slice(&mut reopened, &mut persisted, 0);
    logger.record(
        "card data persisted after remount",
        persisted_read == card::CARD_ERROR_READY
            && persisted[0] == 0x96
            && persisted[1] == 0x69
            && persisted[2..].iter().all(|byte| *byte == 0),
    );
    let status_persisted = card::file_status(slot, reopened.filenum)
        .is_ok_and(|status| status.banner_fmt == status_marker);
    logger.record("card status persisted after remount", status_persisted);
    let mut directory = alloc::vec![card::CardDir::default(); card::CARD_MAXFILES];
    let mut directory_count = 0;
    // SAFETY: the directory allocation can hold every entry returned by the
    // API, and `directory_count` is a live writable output.
    let directory_result = unsafe {
        card::sys::get_directory(
            slot.channel(),
            directory.as_mut_ptr(),
            &mut directory_count,
            0,
        )
    };
    let save_has_expected_identifiers = directory_result == card::CARD_ERROR_READY
        && directory_count >= 0
        && directory
            .iter()
            .take(directory_count as usize)
            .any(|entry| {
                entry.filename[..4] == *b"SAVE"
                    && entry.gamecode == *b"TST1"
                    && entry.company == *b"TS"
            });
    logger.record(
        "CARD game and company setters select the created save",
        save_has_expected_identifiers,
    );
    logger.record(
        "card close remounted save",
        // SAFETY: the reopened file is uniquely borrowed and has no pending request.
        unsafe { card::sys::close(&mut reopened) } == card::CARD_ERROR_READY,
    );

    let delete_request = card::delete_async_owned(slot, b"SAVE");
    let delete_request_slot = delete_request.as_ref().ok().map(|request| request.slot());
    logger.record(
        "card delete request retains its slot",
        delete_request_slot == Some(slot),
    );
    let delete_submit = delete_request
        .as_ref()
        .map_or_else(|status| *status, |_| card::CARD_ERROR_READY);
    let delete_result = delete_request
        .as_ref()
        .ok()
        .and_then(|request| wait_card_owned_status(|| request.status(), 15));
    let delete_completion = match (delete_request, delete_result) {
        (Ok(request), Some(_)) => request.try_complete().ok(),
        _ => None,
    };
    let delete_completion_status = delete_completion
        .as_ref()
        .map(card::CardDeleteCompletion::status);
    logger.record(
        "card delete completion preserves operation status",
        delete_completion_status == Some(card::CARD_ERROR_READY),
    );
    let delete_ok = record_owned_card_async(
        logger,
        "card delete async owned",
        delete_submit,
        delete_result,
        card::CARD_ERROR_READY,
    );
    logger.record(
        "card delete async owned completion",
        delete_ok && delete_completion.is_some_and(|completion| completion.into_result().is_ok()),
    );
    let delete_unmount = remounted.unmount();
    logger.record(
        "card unmount after delete",
        delete_unmount == card::CARD_ERROR_READY,
    );
    if delete_unmount != card::CARD_ERROR_READY {
        return;
    }
    let Some(final_mount) = mount_card_owned(logger, slot, 15) else {
        logger.record("card remount after delete", false);
        return;
    };
    logger.record("card remount after delete", true);
    let mut deleted_file = card::CardFile::default();
    // SAFETY: the static filename and unique file output remain valid for the call.
    let deleted_open =
        unsafe { card::sys::open(slot.channel(), c"SAVE".as_ptr().cast(), &mut deleted_file) };
    logger.record(
        "card delete persisted after remount",
        deleted_open == card::CARD_ERROR_NOFILE,
    );
    let restored_free = card::free_blocks(slot).ok();
    logger.record(
        "card free blocks restored after delete",
        freeblocks_ok && restored_free == Some(freeblocks),
    );
    logger.record(
        "card final unmount",
        final_mount.unmount() == card::CARD_ERROR_READY,
    );
}

fn cleanup_format_marker(logger: &mut ProbeLogger, slot: card::CardSlot) {
    // SAFETY: the name is static and refers only to the marker created by this run.
    let status = unsafe { card::sys::delete(slot.channel(), c"FORMATMARK".as_ptr().cast()) };
    logger.record(
        "card format marker setup cleanup",
        status == card::CARD_ERROR_READY,
    );
}

fn probe_card_mount_lifecycles(logger: &mut ProbeLogger) {
    logger.section("CARD SYNC MOUNT LIFECYCLES");
    let slot = card::CardSlot::A;
    let mut workarea = Workarea([0; card::CARD_WORKAREA_SIZE as usize]);
    let workarea_ptr = workarea.0.as_mut_ptr().cast();

    // SAFETY: `workarea` is aligned and remains live through unmount.
    let wrapper_mount = unsafe { card::mount(slot.channel(), workarea_ptr, None) };
    logger.note_i32("CARD wrapper sync mount result", wrapper_mount);
    logger.record(
        "CARD wrapper sync mount succeeds",
        wrapper_mount == card::CARD_ERROR_READY,
    );
    if wrapper_mount != card::CARD_ERROR_READY {
        logger.note("CARD direct sys mount checks skipped: wrapper mount failed");
        return;
    }
    logger.record(
        "CARD wrapper mount exposes sector size",
        card::sector_size(slot).is_ok_and(|size| size >= card::CARD_READSIZE),
    );
    let wrapper_unmount = card::unmount(slot.channel());
    logger.note_i32("CARD wrapper sync unmount result", wrapper_unmount);
    logger.record(
        "CARD wrapper sync unmount succeeds",
        wrapper_unmount == card::CARD_ERROR_READY,
    );
    let wrapper_unmounted = card::free_blocks(slot) == Err(card::CARD_ERROR_NOCARD);
    logger.record(
        "CARD wrapper unmount clears mounted state",
        wrapper_unmounted,
    );
    if wrapper_unmount != card::CARD_ERROR_READY || !wrapper_unmounted {
        let cleanup = card::sys::unmount(slot.channel());
        logger.record(
            "CARD wrapper unmount cleanup",
            cleanup == card::CARD_ERROR_READY,
        );
        return;
    }

    // SAFETY: the same aligned workarea remains live through the direct sys unmount.
    let sys_mount = unsafe { card::sys::mount(slot.channel(), workarea_ptr, None) };
    logger.note_i32("CARD sys sync mount result", sys_mount);
    logger.record(
        "CARD sys sync mount succeeds",
        sys_mount == card::CARD_ERROR_READY,
    );
    if sys_mount != card::CARD_ERROR_READY {
        return;
    }
    logger.record(
        "CARD sys mount exposes sector size",
        card::sector_size(slot).is_ok_and(|size| size >= card::CARD_READSIZE),
    );
    let sys_unmount = card::sys::unmount(slot.channel());
    logger.note_i32("CARD sys unmount result", sys_unmount);
    logger.record(
        "CARD sys unmount succeeds",
        sys_unmount == card::CARD_ERROR_READY,
    );
    logger.record(
        "CARD sys unmount clears mounted state",
        card::free_blocks(slot) == Err(card::CARD_ERROR_NOCARD),
    );
}

fn probe_card_format(logger: &mut ProbeLogger) {
    logger.section("CARD DESTRUCTIVE FORMAT");
    logger.note("WARNING: formatting memory card in slot A; all card data will be erased");
    let slot = card::CardSlot::A;
    let Some(mounted) = mount_card_owned(logger, slot, 15) else {
        logger.record("card destructive format mount", false);
        return;
    };
    logger.record("card destructive format mount", true);
    let block_count = card::block_count(slot).ok();
    let sector_size = card::sector_size(slot).ok();
    let mut marker = card::CardFile::default();
    // SAFETY: the static name and unique output are valid for the call.
    let created = sector_size.is_some_and(|size| unsafe {
        card::sys::create(
            slot.channel(),
            c"FORMATMARK".as_ptr().cast(),
            size.max(0x2000),
            &mut marker,
        ) == card::CARD_ERROR_READY
    });
    logger.record("card format marker created", created);
    if !created {
        record_card_unmount(logger, "card format setup unmount", mounted);
        return;
    }
    // SAFETY: the marker handle is live and uniquely borrowed.
    let closed = unsafe { card::sys::close(&mut marker) } == card::CARD_ERROR_READY;
    logger.record("card format marker closed", closed);
    if !closed {
        cleanup_format_marker(logger, slot);
        record_card_unmount(logger, "card format setup unmount", mounted);
        return;
    }
    let mut verified_marker = card::CardFile::default();
    // SAFETY: the static name and unique output are valid for the call.
    let opened = unsafe {
        card::sys::open(
            slot.channel(),
            c"FORMATMARK".as_ptr().cast(),
            &mut verified_marker,
        )
    } == card::CARD_ERROR_READY;
    logger.record("card format marker exists before format", opened);
    let verified_closed = opened
        // SAFETY: a successful open produced this uniquely borrowed handle.
        && unsafe { card::sys::close(&mut verified_marker) } == card::CARD_ERROR_READY;
    logger.record("card format marker verification closed", verified_closed);
    if !verified_closed {
        cleanup_format_marker(logger, slot);
        record_card_unmount(logger, "card format setup unmount", mounted);
        return;
    }
    let format_result = card::sys::format(slot.channel());
    logger.note_i32("card destructive format result", format_result);
    logger.record(
        "card destructive format completed",
        format_result == card::CARD_ERROR_READY,
    );
    if format_result != card::CARD_ERROR_READY {
        cleanup_format_marker(logger, slot);
    }
    let unmount_result = mounted.unmount();
    logger.record(
        "card destructive format unmount",
        unmount_result == card::CARD_ERROR_READY,
    );
    if format_result != card::CARD_ERROR_READY || unmount_result != card::CARD_ERROR_READY {
        return;
    }
    let Some(remounted) = mount_card_owned(logger, slot, 15) else {
        logger.record("card destructive format remount", false);
        return;
    };
    logger.record("card destructive format remount", true);
    let mut removed_marker = card::CardFile::default();
    // SAFETY: the static name and unique output are valid for the call.
    let marker_status = unsafe {
        card::sys::open(
            slot.channel(),
            c"FORMATMARK".as_ptr().cast(),
            &mut removed_marker,
        )
    };
    logger.record(
        "card format marker removed after remount",
        marker_status == card::CARD_ERROR_NOFILE,
    );
    if marker_status == card::CARD_ERROR_READY {
        // SAFETY: the unexpected successful open produced a live handle.
        logger.record("card format unexpected marker close", unsafe {
            card::sys::close(&mut removed_marker) == card::CARD_ERROR_READY
        });
    }
    let expected_free = block_count.and_then(|count| u16::try_from(count.checked_sub(5)?).ok());
    logger.record(
        "card destructive format free blocks",
        expected_free.is_some() && card::free_blocks(slot).ok() == expected_free,
    );
    logger.record(
        "card destructive format final unmount",
        remounted.unmount() == card::CARD_ERROR_READY,
    );
}

fn inserted(io: &ogc_wiisd::DiscInterface) -> bool {
    unsafe { io.is_inserted.is_some_and(|callback| callback()) }
}

fn create_hotplug_handle(
    logger: &mut ProbeLogger,
    path: &CStr,
    message: &[u8],
    label: &str,
) -> Option<ProbeDeviceFile> {
    let file = ProbeDeviceFile::open(path, sysbase::O_WRONLY | sysbase::O_CREAT | sysbase::O_EXCL)?;
    let write_result = file.write(message);
    let sync_result = file.fsync();
    let close_result = file.close();
    if write_result.0 != message.len() as isize || sync_result.0 != 0 || close_result.0 != 0 {
        let cleanup = unlink_device_file(path);
        logger.record(
            &alloc::format!("{label} hotplug setup cleanup"),
            cleanup == 0,
        );
        return None;
    }
    match ProbeDeviceFile::open(path, sysbase::O_RDWR) {
        Some(file) => Some(file),
        None => {
            let cleanup = unlink_device_file(path);
            logger.record(
                &alloc::format!("{label} hotplug setup cleanup"),
                cleanup == 0,
            );
            None
        }
    }
}

fn open_hotplug_handle(path: &CStr) -> Option<ProbeDeviceFile> {
    ProbeDeviceFile::open(path, sysbase::O_RDWR)
}

fn open_or_create_hotplug_card(file: &mut card::CardFile, created: &mut bool) -> bool {
    // SAFETY: the filename is static and `file` is uniquely borrowed throughout.
    let open = unsafe { card::open(card::CARD_SLOTA, c"HOTPLUG".as_ptr().cast(), file) };
    if open == card::CARD_ERROR_READY {
        return true;
    }
    if open != card::CARD_ERROR_NOFILE {
        return false;
    }

    let mut sector_size = 0u32;
    // SAFETY: `sector_size` is a valid unique output reference.
    if unsafe { card::get_sector_size(card::CARD_SLOTA, &mut sector_size) }
        != card::CARD_ERROR_READY
    {
        return false;
    }
    // SAFETY: the static filename and unique file output remain valid for the call.
    let status = unsafe {
        card::create(
            card::CARD_SLOTA,
            c"HOTPLUG".as_ptr().cast(),
            sector_size.max(0x2000),
            file,
        )
    };
    if status == card::CARD_ERROR_READY {
        *created = true;
        true
    } else {
        false
    }
}

fn usb_device_has_interface_class(fd: i32, interface_class: u8) -> bool {
    let Ok(mut descriptor) = usb_core::descriptors(fd) else {
        return false;
    };
    let mut found = false;
    unsafe {
        if !descriptor.configurations.is_null() {
            for configuration_index in 0..descriptor.b_num_configurations as usize {
                let configuration = &*descriptor.configurations.add(configuration_index);
                if configuration.interfaces.is_null() {
                    continue;
                }
                for interface_index in 0..configuration.b_num_interfaces as usize {
                    let interface = &*configuration.interfaces.add(interface_index);
                    if interface.b_interface_class == interface_class {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
        }
        usb_core::free_descriptors(&mut descriptor);
    }
    found
}

fn open_v5_mass_storage_device() -> Option<i32> {
    let mut entries = [usb_core::UsbDeviceEntry::default(); 16];
    let (count, _) = collect_usb_storage_candidates(&mut entries);
    for entry in entries.iter().copied().take(count) {
        // libogc routes negative v5 device IDs through /dev/usb/ven.
        if entry.device_id >= 0 {
            continue;
        }
        let Ok(fd) = usb_core::open_device_handle(entry.device_id, entry.vid, entry.pid) else {
            continue;
        };
        if usb_device_has_interface_class(fd, USB_CLASS_MASS_STORAGE) {
            return Some(fd);
        }
        // A v5 handle is the global device ID. Closing it can complete a
        // removal callback owned by another client, so a discovery mismatch
        // is left untouched.
    }
    None
}

struct UsbV5RemovalProbe {
    fd: Option<i32>,
    expected_userdata: u32,
    observed_callbacks: u32,
    confirmed_removals: u32,
    available: bool,
    failed: bool,
}

impl UsbV5RemovalProbe {
    fn start(logger: &mut ProbeLogger) -> Self {
        USB_V5_CALLBACKS.store(0, Ordering::Release);
        OGC_STORAGE_PROBE_USB_V5_REMOVALS.store(0, Ordering::Release);
        OGC_STORAGE_PROBE_USB_V5_LAST_RESULT.store(i32::MIN, Ordering::Release);
        OGC_STORAGE_PROBE_USB_V5_LAST_USERDATA.store(0, Ordering::Release);
        let mut probe = Self {
            fd: None,
            expected_userdata: 0,
            observed_callbacks: 0,
            confirmed_removals: 0,
            available: false,
            failed: false,
        };
        if probe.register_next() {
            probe.available = true;
            logger.note("USB v5 removal callback armed for cycle 1");
        } else {
            logger.note("USB v5 removal callback skipped: no v5 mass-storage device");
        }
        probe
    }

    fn register_next(&mut self) -> bool {
        if self.fd.is_some() || self.failed || self.confirmed_removals >= 2 {
            return false;
        }
        let Some(fd) = open_v5_mass_storage_device() else {
            return false;
        };
        let token = 0x5635_0000 | (self.confirmed_removals + 1);
        let result = unsafe {
            usb_core::device_removal_notify_async(
                fd,
                Some(usb_v5_removal_callback),
                token as usize as *mut c_void,
            )
        };
        if result < 0 {
            return false;
        }
        self.fd = Some(fd);
        self.expected_userdata = token;
        true
    }

    fn poll(&mut self, logger: &mut ProbeLogger) {
        let callbacks = USB_V5_CALLBACKS.load(Ordering::Acquire);
        if callbacks == self.observed_callbacks {
            if self.available && self.fd.is_none() && self.confirmed_removals == 1 {
                let _ = self.register_next();
            }
            return;
        }

        let result = OGC_STORAGE_PROBE_USB_V5_LAST_RESULT.load(Ordering::Acquire);
        let userdata = OGC_STORAGE_PROBE_USB_V5_LAST_USERDATA.load(Ordering::Acquire);
        let exact_count = callbacks == self.observed_callbacks + 1;
        let exact_userdata = userdata == self.expected_userdata;
        let successful_result = result == usb_core::USB_OK;
        let stale_rejected = self.fd.is_some_and(|fd| usb_core::resume_device(fd) < 0);
        self.observed_callbacks = callbacks;
        self.fd = None;

        let cycle = self.confirmed_removals + 1;
        let mut line = LineBuf::new();
        let _ = write!(
            &mut line,
            "usb v5 cycle={} callbacks={} result={} userdata={:08x} stale_rejected={}",
            cycle, callbacks, result, userdata, stale_rejected as u32
        );
        logger.note(line.as_str());

        logger.record("usb v5 callback count exact", exact_count);
        logger.record("usb v5 callback userdata exact", exact_userdata);
        logger.record("usb v5 callback result", successful_result);
        logger.record("usb v5 removed operation rejected", stale_rejected);
        let confirmed = exact_count && exact_userdata && successful_result && stale_rejected;
        if cycle == 1 {
            logger.record("usb v5 first removal callback", confirmed);
        } else {
            logger.record("usb v5 second removal callback", confirmed);
        }
        if confirmed {
            self.confirmed_removals += 1;
            OGC_STORAGE_PROBE_USB_V5_REMOVALS.store(self.confirmed_removals, Ordering::Release);
        } else {
            self.failed = true;
        }
    }

    fn finish(mut self, logger: &mut ProbeLogger) {
        if !self.available {
            return;
        }
        self.poll(logger);
        let callback_count_before_close = USB_V5_CALLBACKS.load(Ordering::Acquire);
        logger.note_u32("usb v5 confirmed removals", self.confirmed_removals);
        logger.note_u32(
            "usb v5 callbacks before cleanup",
            callback_count_before_close,
        );
        logger.record(
            "usb v5 watcher rearmed",
            self.confirmed_removals == 2 && callback_count_before_close == 2,
        );
        if let Some(mut fd) = self.fd {
            // Closing can complete the accepted one-shot callback. It happens
            // after the evidence counters above and cannot fake a removal pass.
            let _ = usb_core::close_device_handle(&mut fd);
        }
    }
}

fn probe_media_hotplug(logger: &mut ProbeLogger) {
    logger.section("MEDIA HOTPLUG SOAK");
    if cfg!(feature = "dolphin-automation") {
        logger.note("hotplug soak skipped: automated Dolphin run has no media operator");
        return;
    }
    if logger.uses_mounted_log() {
        logger.note("hotplug soak skipped: probe log is on removable media");
        return;
    }

    logger.note("For the next 60 seconds, remove and reinsert SD, USB, and CARD media.");
    logger
        .note("Each old FAT handle must fail after removal; remounts are attempted automatically.");

    let sd_mount = fat::mount_sd();
    log_fat_result(logger, "hotplug initial sd mount", &sd_mount);
    let usb_mount = fat::mount_usb();
    log_fat_result(logger, "hotplug initial usb mount", &usb_mount);
    let mut sd_handle = if fat::is_mounted(SD_MOUNT_NAME) {
        create_hotplug_handle(logger, SD_PROBE_PATH, SD_PROBE_MSG, "sd")
    } else {
        None
    };
    let sd_probe_file_created = sd_handle.is_some();
    let mut usb_handle = if fat::is_mounted(USB_MOUNT_NAME) {
        create_hotplug_handle(logger, USB_PROBE_PATH, USB_PROBE_MSG, "usb")
    } else {
        None
    };
    let usb_probe_file_created = usb_handle.is_some();
    let mut usb_v5_probe = UsbV5RemovalProbe::start(logger);

    let mut card_workarea = Workarea([0; card::CARD_WORKAREA_SIZE as usize]);
    CARD_DETACHED.store(false, Ordering::Release);
    // SAFETY: the aligned work area remains live through the complete hotplug probe.
    let mut card_slot_mounted = unsafe {
        card::mount(
            card::CARD_SLOTA,
            card_workarea.0.as_mut_ptr().cast(),
            Some(card_detach_callback),
        )
    } == card::CARD_ERROR_READY;
    let mut card_file = card::CardFile::default();
    let mut card_file_created = false;
    let mut card_file_open =
        card_slot_mounted && open_or_create_hotplug_card(&mut card_file, &mut card_file_created);
    if card_slot_mounted && !card_file_open {
        let unmount = card::unmount(card::CARD_SLOTA);
        logger.record(
            "card hotplug setup unmount",
            unmount == card::CARD_ERROR_READY,
        );
        card_slot_mounted = false;
    }

    let mut sd_removals = 0u32;
    let mut usb_removals = 0u32;
    let mut card_removals = 0u32;
    let mut sd_remounts = 0u32;
    let mut usb_remounts = 0u32;
    let mut card_remounts = 0u32;
    let mut sd_was_mounted = fat::is_mounted(SD_MOUNT_NAME);
    let mut usb_was_mounted = fat::is_mounted(USB_MOUNT_NAME);
    let sleep = (lwp::TICKS_PER_SECOND / 4).max(1);

    for _ in 0..HOTPLUG_SECONDS.saturating_mul(4) {
        let poll_events = fat::poll_media();
        logger.note_u32("hotplug media poll events", poll_events as u32);
        usb_v5_probe.poll(logger);
        let sd_now_mounted = fat::is_mounted(SD_MOUNT_NAME);
        let usb_now_mounted = fat::is_mounted(USB_MOUNT_NAME);

        if sd_was_mounted && !sd_now_mounted {
            sd_removals += 1;
            if let Some(handle) = sd_handle.take() {
                let mut byte = [0u8; 1];
                logger.record("sd stale handle rejected", handle.read_raw(&mut byte) < 0);
                logger.record("sd stale fsync rejected", handle.fsync().0 == -1);
                logger.record("sd stale close reports removal", handle.close().0 == -1);
            }
        }
        if usb_was_mounted && !usb_now_mounted {
            usb_removals += 1;
            if let Some(handle) = usb_handle.take() {
                let mut byte = [0u8; 1];
                logger.record("usb stale handle rejected", handle.read_raw(&mut byte) < 0);
                logger.record("usb stale fsync rejected", handle.fsync().0 == -1);
                logger.record("usb stale close reports removal", handle.close().0 == -1);
            }
        }

        if !sd_now_mounted && inserted(&wiisd::WII_SD_INTERFACE) && fat::mount_sd().is_ok() {
            sd_remounts += 1;
            if sd_probe_file_created {
                sd_handle = open_hotplug_handle(SD_PROBE_PATH);
            }
        }
        if !usb_now_mounted
            && inserted(&usb_storage_sys::USB_STORAGE_INTERFACE)
            && fat::mount_usb().is_ok()
        {
            usb_remounts += 1;
            if usb_probe_file_created {
                usb_handle = open_hotplug_handle(USB_PROBE_PATH);
            }
        }

        if CARD_DETACHED.swap(false, Ordering::AcqRel) {
            card_removals += 1;
            card_slot_mounted = false;
            card_file_open = false;
            let mut byte = [0u8; card::CARD_READSIZE as usize];
            let stale = card::read_slice(&mut card_file, &mut byte, 0);
            logger.record(
                "card stale handle rejected",
                stale == card::CARD_ERROR_NOCARD,
            );
        }
        if !card_slot_mounted && card::sys::probe(card::CARD_SLOTA) == 1 {
            // SAFETY: the aligned work area remains live through this remount.
            card_slot_mounted = unsafe {
                card::sys::mount(
                    card::CARD_SLOTA,
                    card_workarea.0.as_mut_ptr().cast(),
                    Some(card_detach_callback),
                )
            } == card::CARD_ERROR_READY;
            if card_slot_mounted {
                card_file = card::CardFile::default();
                card_file_open =
                    open_or_create_hotplug_card(&mut card_file, &mut card_file_created);
                if card_file_open {
                    card_remounts += 1;
                } else {
                    logger.record("card hotplug remount file open", false);
                    let unmount = card::sys::unmount(card::CARD_SLOTA);
                    logger.record(
                        "card hotplug remount cleanup",
                        unmount == card::CARD_ERROR_READY,
                    );
                    card_slot_mounted = false;
                }
            }
        }

        sd_was_mounted = fat::is_mounted(SD_MOUNT_NAME);
        usb_was_mounted = fat::is_mounted(USB_MOUNT_NAME);
        lwp::sleep_ticks(sleep);
    }

    logger.note_u32("sd removals", sd_removals);
    logger.note_u32("sd remounts", sd_remounts);
    logger.note_u32("usb removals", usb_removals);
    logger.note_u32("usb remounts", usb_remounts);
    logger.note_u32("card removals", card_removals);
    logger.note_u32("card remounts", card_remounts);
    usb_v5_probe.finish(logger);

    if let Some(handle) = sd_handle {
        logger.record("sd hotplug final handle close", handle.close().0 == 0);
    }
    if let Some(handle) = usb_handle {
        logger.record("usb hotplug final handle close", handle.close().0 == 0);
    }
    if sd_probe_file_created {
        logger.record(
            "sd hotplug probe file cleanup",
            unlink_device_file(SD_PROBE_PATH) == 0,
        );
    }
    if usb_probe_file_created {
        logger.record(
            "usb hotplug probe file cleanup",
            unlink_device_file(USB_PROBE_PATH) == 0,
        );
    }
    if fat::is_mounted(SD_MOUNT_NAME) {
        let unmount = fat::unmount(SD_MOUNT_NAME);
        log_fat_result(logger, "hotplug final sd unmount", &unmount);
    }
    if fat::is_mounted(USB_MOUNT_NAME) {
        let unmount = fat::unmount(USB_MOUNT_NAME);
        log_fat_result(logger, "hotplug final usb unmount", &unmount);
    }
    if card_file_open {
        // SAFETY: `card_file` is live, uniquely borrowed, and has no pending
        // asynchronous operation at this cleanup point.
        let close = unsafe { card::sys::close(&mut card_file) };
        logger.record("card hotplug file close", close == card::CARD_ERROR_READY);
    }
    if card_slot_mounted {
        if card_file_created {
            // SAFETY: this path is deleted only when this probe created it.
            let delete = unsafe { card::delete(card::CARD_SLOTA, c"HOTPLUG".as_ptr().cast()) };
            logger.record(
                "card hotplug probe file cleanup",
                delete == card::CARD_ERROR_READY,
            );
        }
        let unmount = card::sys::unmount(card::CARD_SLOTA);
        logger.record(
            "card hotplug unmount cleanup",
            unmount == card::CARD_ERROR_READY,
        );
    } else if card_file_created {
        logger.note("card hotplug cleanup skipped: card is not mounted; probe file may remain");
    }
}

struct LineBuf {
    bytes: [u8; 160],
    len: usize,
}

impl LineBuf {
    const fn new() -> Self {
        Self {
            bytes: [0; 160],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl fmt::Write for LineBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.bytes.len().saturating_sub(self.len);
        if bytes.len() > remaining {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}
