use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ogc_exfat::{Error, Volume};
use ogc_filesystem::DiscInterface;
use ogc_wiisd::WII_SD_INTERFACE;

use crate::probe_support::ProbeLogger;

const SMALL_CARD: u64 = 131_072;
const LARGE_CARD: u64 = 16_777_216;
const LARGE_MARKER: u64 = (1 << 32) + 17;
const PAYLOAD: &[u8] = b"libogc-rs exFAT fixture\n";
static WRITE_ATTEMPTED: AtomicBool = AtomicBool::new(false);
static FAULT_SECTOR: AtomicU32 = AtomicU32::new(u32::MAX);
static FAULT_OFFSET: AtomicU32 = AtomicU32::new(0);

#[repr(align(32))]
struct Sector([u8; 512]);

unsafe extern "C" fn trap_write(_sector: u32, _count: u32, _data: *const c_void) -> bool {
    WRITE_ATTEMPTED.store(true, Ordering::Relaxed);
    false
}

unsafe extern "C" fn checked_read(sector: u32, count: u32, out: *mut c_void) -> bool {
    let Some(read) = WII_SD_INTERFACE.read_sectors else {
        return false;
    };
    // SAFETY: Forward the original aligned buffer and its sector count.
    if !unsafe { read(sector, count, out) } {
        return false;
    }
    if sector == FAULT_SECTOR.load(Ordering::Relaxed) && count == 1 {
        let offset = FAULT_OFFSET.load(Ordering::Relaxed) as usize;
        if offset >= 512 {
            return false;
        }
        // SAFETY: The successful read initialized all 512 bytes in the output.
        unsafe { *out.cast::<u8>().add(offset) ^= 1 };
    }
    true
}

// Reads use the real Wii SD driver. Writes go to the trap, even with Dolphin
// write protection enabled. Read faults are armed only for separate checks.
static READ_ONLY_SD: DiscInterface = DiscInterface {
    read_sectors: Some(checked_read),
    write_sectors: Some(trap_write),
    ..WII_SD_INTERFACE
};

struct Fixture {
    start: u32,
    card_sectors: u64,
    cluster_bytes: u64,
    root: u32,
}

impl Fixture {
    /// Read the addresses needed to position faults and choose the test file.
    fn detect() -> Option<Self> {
        let mut sector = Sector([0; 512]);
        read_sector(0, &mut sector)?;
        let start = if sector.0[3..11] == *b"EXFAT   " {
            0
        } else {
            if sector.0[450] != 7 || sector.0[510..] != [0x55, 0xaa] {
                return None;
            }
            let start = word(&sector.0, 454);
            read_sector(start, &mut sector)?;
            start
        };
        let boot = &sector.0;
        if boot[3..11] != *b"EXFAT   " || boot[108] != 9 {
            return None;
        }
        let volume_length = u64::from_le_bytes(boot[72..80].try_into().ok()?);
        let card_sectors = u64::from(start).checked_add(volume_length)?;
        if !matches!(card_sectors, SMALL_CARD | LARGE_CARD) {
            return None;
        }
        let cluster_sectors = 1u32.checked_shl(u32::from(boot[109]))?;
        let root = word(boot, 96)
            .checked_sub(2)?
            .checked_mul(cluster_sectors)?
            .checked_add(word(boot, 88))?
            .checked_add(start)?;
        Some(Self {
            start,
            card_sectors,
            cluster_bytes: 512 * u64::from(cluster_sectors),
            root,
        })
    }
}

fn word(sector: &[u8; 512], offset: usize) -> u32 {
    u32::from_le_bytes(
        sector[offset..offset + 4]
            .try_into()
            .expect("fixture field size"),
    )
}

fn read_sector(lba: u32, sector: &mut Sector) -> Option<()> {
    let read = WII_SD_INTERFACE.read_sectors?;
    // SAFETY: The private SD images have 512-byte sectors and the buffer is aligned.
    unsafe { read(lba, 1, sector.0.as_mut_ptr().cast()) }.then_some(())
}

