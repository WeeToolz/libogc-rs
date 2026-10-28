//! One clean sector, shared by production FAT entry reads. Writes invalidate it.
extern crate alloc;
use alloc::vec::Vec;

#[derive(Default)]
pub(crate) struct SectorCache {
    sector: Option<u32>,
    bytes: Vec<u8>,
}
impl SectorCache {
    pub(crate) fn invalidate(&mut self, sector: u32) {
        if self.sector == Some(sector) {
            self.sector = None;
        }
    }
    pub(crate) fn read<E>(
        &mut self,
        sector: u32,
        output: &mut [u8],
        read: impl FnOnce(&mut [u8]) -> Result<(), E>,
    ) -> Result<(), E> {
        if self.sector == Some(sector) && self.bytes.len() == output.len() {
            output.copy_from_slice(&self.bytes);
            return Ok(());
        }
        self.sector = None;
        read(output)?;
        self.bytes.clear();
        self.bytes.extend_from_slice(output);
        self.sector = Some(sector);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sequential_fat_entries_read_one_real_sector_then_writes_force_reload() {
        let mut cache = SectorCache::default();
        let mut output = [0; 512];
        let mut reads = 0;
        for _ in 0..128 {
            cache
                .read(10, &mut output, |bytes| {
                    reads += 1;
                    bytes.fill(0xa5);
                    Ok::<_, i32>(())
                })
                .unwrap();
            assert_eq!(output, [0xa5; 512]);
        }
        assert_eq!(reads, 1);
        cache.invalidate(10);
        cache
            .read(10, &mut output, |bytes| {
                reads += 1;
                bytes.fill(0x51);
                Ok::<_, i32>(())
            })
            .unwrap();
        assert_eq!(reads, 2);
        assert_eq!(output, [0x51; 512]);
    }
    #[test]
    fn failure_never_publishes_partial_sector_data() {
        let mut cache = SectorCache::default();
        let mut output = [0; 512];
        assert_eq!(
            cache.read(10, &mut output, |bytes| {
                bytes.fill(0xee);
                Err(-7)
            }),
            Err(-7)
        );
        cache
            .read(10, &mut output, |bytes| {
                bytes.fill(0x26);
                Ok::<_, i32>(())
            })
            .unwrap();
        assert_eq!(output, [0x26; 512]);
    }
    #[test]
    fn different_sector_or_sector_size_requires_new_io() {
        let mut cache = SectorCache::default();
        cache.read(1, &mut [0; 512], |_| Ok::<_, i32>(())).unwrap();
        assert_eq!(cache.read(2, &mut [0; 512], |_| Err(-1)), Err(-1));
        cache.read(1, &mut [0; 512], |_| Ok::<_, i32>(())).unwrap();
        assert_eq!(cache.read(1, &mut [0; 1024], |_| Err(-2)), Err(-2));
    }
}
