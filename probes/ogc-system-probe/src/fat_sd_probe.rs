use crate::probe_support::ProbeLogger;
use core::ffi::c_char;
use ogc_fat as fat;
use ogc_wiisd::WII_SD_INTERFACE;

const MOUNT: &core::ffi::CStr = c"sd";
const PROBE_FILE: &core::ffi::CStr = c"sd:/libogc-fat-api-probe.tmp";
const GCSD_ERROR_NOCARD: i32 = -3;

#[repr(align(32))]
struct CardSector([u8; 512]);

fn probe_gcsd_invalid_drive(logger: &mut ProbeLogger) {
    use ogc_gcsd as gcsd;

    logger.section("GAMECUBE SD INVALID DRIVE API CHECKS");
    let init_io_default = gcsd::sdgecko_initIODefault;
    gcsd::sdgecko_initIODefault();

    let speed = gcsd::sdgecko_get_speed(-1);
    let page_size = gcsd::sdgecko_get_page_size(-1);
    let addressing = gcsd::sdgecko_get_addressing_type(-1);
    logger.record(
        "Rust SD Gecko getters reject an invalid drive",
        speed.is_none() && page_size.is_none() && addressing.is_none(),
    );
    logger.record(
        "Rust SD Gecko setters reject an invalid drive",
        !gcsd::sdgecko_set_speed(-1, 12_345_678) && gcsd::sdgecko_set_page_size(-1, 512).is_none(),
    );

    let init_io = gcsd::sdgecko_initIO;
    let init = init_io(-1);
    let pre_io = gcsd::sdgecko_preIO(-1);
    let cid = gcsd::sdgecko_readCID(-1);
    let csd = gcsd::sdgecko_readCSD(-1);
    let status = gcsd::sdgecko_readStatus(-1);
    let unmount = gcsd::sdgecko_doUnmount(-1);
    logger.record(
        "C SD Gecko lifecycle and metadata APIs reject an invalid drive",
        [init, pre_io, cid, csd, status, unmount]
            .into_iter()
            .all(|result| result == GCSD_ERROR_NOCARD),
    );

    let mut sector = CardSector([0xa5; 512]);
    // SAFETY: The sector buffer is aligned and large enough for one sector.
    let read = unsafe { gcsd::sdgecko_readSectors(-1, 0, 1, sector.0.as_mut_ptr().cast()) };
    // SAFETY: The sector buffer is aligned and large enough for one sector.
    let write = unsafe { gcsd::sdgecko_writeSectors(-1, 0, 1, sector.0.as_ptr().cast()) };
    logger.record(
        "C SD Gecko sector APIs reject an invalid drive",
        read == GCSD_ERROR_NOCARD && write == GCSD_ERROR_NOCARD,
    );

    let speed_before = gcsd::sdgecko_get_speed(0).unwrap_or(0);
    let speed_changed_to = speed_before ^ 1;
    let set_speed = gcsd::sdgecko_set_speed;
    let speed_set = set_speed(0, speed_changed_to);
    let speed_after_set = gcsd::sdgecko_get_speed(0);
    gcsd::sdgecko_insertedCB(0);
    gcsd::sdgecko_ejectedCB(0);
    gcsd::sdgecko_setSpeed(-1, 12_345_678);
    let speed_after_callbacks = gcsd::sdgecko_get_speed(0);
    logger.record(
        "Rust SD Gecko speed setter changes drive state",
        speed_set && speed_after_set == Some(speed_changed_to),
    );
    logger.record(
        "C SD Gecko callbacks and invalid speed setter preserve drive state",
        speed_after_set == speed_after_callbacks,
    );
    init_io_default();
    logger.record(
        "C SD Gecko default initialization restores the original speed",
        gcsd::sdgecko_get_speed(0) == Some(speed_before),
    );

    logger.record(
        "C SD Gecko getters use documented invalid-drive defaults",
        gcsd::sdgecko_getSpeed(-1) == 0
            && gcsd::sdgecko_getPageSize(-1) == 0
            && gcsd::sdgecko_getAddressingType(-1) == gcsd::CARD_IO_SECTOR_ADDRESSING
            && gcsd::sdgecko_setPageSize(-1, 512) == GCSD_ERROR_NOCARD,
    );

    gcsd::sdgecko_initBufferPool();
    // SAFETY: The allocator contract permits null as a no-op free request.
    unsafe { gcsd::sdgecko_freeBuffer(core::ptr::null_mut()) };
    let first = gcsd::sdgecko_allocBuffer();
    let second = gcsd::sdgecko_allocBuffer();
    let third = gcsd::sdgecko_allocBuffer();
    let exhausted = gcsd::sdgecko_allocBuffer();
    let buffers_are_distinct = !first.is_null()
        && !second.is_null()
        && !third.is_null()
        && first != second
        && first != third
        && second != third
        && exhausted.is_null();
    // SAFETY: Free only live buffers returned by the matching SD Gecko pool.
    unsafe {
        gcsd::sdgecko_freeBuffer(first);
        gcsd::sdgecko_freeBuffer(second);
        gcsd::sdgecko_freeBuffer(third);
    }
    let reusable = gcsd::sdgecko_allocBuffer();
    logger.record(
        "C SD Gecko buffer pool ignores null free and allocates, exhausts, frees, and reuses buffers",
        buffers_are_distinct && !reusable.is_null(),
    );
    // SAFETY: Free the live buffer returned by the matching SD Gecko pool.
    unsafe { gcsd::sdgecko_freeBuffer(reusable) };
}

