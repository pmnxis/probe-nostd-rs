//! Raw binary format -- trivial passthrough.
//!
//! A BIN file is just raw bytes at a given base address. No parsing needed.

use crate::segment::DataSegment;

/// Wrap raw binary data as a single `DataSegment`.
///
/// # Example
/// ```
/// use probe_rs_format::bin::parse_bin;
/// let fw = &[0u8; 1024]; // firmware bytes
/// let seg = parse_bin(fw, 0x0800_0000);
/// assert_eq!(seg.address, 0x0800_0000);
/// assert_eq!(seg.len(), 1024);
/// ```
pub const fn parse_bin(data: &[u8], base_address: u32) -> DataSegment<'_> {
    DataSegment {
        address: base_address,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bin() {
        let data = [0xAA, 0xBB, 0xCC, 0xDD];
        let seg = parse_bin(&data, 0x2000_0000);
        assert_eq!(seg.address, 0x2000_0000);
        assert_eq!(seg.data, &[0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(seg.end_address(), 0x2000_0004);
    }

    #[test]
    fn test_parse_bin_empty() {
        let seg = parse_bin(&[], 0);
        assert!(seg.is_empty());
    }
}
