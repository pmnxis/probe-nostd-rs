//! ARM Access Port (AP) definitions.
//!
//! Types and constants for MEM-AP registers (CSW, TAR, DRW).

/// Access Port address (AP index, 0-255 for ADIv5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ApAddress(pub u8);

/// Data transfer size for MEM-AP CSW.SIZE field (bits 2:0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum DataSize {
    /// 8-bit (1 byte).
    U8 = 0b000,
    /// 16-bit (2 bytes).
    U16 = 0b001,
    /// 32-bit (4 bytes).
    U32 = 0b010,
}

impl DataSize {
    /// Number of bytes for this data size.
    pub const fn byte_count(self) -> usize {
        match self {
            DataSize::U8 => 1,
            DataSize::U16 => 2,
            DataSize::U32 => 4,
        }
    }
}

/// Address increment mode for MEM-AP CSW.AddrInc field (bits 5:4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum AddressIncrement {
    /// No increment.
    Off = 0b00,
    /// Increment by SIZE bytes after each transfer.
    Single = 0b01,
    /// Packed transfer (sub-word packing).
    Packed = 0b10,
}

// MEM-AP register offsets (within AP register bank 0)

/// Control/Status Word register offset.
pub const CSW_ADDR: u8 = 0x00;
/// Transfer Address Register offset.
pub const TAR_ADDR: u8 = 0x04;
/// Data Read/Write register offset.
pub const DRW_ADDR: u8 = 0x0C;

/// Build a CSW register value.
///
/// Sets SIZE, AddrInc, and DbgSwEnable (bit 31).
/// Const-evaluable so common CSW values can be precomputed.
pub const fn build_csw(size: DataSize, inc: AddressIncrement) -> u32 {
    (size as u32) | ((inc as u32) << 4) | (1 << 31)
}

/// Pre-computed CSW values for common configurations.
pub mod csw_precomputed {
    use super::*;

    /// CSW: 32-bit word access with single auto-increment.
    pub const U32_SINGLE: u32 = build_csw(DataSize::U32, AddressIncrement::Single);
    /// CSW: 16-bit halfword access with single auto-increment.
    pub const U16_SINGLE: u32 = build_csw(DataSize::U16, AddressIncrement::Single);
    /// CSW: 8-bit byte access with single auto-increment.
    pub const U8_SINGLE: u32 = build_csw(DataSize::U8, AddressIncrement::Single);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_csw_u32_single() {
        let csw = build_csw(DataSize::U32, AddressIncrement::Single);
        // SIZE = 0b010, AddrInc = 0b01 << 4 = 0x10, DbgSwEnable = bit 31
        assert_eq!(csw & 0x07, 0b010); // SIZE
        assert_eq!((csw >> 4) & 0x03, 0b01); // AddrInc
        assert_ne!(csw & (1 << 31), 0); // DbgSwEnable
    }

    #[test]
    fn test_build_csw_u8() {
        let csw = build_csw(DataSize::U8, AddressIncrement::Single);
        assert_eq!(csw & 0x07, 0b000);
    }

    #[test]
    fn test_data_size_byte_count() {
        assert_eq!(DataSize::U8.byte_count(), 1);
        assert_eq!(DataSize::U16.byte_count(), 2);
        assert_eq!(DataSize::U32.byte_count(), 4);
    }
}
