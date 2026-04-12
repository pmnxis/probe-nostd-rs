//! MEM-AP memory access with TAR boundary handling.
//!
//! Provides `MemoryAccessor` for reading and writing target memory through
//! the ARM ADIv5 MEM-AP interface. Handles TAR auto-increment boundary
//! splitting (1KB) and byte-lane shifting for sub-word accesses.

use crate::ap::{self, ApAddress, DataSize};
use crate::dap_access::DapAccess;
use crate::error::AdiError;

/// TAR auto-increment wraps at 1KB boundary (10-bit address space).
const TAR_AUTOINCR_LIMIT: u32 = 0x400;

/// Maximum number of u32 words in a single sub-word transfer chunk.
/// Keeps stack usage bounded (256 bytes for the buffer).
const SUB_WORD_CHUNK: usize = 64;

/// Compute the maximum number of bytes transferable before hitting the
/// TAR auto-increment wrap boundary.
const fn autoincr_max_bytes(address: u32) -> usize {
    let next_boundary = (address | (TAR_AUTOINCR_LIMIT - 1)) + 1;
    (next_boundary - address) as usize
}

/// MEM-AP memory access interface.
///
/// Short-lived borrow: create one for a burst of memory operations,
/// then drop it to release the `DapAccess` borrow.
pub struct MemoryAccessor<'a, D: DapAccess> {
    dap: &'a mut D,
    ap: ApAddress,
}

impl<'a, D: DapAccess> MemoryAccessor<'a, D> {
    /// Create a new memory accessor for the given AP.
    pub fn new(dap: &'a mut D, ap: ApAddress) -> Self {
        Self { dap, ap }
    }

    /// Write the CSW register with the given data size and single-increment mode.
    ///
    /// Uses precomputed CSW values when possible to avoid runtime computation.
    fn set_csw(&mut self, size: DataSize) -> Result<(), AdiError> {
        let csw = match size {
            DataSize::U32 => ap::csw_precomputed::U32_SINGLE,
            DataSize::U16 => ap::csw_precomputed::U16_SINGLE,
            DataSize::U8 => ap::csw_precomputed::U8_SINGLE,
        };
        self.dap.write_ap(self.ap, ap::CSW_ADDR, csw)
    }

    /// Write the TAR register.
    fn set_tar(&mut self, address: u32) -> Result<(), AdiError> {
        self.dap.write_ap(self.ap, ap::TAR_ADDR, address)
    }

    // -- 32-bit access --

    /// Read 32-bit words from target memory.
    ///
    /// `address` must be 4-byte aligned. Reads are split at 1KB TAR boundaries.
    pub fn read_32(&mut self, address: u32, data: &mut [u32]) -> Result<(), AdiError> {
        if !address.is_multiple_of(4) {
            return Err(AdiError::MemoryNotAligned {
                address,
                required_alignment: 4,
            });
        }
        if data.is_empty() {
            return Ok(());
        }

        self.set_csw(DataSize::U32)?;

        let mut offset = 0;
        let mut addr = address;

        while offset < data.len() {
            let max_words = autoincr_max_bytes(addr) / 4;
            let chunk_len = (data.len() - offset).min(max_words);

            self.set_tar(addr)?;
            self.dap.read_ap_repeated(
                self.ap,
                ap::DRW_ADDR,
                &mut data[offset..offset + chunk_len],
            )?;

            offset += chunk_len;
            addr += (chunk_len * 4) as u32;
        }

        Ok(())
    }

    /// Write 32-bit words to target memory.
    ///
    /// `address` must be 4-byte aligned.
    pub fn write_32(&mut self, address: u32, data: &[u32]) -> Result<(), AdiError> {
        if !address.is_multiple_of(4) {
            return Err(AdiError::MemoryNotAligned {
                address,
                required_alignment: 4,
            });
        }
        if data.is_empty() {
            return Ok(());
        }

        self.set_csw(DataSize::U32)?;

        let mut offset = 0;
        let mut addr = address;

        while offset < data.len() {
            let max_words = autoincr_max_bytes(addr) / 4;
            let chunk_len = (data.len() - offset).min(max_words);

            self.set_tar(addr)?;
            self.dap
                .write_ap_repeated(self.ap, ap::DRW_ADDR, &data[offset..offset + chunk_len])?;

            offset += chunk_len;
            addr += (chunk_len * 4) as u32;
        }

        Ok(())
    }

    // -- 8-bit access --

    /// Read bytes from target memory.
    ///
    /// Uses CSW SIZE=U8 and byte-lane shifting to extract individual bytes.
    pub fn read_8(&mut self, address: u32, data: &mut [u8]) -> Result<(), AdiError> {
        if data.is_empty() {
            return Ok(());
        }

        self.set_csw(DataSize::U8)?;

        let mut offset = 0;
        let mut addr = address;

        while offset < data.len() {
            let max_bytes = autoincr_max_bytes(addr);
            let chunk_len = (data.len() - offset).min(max_bytes).min(SUB_WORD_CHUNK);

            self.set_tar(addr)?;

            // Read DRW values into stack buffer
            let mut buf = [0u32; SUB_WORD_CHUNK];
            self.dap
                .read_ap_repeated(self.ap, ap::DRW_ADDR, &mut buf[..chunk_len])?;

            // Extract bytes using byte-lane shifting
            for i in 0..chunk_len {
                let byte_lane = ((addr + i as u32) % 4) * 8;
                data[offset + i] = ((buf[i] >> byte_lane) & 0xFF) as u8;
            }

            offset += chunk_len;
            addr += chunk_len as u32;
        }

        Ok(())
    }

