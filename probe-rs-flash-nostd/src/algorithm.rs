//! Flash algorithm assembly -- compute runtime memory layout from FlashAlgoDef.
//!
//! A CMSIS flash algorithm is a small ARM binary that runs in the target's RAM.
//! This module computes the absolute addresses for code, data, stack, and page
//! buffer given the algorithm definition and available RAM.

use probe_rs_target_nostd::{FlashAlgoDef, FlashProperties};

use crate::error::FlashError;

/// BKPT instruction (Thumb encoding) used as the LR target.
/// When the algorithm function returns (BX LR), it hits this breakpoint
/// and the core halts, signaling completion.
const THUMB_BKPT: u32 = 0xBE00_BE00;

/// Header size: one u32 word containing the BKPT instruction.
const HEADER_SIZE: u64 = 4;

/// Default stack size when not specified by the algorithm.
const DEFAULT_STACK_SIZE: u32 = 512;

/// Assembled flash algorithm with absolute addresses ready for execution.
///
/// Constructed from a `FlashAlgoDef` (compile-time data from Layer 0)
/// and RAM region bounds. All addresses are absolute target addresses.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AssembledAlgorithm {
    /// Address where the header (BKPT) is placed. LR points here.
    pub load_address: u64,
    /// Reference to the raw instruction bytes (from FlashAlgoDef).
    pub instructions: &'static [u8],
    /// Start of actual code (load_address + HEADER_SIZE).
    pub code_start: u64,
    /// Absolute address: Init() entry point.
    pub pc_init: Option<u64>,
    /// Absolute address: UnInit() entry point.
    pub pc_uninit: Option<u64>,
    /// Absolute address: ProgramPage() entry point.
    pub pc_program_page: u64,
    /// Absolute address: EraseSector() entry point.
    pub pc_erase_sector: u64,
    /// Absolute address: EraseAll() entry point.
    pub pc_erase_all: Option<u64>,
    /// R9 value: base address for position-independent data (.data section).
    pub static_base: u64,
    /// Stack top address (SP initial value).
    pub stack_top: u64,
    /// Stack size in bytes.
    pub stack_size: u32,
    /// Address of the page buffer in RAM (for programming data).
    pub page_buffer_addr: u64,
    /// Page size in bytes.
    pub page_size: u32,
    /// Flash properties (address range, sector descriptions, timeouts).
    pub flash_properties: FlashProperties,
}

impl AssembledAlgorithm {
    /// Assemble a flash algorithm for the given RAM region.
    ///
    /// Computes the memory layout:
    /// ```text
    /// RAM start (load_address)
    ///   [BKPT header]    4 bytes   <- LR target
    ///   [instructions]   N bytes   <- algorithm code
    ///   [.data section]            <- static_base (R9)
    ///   [stack]          S bytes   <- grows downward from stack_top
    ///   [page buffer]    P bytes   <- programming data staged here
    /// RAM end
    /// ```
    pub fn from_algo_def(
        algo: &FlashAlgoDef,
        ram_start: u64,
        ram_end: u64,
    ) -> Result<Self, FlashError> {
        let load_address = if algo.load_address != 0 {
            algo.load_address
        } else {
            ram_start
        };

        let code_start = load_address + HEADER_SIZE;
        let instructions_size = algo.instructions.len() as u64;
        let code_end = code_start + instructions_size;

        // Data section follows code
        let data_start = code_end;
        let static_base = code_start + algo.data_section_offset;

        // Stack after data
        let stack_size = if algo.stack_size > 0 {
            algo.stack_size
        } else {
            DEFAULT_STACK_SIZE
        };
        let stack_bottom = data_start;
        let stack_top = stack_bottom + stack_size as u64;

        // Page buffer after stack
        let page_buffer_addr = stack_top;
        let page_size = algo.flash_properties.page_size;
        let page_buffer_end = page_buffer_addr + page_size as u64;

        // Verify everything fits in RAM
        if page_buffer_end > ram_end {
            return Err(FlashError::InsufficientRam);
        }

        Ok(Self {
            load_address,
            instructions: algo.instructions,
            code_start,
            pc_init: algo.pc_init.map(|offset| code_start + offset),
            pc_uninit: algo.pc_uninit.map(|offset| code_start + offset),
            pc_program_page: code_start + algo.pc_program_page,
            pc_erase_sector: code_start + algo.pc_erase_sector,
            pc_erase_all: algo.pc_erase_all.map(|offset| code_start + offset),
            static_base,
            stack_top,
            stack_size,
            page_buffer_addr,
            page_size,
            flash_properties: algo.flash_properties,
        })
    }

    /// The BKPT header word (written at load_address).
    pub const fn header_word() -> u32 {
        THUMB_BKPT
    }

    /// Total RAM footprint: header + code + stack + page buffer.
    pub fn ram_footprint(&self) -> u64 {
        self.page_buffer_addr + self.page_size as u64 - self.load_address
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use probe_rs_target_nostd::{FlashProperties, SectorDescription};

    fn test_algo() -> FlashAlgoDef {
        FlashAlgoDef {
            name: "test",
            instructions: &[0u8; 256],
            load_address: 0x2000_0000,
            data_section_offset: 200,
            pc_init: Some(0),
            pc_uninit: Some(4),
            pc_program_page: 8,
            pc_erase_sector: 12,
            pc_erase_all: Some(16),
            stack_size: 512,
            flash_properties: FlashProperties {
                address_range_start: 0x0800_0000,
                address_range_end: 0x0810_0000,
                page_size: 256,
                erased_byte_value: 0xFF,
                program_page_timeout: 100,
                erase_sector_timeout: 1000,
                sectors: &[SectorDescription {
                    size: 4096,
                    address: 0x0800_0000,
                }],
            },
        }
    }

    #[test]
    fn test_assemble_basic() {
        let algo = test_algo();
        let assembled = AssembledAlgorithm::from_algo_def(&algo, 0x2000_0000, 0x2004_0000).unwrap();

        assert_eq!(assembled.load_address, 0x2000_0000);
        assert_eq!(assembled.code_start, 0x2000_0000 + HEADER_SIZE);
        // pc_init = code_start + 0 = 0x2000_0004
        assert_eq!(assembled.pc_init, Some(0x2000_0004));
        assert_eq!(assembled.pc_erase_sector, 0x2000_0004 + 12);
        assert_eq!(assembled.page_size, 256);
    }

    #[test]
    fn test_insufficient_ram() {
        let algo = test_algo();
        // Only 100 bytes of RAM -- not enough
        let result = AssembledAlgorithm::from_algo_def(&algo, 0x2000_0000, 0x2000_0064);
        assert_eq!(result, Err(FlashError::InsufficientRam));
    }

    #[test]
    fn test_header_word() {
        assert_eq!(AssembledAlgorithm::header_word(), 0xBE00_BE00);
    }
}
