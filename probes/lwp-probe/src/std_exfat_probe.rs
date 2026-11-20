use ogc_system_probe::probe_support::ProbeLogger;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};

const LARGE_MARKER: u64 = (1 << 32) + 17;

pub(crate) fn run(logger: &mut ProbeLogger) {
    logger.section("WII STD EXFAT");
    logger.record(
        "filesystem ABI rejects a null request",
        unsafe { ogc_std_runtime::__wii_std_fs(core::ptr::null_mut()) }
            == -i64::from(ogc_sysbase::EINVAL),
    );
    logger.record(
        "blockfs maps removed media to the device errno",
        ogc_blockfs::errno(&ogc_blockfs::Error::MediaRemoved) == ogc_sysbase::ENODEV,
    );
    let mounted = ogc_blockfs::mount_sd();
    logger.record(
        "std exFAT mount selects the exFAT backend",
        mounted == Ok(ogc_blockfs::Format::Exfat),
    );
    if mounted.is_err() {
        return;
    }

    let root = match fs::read_dir("sd:/") {
        Ok(directory) => directory,
        Err(error) => {
            logger.record("std lists the exFAT root directory", false);
            logger.note(&format!("read_dir error: {error}"));
            let _ = ogc_blockfs::unmount_sd();
            return;
        }
    };
    let mut names = Vec::new();
    let mut list_ok = true;
    for entry in root {
        match entry.and_then(|entry| {
            entry.file_name().into_string().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "non-UTF-8 exFAT name")
            })
        }) {
            Ok(name) => names.push(name),
            Err(_) => list_ok = false,
        }
    }
    logger.record(
        "std lists the exFAT root directory",
        list_ok && !names.is_empty(),
    );
    probe_blockfs_api(logger, &names);

    if names.iter().any(|name| name == "Large.dat") {
        probe_large_file(logger);
    } else {
        probe_fragmented_file(logger, &names);
    }

    let test_file = if names.iter().any(|name| name == "Large.dat") {
        "sd:/Large.dat"
    } else {
        "sd:/ReadMe.txt"
    };
    let readonly_open = OpenOptions::new().write(true).open(test_file);
    logger.record(
        "std rejects write access to exFAT with EROFS",
        readonly_open
            .err()
            .is_some_and(|error| error.raw_os_error() == Some(ogc_sysbase::EROFS)),
    );
    let create = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open("sd:/std-exfat-must-not-exist.tmp");
    logger.record(
        "std rejects exFAT file creation with EROFS",
        create
            .err()
            .is_some_and(|error| error.raw_os_error() == Some(ogc_sysbase::EROFS)),
    );
    logger.record(
        "std rejects exFAT file removal",
        fs::remove_file(test_file)
            .err()
            .is_some_and(|error| error.raw_os_error() == Some(ogc_sysbase::EROFS)),
    );
    logger.record(
        "std metadata reads file size and type",
        fs::metadata(if names.iter().any(|name| name == "Large.dat") {
            "sd:/Large.dat"
        } else {
            "sd:/Fragment.dat"
        })
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().readonly()),
    );

    logger.record(
        "exFAT std mount closes after all std handles drop",
        ogc_blockfs::unmount_sd().is_ok() && !ogc_blockfs::exfat_is_mounted(),
    );
}

fn probe_blockfs_api(logger: &mut ProbeLogger, names: &[String]) {
    let large = names.iter().any(|name| name == "Large.dat");
    let path = if large { "Large.dat" } else { "ReadMe.txt" };
    let info = ogc_blockfs::stat(path);
    let size = info.as_ref().map_or(0, |info| info.size);
    logger.record(
        "blockfs stat returns the fixture file metadata",
        info.as_ref()
            .is_ok_and(|info| info.name == path && !info.is_dir && info.size > 0),
    );
    let file = ogc_blockfs::open(path);
    logger.record("blockfs opens an exFAT read handle", file.is_ok());
    let Ok(mut file) = file else { return };
    let mut prefix = [0; 4];
    let read = file.read(&mut prefix).is_ok_and(|count| count == 4);
    logger.record("blockfs reads from an exFAT handle", read);
    logger.record(
        "blockfs reports the handle position and 64-bit file size",
        file.position() == 4 && file.len().is_ok_and(|length| length == size),
    );
    let seek = file.seek(0).is_ok() && file.position() == 0;
    logger.record("blockfs seeks an exFAT handle", seek);
    let mut directory = ogc_blockfs::open_dir("/");
    logger.record("blockfs opens an exFAT directory handle", directory.is_ok());
    let next = directory
        .as_mut()
        .is_ok_and(|directory| directory.next().is_ok_and(|entry| entry.is_some()));
    logger.record("blockfs reads the next directory entry", next);
    drop(directory);
    logger.record(
        "blockfs keeps the mount busy while a handle is open",
        ogc_blockfs::unmount_sd() == Err(ogc_blockfs::Error::Busy),
    );
}

fn probe_fragmented_file(logger: &mut ProbeLogger, names: &[String]) {
    logger.record(
        "std root includes the fixture files",
        names.iter().any(|name| name == "ReadMe.txt")
            && names.iter().any(|name| name == "Fragment.dat"),
    );
    let readme = File::open("sd:/ReadMe.txt");
    let fragment = File::open("sd:/Fragment.dat");
    logger.record(
        "std opens two exFAT files at the same time",
        readme.is_ok() && fragment.is_ok(),
    );
    let (Ok(mut readme), Ok(mut fragment)) = (readme, fragment) else {
        return;
    };
    let mut header = [0; 4];
    let readme_ok = readme.read_exact(&mut header).is_ok()
        && header == *b"libo"
        && readme.stream_position().is_ok_and(|position| position == 4);
    let mut boundary = [0xa5; 8];
    let fragment_ok = fragment.seek(SeekFrom::Start(4094)).is_ok()
        && fragment.read_exact(&mut boundary).is_ok()
        && boundary == *b"ABCD\0\0\0\0";
    logger.record(
        "std reads independent exFAT file handles",
        readme_ok && fragment_ok,
    );
    logger.record("std seek and read follow fragmented clusters", fragment_ok);
    logger.record(
        "std unmount rejects live file handles",
        ogc_blockfs::unmount_sd() == Err(ogc_blockfs::Error::Busy),
    );
    drop(fragment);
    drop(readme);

    let nested = fs::read_dir("sd:/Sub").is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name() == "Child.txt")
    });
    logger.record("std lists a nested exFAT directory", nested);
}

fn probe_large_file(logger: &mut ProbeLogger) {
    let metadata = fs::metadata("sd:/Large.dat");
    logger.record(
        "std metadata preserves an exFAT file size above 4 GiB",
        metadata.is_ok_and(|metadata| metadata.len() == LARGE_MARKER + 64),
    );
    let file = File::open("sd:/Large.dat");
    logger.record("std opens an exFAT file larger than 4 GiB", file.is_ok());
    let Ok(mut file) = file else { return };
    let mut marker = [0xa5; 8];
    let read = file.seek(SeekFrom::Start(LARGE_MARKER)).is_ok()
        && file.read_exact(&mut marker).is_ok()
        && marker == *b"OVER4G\0\0";
    logger.record("std seeks and reads above 4 GiB", read);
    logger.record(
        "std unmount rejects a live large-file handle",
        ogc_blockfs::unmount_sd() == Err(ogc_blockfs::Error::Busy),
    );
    drop(file);
}
