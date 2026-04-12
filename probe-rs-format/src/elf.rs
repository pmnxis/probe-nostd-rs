//! Minimal ELF parser for extracting loadable segments.
//!
//! Only parses ELF32 and ELF64 headers and program headers to find
//! PT_LOAD segments. Does NOT parse section headers, symbols, debug
//! info, or relocations.
//!
//! This covers the 95% use case for flashing ARM/RISC-V firmware.

use crate::error::FormatError;
use crate::segment::DataSegment;

// ELF magic
const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

// ELF class (32 vs 64 bit)
const ELFCLASS32: u8 = 1;
const ELFCLASS64: u8 = 2;

// ELF endianness
const ELFDATA2LSB: u8 = 1; // Little-endian
const ELFDATA2MSB: u8 = 2; // Big-endian

// Program header type
const PT_LOAD: u32 = 1;

// ELF header offsets (common to 32/64)
const EI_CLASS: usize = 4;
const EI_DATA: usize = 5;

/// Parse an ELF file and extract all PT_LOAD segments.
///
/// Only loadable segments with non-zero file size are returned.
/// Physical address (`p_paddr`) is used as the target address.
pub fn parse_elf<E>(
    data: &[u8],
    mut callback: impl FnMut(DataSegment<'_>) -> Result<(), E>,
) -> Result<(), ElfError<E>> {
    // Validate ELF magic
    if data.len() < 16 || data[..4] != ELF_MAGIC {
        return Err(ElfError::Format(FormatError::InvalidMagic));
    }

    let class = data[EI_CLASS];
    let endian = data[EI_DATA];

    match class {
        ELFCLASS32 => parse_elf32(data, endian, &mut callback),
        ELFCLASS64 => parse_elf64(data, endian, &mut callback),
        _ => Err(ElfError::Format(FormatError::UnsupportedFormat)),
    }
}

/// Parse ELF32 program headers and extract PT_LOAD segments.
fn parse_elf32<E>(
    data: &[u8],
    endian: u8,
    callback: &mut impl FnMut(DataSegment<'_>) -> Result<(), E>,
) -> Result<(), ElfError<E>> {
    // ELF32 header size: 52 bytes minimum
    if data.len() < 52 {
        return Err(ElfError::Format(FormatError::UnexpectedEof));
    }

    let read_u16 = |off| read_u16_endian(data, off, endian);
    let read_u32 = |off| read_u32_endian(data, off, endian);

    // e_phoff: program header table offset (offset 28, 4 bytes)
    let e_phoff = read_u32(28) as usize;
    // e_phentsize: program header entry size (offset 42, 2 bytes)
    let e_phentsize = read_u16(42) as usize;
    // e_phnum: number of program header entries (offset 44, 2 bytes)
    let e_phnum = read_u16(44) as usize;

    if e_phentsize < 32 {
        return Err(ElfError::Format(FormatError::InvalidRecord));
    }

    for i in 0..e_phnum {
        let ph_off = e_phoff + i * e_phentsize;
        if ph_off + e_phentsize > data.len() {
            return Err(ElfError::Format(FormatError::UnexpectedEof));
        }

        // ELF32 Phdr: p_type(4) p_offset(4) p_vaddr(4) p_paddr(4) p_filesz(4) p_memsz(4) ...
        let p_type = read_u32_endian(data, ph_off, endian);
        let p_offset = read_u32_endian(data, ph_off + 4, endian) as usize;
        let p_paddr = read_u32_endian(data, ph_off + 12, endian);
        let p_filesz = read_u32_endian(data, ph_off + 16, endian) as usize;

        if p_type != PT_LOAD || p_filesz == 0 {
            continue;
        }

        if p_offset + p_filesz > data.len() {
            return Err(ElfError::Format(FormatError::UnexpectedEof));
        }

        let segment = DataSegment {
            address: p_paddr,
            data: &data[p_offset..p_offset + p_filesz],
        };
        callback(segment).map_err(ElfError::Callback)?;
    }

    Ok(())
}

/// Parse ELF64 program headers and extract PT_LOAD segments.
fn parse_elf64<E>(
    data: &[u8],
    endian: u8,
    callback: &mut impl FnMut(DataSegment<'_>) -> Result<(), E>,
) -> Result<(), ElfError<E>> {
    // ELF64 header size: 64 bytes minimum
    if data.len() < 64 {
        return Err(ElfError::Format(FormatError::UnexpectedEof));
    }

    let read_u16 = |off| read_u16_endian(data, off, endian);
    let read_u64 = |off| read_u64_endian(data, off, endian);

    // e_phoff: offset 32, 8 bytes
    let e_phoff = read_u64(32) as usize;
    // e_phentsize: offset 54, 2 bytes
    let e_phentsize = read_u16(54) as usize;
    // e_phnum: offset 56, 2 bytes
    let e_phnum = read_u16(56) as usize;

    if e_phentsize < 56 {
        return Err(ElfError::Format(FormatError::InvalidRecord));
    }

    for i in 0..e_phnum {
        let ph_off = e_phoff + i * e_phentsize;
        if ph_off + e_phentsize > data.len() {
            return Err(ElfError::Format(FormatError::UnexpectedEof));
        }

        // ELF64 Phdr: p_type(4) p_flags(4) p_offset(8) p_vaddr(8) p_paddr(8) p_filesz(8) p_memsz(8) ...
        let p_type = read_u32_endian(data, ph_off, endian);
        let p_offset = read_u64_endian(data, ph_off + 8, endian) as usize;
        let p_paddr = read_u64_endian(data, ph_off + 24, endian);
        let p_filesz = read_u64_endian(data, ph_off + 32, endian) as usize;

        if p_type != PT_LOAD || p_filesz == 0 {
            continue;
        }

        if p_offset + p_filesz > data.len() {
            return Err(ElfError::Format(FormatError::UnexpectedEof));
        }

        // Truncate 64-bit address to 32-bit (embedded targets are 32-bit)
        let segment = DataSegment {
            address: p_paddr as u32,
            data: &data[p_offset..p_offset + p_filesz],
        };
        callback(segment).map_err(ElfError::Callback)?;
    }

    Ok(())
}

/// Error from `parse_elf`.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ElfError<E> {
    Format(FormatError),
    Callback(E),
}

// -- Endian-aware reading helpers --

fn read_u16_endian(data: &[u8], offset: usize, endian: u8) -> u16 {
    let bytes = [data[offset], data[offset + 1]];
    match endian {
        ELFDATA2LSB => u16::from_le_bytes(bytes),
        ELFDATA2MSB => u16::from_be_bytes(bytes),
        _ => u16::from_le_bytes(bytes), // default to LE
    }
}

fn read_u32_endian(data: &[u8], offset: usize, endian: u8) -> u32 {
    let bytes = [
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ];
    match endian {
        ELFDATA2LSB => u32::from_le_bytes(bytes),
        ELFDATA2MSB => u32::from_be_bytes(bytes),
        _ => u32::from_le_bytes(bytes),
    }
}

fn read_u64_endian(data: &[u8], offset: usize, endian: u8) -> u64 {
    let bytes = [
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ];
    match endian {
        ELFDATA2LSB => u64::from_le_bytes(bytes),
        ELFDATA2MSB => u64::from_be_bytes(bytes),
        _ => u64::from_le_bytes(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    /// Build a minimal ELF32 LE with one PT_LOAD segment.
    fn make_minimal_elf32(paddr: u32, payload: &[u8]) -> Vec<u8> {
        let mut elf = vec![0u8; 52 + 32 + payload.len()]; // header + 1 phdr + data

        // ELF magic
        elf[0..4].copy_from_slice(&ELF_MAGIC);
        elf[EI_CLASS] = ELFCLASS32;
        elf[EI_DATA] = ELFDATA2LSB;

        // e_phoff = 52 (right after header)
        elf[28..32].copy_from_slice(&52u32.to_le_bytes());
        // e_phentsize = 32
        elf[42..44].copy_from_slice(&32u16.to_le_bytes());
        // e_phnum = 1
        elf[44..46].copy_from_slice(&1u16.to_le_bytes());

        // Program header at offset 52
        let ph = 52;
        // p_type = PT_LOAD
        elf[ph..ph + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
        // p_offset = 84 (after header + phdr)
        let data_off = 52 + 32;
        elf[ph + 4..ph + 8].copy_from_slice(&(data_off as u32).to_le_bytes());
        // p_vaddr = paddr
        elf[ph + 8..ph + 12].copy_from_slice(&paddr.to_le_bytes());
        // p_paddr = paddr
        elf[ph + 12..ph + 16].copy_from_slice(&paddr.to_le_bytes());
        // p_filesz
        elf[ph + 16..ph + 20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        // p_memsz
        elf[ph + 20..ph + 24].copy_from_slice(&(payload.len() as u32).to_le_bytes());

        // Data
        elf[data_off..data_off + payload.len()].copy_from_slice(payload);

        elf
    }

    #[test]
    fn test_parse_elf32_basic() {
        let payload = [0xDE, 0xAD, 0xBE, 0xEF];
        let elf = make_minimal_elf32(0x0800_0000, &payload);

        let mut segments = std::vec![];
        parse_elf::<FormatError>(&elf, |seg| {
            segments.push((seg.address, seg.data.to_vec()));
            Ok(())
        })
        .unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, 0x0800_0000);
        assert_eq!(segments[0].1, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_bad_magic() {
        let data = [0x00; 64];
        let result = parse_elf::<FormatError>(&data, |_| Ok(()));
        assert!(matches!(
            result,
            Err(ElfError::Format(FormatError::InvalidMagic))
        ));
    }

    #[test]
    fn test_too_short() {
        let result = parse_elf::<FormatError>(&[0x7F, b'E', b'L', b'F'], |_| Ok(()));
        // Class byte missing or invalid
        assert!(result.is_err());
    }
}