pub(crate) fn run(logger: &mut ProbeLogger) {
    logger.section("READ-ONLY EXFAT");
    let startup = WII_SD_INTERFACE.startup.expect("Wii SD startup callback");
    let shutdown = WII_SD_INTERFACE.shutdown.expect("Wii SD shutdown callback");
    // SAFETY: The probe has exclusive access to the private Dolphin SD image.
    let started = unsafe { startup() };
    logger.record("Wii SD started for exFAT", started);
    if !started {
        return;
    }

    let fixture = Fixture::detect();
    logger.record("exFAT SD fixture geometry recognized", fixture.is_some());
    if let Some(fixture) = fixture {
        logger.note(&alloc::format!(
            "fixture start={} sectors={} cluster-bytes={}",
            fixture.start,
            fixture.card_sectors,
            fixture.cluster_bytes,
        ));
        probe_exfat_apis(logger, &fixture);
        if fixture.card_sectors == SMALL_CARD {
            probe_mount_errors(logger, &fixture);
        }
    }
    logger.record(
        "exFAT never requested a device write",
        !WRITE_ATTEMPTED.load(Ordering::Relaxed),
    );

    // SAFETY: The probe finished all reads and dropped the volume.
    logger.record("Wii SD shut down after exFAT", unsafe { shutdown() });
}

fn probe_exfat_apis(logger: &mut ProbeLogger, fixture: &Fixture) {
    // SAFETY: The fixture is private, immutable, and was generated with this geometry.
    let mounted = unsafe { Volume::mount(&READ_ONLY_SD, fixture.card_sectors, 512) };
    logger.record("exFAT mounted from Wii SD", mounted.is_ok());
    let mut volume = match mounted {
        Ok(volume) => volume,
        Err(error) => {
            logger.note(&alloc::format!("mount error: {error:?}"));
            return;
        }
    };
    if fixture.card_sectors == LARGE_CARD {
        probe_large_file(logger, &mut volume);
        return;
    }
    probe_listing(logger, &mut volume);
    probe_contiguous_file(logger, &mut volume);
    probe_fragmented_file(logger, &mut volume, fixture.cluster_bytes);
}

fn probe_listing(logger: &mut ProbeLogger, volume: &mut Volume) {
    let mut found = false;
    let listed = volume.visit_dir("/", |entry| {
        if entry.name == "ReadMe.txt" && entry.size == PAYLOAD.len() as u64 && !entry.is_dir {
            found = true;
        }
    });
    logger.record(
        "exFAT root lists the expected file",
        listed.is_ok() && found,
    );
    let mut found = false;
    let listed = volume.visit_dir("/sUB", |entry| {
        if entry.name == "Child.txt" && entry.size == 6 && !entry.is_dir {
            found = true;
        }
    });
    logger.record("exFAT lists a nested directory", listed.is_ok() && found);
    logger.record(
        "exFAT stat preserves name and size with case-insensitive lookup",
        volume.stat("/readme.TXT").is_ok_and(|info| {
            info.name == "ReadMe.txt" && info.size == PAYLOAD.len() as u64 && !info.is_dir
        }),
    );
    logger.record(
        "exFAT stat rejects a missing path",
        volume.stat("missing") == Err(Error::NotFound),
    );
    logger.record(
        "exFAT rejects an invalid path component",
        volume.stat("../ReadMe.txt") == Err(Error::InvalidPath),
    );
    logger.record(
        "exFAT rejects listing a file",
        volume.visit_dir("ReadMe.txt", |_| {}) == Err(Error::NotDirectory),
    );
    logger.record(
        "exFAT rejects opening a directory as a file",
        matches!(volume.open("Sub"), Err(Error::IsDirectory)),
    );
}

