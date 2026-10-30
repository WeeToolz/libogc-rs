//! ELF32 PowerPC to DOL conversion.

mod channel;
pub use channel::prepare_channel_dol;

use gc_binary::{align_up, checked_range, read_u16_be, read_u32_be, write_u32_be};
use std::fmt;
use std::fs;
use std::path::Path;

const DOL_HEADER_SIZE: usize = 0x100;
const TEXT_SLOTS: usize = 7;
const DATA_SLOTS: usize = 11;
const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DolProfile {
    /// Match elf2dol's physical-address mapping and 64-byte section alignment.
    LegacyElf2Dol,
    /// Match the converter used by libogc-rs xtask.
    Libogc,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ElfError(pub String);

impl fmt::Display for ElfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ElfError {}

type Result<T> = std::result::Result<T, ElfError>;

#[derive(Clone, Copy, Debug)]
struct Segment {
    offset: u32,
    address: u32,
    file_size: u32,
    memory_size: u32,
    flags: u32,
}

#[derive(Debug)]
struct ElfImage {
    entry: u32,
    segments: Vec<Segment>,
}

#[derive(Default)]
struct DolHeader {
    text_offsets: [u32; TEXT_SLOTS],
    data_offsets: [u32; DATA_SLOTS],
    text_addresses: [u32; TEXT_SLOTS],
    data_addresses: [u32; DATA_SLOTS],
    text_sizes: [u32; TEXT_SLOTS],
    data_sizes: [u32; DATA_SLOTS],
    bss_address: u32,
    bss_size: u32,
    entry: u32,
}

impl DolHeader {
    fn write(&self, output: &mut [u8]) -> Result<()> {
        for (base, values) in [
            (0x00, &self.text_offsets[..]),
            (0x1c, &self.data_offsets[..]),
            (0x48, &self.text_addresses[..]),
            (0x64, &self.data_addresses[..]),
            (0x90, &self.text_sizes[..]),
            (0xac, &self.data_sizes[..]),
        ] {
            for (index, value) in values.iter().enumerate() {
                write_u32_be(output, base + index * 4, *value).map_err(binary_error)?;
            }
        }
        write_u32_be(output, 0xd8, self.bss_address).map_err(binary_error)?;
        write_u32_be(output, 0xdc, self.bss_size).map_err(binary_error)?;
        write_u32_be(output, 0xe0, self.entry).map_err(binary_error)?;
        Ok(())
    }
}

/// Convert ELF bytes to DOL bytes with an explicitly selected layout profile.
pub fn convert_elf_to_dol(input: &[u8], profile: DolProfile) -> Result<Vec<u8>> {
    let elf = parse_elf(input, profile)?;
    match profile {
        DolProfile::LegacyElf2Dol => pack_legacy(input, &elf),
        DolProfile::Libogc => pack_libogc(input, &elf),
    }
}

/// Convert an ELF file to a DOL file using the legacy command's file behavior.
pub fn convert_elf_file(input: &Path, output: &Path, profile: DolProfile) -> Result<()> {
    let bytes =
        fs::read(input).map_err(|error| ElfError(format!("{}: {error}", input.display())))?;
    let dol = convert_elf_to_dol(&bytes, profile)?;
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|error| ElfError(format!("{}: {error}", parent.display())))?;
    }
    fs::write(output, dol).map_err(|error| ElfError(format!("{}: {error}", output.display())))
}

