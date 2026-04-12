//! Flash programming operations.
//!
//! The `Flasher` loads a CMSIS flash algorithm into target RAM, then
//! executes erase/program/verify by calling algorithm functions via
//! register setup + run + poll halt pattern.

use crate::algorithm::AssembledAlgorithm;
use crate::error::FlashError;
use probe_rs_adi::ap::ApAddress;
use probe_rs_adi::cortex_m::CortexMControl;
use probe_rs_adi::dap_access::DapAccess;
use probe_rs_adi::memory::MemoryAccessor;

/// CMSIS flash algorithm operation codes (passed in R2 to Init).
mod operation {
    pub const ERASE: u32 = 1;
    pub const PROGRAM: u32 = 2;
    #[allow(dead_code)]
    pub const VERIFY: u32 = 3;
}

/// Maximum poll iterations when waiting for algorithm function to complete.
/// Each iteration is roughly one SWD register read (~10us at 1MHz).
const DEFAULT_MAX_POLLS: u32 = 500_000; // ~5 seconds at 1MHz

/// Maximum poll iterations for erase-all (much longer).
const ERASE_ALL_MAX_POLLS: u32 = 4_000_000; // ~40 seconds

/// Chunk size for algorithm load verification.
const VERIFY_CHUNK_SIZE: usize = 256;

/// Flash programmer.
///
/// Manages the lifecycle of a CMSIS flash algorithm on the target:
/// load -> init -> erase/program -> uninit.
pub struct Flasher<'a, D: DapAccess> {
    core: CortexMControl<'a, D>,
    algo: AssembledAlgorithm,
    loaded: bool,
}

impl<'a, D: DapAccess> Flasher<'a, D> {
    /// Create a new flasher. Does not load the algorithm yet.
    pub fn new(dap: &'a mut D, ap: ApAddress, algo: AssembledAlgorithm) -> Self {
        Self {
            core: CortexMControl::new(dap, ap),
            algo,
            loaded: false,
        }
    }

    /// Load the flash algorithm binary into target RAM.
    ///
    /// Writes the BKPT header + instruction bytes, then reads back
    /// and verifies the content matches.
    pub fn load(&mut self) -> Result<(), FlashError> {
        // Halt the core first
        self.core.halt()?;

        let mem = self.core.memory();

        // Write BKPT header at load_address
        mem.write_word_32(
            self.algo.load_address as u32,
            AssembledAlgorithm::header_word(),
        )?;

        // Write algorithm instructions after header
        let code_addr = self.algo.code_start as u32;
        let instructions = self.algo.instructions;

        // Write in 32-bit word chunks (instructions are typically word-aligned)
        // Pad to word boundary if needed
        let mut word_buf = [0u32; 64]; // 256 bytes per chunk

        let mut offset = 0;
        while offset < instructions.len() {
            let chunk_bytes = (instructions.len() - offset).min(word_buf.len() * 4);
            let chunk_words = chunk_bytes.div_ceil(4);

            // Pack bytes into u32 words (little-endian)
            for (i, slot) in word_buf[..chunk_words].iter_mut().enumerate() {
                let mut word = 0u32;
                for b in 0..4 {
                    let idx = offset + i * 4 + b;
                    if idx < instructions.len() {
                        word |= (instructions[idx] as u32) << (b * 8);
                    }
                }
                *slot = word;
            }

            mem.write_32(code_addr + offset as u32, &word_buf[..chunk_words])?;
            offset += chunk_words * 4;
        }

        // Verify readback
        let mut read_buf = [0u32; 64];
        offset = 0;
        while offset < instructions.len() {
            let chunk_bytes = (instructions.len() - offset).min(VERIFY_CHUNK_SIZE);
            let chunk_words = chunk_bytes.div_ceil(4);

            mem.read_32(code_addr + offset as u32, &mut read_buf[..chunk_words])?;

            // Compare byte-by-byte
            for i in 0..chunk_bytes {
                let word_idx = i / 4;
                let byte_idx = i % 4;
                let read_byte = ((read_buf[word_idx] >> (byte_idx * 8)) & 0xFF) as u8;
                if read_byte != instructions[offset + i] {
                    core::hint::cold_path();
                    return Err(FlashError::AlgorithmVerifyFailed);
                }
            }

            offset += chunk_bytes;
        }

        self.loaded = true;
        Ok(())
    }

    /// Call a flash algorithm function and wait for it to complete.
    ///
    /// Sets R0-R3, optionally R9 (static_base) and SP (stack_top),
    /// then runs the core and polls until it halts.
    /// Returns the R0 value (function return code).
    #[allow(clippy::too_many_arguments)]
    fn call_function(
        &mut self,
        pc: u64,
        r0: u32,
        r1: u32,
        r2: u32,
        r3: u32,
        init: bool,
        max_polls: u32,
    ) -> Result<u32, FlashError> {
        // Set argument registers
        self.core.write_core_reg(0, r0)?; // R0
        self.core.write_core_reg(1, r1)?; // R1
        self.core.write_core_reg(2, r2)?; // R2
        self.core.write_core_reg(3, r3)?; // R3

        if init {
            // Set R9 = static_base (position-independent data base)
            self.core.write_core_reg(9, self.algo.static_base as u32)?;
            // Set SP = stack_top
            self.core.write_core_reg(13, self.algo.stack_top as u32)?;
        }

        // LR = load_address + 1 (Thumb mode bit set)
        // When function returns (BX LR), it hits the BKPT at load_address
        self.core
            .write_core_reg(14, (self.algo.load_address as u32) | 1)?;

        // PC = function entry point (Thumb mode bit set)
        self.core.write_core_reg(15, (pc as u32) | 1)?;

        // Run the core
        self.core.run()?;

        // Poll until halted (BKPT reached)
        for _ in 0..max_polls {
            if self.core.is_halted()? {
                // Read return value from R0
                return Ok(self.core.read_core_reg(0)?);
            }
        }

        core::hint::cold_path();
        Err(FlashError::Timeout)
    }

