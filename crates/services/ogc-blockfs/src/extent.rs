//! Identify an exFAT extent before selecting the filesystem backend.

#[repr(C, align(32))]
pub(crate) struct Sector(pub(crate) [u8; 512]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtentError {
    Io,
    InvalidGeometry,
}

pub(crate) fn exfat_extent(
    mut read_sector: impl FnMut(u32, &mut Sector) -> Result<(), ExtentError>,
) -> Result<Option<u64>, ExtentError> {
    let mut boot = Sector([0; 512]);
    read_sector(0, &mut boot)?;
    if boot.0[3..11] == *b"EXFAT   " {
        let sectors = u64::from_le_bytes(boot.0[72..80].try_into().expect("8-byte field"));
        if sectors == 0 || sectors > u64::from(u32::MAX) + 1 {
            return Err(ExtentError::InvalidGeometry);
        }
        return Ok(Some(sectors));
    }
    if boot.0[510..512] != [0x55, 0xaa] {
        return Ok(None);
    }
    // Keep the original MBR entries. Reading a candidate boot sector
    // overwrites the aligned scratch buffer.
    let mut entries = [0; 64];
    entries.copy_from_slice(&boot.0[446..510]);
    for entry in entries.chunks_exact(16) {
        if entry[4] != 0x07 {
            continue;
        }
        let start = u64::from(u32::from_le_bytes(
            entry[8..12].try_into().expect("4-byte LBA"),
        ));
        let length = u64::from(u32::from_le_bytes(
            entry[12..16].try_into().expect("4-byte size"),
        ));
        let end = start
            .checked_add(length)
            .ok_or(ExtentError::InvalidGeometry)?;
        if start == 0 || length == 0 || end > u64::from(u32::MAX) + 1 {
            return Err(ExtentError::InvalidGeometry);
        }
        let start_lba = u32::try_from(start).map_err(|_| ExtentError::InvalidGeometry)?;
        read_sector(start_lba, &mut boot)?;
        if boot.0[3..11] == *b"EXFAT   " {
            let volume_length =
                u64::from_le_bytes(boot.0[72..80].try_into().expect("8-byte field"));
            if volume_length == 0 || volume_length > length {
                return Err(ExtentError::InvalidGeometry);
            }
            return Ok(Some(end));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partition(mbr: &mut [u8; 512], slot: usize, kind: u8, start: u32, length: u32) {
        let offset = 446 + slot * 16;
        mbr[offset + 4] = kind;
        mbr[offset + 8..offset + 12].copy_from_slice(&start.to_le_bytes());
        mbr[offset + 12..offset + 16].copy_from_slice(&length.to_le_bytes());
        mbr[510..512].copy_from_slice(&[0x55, 0xaa]);
    }

    fn exfat_boot(length: u64) -> [u8; 512] {
        let mut boot = [0; 512];
        boot[3..11].copy_from_slice(b"EXFAT   ");
        boot[72..80].copy_from_slice(&length.to_le_bytes());
        boot
    }

    #[test]
    fn later_exfat_partition_is_found_after_non_exfat_candidate() {
        let mut mbr = [0; 512];
        partition(&mut mbr, 0, 0x07, 32, 16);
        partition(&mut mbr, 2, 0x07, 64, 128);
        let mut reads = [u32::MAX; 3];
        let mut count = 0;
        let extent = exfat_extent(|lba, out| {
            reads[count] = lba;
            count += 1;
            out.0 = match lba {
                0 => mbr,
                32 => [0; 512], // e.g. NTFS, not an exFAT boot record
                64 => exfat_boot(120),
                _ => return Err(ExtentError::Io),
            };
            Ok(())
        });
        assert_eq!(extent, Ok(Some(192)));
        assert_eq!(reads, [0, 32, 64]);
    }

    #[test]
    fn candidate_boot_bytes_are_not_treated_as_partition_entries() {
        let mut mbr = [0; 512];
        partition(&mut mbr, 0, 0x07, 32, 16);
        let mut candidate = [0; 512];
        partition(&mut candidate, 1, 0x07, 100, 20);
        let extent = exfat_extent(|lba, out| {
            out.0 = match lba {
                0 => mbr,
                32 => candidate,
                _ => return Err(ExtentError::Io),
            };
            Ok(())
        });
        assert_eq!(extent, Ok(None));
    }

    #[test]
    fn sector_zero_and_fat_fallback_keep_existing_behavior() {
        let limit = u64::from(u32::MAX) + 1;
        for (length, expected) in [
            (1, Ok(Some(1))),
            (limit, Ok(Some(limit))),
            (0, Err(ExtentError::InvalidGeometry)),
            (limit + 1, Err(ExtentError::InvalidGeometry)),
        ] {
            assert_eq!(
                exfat_extent(|lba, out| {
                    assert_eq!(lba, 0);
                    out.0 = exfat_boot(length);
                    Ok(())
                }),
                expected
            );
        }
        let mut mbr = [0; 512];
        partition(&mut mbr, 0, 0x0c, 32, 16);
        assert_eq!(
            exfat_extent(|lba, out| {
                assert_eq!(lba, 0);
                out.0 = mbr;
                Ok(())
            }),
            Ok(None)
        );
    }

    #[test]
    fn partition_geometry_and_read_errors_are_not_fat_fallback() {
        for (start, length) in [(0, 16), (32, 0), (u32::MAX, 2)] {
            let mut mbr = [0; 512];
            partition(&mut mbr, 0, 0x07, start, length);
            assert_eq!(
                exfat_extent(|lba, out| {
                    assert_eq!(lba, 0);
                    out.0 = mbr;
                    Ok(())
                }),
                Err(ExtentError::InvalidGeometry)
            );
        }
        let mut mbr = [0; 512];
        partition(&mut mbr, 0, 0x07, 32, 16);
        assert_eq!(
            exfat_extent(|lba, out| {
                if lba != 0 {
                    return Err(ExtentError::Io);
                }
                out.0 = mbr;
                Ok(())
            }),
            Err(ExtentError::Io)
        );
        for volume_length in [0, 17] {
            assert_eq!(
                exfat_extent(|lba, out| {
                    out.0 = if lba == 0 {
                        mbr
                    } else {
                        exfat_boot(volume_length)
                    };
                    Ok(())
                }),
                Err(ExtentError::InvalidGeometry)
            );
        }
    }
}
