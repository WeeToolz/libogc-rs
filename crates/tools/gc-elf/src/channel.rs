//! Prepare the real-mode reset vector required for an IOS channel launch.
use super::{ElfError, Result};
use gc_binary::{align_up, checked_range, read_u32_be, write_u32_be};

fn word(bytes: &[u8], off: usize) -> Result<u32> {
    read_u32_be(bytes, off).map_err(|e| ElfError(e.to_string()))
}
fn set(bytes: &mut [u8], off: usize, value: u32) -> Result<()> {
    write_u32_be(bytes, off, value).map_err(|e| ElfError(e.to_string()))
}
fn memory(address: u32, size: u32) -> Result<std::ops::Range<u32>> {
    let end = address
        .checked_add(size)
        .ok_or_else(|| ElfError("DOL memory range overflow".into()))?;
    if !((address >= 0x80003400 && end <= 0x81800000)
        || (address >= 0x90000000 && end <= 0x94000000))
    {
        return Err(ElfError("DOL section lies outside cached Wii RAM".into()));
    }
    Ok(address..end)
}
/// Add HBC Reborn's native real-mode entry at 0x80003400 and branch to the
/// validated DOL entry. This is needed when IOS starts a channel at physical
/// 0x3400. The normal DOL entry field remains unchanged for DOL loaders.
pub fn prepare_channel_dol(input: &[u8]) -> Result<Vec<u8>> {
    checked_range(input.len(), 0, 0x100).map_err(|e| ElfError(e.to_string()))?;
    const VECTOR: &[u8] = include_bytes!("channel-entry.bin");
    let vector_range = 0x80003400..0x80003400 + VECTOR.len() as u32;
    let entry = word(input, 0xe0)?;
    if entry & 3 != 0 {
        return Err(ElfError("DOL entry is not instruction aligned".into()));
    }
    let mut slot = None;
    let mut entry_found = false;
    let mut ranges = Vec::new();
    let mut files = Vec::new();
    for n in 0..18 {
        let size = word(input, 0x90 + n * 4)?;
        if size == 0 {
            if n < 7 && slot.is_none() {
                slot = Some(n);
            }
            continue;
        }
        let address = word(input, 0x48 + n * 4)?;
        let file = word(input, n * 4)? as usize;
        if file < 0x100 {
            return Err(ElfError("DOL section overlaps its header".into()));
        }
        let file_range =
            checked_range(input.len(), file, size as usize).map_err(|e| ElfError(e.to_string()))?;
        let mem = memory(address, size)?;
        if mem.start < vector_range.end && vector_range.start < mem.end {
            return Err(ElfError("DOL overlaps channel entry vector".into()));
        }
        for previous in &ranges {
            let previous: &std::ops::Range<u32> = previous;
            if mem.start < previous.end && previous.start < mem.end {
                return Err(ElfError("DOL memory sections overlap".into()));
            }
        }
        for previous in &files {
            let previous: &std::ops::Range<usize> = previous;
            if file_range.start < previous.end && previous.start < file_range.end {
                return Err(ElfError("DOL file sections overlap".into()));
            }
        }
        if n < 7 && mem.contains(&entry) && entry.checked_add(4).is_some_and(|end| end <= mem.end) {
            entry_found = true;
        }
        ranges.push(mem);
        files.push(file_range);
    }
    let bss_size = word(input, 0xdc)?;
    if bss_size != 0 {
        let bss = memory(word(input, 0xd8)?, bss_size)?;
        if bss.start < vector_range.end && vector_range.start < bss.end {
            return Err(ElfError("DOL BSS overlaps channel entry vector".into()));
        }
    }
    if !entry_found {
        return Err(ElfError("DOL entry is outside its text sections".into()));
    }
    let slot =
        slot.ok_or_else(|| ElfError("DOL has no free text slot for channel entry".into()))?;
    let mut vector = VECTOR.to_vec();
    // These two instructions are lis r3,hi16(entry); ori r3,r3,lo16(entry).
    set(&mut vector, 0x124, 0x3c600000 | (entry >> 16))?;
    set(&mut vector, 0x128, 0x60630000 | (entry & 0xffff))?;
    let mut output = input.to_vec();
    let file_offset =
        align_up(output.len(), 32).ok_or_else(|| ElfError("DOL size overflow".into()))?;
    let offset32 =
        u32::try_from(file_offset).map_err(|_| ElfError("DOL file exceeds u32".into()))?;
    output.resize(file_offset, 0);
    output.extend_from_slice(&vector);
    set(&mut output, slot * 4, offset32)?;
    set(&mut output, 0x48 + slot * 4, vector_range.start)?;
    set(&mut output, 0x90 + slot * 4, VECTOR.len() as u32)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dol() -> Vec<u8> {
        let mut bytes = vec![0; 0x104];
        for (offset, value) in [
            (0, 0x100),
            (0x48, 0x80004000),
            (0x90, 4),
            (0xe0, 0x80004000),
        ] {
            set(&mut bytes, offset, value).unwrap();
        }
        bytes[0x100..].copy_from_slice(&[0x48, 0, 0, 0]);
        bytes
    }
    #[test]
    fn adds_real_entry_preserves_payload_and_sets_actual_destination() {
        let original = dol();
        let bytes = prepare_channel_dol(&original).unwrap();
        assert_eq!(&bytes[0x100..0x104], &[0x48, 0, 0, 0]);
        assert_eq!(word(&bytes, 4).unwrap(), 0x120);
        assert_eq!(word(&bytes, 0x4c).unwrap(), 0x80003400);
        assert_eq!(word(&bytes, 0x94).unwrap(), 0x140);
        assert_eq!(word(&bytes, 0xe0).unwrap(), 0x80004000);
        assert_eq!(&bytes[0x120..0x124], &[0x3c, 0x60, 0, 0x11]);
        assert_eq!(
            &bytes[0x244..0x24c],
            &[0x3c, 0x60, 0x80, 0, 0x60, 0x63, 0x40, 0]
        );
        assert_eq!(&bytes[bytes.len() - 4..], &[0x4c, 0, 0, 0x64]);
        assert!(prepare_channel_dol(&bytes).is_err());
    }
    #[test]
    fn rejects_bad_dol_ranges_entry_and_slots() {
        for (off, value) in [
            (0, 0x103),
            (0x48, 0x80003400),
            (0x48, 0xdead0000),
            (0xe0, 0x80004002),
            (0xe0, 0x80006000),
            (0xd8, 0x80003400),
        ] {
            let mut bytes = dol();
            set(&mut bytes, off, value).unwrap();
            if off == 0xd8 {
                set(&mut bytes, 0xdc, 4).unwrap();
            }
            assert!(prepare_channel_dol(&bytes).is_err());
        }
        let mut bytes = dol();
        for n in 1..7 {
            set(&mut bytes, n * 4, 0x100).unwrap();
            set(&mut bytes, 0x48 + n * 4, 0x80004000 + n as u32 * 4).unwrap();
            set(&mut bytes, 0x90 + n * 4, 4).unwrap();
        }
        assert!(prepare_channel_dol(&bytes).is_err());
        assert!(prepare_channel_dol(&[0; 255]).is_err());
    }
}