fn parse_elf(bytes: &[u8], profile: DolProfile) -> Result<ElfImage> {
    if bytes.len() < 52 {
        return Err(ElfError("input is too small to be an ELF32 header".into()));
    }
    if bytes.get(..4) != Some(b"\x7fELF") {
        return Err(ElfError("input is not an ELF image".into()));
    }
    if bytes[4] != 1 || bytes[5] != 2 || bytes[6] != 1 {
        return Err(ElfError(
            "expected a current-version, big-endian ELF32 image".into(),
        ));
    }
    if profile == DolProfile::LegacyElf2Dol {
        if read_u16_be(bytes, 16).map_err(binary_error)? != 2 {
            return Err(ElfError("ELF is not an executable".into()));
        }
        if read_u16_be(bytes, 18).map_err(binary_error)? != 20 {
            return Err(ElfError("ELF machine is not PowerPC".into()));
        }
        if read_u32_be(bytes, 20).map_err(binary_error)? != 1 {
            return Err(ElfError("invalid ELF version".into()));
        }
    }
    let entry = read_u32_be(bytes, 24).map_err(binary_error)?;
    if profile == DolProfile::LegacyElf2Dol && entry == 0 {
        return Err(ElfError("ELF has no entry point".into()));
    }
    let table_offset = usize::try_from(read_u32_be(bytes, 28).map_err(binary_error)?)
        .map_err(|_| ElfError("program header offset does not fit this host".into()))?;
    let header_size = usize::from(read_u16_be(bytes, 42).map_err(binary_error)?);
    let count = usize::from(read_u16_be(bytes, 44).map_err(binary_error)?);
    if header_size < 32 {
        return Err(ElfError(format!(
            "unexpected program header size {header_size}"
        )));
    }
    let table_size = header_size
        .checked_mul(count)
        .ok_or_else(|| ElfError("program header table size overflows".into()))?;
    checked_range(bytes.len(), table_offset, table_size).map_err(binary_error)?;
    if profile == DolProfile::LegacyElf2Dol
        && (count == 0 || table_offset == 0 || header_size != 32)
    {
        return Err(ElfError("ELF has no valid program header table".into()));
    }

    let mut segments = Vec::new();
    for index in 0..count {
        let base = table_offset + index * header_size;
        if read_u32_be(bytes, base).map_err(binary_error)? != PT_LOAD {
            continue;
        }
        let offset = read_u32_be(bytes, base + 4).map_err(binary_error)?;
        let virtual_address = read_u32_be(bytes, base + 8).map_err(binary_error)?;
        let physical_address = read_u32_be(bytes, base + 12).map_err(binary_error)?;
        let file_size = read_u32_be(bytes, base + 16).map_err(binary_error)?;
        let memory_size = read_u32_be(bytes, base + 20).map_err(binary_error)?;
        let flags = read_u32_be(bytes, base + 24).map_err(binary_error)?;
        if memory_size == 0 && profile == DolProfile::LegacyElf2Dol {
            continue;
        }
        if file_size > memory_size {
            return Err(ElfError(format!(
                "load segment {index} file size exceeds memory size"
            )));
        }
        let file_range =
            checked_range(bytes.len(), offset as usize, file_size as usize).map_err(|_| {
                ElfError(format!(
                    "load segment {index} extends past the end of the ELF file"
                ))
            })?;
        let _ = file_range;
        let address = if profile == DolProfile::LegacyElf2Dol {
            physical_address
        } else {
            virtual_address
        };
        address
            .checked_add(memory_size)
            .ok_or_else(|| ElfError(format!("load segment {index} overflows the address space")))?;
        segments.push(Segment {
            offset,
            address,
            file_size,
            memory_size,
            flags,
        });
    }
    if segments.is_empty() && profile == DolProfile::Libogc {
        return Err(ElfError("ELF image has no loadable segments".into()));
    }
    if profile == DolProfile::Libogc {
        segments.sort_by_key(|segment| (segment.address, segment.offset));
    }
    Ok(ElfImage { entry, segments })
}

fn pack_legacy(input: &[u8], elf: &ElfImage) -> Result<Vec<u8>> {
    let mut header = DolHeader {
        entry: elf.entry,
        ..DolHeader::default()
    };
    let mut output = vec![0; DOL_HEADER_SIZE];
    let mut position = DOL_HEADER_SIZE;
    let mut text_count = 0;
    let mut data_count = 0;

    for segment in elf
        .segments
        .iter()
        .filter(|segment| segment.flags & PF_X != 0)
        .chain(
            elf.segments
                .iter()
                .filter(|segment| segment.flags & PF_X == 0),
        )
    {
        let is_text = segment.flags & PF_X != 0;
        let slot = if is_text {
            let slot = text_count;
            text_count += 1;
            if slot >= TEXT_SLOTS {
                return Err(ElfError("too many TEXT segments for DOL".into()));
            }
            slot
        } else if segment.file_size != 0 {
            let slot = data_count;
            data_count += 1;
            if slot >= DATA_SLOTS {
                return Err(ElfError("too many DATA segments for DOL".into()));
            }
            slot
        } else {
            continue;
        };

        position =
            align_up(position, 64).ok_or_else(|| ElfError("DOL output size overflows".into()))?;
        let file_offset = u32::try_from(position)
            .map_err(|_| ElfError("DOL output exceeds 32-bit offsets".into()))?;
        if is_text {
            header.text_offsets[slot] = file_offset;
            header.text_addresses[slot] = segment.address;
            header.text_sizes[slot] = segment.file_size;
        } else {
            header.data_offsets[slot] = file_offset;
            header.data_addresses[slot] = segment.address;
            header.data_sizes[slot] = segment.file_size;
        }
        output.resize(position, 0);
        let start = segment.offset as usize;
        let end = start + segment.file_size as usize;
        output.extend_from_slice(&input[start..end]);
        position += segment.file_size as usize;
    }
    if text_count == 0 {
        header.text_offsets[0] = DOL_HEADER_SIZE as u32;
    }
    if data_count == 0 {
        header.data_offsets[0] = DOL_HEADER_SIZE as u32;
    }
    set_legacy_bss(&mut header, &elf.segments)?;
    header.write(&mut output)?;
    Ok(output)
}

