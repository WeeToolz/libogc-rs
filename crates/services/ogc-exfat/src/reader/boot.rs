//! exFAT boot region validation.

use crate::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Geometry {
    pub(crate) volume_sectors: u64,
    pub(crate) fat_sector: u32,
    pub(crate) heap_sector: u32,
    pub(crate) cluster_count: u32,
    pub(crate) sectors_per_cluster: u32,
    pub(crate) root_cluster: u32,
}

impl Geometry {
    pub(crate) fn cluster_sector(self, cluster: u32) -> Option<u64> {
        let index = cluster.checked_sub(2)?;
        if index >= self.cluster_count {
            return None;
        }
        Some(u64::from(self.heap_sector) + u64::from(index) * u64::from(self.sectors_per_cluster))
    }
}

const MAX_CLUSTER_COUNT: u64 = (1 << 32) - 11;
const BOOT_REGION_SECTORS: u64 = 12;

pub(crate) fn read_boot<E: From<Error>>(
    start: u64,
    partition_sectors: u64,
    scratch: &mut [u8],
    read: &mut impl FnMut(u64, &mut [u8]) -> Result<(), E>,
) -> Result<Geometry, E> {
    if partition_sectors < 2 * BOOT_REGION_SECTORS
        || start.checked_add(BOOT_REGION_SECTORS - 1).is_none()
        || !matches!(scratch.len(), 512 | 1024 | 2048 | 4096)
    {
        return Err(Error::InvalidBoot.into());
    }
    read(start, scratch)?;
    let geometry = parse_geometry(scratch, partition_sectors)?;
    let mut checksum = 0u32;
    for (offset, byte) in scratch.iter().copied().enumerate() {
        // Active status fields can change without an update to the checksum.
        if !matches!(offset, 106 | 107 | 112) {
            checksum = checksum.rotate_right(1).wrapping_add(u32::from(byte));
        }
    }

    for sector in 1..=10 {
        read(start + sector, scratch)?;
        if sector <= 8 && !scratch.ends_with(&[0, 0, 0x55, 0xaa]) {
            return Err(Error::InvalidBoot.into());
        }
        for &byte in scratch.iter() {
            checksum = checksum.rotate_right(1).wrapping_add(u32::from(byte));
        }
    }

    read(start + 11, scratch)?;
    if !scratch
        .chunks_exact(4)
        .all(|word| word == checksum.to_le_bytes())
    {
        return Err(Error::InvalidBoot.into());
    }
    Ok(geometry)
}

fn parse_geometry(boot: &[u8], partition_sectors: u64) -> Result<Geometry, Error> {
    if boot[..11] != *b"\xeb\x76\x90EXFAT   "
        || boot[11..64].iter().any(|&byte| byte != 0)
        || boot[510..512] != [0x55, 0xaa]
    {
        return Err(Error::InvalidBoot);
    }
    if boot[105] != 1 || boot[110] != 1 {
        return Err(Error::Unsupported);
    }
    let sector_shift = boot[108];
    let cluster_shift = boot[109];
    if !(9..=12).contains(&sector_shift)
        || usize::from(sector_shift) != boot.len().trailing_zeros() as usize
        || cluster_shift > 25 - sector_shift
        || boot[106] & 1 != 0
        || (boot[112] > 100 && boot[112] != 0xff)
    {
        return Err(Error::InvalidBoot);
    }

    let volume_sectors = u64::from_le_bytes(boot[72..80].try_into().expect("boot field length"));
    let fat_sector = word(boot, 80);
    let fat_length = word(boot, 84);
    let heap_sector = word(boot, 88);
    let cluster_count = word(boot, 92);
    let root_cluster = word(boot, 96);
    let sectors_per_cluster = 1u32 << cluster_shift;
    let heap_sectors = volume_sectors
        .checked_sub(u64::from(heap_sector))
        .ok_or(Error::InvalidBoot)?;
    if volume_sectors < (1 << (20 - sector_shift))
        || volume_sectors > partition_sectors
        || u64::from(fat_sector) < 2 * BOOT_REGION_SECTORS
        || u64::from(fat_sector) + u64::from(fat_length) > u64::from(heap_sector)
        || (u64::from(fat_length) << sector_shift) < (u64::from(cluster_count) + 2) * 4
        || cluster_count == 0
        || u64::from(cluster_count)
            != (heap_sectors / u64::from(sectors_per_cluster)).min(MAX_CLUSTER_COUNT)
    {
        return Err(Error::InvalidBoot);
    }
    let geometry = Geometry {
        volume_sectors,
        fat_sector,
        heap_sector,
        cluster_count,
        sectors_per_cluster,
        root_cluster,
    };
    geometry
        .cluster_sector(root_cluster)
        .ok_or(Error::InvalidBoot)?;
    Ok(geometry)
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("boot field length"),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn boot_region() -> [[u8; 512]; 12] {
        let mut region = [[0; 512]; 12];
        let boot = &mut region[0];
        boot[..11].copy_from_slice(b"\xeb\x76\x90EXFAT   ");
        boot[72..80].copy_from_slice(&4096u64.to_le_bytes());
        boot[80..84].copy_from_slice(&24u32.to_le_bytes());
        boot[84..88].copy_from_slice(&32u32.to_le_bytes());
        boot[88..92].copy_from_slice(&56u32.to_le_bytes());
        boot[92..96].copy_from_slice(&4040u32.to_le_bytes());
        boot[96..100].copy_from_slice(&2u32.to_le_bytes());
        boot[100..104].copy_from_slice(&0x1010_1010u32.to_le_bytes());
        boot[104..106].copy_from_slice(&0x0100u16.to_le_bytes());
        boot[108..113].copy_from_slice(&[9, 0, 1, 0x80, 0xff]);
        boot[510..].copy_from_slice(&[0x55, 0xaa]);
        for sector in &mut region[1..9] {
            sector[510..].copy_from_slice(&[0x55, 0xaa]);
        }
        // Computed independently over the 11 sectors above.
        for word in region[11].chunks_exact_mut(4) {
            word.copy_from_slice(&0x2299_4472u32.to_le_bytes());
        }
        region
    }

    #[test]
    fn test_read_boot_checks_geometry_and_checksum() {
        let mut region = boot_region();
        let mut scratch = [0; 512];
        let mut read = |lba: u64, out: &mut [u8]| {
            out.copy_from_slice(&region[lba as usize]);
            Ok::<(), Error>(())
        };
        let geometry = read_boot(0, 4096, &mut scratch, &mut read).unwrap();
        assert_eq!(geometry.root_cluster, 2);
        assert_eq!(geometry.cluster_sector(4041), Some(4095));
        assert_eq!(geometry.cluster_sector(4042), None);
        assert!(read_boot(0, 4095, &mut scratch, &mut read).is_err());

        region[2][16] = 1;
        assert_eq!(
            read_boot(0, 4096, &mut scratch, &mut |lba, out| {
                out.copy_from_slice(&region[lba as usize]);
                Ok::<(), Error>(())
            }),
            Err(Error::InvalidBoot)
        );
    }
}
