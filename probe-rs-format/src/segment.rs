//! Common output type for all format parsers.

/// A contiguous block of data destined for a target address.
///
/// This is the universal output of all format parsers. Each parser
/// produces one or more `DataSegment` values from the input data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DataSegment<'a> {
    /// Target memory address where this data should be written.
    pub address: u32,
    /// The data bytes to write.
    pub data: &'a [u8],
}

impl<'a> DataSegment<'a> {
    /// End address (exclusive): address + data length.
    pub const fn end_address(&self) -> u32 {
        self.address + self.data.len() as u32
    }

    /// Number of bytes in this segment.
    pub const fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether this segment is empty.
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
