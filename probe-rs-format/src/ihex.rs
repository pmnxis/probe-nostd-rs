//! Intel HEX format parser.
//!
//! Parses Intel HEX text format into `DataSegment` pairs. Supports:
//! - Type 00: Data records
//! - Type 01: End of File
//! - Type 02: Extended Segment Address (20-bit)
//! - Type 03: Start Segment Address (ignored)
//! - Type 04: Extended Linear Address (32-bit)
//! - Type 05: Start Linear Address (ignored)
//!
//! Two usage modes:
//! - **Batch**: `parse_ihex(data, callback)` -- processes entire buffer
//! - **Streaming**: `IhexParser::new()` + `parse_line()` -- line-by-line

use crate::error::FormatError;
use crate::segment::DataSegment;

/// Intel HEX record types.
mod record_type {
    pub const DATA: u8 = 0x00;
    pub const EOF: u8 = 0x01;
    pub const EXTENDED_SEGMENT_ADDR: u8 = 0x02;
    #[allow(dead_code)]
    pub const START_SEGMENT_ADDR: u8 = 0x03;
    pub const EXTENDED_LINEAR_ADDR: u8 = 0x04;
    #[allow(dead_code)]
    pub const START_LINEAR_ADDR: u8 = 0x05;
}

/// Streaming Intel HEX parser.
///
/// Maintains the base address state across lines. Feed lines one at a time
/// via `parse_line()` for network/UART streaming, or use `parse_ihex()`
/// for batch processing.
pub struct IhexParser {
    base_address: u32,
}

impl IhexParser {
    /// Create a new parser with base address 0.
    pub const fn new() -> Self {
        Self { base_address: 0 }
    }

    /// Parse a single Intel HEX line.
    ///
    /// Returns `Ok(Some(segment))` for data records, `Ok(None)` for
    /// address/EOF/start records, or `Err` for malformed lines.
    ///
    /// The returned `DataSegment` borrows from `line` -- the data bytes
    /// are NOT copied, they reference the hex-decoded portion of the input.
    ///
    /// **Important**: Intel HEX data is hex-encoded ASCII, so we must decode
    /// it. Since we cannot allocate, we decode in-place into the caller's
    /// buffer. Use `parse_line_into()` for zero-alloc operation.
    pub fn parse_line(&mut self, line: &[u8]) -> Result<Option<IhexRecord>, FormatError> {
        let line = trim_line(line);
        if line.is_empty() {
            return Ok(None);
        }

        // Must start with ':'
        if line.first() != Some(&b':') {
            return Err(FormatError::InvalidRecord);
        }
        let hex = &line[1..];

        // Minimum: LL AAAA TT CC = 5 bytes = 10 hex chars
        if hex.len() < 10 {
            return Err(FormatError::InvalidRecord);
        }

        let byte_count = decode_hex_byte(hex, 0)? as usize;
        let address = ((decode_hex_byte(hex, 2)? as u16) << 8) | decode_hex_byte(hex, 4)? as u16;
        let record_type = decode_hex_byte(hex, 6)?;

        // Verify we have enough hex chars: (1 + 2 + 1 + byte_count + 1) * 2
        let expected_hex_len = (4 + byte_count + 1) * 2;
        if hex.len() < expected_hex_len {
            return Err(FormatError::UnexpectedEof);
        }

        // Verify checksum (sum of all bytes including checksum == 0 mod 256)
        let mut checksum: u8 = 0;
        for i in 0..(4 + byte_count + 1) {
            checksum = checksum.wrapping_add(decode_hex_byte(hex, i * 2)?);
        }
        if checksum != 0 {
            return Err(FormatError::InvalidChecksum);
        }

        match record_type {
            record_type::DATA => {
                let abs_address = self.base_address + address as u32;
                Ok(Some(IhexRecord::Data {
                    address: abs_address,
                    data_offset: 8, // hex offset where data bytes start
                    data_len: byte_count,
                }))
            }
            record_type::EOF => Ok(None),
            record_type::EXTENDED_SEGMENT_ADDR => {
                if byte_count != 2 {
                    return Err(FormatError::InvalidRecord);
                }
                let segment =
                    ((decode_hex_byte(hex, 8)? as u32) << 8) | decode_hex_byte(hex, 10)? as u32;
                self.base_address = segment << 4; // segment * 16
                Ok(None)
            }
            record_type::EXTENDED_LINEAR_ADDR => {
                if byte_count != 2 {
                    return Err(FormatError::InvalidRecord);
                }
                let upper =
                    ((decode_hex_byte(hex, 8)? as u32) << 8) | decode_hex_byte(hex, 10)? as u32;
                self.base_address = upper << 16;
                Ok(None)
            }
            // Start address records -- ignored (entry point, not data)
            0x03 | 0x05 => Ok(None),
            _ => Err(FormatError::InvalidRecord),
        }
    }