fn probe_contiguous_file(logger: &mut ProbeLogger, volume: &mut Volume) {
    let mut file = match volume.open("/README.TXT") {
        Ok(file) => file,
        Err(error) => {
            logger.record("exFAT opened the expected file", false);
            logger.note(&alloc::format!("open error: {error:?}"));
            return;
        }
    };
    logger.record("exFAT opened the expected file", true);
    let mut bytes = [0u8; 64];
    logger.record(
        "exFAT read matches the independent payload",
        file.read(&mut bytes) == Ok(PAYLOAD.len()) && &bytes[..PAYLOAD.len()] == PAYLOAD,
    );
    file.seek(4);
    logger.record(
        "exFAT seek and read use the requested offset",
        file.read(&mut bytes[..5]) == Ok(5) && bytes[..5] == PAYLOAD[4..9],
    );
    file.seek(PAYLOAD.len() as u64);
    bytes.fill(0xa5);
    logger.record(
        "exFAT end-of-file read preserves the output buffer",
        file.read(&mut bytes) == Ok(0) && bytes == [0xa5; 64],
    );
}

fn probe_fragmented_file(logger: &mut ProbeLogger, volume: &mut Volume, cluster_bytes: u64) {
    logger.record(
        "exFAT stat reports both clusters of the fragmented file",
        volume
            .stat("Fragment.dat")
            .is_ok_and(|entry| entry.size == 2 * cluster_bytes),
    );
    let opened = volume.open("Fragment.dat");
    logger.record("exFAT opens a fragmented file", opened.is_ok());
    let Ok(mut file) = opened else { return };
    let mut out = [0xa5; 8];
    file.seek(cluster_bytes - 2);
    logger.record(
        "exFAT follows a nonadjacent FAT link and zero fills the uninitialized tail",
        file.read(&mut out) == Ok(8) && out == *b"ABCD\0\0\0\0",
    );
    file.seek(0);
    logger.record(
        "exFAT can seek backward after following a FAT link",
        file.read(&mut out) == Ok(8) && out == *b"AAAAAAAA",
    );
}

fn probe_large_file(logger: &mut ProbeLogger, volume: &mut Volume) {
    logger.record(
        "exFAT stat preserves the size above 4 GiB",
        volume
            .stat("Large.dat")
            .is_ok_and(|info| info.size == LARGE_MARKER + 64),
    );
    let opened = volume.open("Large.dat");
    logger.record("exFAT opens the large file", opened.is_ok());
    let Ok(mut file) = opened else { return };
    let mut out = [0xa5; 8];
    file.seek(LARGE_MARKER);
    logger.record(
        "exFAT seeks above 4 GiB and reads the marker and zero-filled tail",
        file.read(&mut out) == Ok(8) && out == *b"OVER4G\0\0",
    );
    file.seek(0);
    logger.record(
        "exFAT can seek back from above 4 GiB",
        file.read(&mut out[..4]) == Ok(4) && out[..4] == *b"HEAD",
    );
}

fn probe_mount_errors(logger: &mut ProbeLogger, fixture: &Fixture) {
    let inject = |lba, offset, expected| {
        FAULT_OFFSET.store(offset, Ordering::Relaxed);
        FAULT_SECTOR.store(lba, Ordering::Relaxed);
        // SAFETY: The callback changes only the returned data for the selected read.
        let result = unsafe { Volume::mount(&READ_ONLY_SD, fixture.card_sectors, 512) };
        FAULT_SECTOR.store(u32::MAX, Ordering::Relaxed);
        matches!(result, Err(error) if error == expected)
    };
    logger.record(
        "exFAT rejects a corrupted boot checksum",
        inject(fixture.start + 2, 16, Error::InvalidBoot),
    );
    let mut root = Sector([0; 512]);
    let entry = read_sector(fixture.root, &mut root)
        .and_then(|()| root.0.chunks_exact(32).position(|slot| slot[0] == 0x85));
    logger.record(
        "exFAT rejects a corrupted directory entry checksum",
        entry.is_some_and(|entry| inject(fixture.root, (entry * 32 + 2) as u32, Error::Corrupt)),
    );
    logger.record(
        "exFAT propagates a failed device read",
        inject(0, 512, Error::Io),
    );
}