    /// Write bytes to target memory.
    ///
    /// Uses CSW SIZE=U8 and byte-lane shifting to place bytes correctly.
    pub fn write_8(&mut self, address: u32, data: &[u8]) -> Result<(), AdiError> {
        if data.is_empty() {
            return Ok(());
        }

        self.set_csw(DataSize::U8)?;

        let mut offset = 0;
        let mut addr = address;

        while offset < data.len() {
            let max_bytes = autoincr_max_bytes(addr);
            let chunk_len = (data.len() - offset).min(max_bytes).min(SUB_WORD_CHUNK);

            self.set_tar(addr)?;

            // Build DRW values with byte-lane shifting
            let mut buf = [0u32; SUB_WORD_CHUNK];
            for i in 0..chunk_len {
                let byte_lane = ((addr + i as u32) % 4) * 8;
                buf[i] = (data[offset + i] as u32) << byte_lane;
            }

            self.dap
                .write_ap_repeated(self.ap, ap::DRW_ADDR, &buf[..chunk_len])?;

            offset += chunk_len;
            addr += chunk_len as u32;
        }

        Ok(())
    }

    // -- 16-bit access --

    /// Read 16-bit halfwords from target memory.
    ///
    /// `address` must be 2-byte aligned.
    pub fn read_16(&mut self, address: u32, data: &mut [u16]) -> Result<(), AdiError> {
        if !address.is_multiple_of(2) {
            return Err(AdiError::MemoryNotAligned {
                address,
                required_alignment: 2,
            });
        }
        if data.is_empty() {
            return Ok(());
        }

        self.set_csw(DataSize::U16)?;

        let mut offset = 0;
        let mut addr = address;

        while offset < data.len() {
            let max_halfwords = autoincr_max_bytes(addr) / 2;
            let chunk_len = (data.len() - offset).min(max_halfwords).min(SUB_WORD_CHUNK);

            self.set_tar(addr)?;

            let mut buf = [0u32; SUB_WORD_CHUNK];
            self.dap
                .read_ap_repeated(self.ap, ap::DRW_ADDR, &mut buf[..chunk_len])?;

            for i in 0..chunk_len {
                let byte_lane = ((addr + (i as u32) * 2) % 4) * 8;
                data[offset + i] = ((buf[i] >> byte_lane) & 0xFFFF) as u16;
            }

            offset += chunk_len;
            addr += (chunk_len * 2) as u32;
        }

        Ok(())
    }

    /// Write 16-bit halfwords to target memory.
    ///
    /// `address` must be 2-byte aligned.
    pub fn write_16(&mut self, address: u32, data: &[u16]) -> Result<(), AdiError> {
        if !address.is_multiple_of(2) {
            return Err(AdiError::MemoryNotAligned {
                address,
                required_alignment: 2,
            });
        }
        if data.is_empty() {
            return Ok(());
        }

        self.set_csw(DataSize::U16)?;

        let mut offset = 0;
        let mut addr = address;

        while offset < data.len() {
            let max_halfwords = autoincr_max_bytes(addr) / 2;
            let chunk_len = (data.len() - offset).min(max_halfwords).min(SUB_WORD_CHUNK);

            self.set_tar(addr)?;

            let mut buf = [0u32; SUB_WORD_CHUNK];
            for i in 0..chunk_len {
                let byte_lane = ((addr + (i as u32) * 2) % 4) * 8;
                buf[i] = (data[offset + i] as u32) << byte_lane;
            }

            self.dap
                .write_ap_repeated(self.ap, ap::DRW_ADDR, &buf[..chunk_len])?;

            offset += chunk_len;
            addr += (chunk_len * 2) as u32;
        }

        Ok(())
    }

    // -- Convenience single-word access --

    /// Read a single 32-bit word from target memory.
    pub fn read_word_32(&mut self, address: u32) -> Result<u32, AdiError> {
        let mut val = [0u32];
        self.read_32(address, &mut val)?;
        Ok(val[0])
    }

    /// Write a single 32-bit word to target memory.
    pub fn write_word_32(&mut self, address: u32, value: u32) -> Result<(), AdiError> {
        self.write_32(address, &[value])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autoincr_max_bytes_aligned() {
        // Address 0x000: next boundary is 0x400, max = 1024
        assert_eq!(autoincr_max_bytes(0x000), 1024);
    }

    #[test]
    fn test_autoincr_max_bytes_near_boundary() {
        // Address 0x3FC: 4 bytes to boundary 0x400
        assert_eq!(autoincr_max_bytes(0x3FC), 4);
    }

    #[test]
    fn test_autoincr_max_bytes_one_before() {
        // Address 0x3FF: 1 byte to boundary
        assert_eq!(autoincr_max_bytes(0x3FF), 1);
    }

    #[test]
    fn test_autoincr_max_bytes_at_boundary() {
        // Address 0x400: next boundary is 0x800, max = 1024
        assert_eq!(autoincr_max_bytes(0x400), 1024);
    }

    #[test]
    fn test_autoincr_max_bytes_mid() {
        // Address 0x100: next boundary is 0x400, max = 768
        assert_eq!(autoincr_max_bytes(0x100), 768);
    }
}
