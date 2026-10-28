use alloc::alloc::{alloc_zeroed, dealloc};
use core::alloc::Layout;
use core::ffi::{CStr, c_char, c_void};
use core::fmt::{self, Write};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use ogc_card as card;
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

#[repr(align(32))]
struct SectorBuf([u8; RAW_USB_SECTOR_BUFFER_SIZE]);

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

static ASYNC_RETURNED: AtomicBool = AtomicBool::new(false);
static ASYNC_DONE: AtomicBool = AtomicBool::new(false);
static ASYNC_AFTER_RETURN: AtomicBool = AtomicBool::new(false);
static ASYNC_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
static CARD_DETACHED: AtomicBool = AtomicBool::new(false);
static USB_V5_CALLBACKS: AtomicU32 = AtomicU32::new(0);

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
        logger.note_i32("card shutdown", card::shutdown());
        logger.note_i32("dvd shutdown", dvd::shutdown());
    }
    OGC_STORAGE_PROBE_STAGE.store(6, Ordering::Release);
    logger.note_u32(
        "storage lwp debug events",
        OGC_STORAGE_PROBE_LWP_EVENTS.load(Ordering::Acquire),
    );
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

unsafe extern "C" fn card_detach_callback(_chn: i32, result: i32) {
    CARD_DETACHED.store(true, Ordering::Release);
    OGC_STORAGE_PROBE_LAST_RESULT.store(result, Ordering::Release);
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

fn probe_dvd(logger: &mut ProbeLogger) {
    logger.section("DVD /DEV/DI");
    dvd::init();
    let initial_status = dvd::get_drive_status();
    logger.note_i32("dvd initial status", initial_status);
    logger.note_i32("dvd cover status", dvd::get_cover_status());
    logger.note_i32("dvd disc present", dvd::disk_present() as i32);

    if dvd::disk_present() == 0 {
        logger.note("dvd tests skipped: insert a readable Wii/GameCube disc");
        logger.record("dvd no fake success without media", dvd::mount() < 0);
        return;
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
    let id_ok = record_async(logger, "dvd disk id async", submit, dvd::DVD_ERROR_OK, 10);
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
    let read_ok = record_async(logger, "dvd sector0 async", submit, dvd::DVD_ERROR_OK, 10);
    logger.record(
        "dvd async byte count",
        read_ok && async_read_block.txdsize == sector.0.len() as u32,
    );
    logger.note_u32("dvd last drive error", dvd::get_last_error());
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
    logger.record("usb raw inserted", inserted);
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
        logger.note("usb raw read sector0 skipped: no inserted device");
    }

    logger.section("FAT USB MOUNT");
    let mount_result = fat::mount_usb();
    let mounted = matches!(mount_result, Ok(()) | Err(fat::FatError::AlreadyMounted));
    let owned_mount = mount_result.is_ok();
    log_fat_result(logger, "fat mount usb", &mount_result);
    logger.record("usb fat mount", mounted);

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
        let persistence_written = write_usb_close_probe();
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

fn write_usb_close_probe() -> bool {
    let _ = unlink_device_file(USB_CLOSE_PROBE_PATH);
    let Some(file) =
        ProbeDeviceFile::open(USB_CLOSE_PROBE_PATH, sysbase::O_WRONLY | sysbase::O_CREAT)
    else {
        return false;
    };
    let written = file.write(USB_CLOSE_PROBE_MSG);
    let sync_result = file.fsync();
    let close_result = file.close();
    written.0 == USB_CLOSE_PROBE_MSG.len() as isize && sync_result.0 == 0 && close_result.0 == 0
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
        logger.record("argv handoff", false);
        logger.note("argv unavailable");
        return;
    };
    logger.note_i32("argc", argv.argc);
    if argv.argc <= 0 || argv.argv.is_null() {
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
    let _ = unlink_device_file(path);

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

    if let Some(file) = ProbeDeviceFile::open(path, sysbase::O_WRONLY | sysbase::O_CREAT) {
        result.write_open = true;
        (result.write_rc, result.write_errno) = file.write(message);
        (result.write_sync_rc, result.write_sync_errno) = file.fsync();
        (result.write_close_rc, result.write_close_errno) = file.close();
    }

    if let Some(file) = ProbeDeviceFile::open(path, sysbase::O_RDWR) {
        result.read_open = true;
        (result.read_rc, result.read_errno) = file.read(&mut result.read_back);
        (result.read_close_rc, result.read_close_errno) = file.close();
    }

    result.remove_rc = unlink_device_file(path);
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
    let mount_result = fat::mount_sd();
    let mounted = matches!(mount_result, Ok(()) | Err(fat::FatError::AlreadyMounted));
    let owned_mount = mount_result.is_ok();
    log_fat_result(logger, "fat mount sd", &mount_result);
    logger.record("sd fat mount", mounted);

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

fn mount_card_owned(slot: card::CardSlot, timeout_seconds: u32) -> Option<card::MountedCard> {
    let request = card::mount_async_owned(slot).ok()?;
    wait_card_owned_status(|| request.status(), timeout_seconds)?;
    match request.try_complete().ok()? {
        card::CardMountCompletion::Mounted(mounted) => Some(mounted),
        card::CardMountCompletion::Failed(_) => None,
    }
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
    let init = card::init(Some(gamecode), Some(company));
    logger.note_i32("card init", init);
    let slot = card::CardSlot::A;
    if card::probe_slot(slot) != card::CARD_ERROR_READY {
        logger.note("card tests skipped: insert a memory card in slot A");
        return;
    }

    prepare_async_probe();
    let mount_request = card::mount_async_owned(slot);
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

    let (sector_rc, sector_size) = match card::sector_size(slot) {
        Ok(value) => (card::CARD_ERROR_READY, value),
        Err(status) => (status, 0),
    };
    logger.note_i32("card sector size rc", sector_rc);
    logger.note_u32("card sector size", sector_size);
    // SAFETY: the filename is a static NUL-terminated string.
    let cleanup = unsafe { card::delete(card::CARD_SLOTA, c"SAVE".as_ptr().cast()) };
    logger.record(
        "card pre-create cleanup",
        cleanup == card::CARD_ERROR_READY || cleanup == card::CARD_ERROR_NOFILE,
    );
    let (freeblocks_ok, freeblocks) = match card::free_blocks(slot) {
        Ok(value) => (true, value),
        Err(_) => (false, 0),
    };
    logger.record("card free blocks before create", freeblocks_ok);
    logger.note_u32("card free block count before create", freeblocks as u32);
    let create_request = card::create_async_owned(slot, b"SAVE", sector_size.max(0x2000));
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
        let _ = mounted.unmount();
        return;
    };
    if !create_ok {
        let _ = mounted.unmount();
        return;
    }

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

    // SAFETY: this best-effort cleanup uses a static NUL-terminated filename.
    let _ = unsafe { card::delete(slot.channel(), c"ENTRY".as_ptr().cast()) };
    let mut entry = card::CardDir {
        chn: slot.channel(),
        filelen: sector_size.max(0x2000),
        gamecode: *b"RUST",
        company: *b"RS",
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
            let _ = unsafe { card::close(&mut entry_file) };
            let entry_delete_request = card::delete_entry_async_owned(slot, returned_entry);
            let entry_delete_result = entry_delete_request
                .as_ref()
                .ok()
                .and_then(|request| wait_card_owned_status(|| request.status(), 15));
            let entry_delete_completion = match (entry_delete_request, entry_delete_result) {
                (Ok(request), Some(_)) => request.try_complete().ok(),
                _ => None,
            };
            logger.record(
                "card delete entry async owned",
                entry_delete_result == Some(card::CARD_ERROR_READY)
                    && entry_delete_completion
                        .is_some_and(|completion| completion.status() == card::CARD_ERROR_READY),
            );
        }
    }

    let mut write_bytes = alloc::vec![0u8; card::CARD_READSIZE as usize];
    write_bytes[0] = 0x5a;
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
        let _ = mounted.unmount();
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
        let _ = mounted.unmount();
        return;
    };
    let (_, returned_file, mut bytes) = read_completion.into_parts();
    file = returned_file;
    logger.record(
        "card async owned roundtrip",
        write_ok && read_ok && bytes[0] == 0x5a && bytes[1] == 0xa5,
    );

    bytes.fill(0);
    bytes[0] = 0x3c;
    bytes[1] = 0xc3;
    let sync_write = card::write_slice(&mut file, &bytes, 0) == card::CARD_ERROR_READY;
    bytes.fill(0);
    let sync_read = card::read_slice(&mut file, &mut bytes, 0) == card::CARD_ERROR_READY;
    logger.record(
        "card safe sync roundtrip",
        sync_write && sync_read && bytes[0] == 0x3c && bytes[1] == 0xc3,
    );

    logger.record(
        "card close save",
        // SAFETY: `file` is a live, uniquely borrowed card handle and no
        // asynchronous request remains pending.
        unsafe { card::close(&mut file) } == card::CARD_ERROR_READY,
    );

    let first_unmount = mounted.unmount();
    logger.record(
        "card unmount after create",
        first_unmount == card::CARD_ERROR_READY,
    );
    if first_unmount != card::CARD_ERROR_READY {
        return;
    }

    let Some(remounted) = mount_card_owned(slot, 15) else {
        logger.record("card remount after create", false);
        return;
    };
    logger.record("card remount after create", true);
    let mut reopened = card::CardFile::default();
    // SAFETY: the static filename and unique file output remain valid for the call.
    let reopen = unsafe { card::open(slot.channel(), c"SAVE".as_ptr().cast(), &mut reopened) };
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
        persisted_read == card::CARD_ERROR_READY && persisted[0] == 0x3c && persisted[1] == 0xc3,
    );
    let status_persisted = card::file_status(slot, reopened.filenum)
        .is_ok_and(|status| status.banner_fmt == status_marker);
    logger.record("card status persisted after remount", status_persisted);
    logger.record(
        "card close remounted save",
        // SAFETY: the reopened file is uniquely borrowed and has no pending request.
        unsafe { card::close(&mut reopened) } == card::CARD_ERROR_READY,
    );

    let delete_request = card::delete_async_owned(slot, b"SAVE");
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
    let Some(final_mount) = mount_card_owned(slot, 15) else {
        logger.record("card remount after delete", false);
        return;
    };
    logger.record("card remount after delete", true);
    let mut deleted_file = card::CardFile::default();
    // SAFETY: the static filename and unique file output remain valid for the call.
    let deleted_open =
        unsafe { card::open(slot.channel(), c"SAVE".as_ptr().cast(), &mut deleted_file) };
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