    /// Parse a line and decode data bytes into the provided buffer.
    ///
    /// This is the zero-alloc API. The hex-encoded data bytes are decoded
    /// into `decode_buf`, and the returned `DataSegment` references that buffer.
    pub fn parse_line_into<'buf>(
        &mut self,
        line: &[u8],
        decode_buf: &'buf mut [u8],
    ) -> Result<Option<DataSegment<'buf>>, FormatError> {
        let line = trim_line(line);
        let record = self.parse_line(line)?;

        match record {
            Some(IhexRecord::Data {
                address,
                data_offset,
                data_len,
            }) => {
                if data_len > decode_buf.len() {
                    return Err(FormatError::UnexpectedEof);
                }
                let hex = &line[1..]; // skip ':'
                for (i, byte) in decode_buf[..data_len].iter_mut().enumerate() {
                    *byte = decode_hex_byte(hex, data_offset + i * 2)?;
                }
                Ok(Some(DataSegment {
                    address,
                    data: &decode_buf[..data_len],
                }))
            }
            _ => Ok(None),
        }
    }
}

impl Default for IhexParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed Intel HEX record (intermediate, before data decoding).
#[derive(Debug, Clone, Copy)]
pub enum IhexRecord {
    /// Data record with address and position in the hex line.
    Data {
        address: u32,
        data_offset: usize, // hex char offset within the line (after ':')
        data_len: usize,    // number of data bytes
    },
}

/// Parse an entire Intel HEX buffer, calling `callback` for each data segment.
///
/// Uses a stack-allocated decode buffer (256 bytes max per record).
pub fn parse_ihex<E>(
    data: &[u8],
    mut callback: impl FnMut(DataSegment<'_>) -> Result<(), E>,
) -> Result<(), IhexError<E>> {
    let mut parser = IhexParser::new();
    let mut decode_buf = [0u8; 256]; // max Intel HEX record data = 255 bytes

    for line in data.split(|&b| b == b'\n') {
        match parser.parse_line_into(line, &mut decode_buf) {
            Ok(Some(segment)) => {
                callback(segment).map_err(IhexError::Callback)?;
            }
            Ok(None) => {}
            Err(e) => return Err(IhexError::Format(e)),
        }
    }

    Ok(())
}

/// Error from `parse_ihex` batch function.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum IhexError<E> {
    /// Format parsing error.
    Format(FormatError),
    /// Callback returned an error.
    Callback(E),
}

// -- Hex decoding helpers --

/// Decode a single hex character to its 4-bit value.
const fn hex_nibble(c: u8) -> Result<u8, FormatError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(FormatError::InvalidRecord),
    }
}

/// Decode two hex characters at `offset` into a byte.
fn decode_hex_byte(hex: &[u8], offset: usize) -> Result<u8, FormatError> {
    if offset + 2 > hex.len() {
        return Err(FormatError::UnexpectedEof);
    }
    let hi = hex_nibble(hex[offset])?;
    let lo = hex_nibble(hex[offset + 1])?;
    Ok((hi << 4) | lo)
}