    /// Initialize the flash algorithm for the given operation.
    pub fn init(&mut self, op: u32) -> Result<(), FlashError> {
        if let Some(pc_init) = self.algo.pc_init {
            let result = self.call_function(
                pc_init,
                self.algo.flash_properties.address_range_start as u32, // R0: flash base
                0,                                                     // R1: clock (0 = default)
                op,                                                    // R2: operation code
                0,                                                     // R3: reserved
                true,                                                  // init: set R9 + SP
                DEFAULT_MAX_POLLS,
            )?;
            if result != 0 {
                core::hint::cold_path();
                return Err(FlashError::InitFailed { error_code: result });
            }
        }
        Ok(())
    }

    /// Uninitialize the flash algorithm.
    pub fn uninit(&mut self, op: u32) -> Result<(), FlashError> {
        if let Some(pc_uninit) = self.algo.pc_uninit {
            let result = self.call_function(pc_uninit, op, 0, 0, 0, false, DEFAULT_MAX_POLLS)?;
            if result != 0 {
                core::hint::cold_path();
                return Err(FlashError::UninitFailed { error_code: result });
            }
        }
        Ok(())
    }

    /// Erase a single flash sector.
    pub fn erase_sector(&mut self, address: u64) -> Result<(), FlashError> {
        let result = self.call_function(
            self.algo.pc_erase_sector,
            address as u32,
            0,
            0,
            0,
            false,
            DEFAULT_MAX_POLLS,
        )?;
        if result != 0 {
            core::hint::cold_path();
            return Err(FlashError::EraseFailed {
                address,
                error_code: result,
            });
        }
        Ok(())
    }

    /// Erase the entire flash (chip erase).
    ///
    /// Returns `Ok(true)` if chip erase was performed, `Ok(false)` if not supported.
    pub fn erase_all(&mut self) -> Result<bool, FlashError> {
        if let Some(pc_erase_all) = self.algo.pc_erase_all {
            let result =
                self.call_function(pc_erase_all, 0, 0, 0, 0, false, ERASE_ALL_MAX_POLLS)?;
            if result != 0 {
                core::hint::cold_path();
                return Err(FlashError::EraseFailed {
                    address: 0,
                    error_code: result,
                });
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Program a single page of data to flash.
    ///
    /// `data` must not exceed the algorithm's page size.
    pub fn program_page(&mut self, address: u64, data: &[u8]) -> Result<(), FlashError> {
        if data.len() > self.algo.page_size as usize {
            core::hint::cold_path();
            return Err(FlashError::PageSizeExceeded {
                max: self.algo.page_size,
                actual: data.len(),
            });
        }

        // Write data to the page buffer in target RAM
        let mem = self.core.memory();
        mem.write_8(self.algo.page_buffer_addr as u32, data)?;

        // Call ProgramPage(address, size, buffer_addr)
        let result = self.call_function(
            self.algo.pc_program_page,
            address as u32,                    // R0: flash address
            data.len() as u32,                 // R1: data size
            self.algo.page_buffer_addr as u32, // R2: buffer address
            0,                                 // R3: reserved
            false,
            DEFAULT_MAX_POLLS,
        )?;
        if result != 0 {
            core::hint::cold_path();
            return Err(FlashError::ProgramFailed {
                address,
                error_code: result,
            });
        }
        Ok(())
    }

    /// Access the underlying memory accessor for direct reads.
    pub fn memory(&mut self) -> &mut MemoryAccessor<'a, D> {
        self.core.memory()
    }
}

// -- High-level convenience functions --

/// Flash an entire image: init -> erase affected sectors -> program pages -> uninit.
///
/// `base_addr` is the flash start address. `image` is the raw binary data.
pub fn flash_image<D: DapAccess>(
    flasher: &mut Flasher<'_, D>,
    base_addr: u64,
    image: &[u8],
) -> Result<(), FlashError> {
    let page_size = flasher.algo.page_size as usize;

    // Init for erase
    flasher.init(operation::ERASE)?;

    // Erase affected sectors
    let image_end = base_addr + image.len() as u64;
    for sector in flasher.algo.flash_properties.sectors.iter() {
        let sector_end = sector.address + sector.size;
        if sector.address < image_end && sector_end > base_addr {
            flasher.erase_sector(sector.address)?;
        }
    }

    flasher.uninit(operation::ERASE)?;

    // Init for program
    flasher.init(operation::PROGRAM)?;

    // Program page by page
    for (i, chunk) in image.chunks(page_size).enumerate() {
        let addr = base_addr + (i * page_size) as u64;
        flasher.program_page(addr, chunk)?;
    }

    flasher.uninit(operation::PROGRAM)?;

    Ok(())
}

/// Verify flashed data by reading back and comparing.
///
/// Uses the memory accessor (MEM-AP) to read flash directly.
pub fn verify_image<D: DapAccess>(
    flasher: &mut Flasher<'_, D>,
    base_addr: u64,
    image: &[u8],
) -> Result<(), FlashError> {
    let mem = flasher.memory();
    let mut buf = [0u8; VERIFY_CHUNK_SIZE];

    for (i, chunk) in image.chunks(VERIFY_CHUNK_SIZE).enumerate() {
        let addr = base_addr + (i * VERIFY_CHUNK_SIZE) as u64;
        mem.read_8(addr as u32, &mut buf[..chunk.len()])?;

        if buf[..chunk.len()] != *chunk {
            core::hint::cold_path();
            return Err(FlashError::VerifyMismatch { address: addr });
        }
    }

    Ok(())
}
