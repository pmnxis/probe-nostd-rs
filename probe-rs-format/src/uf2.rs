//! UF2 (USB Flashing Format) parser.
//!
//! UF2 files consist of fixed 512-byte blocks, each containing up to 476
//! bytes of payload data at a specified target address. Used by RP2040,
//! SAMD, and other microcontrollers for drag-and-drop flashing.
//!
//! Spec: <https://github.com/microsoft/uf2>

use crate::error::FormatError;
use crate::segment::DataSegment;

/// UF2 block size (always 512 bytes).
pub const BLOCK_SIZE: usize = 512;

/// Maximum payload data per block.
pub const MAX_PAYLOAD_SIZE: usize = 476;

// Magic numbers
const MAGIC_START0: u32 = 0x0A32_4655; // "UF2\n"
const MAGIC_START1: u32 = 0x9E5D_5157;
const MAGIC_END: u32 = 0x0AB1_6F30;

// Header offsets (all little-endian u32)
const OFF_MAGIC0: usize = 0;
const OFF_MAGIC1: usize = 4;
const OFF_FLAGS: usize = 8;
const OFF_TARGET_ADDR: usize = 12;
const OFF_PAYLOAD_SIZE: usize = 16;
const OFF_BLOCK_NO: usize = 20;
const OFF_NUM_BLOCKS: usize = 24;
const OFF_FAMILY_ID: usize = 28;
const OFF_DATA: usize = 32;
const OFF_MAGIC_END: usize = BLOCK_SIZE - 4;

/// Parsed UF2 block header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Uf2BlockHeader {
    /// Flags (bit 0x2000 = family ID present, etc.)
    pub flags: u32,
    /// Target address for this block's payload.
    pub target_address: u32,
    /// Number of valid payload bytes (max 476).
    pub payload_size: u32,
    /// Block sequence number (0-based).
    pub block_number: u32,
    /// Total number of blocks in the file.
    pub total_blocks: u32,
    /// Family ID (if flag 0x2000 set) or file size.
    pub family_id: u32,
}

/// Parse a single 512-byte UF2 block.
///
/// Returns the block header and a slice of the payload data.
pub fn parse_block(block: &[u8; BLOCK_SIZE]) -> Result<(Uf2BlockHeader, &[u8]), FormatError> {
    // Validate magic numbers
    if read_u32_le(block, OFF_MAGIC0) != MAGIC_START0
        || read_u32_le(block, OFF_MAGIC1) != MAGIC_START1
        || read_u32_le(block, OFF_MAGIC_END) != MAGIC_END
    {
        return Err(FormatError::InvalidMagic);
    }

    let payload_size = read_u32_le(block, OFF_PAYLOAD_SIZE);
    if payload_size as usize > MAX_PAYLOAD_SIZE {
        return Err(FormatError::InvalidRecord);
    }

    let header = Uf2BlockHeader {
        flags: read_u32_le(block, OFF_FLAGS),
        target_address: read_u32_le(block, OFF_TARGET_ADDR),
        payload_size,
        block_number: read_u32_le(block, OFF_BLOCK_NO),
        total_blocks: read_u32_le(block, OFF_NUM_BLOCKS),
        family_id: read_u32_le(block, OFF_FAMILY_ID),
    };

    let payload = &block[OFF_DATA..OFF_DATA + payload_size as usize];
    Ok((header, payload))
}

/// Parse an entire UF2 buffer block by block, calling `callback` for each.
pub fn parse_uf2<E>(
    data: &[u8],
    mut callback: impl FnMut(DataSegment<'_>) -> Result<(), E>,
) -> Result<(), Uf2Error<E>> {
    if !data.len().is_multiple_of(BLOCK_SIZE) {
        return Err(Uf2Error::Format(FormatError::InvalidRecord));
    }

    for chunk in data.chunks_exact(BLOCK_SIZE) {
        let block: &[u8; BLOCK_SIZE] = chunk.try_into().map_err(|_| {
            core::hint::cold_path();
            Uf2Error::Format(FormatError::UnexpectedEof)
        })?;

        let (header, payload) = parse_block(block).map_err(Uf2Error::Format)?;

        let segment = DataSegment {
            address: header.target_address,
            data: payload,
        };
        callback(segment).map_err(Uf2Error::Callback)?;
    }

    Ok(())
}

/// Error from `parse_uf2` batch function.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Uf2Error<E> {
    Format(FormatError),
    Callback(E),
}

/// Read a little-endian u32 at the given byte offset.
const fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    (data[offset] as u32)
        | ((data[offset + 1] as u32) << 8)
        | ((data[offset + 2] as u32) << 16)
        | ((data[offset + 3] as u32) << 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uf2_block(target_addr: u32, payload: &[u8], block_no: u32, total: u32) -> [u8; 512] {
        let mut block = [0u8; 512];

        // Magic start
        block[0..4].copy_from_slice(&MAGIC_START0.to_le_bytes());
        block[4..8].copy_from_slice(&MAGIC_START1.to_le_bytes());

        // Flags
        block[8..12].copy_from_slice(&0u32.to_le_bytes());

        // Target address
        block[12..16].copy_from_slice(&target_addr.to_le_bytes());

        // Payload size
        block[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());

        // Block number
        block[20..24].copy_from_slice(&block_no.to_le_bytes());

        // Total blocks
        block[24..28].copy_from_slice(&total.to_le_bytes());

        // Family ID
        block[28..32].copy_from_slice(&0u32.to_le_bytes());

        // Data
        block[32..32 + payload.len()].copy_from_slice(payload);

        // Magic end
        block[508..512].copy_from_slice(&MAGIC_END.to_le_bytes());

        block
    }

    #[test]
    fn test_parse_single_block() {
        let payload = [0xDE, 0xAD, 0xBE, 0xEF];
        let block = make_uf2_block(0x1000_0000, &payload, 0, 1);

        let (header, data) = parse_block(&block).unwrap();
        assert_eq!(header.target_address, 0x1000_0000);
        assert_eq!(header.payload_size, 4);
        assert_eq!(header.block_number, 0);
        assert_eq!(header.total_blocks, 1);
        assert_eq!(data, &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_parse_bad_magic() {
        let block = [0u8; 512];
        assert_eq!(parse_block(&block), Err(FormatError::InvalidMagic));
    }

    #[test]
    fn test_parse_uf2_multi_block() {
        let b0 = make_uf2_block(0x0800_0000, &[0x01, 0x02], 0, 2);
        let b1 = make_uf2_block(0x0800_0100, &[0x03, 0x04], 1, 2);

        let mut buf = [0u8; 1024];
        buf[..512].copy_from_slice(&b0);
        buf[512..].copy_from_slice(&b1);

        let mut segments = [(0u32, 0u8, 0u8); 2];
        let mut idx = 0;

        parse_uf2::<FormatError>(&buf, |seg| {
            if idx < 2 {
                segments[idx] = (seg.address, seg.data[0], seg.data[1]);
                idx += 1;
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(segments[0], (0x0800_0000, 0x01, 0x02));
        assert_eq!(segments[1], (0x0800_0100, 0x03, 0x04));
    }
}