/// Trim trailing CR/LF/whitespace from a line.
fn trim_line(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    while end > 0 && matches!(line[end - 1], b'\r' | b'\n' | b' ' | b'\t') {
        end -= 1;
    }
    &line[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_nibble() {
        assert_eq!(hex_nibble(b'0'), Ok(0));
        assert_eq!(hex_nibble(b'9'), Ok(9));
        assert_eq!(hex_nibble(b'A'), Ok(10));
        assert_eq!(hex_nibble(b'F'), Ok(15));
        assert_eq!(hex_nibble(b'a'), Ok(10));
        assert_eq!(hex_nibble(b'f'), Ok(15));
        assert!(hex_nibble(b'G').is_err());
    }

    #[test]
    fn test_parse_data_record() {
        // :0B0010006164647265737320676170A7
        // 11 bytes at address 0x0010: "address gap"
        let line = b":0B0010006164647265737320676170A7";
        let mut parser = IhexParser::new();
        let mut buf = [0u8; 256];
        let seg = parser.parse_line_into(line, &mut buf).unwrap().unwrap();
        assert_eq!(seg.address, 0x0010);
        assert_eq!(seg.data, b"address gap");
    }

    #[test]
    fn test_parse_eof() {
        let line = b":00000001FF";
        let mut parser = IhexParser::new();
        let mut buf = [0u8; 256];
        assert!(parser.parse_line_into(line, &mut buf).unwrap().is_none());
    }

    #[test]
    fn test_extended_linear_address() {
        // :02000004FFFFFC  -- set upper 16 bits to 0xFFFF
        let line = b":02000004FFFFFC";
        let mut parser = IhexParser::new();
        let mut buf = [0u8; 256];
        assert!(parser.parse_line_into(line, &mut buf).unwrap().is_none());
        assert_eq!(parser.base_address, 0xFFFF_0000);

        // Data record at offset 0x0000 -> absolute address 0xFFFF0000
        let data_line = b":0100000042BD";
        let seg = parser
            .parse_line_into(data_line, &mut buf)
            .unwrap()
            .unwrap();
        assert_eq!(seg.address, 0xFFFF_0000);
        assert_eq!(seg.data, &[0x42]);
    }

    #[test]
    fn test_extended_segment_address() {
        // :020000021200EA  -- segment = 0x1200, base = 0x12000
        let line = b":020000021200EA";
        let mut parser = IhexParser::new();
        let mut buf = [0u8; 256];
        assert!(parser.parse_line_into(line, &mut buf).unwrap().is_none());
        assert_eq!(parser.base_address, 0x0001_2000);
    }

    #[test]
    fn test_bad_checksum() {
        let line = b":0100000042BE"; // checksum should be BD, not BE
        let mut parser = IhexParser::new();
        let mut buf = [0u8; 256];
        assert_eq!(
            parser.parse_line_into(line, &mut buf),
            Err(FormatError::InvalidChecksum)
        );
    }

    #[test]
    fn test_batch_parse() {
        // 1 byte (0x08) at 0x0000, 1 byte (0x42) at 0x0002, EOF
        let hex = b":0100000008F7\n:0100020042BB\n:00000001FF\n";
        let mut segments = [(0u32, 0u8); 2];
        let mut idx = 0;

        parse_ihex::<FormatError>(hex, |seg| {
            if idx < segments.len() {
                segments[idx] = (seg.address, seg.data[0]);
                idx += 1;
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(idx, 2);
        assert_eq!(segments[0], (0x0000, 0x08));
        assert_eq!(segments[1], (0x0002, 0x42));
    }

    #[test]
    fn test_empty_lines_skipped() {
        let hex = b"\n\n:00000001FF\n\n";
        parse_ihex::<FormatError>(hex, |_| Ok(())).unwrap();
    }
}