fn pack_libogc(input: &[u8], elf: &ElfImage) -> Result<Vec<u8>> {
    let executable_base = elf
        .segments
        .iter()
        .filter(|segment| segment.flags & PF_X != 0)
        .map(|segment| segment.address)
        .min()
        .unwrap_or(0);
    let mut header = DolHeader {
        entry: elf.entry,
        ..DolHeader::default()
    };
    let mut output = vec![0; DOL_HEADER_SIZE];
    let mut position = DOL_HEADER_SIZE;
    let mut text_count = 0;
    let mut data_count = 0;
    let mut bss = Vec::new();
    for segment in &elf.segments {
        if executable_base != 0 && segment.address < executable_base {
            continue;
        }
        if segment.file_size == 0 {
            if segment.memory_size > 0 {
                bss.push((segment.address, segment.address + segment.memory_size));
            }
            continue;
        }
        let is_text = segment.flags & PF_X != 0;
        let slot = if is_text {
            let slot = text_count;
            text_count += 1;
            if slot >= TEXT_SLOTS {
                return Err(ElfError("too many executable segments for DOL".into()));
            }
            slot
        } else {
            let slot = data_count;
            data_count += 1;
            if slot >= DATA_SLOTS {
                return Err(ElfError("too many data segments for DOL".into()));
            }
            slot
        };
        position =
            align_up(position, 32).ok_or_else(|| ElfError("DOL output size overflows".into()))?;
        let file_offset = u32::try_from(position)
            .map_err(|_| ElfError("DOL output exceeds 32-bit offsets".into()))?;
        if is_text {
            header.text_offsets[slot] = file_offset;
            header.text_addresses[slot] = segment.address;
            header.text_sizes[slot] = segment.file_size;
        } else {
            header.data_offsets[slot] = file_offset;
            header.data_addresses[slot] = segment.address;
            header.data_sizes[slot] = segment.file_size;
        }
        output.resize(position, 0);
        let start = segment.offset as usize;
        let end = start + segment.file_size as usize;
        output.extend_from_slice(&input[start..end]);
        position += segment.file_size as usize;
        if segment.memory_size > segment.file_size {
            bss.push((
                segment.address + segment.file_size,
                segment.address + segment.memory_size,
            ));
        }
    }
    // Loaders read each section in 32-byte blocks, including the final section.
    // Keep the original sizes in the header so padding does not change BSS.
    let padded_size =
        align_up(position, 32).ok_or_else(|| ElfError("DOL output size overflows".into()))?;
    output.resize(padded_size, 0);
    set_bss(&mut header, bss)?;
    header.write(&mut output)?;
    Ok(output)
}

fn legacy_bss_range(segments: &[Segment]) -> Result<Option<(u32, u32)>> {
    let mut range: Option<(u32, u32)> = None;

    for segment in segments {
        let is_text = segment.flags & PF_X != 0;
        if segment.memory_size <= segment.file_size || (!is_text && segment.file_size != 0) {
            continue;
        }

        let start = segment
            .address
            .checked_add(segment.file_size)
            .ok_or_else(|| ElfError("BSS start overflows address space".into()))?;
        let end = segment
            .address
            .checked_add(segment.memory_size)
            .ok_or_else(|| ElfError("BSS end overflows address space".into()))?;

        match range {
            None => range = Some((start, end)),
            Some((current_start, current_end)) if current_end == start => {
                range = Some((current_start, end));
            }
            Some(_) => {}
        }
    }

    Ok(range)
}

fn set_legacy_bss(header: &mut DolHeader, segments: &[Segment]) -> Result<()> {
    if let Some((start, end)) = legacy_bss_range(segments)? {
        header.bss_address = start;
        header.bss_size = end - start;
    }
    Ok(())
}

fn set_bss(header: &mut DolHeader, mut ranges: Vec<(u32, u32)>) -> Result<()> {
    if ranges.is_empty() {
        return Ok(());
    }
    ranges.sort_unstable();
    let start = ranges[0].0;
    let mut end = ranges[0].1;
    for (_, next_end) in ranges.into_iter().skip(1) {
        end = end.max(next_end);
    }
    header.bss_address = start;
    header.bss_size = end - start;
    Ok(())
}