fn probe_card_format(logger: &mut ProbeLogger) {
    logger.section("CARD DESTRUCTIVE FORMAT");
    logger.note("WARNING: formatting memory card in slot A; all card data will be erased");
    let slot = card::CardSlot::A;
    let Some(mounted) = mount_card_owned(slot, 15) else {
        logger.record("card destructive format mount", false);
        return;
    };
    logger.record("card destructive format mount", true);
    let block_count = card::block_count(slot).ok();
    let format_result = card::format(slot.channel());
    logger.note_i32("card destructive format result", format_result);
    logger.record(
        "card destructive format completed",
        format_result == card::CARD_ERROR_READY,
    );
    let unmount_result = mounted.unmount();
    logger.record(
        "card destructive format unmount",
        unmount_result == card::CARD_ERROR_READY,
    );
    if format_result != card::CARD_ERROR_READY || unmount_result != card::CARD_ERROR_READY {
        return;
    }
    let Some(remounted) = mount_card_owned(slot, 15) else {
        logger.record("card destructive format remount", false);
        return;
    };
    logger.record("card destructive format remount", true);
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

fn create_hotplug_handle(path: &CStr, message: &[u8]) -> Option<ProbeDeviceFile> {
    let _ = unlink_device_file(path);
    let file = ProbeDeviceFile::open(path, sysbase::O_WRONLY | sysbase::O_CREAT)?;
    if file.write(message).0 != message.len() as isize || file.fsync().0 != 0 || file.close().0 != 0
    {
        return None;
    }
    ProbeDeviceFile::open(path, sysbase::O_RDWR)
}

fn open_or_create_hotplug_card(file: &mut card::CardFile) -> bool {
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
    (unsafe {
        card::create(
            card::CARD_SLOTA,
            c"HOTPLUG".as_ptr().cast(),
            sector_size.max(0x2000),
            file,
        )
    }) == card::CARD_ERROR_READY
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

    let _ = fat::mount_sd();
    let _ = fat::mount_usb();
    let mut sd_handle = if fat::is_mounted(SD_MOUNT_NAME) {
        create_hotplug_handle(SD_PROBE_PATH, SD_PROBE_MSG)
    } else {
        None
    };
    let mut usb_handle = if fat::is_mounted(USB_MOUNT_NAME) {
        create_hotplug_handle(USB_PROBE_PATH, USB_PROBE_MSG)
    } else {
        None
    };
    let mut usb_v5_probe = UsbV5RemovalProbe::start(logger);

    let mut card_workarea = Workarea([0; card::CARD_WORKAREA_SIZE as usize]);
    CARD_DETACHED.store(false, Ordering::Release);
    // SAFETY: the aligned work area remains live through the complete hotplug probe.
    let mut card_mounted = unsafe {
        card::mount(
            card::CARD_SLOTA,
            card_workarea.0.as_mut_ptr().cast(),
            Some(card_detach_callback),
        )
    } == card::CARD_ERROR_READY;
    let mut card_file = card::CardFile::default();
    if card_mounted {
        card_mounted = open_or_create_hotplug_card(&mut card_file);
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
        let _ = fat::poll_media();
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
            sd_handle = create_hotplug_handle(SD_PROBE_PATH, SD_PROBE_MSG);
        }
        if !usb_now_mounted
            && inserted(&usb_storage_sys::USB_STORAGE_INTERFACE)
            && fat::mount_usb().is_ok()
        {
            usb_remounts += 1;
            usb_handle = create_hotplug_handle(USB_PROBE_PATH, USB_PROBE_MSG);
        }

        if CARD_DETACHED.swap(false, Ordering::AcqRel) {
            card_removals += 1;
            card_mounted = false;
            let mut byte = [0u8; card::CARD_READSIZE as usize];
            let stale = card::read_slice(&mut card_file, &mut byte, 0);
            logger.record(
                "card stale handle rejected",
                stale == card::CARD_ERROR_NOCARD,
            );
        }
        if !card_mounted && card::probe(card::CARD_SLOTA) == card::CARD_ERROR_READY {
            // SAFETY: the aligned work area remains live through this remount.
            card_mounted = unsafe {
                card::mount(
                    card::CARD_SLOTA,
                    card_workarea.0.as_mut_ptr().cast(),
                    Some(card_detach_callback),
                )
            } == card::CARD_ERROR_READY;
            if card_mounted {
                card_file = card::CardFile::default();
                card_mounted = open_or_create_hotplug_card(&mut card_file);
                if card_mounted {
                    card_remounts += 1;
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
        let _ = handle.close();
    }
    if let Some(handle) = usb_handle {
        let _ = handle.close();
    }
    let _ = unlink_device_file(SD_PROBE_PATH);
    let _ = unlink_device_file(USB_PROBE_PATH);
    if fat::is_mounted(SD_MOUNT_NAME) {
        let _ = fat::unmount(SD_MOUNT_NAME);
    }
    if fat::is_mounted(USB_MOUNT_NAME) {
        let _ = fat::unmount(USB_MOUNT_NAME);
    }
    if card_mounted {
        // SAFETY: `card_file` is live, uniquely borrowed, and has no pending
        // asynchronous operation at this cleanup point.
        let _ = unsafe { card::close(&mut card_file) };
        // SAFETY: the filename is a static NUL-terminated string.
        let _ = unsafe { card::delete(card::CARD_SLOTA, c"HOTPLUG".as_ptr().cast()) };
        let _ = card::unmount(card::CARD_SLOTA);
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
