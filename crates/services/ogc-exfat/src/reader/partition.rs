//! Find a read-only exFAT volume on sector-zero or primary MBR media.

use crate::Error;

use super::boot::{Geometry, read_boot};

pub(crate) fn locate<E: From<Error>>(
    sectors: u64,
    scratch: &mut [u8],
    read: &mut impl FnMut(u64, &mut [u8]) -> Result<(), E>,
) -> Result<(u64, Geometry), E> {
    if sectors == 0 || !matches!(scratch.len(), 512 | 1024 | 2048 | 4096) {
        return Err(Error::InvalidBoot.into());
    }
    read(0, scratch)?;
    if is_exfat(scratch) {
        return checked_volume(0, sectors, scratch, read);
    }
    if scratch[510..512] != [0x55, 0xaa] {
        return Err(Error::InvalidBoot.into());
    }
    let mut entries = [0; 64];
    entries.copy_from_slice(&scratch[446..510]);
    for entry in entries.chunks_exact(16) {
        if entry[4] != 0x07 {
            continue;
        }
        let start = mbr_word(entry, 8);
        let length = mbr_word(entry, 12);
        if start == 0 || length == 0 || start + length > sectors {
            return Err(Error::InvalidBoot.into());
        }
        read(start, scratch)?;
        if is_exfat(scratch) {
            return checked_volume(start, length, scratch, read);
        }
    }
    Err(Error::InvalidBoot.into())
}

fn checked_volume<E: From<Error>>(
    start: u64,
    sectors: u64,
    scratch: &mut [u8],
    read: &mut impl FnMut(u64, &mut [u8]) -> Result<(), E>,
) -> Result<(u64, Geometry), E> {
    let main = read_boot(start, sectors, scratch, read)?;
    let backup = read_boot(start + 12, sectors, scratch, read)?;
    if main != backup {
        return Err(Error::InvalidBoot.into());
    }
    Ok((start, main))
}

fn is_exfat(boot: &[u8]) -> bool {
    boot[3..11] == *b"EXFAT   "
}

fn mbr_word(entry: &[u8], offset: usize) -> u64 {
    u32::from_le_bytes(
        entry[offset..offset + 4]
            .try_into()
            .expect("MBR field length"),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::super::boot::tests::boot_region;
    use super::*;

    #[test]
    fn test_locate_bounds_mbr_and_checks_backup() {
        let region = boot_region();
        let mut mbr = [0; 512];
        mbr[450] = 0x07;
        mbr[454..458].copy_from_slice(&2048u32.to_le_bytes());
        mbr[458..462].copy_from_slice(&4096u32.to_le_bytes());
        mbr[510..].copy_from_slice(&[0x55, 0xaa]);
        let mut scratch = [0; 512];

        let mut run = |length, bad_backup| {
            locate(length, &mut scratch, &mut |lba, out| {
                if lba == 0 {
                    out.copy_from_slice(&mbr);
                } else {
                    out.copy_from_slice(&region[((lba - 2048) % 12) as usize]);
                    if bad_backup && lba == 2071 {
                        out[511] ^= 1;
                    }
                }
                Ok::<(), Error>(())
            })
        };
        assert_eq!(run(6144, false).unwrap().0, 2048);
        assert_eq!(run(6143, false), Err(Error::InvalidBoot));
        assert_eq!(run(6144, true), Err(Error::InvalidBoot));
    }

    #[test]
    fn test_locate_sector_zero() {
        let region = boot_region();
        let (base, geometry) = locate(4096, &mut [0; 512], &mut |lba, out| {
            out.copy_from_slice(&region[(lba % 12) as usize]);
            Ok::<(), Error>(())
        })
        .unwrap();
        assert_eq!(base, 0);
        assert_eq!(geometry.volume_sectors, 4096);
    }
}