fn probe_gcsd_without_adapter(logger: &mut ProbeLogger) {
    use ogc_gcsd as gcsd;

    logger.section("GAMECUBE SD BACKEND WITHOUT AN ADAPTER");
    let init = gcsd::sdgecko_initIO(0);
    let pre_io = gcsd::sdgecko_preIO(0);
    let cid = gcsd::sdgecko_readCID(0);
    let csd = gcsd::sdgecko_readCSD(0);
    let status = gcsd::sdgecko_readStatus(0);
    logger.record(
        "SD Gecko init and metadata APIs report no card in slot A",
        [init, pre_io, cid, csd, status]
            .into_iter()
            .all(|result| result == GCSD_ERROR_NOCARD),
    );

    let interface = &gcsd::GAMECUBE_SD_A_INTERFACE;
    let startup = interface
        .startup
        .expect("slot A interface must provide startup");
    let is_inserted = interface
        .is_inserted
        .expect("slot A interface must provide insertion query");
    let read_sectors = interface
        .read_sectors
        .expect("slot A interface must provide sector reads");
    let write_sectors = interface
        .write_sectors
        .expect("slot A interface must provide sector writes");
    let clear_status = interface
        .clear_status
        .expect("slot A interface must provide status cleanup");
    let shutdown = interface
        .shutdown
        .expect("slot A interface must provide shutdown");
    let started = unsafe { startup() };
    let present = unsafe { is_inserted() };
    let mut sector = CardSector([0xa5; 512]);
    // SAFETY: The buffers are aligned and each transfer requests one sector.
    let read = unsafe { read_sectors(0, 1, sector.0.as_mut_ptr().cast()) };
    // SAFETY: The buffers are aligned and each transfer requests one sector.
    let write = unsafe { write_sectors(0, 1, sector.0.as_ptr().cast()) };
    let cleared = unsafe { clear_status() };
    let stopped = unsafe { shutdown() };
    logger.record(
        "slot A interface callbacks report no card and clean up",
        !started && !present && !read && !write && !cleared && stopped,
    );

    let do_unmount = gcsd::sdgecko_doUnmount(0);
    logger.record(
        "SD Gecko unmount succeeds when slot A has no card",
        do_unmount == 0,
    );

    let get_speed = gcsd::sdgecko_get_speed;
    let get_page_size = gcsd::sdgecko_get_page_size;
    let get_addressing_type = gcsd::sdgecko_get_addressing_type;
    let speed = get_speed(0);
    let page_size = get_page_size(0);
    let addressing = get_addressing_type(0);
    logger.record(
        "SD Gecko backend getters return slot A state",
        speed.is_some() && page_size.is_some() && addressing == Some(gcsd::CARD_IO_BYTE_ADDRESSING),
    );

    let original_page_size = page_size.unwrap_or(0);
    let set_page_size = gcsd::sdgecko_set_page_size;
    let changed_page_size = set_page_size(0, 512);
    let page_size_changed = get_page_size(0) == Some(512);
    let restored_page_size = set_page_size(0, original_page_size);
    logger.record(
        "SD Gecko page-size setter restores slot state after no-card errors",
        changed_page_size.is_some_and(|result| result < 0)
            && page_size_changed
            && restored_page_size.is_some_and(|result| result < 0)
            && get_page_size(0) == Some(original_page_size),
    );

    let changed_c_page_size = gcsd::sdgecko_setPageSize(0, 1024);
    let c_page_size_after = gcsd::sdgecko_getPageSize(0);
    let restored_c_page_size = set_page_size(0, original_page_size);
    logger.record(
        "C SD Gecko page-size setter reports no card and restores slot state",
        changed_c_page_size < 0
            && c_page_size_after == 1024
            && restored_c_page_size.is_some_and(|result| result < 0)
            && gcsd::sdgecko_getPageSize(0) == original_page_size,
    );

    let original_speed = speed.unwrap_or(0);
    let changed_c_speed = original_speed ^ 1;
    gcsd::sdgecko_setSpeed(0, changed_c_speed);
    let c_speed_after = gcsd::sdgecko_getSpeed(0);
    let restored_c_speed = gcsd::sdgecko_set_speed(0, original_speed);
    logger.record(
        "C SD Gecko speed setter updates and restores slot state",
        c_speed_after == changed_c_speed
            && restored_c_speed
            && gcsd::sdgecko_getSpeed(0) == original_speed,
    );

    logger.record(
        "C SD Gecko getters expose valid slot configuration",
        gcsd::sdgecko_getSpeed(0) == get_speed(0).unwrap_or(0)
            && gcsd::sdgecko_getPageSize(0) == get_page_size(0).unwrap_or(0)
            && gcsd::sdgecko_getAddressingType(0)
                == get_addressing_type(0).unwrap_or(gcsd::CARD_IO_SECTOR_ADDRESSING),
    );

    let read_sectors_result = gcsd::sdgecko_readSectors;
    let read_result = unsafe { read_sectors_result(0, 0, 1, sector.0.as_mut_ptr().cast()) };
    let write_sectors_result = gcsd::sdgecko_writeSectors;
    let write_result = unsafe { write_sectors_result(0, 0, 1, sector.0.as_ptr().cast()) };
    logger.record(
        "SD Gecko sector result APIs preserve no-card errors",
        read_result == GCSD_ERROR_NOCARD && write_result == GCSD_ERROR_NOCARD,
    );
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("FAT SD API PROBE");
    probe_gcsd_invalid_drive(logger);
    probe_gcsd_without_adapter(logger);

    let presence_result = fat::device_is_inserted(fat::MountSource::Sd);
    if !matches!(presence_result, Ok(true)) {
        let no_media = matches!(
            presence_result,
            Ok(false) | Err(fat::FatError::DeviceUnavailable)
        );
        logger.record("FAT SD presence reports no inserted media", no_media);
        let mount_result = fat::mount_sd();
        logger.record(
            "FAT SD mount reports unavailable without media",
            matches!(mount_result, Err(fat::FatError::DeviceUnavailable)),
        );
        if !no_media {
            logger.note("FAT API checks skipped after an unexpected SD presence result");
        } else {
            logger.note("FAT file checks skipped because no Wii SD media is present");
        }
        return;
    }

    logger.record("FAT SD presence reports inserted media", true);
    let mount_result = fat::mount_sd();
    logger.record("FAT SD mount", mount_result.is_ok());
    if mount_result.is_err() {
        logger.note("FAT API checks skipped because the virtual SD volume did not mount");
        return;
    }
    logger.note_i32(
        "FAT SD lookup immediately after mount",
        ogc_sysbase::find_device(c"sd:"),
    );

    let inserted = fat::device_is_inserted(fat::MountSource::Sd);
    logger.record("FAT SD insertion query", inserted == Ok(true));

    let generation = fat::media_generation(MOUNT);
    logger.record(
        "FAT mounted media generation is available",
        generation.is_some(),
    );

    let rust_label = fat::volume_label(MOUNT);
    let mut c_label = [0x5a as c_char; 12];
    // SAFETY: The mount name is a static C string and the output has 12 writable bytes.
    unsafe { fat::fatGetVolumeLabel(MOUNT.as_ptr(), c_label.as_mut_ptr()) };
    let label_matches = rust_label.as_ref().is_ok_and(|label| {
        label
            .iter()
            .enumerate()
            .all(|(index, byte)| c_label.get(index).is_some_and(|value| *value == *byte))
            && c_label.get(label.len()) == Some(&0)
    });
    logger.record("C and Rust FAT volume labels match", label_matches);

    // SAFETY: Null pointers are explicit error inputs for these C exports.
    let null_get_attr = unsafe { fat::FAT_getAttr(core::ptr::null()) };
    // SAFETY: Null pointers are explicit error inputs for these C exports.
    let null_set_attr = unsafe { fat::FAT_setAttr(core::ptr::null(), fat::ATTR_HIDDEN) };
    logger.record(
        "C FAT attribute exports reject null paths",
        null_get_attr == -1 && null_set_attr == -1,
    );

    let mut file = match fat::FatFile::create_new(PROBE_FILE) {
        Ok(file) => file,
        Err(_) => {
            logger.record("FAT temporary file create and cleanup", false);
            let _ = fat::unmount(MOUNT);
            return;
        }
    };
    let written = file.write(b"abcdef");
    file.seek(2);
    let seek_ok = file.position() == 2;
    let truncate_result = file.truncate(4);
    file.seek(0);
    let mut bytes = [0; 6];
    let read = file.read(&mut bytes);
    logger.record("FAT seek updates the file cursor", seek_ok && read == Ok(4));
    logger.record(
        "FAT truncate persists the requested length",
        written == Ok(6) && truncate_result.is_ok() && file.len() == 4 && &bytes[..4] == b"abcd",
    );

    let original_attr = unsafe { fat::FAT_getAttr(PROBE_FILE.as_ptr()) };
    let set_hidden = if original_attr >= 0 {
        // SAFETY: The static path is valid and the temporary file is mounted.
        unsafe { fat::FAT_setAttr(PROBE_FILE.as_ptr(), original_attr as u8 | fat::ATTR_HIDDEN) }
    } else {
        -1
    };
    // SAFETY: The static path is valid and the temporary file remains present.
    let hidden_attr = unsafe { fat::FAT_getAttr(PROBE_FILE.as_ptr()) };
    let restore_attr = if original_attr >= 0 {
        // SAFETY: Restore the exact attribute byte read from this temporary file.
        unsafe { fat::FAT_setAttr(PROBE_FILE.as_ptr(), original_attr as u8) }
    } else {
        -1
    };
    logger.record(
        "C FAT attribute exports set read back and restore the hidden bit",
        original_attr >= 0
            && set_hidden == 0
            && hidden_attr == (original_attr | i32::from(fat::ATTR_HIDDEN))
            && restore_attr == 0
            // SAFETY: The temporary path is valid and the file has not been removed.
            && unsafe { fat::FAT_getAttr(PROBE_FILE.as_ptr()) } == original_attr,
    );

    let closed = file.close().is_ok();
    let reopened = match fat::FatFile::open(PROBE_FILE) {
        Ok(mut reopened) => {
            let mut persisted = [0xa5; 4];
            let length = reopened.len();
            let read = reopened.read(&mut persisted);
            let write = reopened.write(b"x");
            let close = reopened.close();
            length == 4
                && read == Ok(4)
                && persisted == *b"abcd"
                && write == Err(fat::FatError::BadDescriptor)
                && close.is_ok()
        }
        Err(_) => false,
    };
    logger.record(
        "FAT read-only reopen preserves bytes and rejects writes",
        reopened,
    );
    let persistence_unmount = fat::unmount(MOUNT).is_ok();
    let persistence_remount = persistence_unmount && fat::mount_sd().is_ok();
    let persisted_after_remount = if persistence_remount {
        match fat::FatFile::open(PROBE_FILE) {
            Ok(mut file) => {
                let mut bytes = [0xa5; 4];
                let length = file.len();
                let read = file.read(&mut bytes);
                let close = file.close();
                length == 4 && read == Ok(4) && bytes == *b"abcd" && close.is_ok()
            }
            Err(_) => false,
        }
    } else {
        false
    };
    logger.record(
        "FAT truncated file persists across unmount and remount",
        closed && persistence_unmount && persistence_remount && persisted_after_remount,
    );
    if !persistence_remount {
        if fat::is_mounted(MOUNT) {
            let removed = fat::remove_file(PROBE_FILE).is_ok();
            let unmounted = fat::unmount(MOUNT).is_ok();
            logger.record("FAT remount failure cleanup", removed && unmounted);
        } else {
            logger.record("FAT remount failure cleanup", false);
        }
        return;
    }
    let generation = fat::media_generation(MOUNT);
    let removed = fat::remove_file(PROBE_FILE).is_ok();
    logger.record(
        "FAT temporary file closes and is removed",
        closed && removed,
    );
    let (created, created_cleanup) = if removed {
        match fat::FatFile::create(PROBE_FILE) {
            Ok(mut created) => {
                let written = created.write(b"new");
                let closed = created.close();
                let cleanup = fat::remove_file(PROBE_FILE);
                (written == Ok(3) && closed.is_ok(), cleanup.is_ok())
            }
            Err(_) => (false, false),
        }
    } else {
        (false, false)
    };
    logger.record(
        "FAT create writes a new file and cleanup removes it",
        created
            && created_cleanup
            && matches!(fat::FatFile::open(PROBE_FILE), Err(fat::FatError::NotFound)),
    );

    let init = fat::fatInit(2, false);
    logger.note_i32("C fatInit result", init as i32);
    let sd_device_index = ogc_sysbase::find_device(c"sd:");
    let sd_file_device_index = ogc_sysbase::find_device(c"sd:/libogc-fat-api-probe.tmp");
    logger.note_i32("devoptab FindDevice sd path", sd_device_index);
    logger.note_i32("devoptab FindDevice sd file path", sd_file_device_index);
    logger.note_i32(
        "FAT SD mounted after fatInit",
        fat::is_mounted(MOUNT) as i32,
    );
    let init_default = fat::fatInitDefault();
    logger.note_i32("C fatInitDefault result", init_default as i32);
    logger.note_i32(
        "FAT SD mounted after fatInitDefault",
        fat::is_mounted(MOUNT) as i32,
    );
    logger.record(
        "FAT init exports retain the mounted SD volume",
        init && init_default
            && fat::is_mounted(MOUNT)
            && sd_device_index >= 0
            && sd_file_device_index == sd_device_index,
    );

    let interface = core::ptr::from_ref(&WII_SD_INTERFACE);
    // SAFETY: The virtual SD interface is static and the volume is already online.
    let simple_mount = unsafe { fat::fatMountSimple(c"fatapi1".as_ptr(), interface) };
    // SAFETY: The static mount name identifies the alias mounted above.
    unsafe { fat::fatUnmount(c"fatapi1".as_ptr()) };
    logger.record(
        "C FAT simple mount registers and unregisters an alias",
        simple_mount && !fat::is_mounted(c"fatapi1"),
    );

    // SAFETY: The virtual SD interface is static and the volume is already online.
    let configured_mount = unsafe { fat::fatMount(c"fatapi2".as_ptr(), interface, 0, 2, 8) };
    // SAFETY: The static mount name identifies the alias mounted above.
    unsafe { fat::fatUnmount(c"fatapi2".as_ptr()) };
    // SAFETY: Null is a documented no-op input for fatUnmount.
    unsafe { fat::fatUnmount(core::ptr::null()) };
    logger.record(
        "C FAT configured mount registers and unregisters an alias",
        configured_mount && !fat::is_mounted(c"fatapi2"),
    );

    let same_path = fat::repair_interrupted_directory_rename(c"sd:/same", c"sd:/SAME");
    let missing_path = fat::repair_interrupted_directory_rename(c"sd:/missing-a", c"sd:/missing-b");
    logger.record(
        "FAT directory repair rejects duplicate and missing paths without changes",
        same_path == Ok(false) && missing_path == Err(fat::FatError::NotFound),
    );
    logger.record(
        "FAT media generation stays stable during mounted access",
        generation.is_some() && fat::media_generation(MOUNT) == generation,
    );

    let card_a = fat::mount_card_a();
    let card_b = fat::mount_card_b();
    logger.record(
        "FAT GameCube SD A reports unavailable without an EXI adapter",
        card_a == Err(fat::FatError::DeviceUnavailable) && !fat::is_mounted(c"carda"),
    );
    logger.record(
        "FAT GameCube SD B reports unavailable without an EXI adapter",
        card_b == Err(fat::FatError::DeviceUnavailable) && !fat::is_mounted(c"cardb"),
    );

    let unmounted = fat::unmount(MOUNT).is_ok();
    logger.record("FAT SD cleanup unmounts the volume", unmounted);
    let mounted = fat::mount(
        fat::MountSource::Sd,
        fat::MountOptions {
            mount_name: MOUNT,
            read_only: true,
        },
    );
    let registered = fat::is_mounted(MOUNT);
    let (create_rejected, unexpected_file_closed) = if mounted.is_ok() {
        match fat::FatFile::create_new(PROBE_FILE) {
            Err(fat::FatError::ReadOnly) => (true, true),
            Err(_) => (false, true),
            Ok(file) => (false, file.close().is_ok()),
        }
    } else {
        (false, false)
    };
    let absent = matches!(fat::FatFile::open(PROBE_FILE), Err(fat::FatError::NotFound));
    logger.record(
        "FAT read-only mount rejects file creation and preserves absence",
        create_rejected && absent && unexpected_file_closed,
    );
    let unmount = if mounted.is_ok() {
        fat::unmount(MOUNT).is_ok()
    } else {
        false
    };
    logger.record(
        "FAT configured read-only mount registers and cleans up",
        unmounted && mounted.is_ok() && registered && unmount && !fat::is_mounted(MOUNT),
    );
    if !absent && unmount {
        let cleanup_mount = fat::mount_sd().is_ok();
        let removed = cleanup_mount && fat::remove_file(PROBE_FILE).is_ok();
        let cleanup_unmount = cleanup_mount && fat::unmount(MOUNT).is_ok();
        logger.record("FAT unexpected file cleanup", removed && cleanup_unmount);
    }
}