fn binary_error(error: gc_binary::BoundsError) -> ElfError {
    ElfError(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 0x180];
        bytes[..7].copy_from_slice(b"\x7fELF\x01\x02\x01");
        bytes[16..18].copy_from_slice(&2u16.to_be_bytes());
        bytes[18..20].copy_from_slice(&20u16.to_be_bytes());
        bytes[20..24].copy_from_slice(&1u32.to_be_bytes());
        bytes[24..28].copy_from_slice(&0x8000_3100u32.to_be_bytes());
        bytes[28..32].copy_from_slice(&52u32.to_be_bytes());
        bytes[40..42].copy_from_slice(&52u16.to_be_bytes());
        bytes[42..44].copy_from_slice(&32u16.to_be_bytes());
        bytes[44..46].copy_from_slice(&2u16.to_be_bytes());
        for (index, offset, vaddr, paddr, size, mem, flags) in [
            (0, 0x80u32, 0x8000_3100u32, 0x100u32, 4u32, 4u32, PF_X),
            (1, 0x90u32, 0x8000_4000u32, 0x200u32, 8u32, 0x20u32, 0),
        ] {
            let base = 52 + index * 32;
            bytes[base..base + 4].copy_from_slice(&PT_LOAD.to_be_bytes());
            bytes[base + 4..base + 8].copy_from_slice(&offset.to_be_bytes());
            bytes[base + 8..base + 12].copy_from_slice(&vaddr.to_be_bytes());
            bytes[base + 12..base + 16].copy_from_slice(&paddr.to_be_bytes());
            bytes[base + 16..base + 20].copy_from_slice(&size.to_be_bytes());
            bytes[base + 20..base + 24].copy_from_slice(&mem.to_be_bytes());
            bytes[base + 24..base + 28].copy_from_slice(&flags.to_be_bytes());
        }
        bytes[0x80..0x84].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        bytes[0x90..0x98].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x11, 0x22]);
        bytes
    }

    #[test]
    fn legacy_profile_uses_physical_addresses_and_64_byte_alignment() {
        let dol = convert_elf_to_dol(&fixture(), DolProfile::LegacyElf2Dol).unwrap();
        assert_eq!(read_u32_be(&dol, 0).unwrap(), 0x100);
        assert_eq!(read_u32_be(&dol, 0x1c).unwrap(), 0x140);
        assert_eq!(read_u32_be(&dol, 0x48).unwrap(), 0x100);
        assert_eq!(read_u32_be(&dol, 0x64).unwrap(), 0x200);
        assert_eq!(read_u32_be(&dol, 0xd8).unwrap(), 0);
        assert_eq!(read_u32_be(&dol, 0xdc).unwrap(), 0);
        assert_eq!(read_u32_be(&dol, 0xe0).unwrap(), 0x8000_3100);
        assert_eq!(&dol[0x100..0x104], &[0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn legacy_profile_omits_initialized_data_bss_tail() {
        let dol = convert_elf_to_dol(&fixture(), DolProfile::LegacyElf2Dol).unwrap();
        assert_eq!(read_u32_be(&dol, 0x64).unwrap(), 0x200);
        assert_eq!(read_u32_be(&dol, 0xac).unwrap(), 8);
        assert_eq!(
            &dol[0x140..0x148],
            &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x11, 0x22]
        );
        assert_eq!(read_u32_be(&dol, 0xd8).unwrap(), 0);
        assert_eq!(read_u32_be(&dol, 0xdc).unwrap(), 0);
    }

    #[test]
    fn legacy_profile_converts_images_without_nonempty_load_segments() {
        for empty_kind in [0, 1] {
            let mut bytes = fixture();
            for index in 0..2 {
                let base = 52 + index * 32;
                if empty_kind == 0 {
                    bytes[base..base + 4].copy_from_slice(&0u32.to_be_bytes());
                } else {
                    bytes[base + 20..base + 24].copy_from_slice(&0u32.to_be_bytes());
                }
            }

            let dol = convert_elf_to_dol(&bytes, DolProfile::LegacyElf2Dol).unwrap();
            assert_eq!(dol.len(), DOL_HEADER_SIZE);
            assert_eq!(read_u32_be(&dol, 0).unwrap(), DOL_HEADER_SIZE as u32);
            assert_eq!(read_u32_be(&dol, 0x1c).unwrap(), DOL_HEADER_SIZE as u32);
            assert_eq!(read_u32_be(&dol, 0xe0).unwrap(), 0x8000_3100);
            assert!(convert_elf_to_dol(&bytes, DolProfile::Libogc).is_err());
        }
    }

    #[test]
    fn legacy_bss_matches_c_add_bss_order_and_adjacency_rules() {
        let text = |address, file_size, memory_size| Segment {
            offset: 0,
            address,
            file_size,
            memory_size,
            flags: PF_X,
        };
        let bss = |address, memory_size| Segment {
            offset: 0,
            address,
            file_size: 0,
            memory_size,
            flags: 0,
        };

        assert_eq!(
            legacy_bss_range(&[text(0x1000, 0x10, 0x20), bss(0x1020, 0x10)]).unwrap(),
            Some((0x1010, 0x1030))
        );
        assert_eq!(
            legacy_bss_range(&[bss(0x2000, 0x10), bss(0x3000, 0x10)]).unwrap(),
            Some((0x2000, 0x2010))
        );
        assert_eq!(
            legacy_bss_range(&[bss(0x3000, 0x20), bss(0x3010, 0x10)]).unwrap(),
            Some((0x3000, 0x3020))
        );
        assert_eq!(
            legacy_bss_range(&[bss(0x4000, 0x10), bss(0x3ff0, 0x10)]).unwrap(),
            Some((0x4000, 0x4010))
        );
    }

    #[test]
    fn libogc_profile_uses_virtual_addresses_and_32_byte_alignment() {
        let dol = convert_elf_to_dol(&fixture(), DolProfile::Libogc).unwrap();
        assert_eq!(read_u32_be(&dol, 0).unwrap(), 0x100);
        assert_eq!(read_u32_be(&dol, 0x1c).unwrap(), 0x120);
        assert_eq!(read_u32_be(&dol, 0x48).unwrap(), 0x8000_3100);
        assert_eq!(read_u32_be(&dol, 0x64).unwrap(), 0x8000_4000);
        assert_eq!(&dol[0x100..0x104], &[0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn libogc_profile_pads_the_final_data_section_for_loader_reads() {
        let dol = convert_elf_to_dol(&fixture(), DolProfile::Libogc).unwrap();
        assert_eq!(dol.len(), 0x140);
        assert_eq!(read_u32_be(&dol, 0xac).unwrap(), 8);
        assert_eq!(
            &dol[0x120..0x128],
            &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x11, 0x22]
        );
        assert_eq!(&dol[0x128..0x140], &[0; 24]);
        assert_eq!(read_u32_be(&dol, 0xd8).unwrap(), 0x8000_4008);
        assert_eq!(read_u32_be(&dol, 0xdc).unwrap(), 0x18);
    }

    #[test]
    fn libogc_profile_pads_the_final_text_section_for_loader_reads() {
        let mut input = fixture();
        input[44..46].copy_from_slice(&1u16.to_be_bytes());
        let dol = convert_elf_to_dol(&input, DolProfile::Libogc).unwrap();
        assert_eq!(dol.len(), 0x120);
        assert_eq!(read_u32_be(&dol, 0x90).unwrap(), 4);
        assert_eq!(&dol[0x100..0x104], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(&dol[0x104..0x120], &[0; 28]);
    }

    #[test]
    fn rejects_invalid_load_segments_in_both_profiles() {
        for profile in [DolProfile::LegacyElf2Dol, DolProfile::Libogc] {
            for (offset, value, expected) in [
                (
                    52 + 4,
                    0x17fu32,
                    "load segment 0 extends past the end of the ELF file",
                ),
                (
                    52 + 16,
                    5u32,
                    "load segment 0 file size exceeds memory size",
                ),
            ] {
                let mut bytes = fixture();
                bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                assert_eq!(convert_elf_to_dol(&bytes, profile).unwrap_err().0, expected);
            }
            let mut bytes = fixture();
            let address_offset = if profile == DolProfile::LegacyElf2Dol {
                52 + 12
            } else {
                52 + 8
            };
            bytes[address_offset..address_offset + 4].copy_from_slice(&u32::MAX.to_be_bytes());
            assert_eq!(
                convert_elf_to_dol(&bytes, profile).unwrap_err().0,
                "load segment 0 overflows the address space"
            );
        }
    }

    #[test]
    fn rejects_truncated_and_incompatible_images() {
        assert!(convert_elf_to_dol(b"no", DolProfile::LegacyElf2Dol).is_err());
        let mut bytes = fixture();
        bytes[4] = 2;
        assert!(convert_elf_to_dol(&bytes, DolProfile::Libogc).is_err());
    }
}
