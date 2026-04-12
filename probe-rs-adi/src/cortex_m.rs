//! Cortex-M core debug control.
//!
//! Provides `CortexMControl` for halting, running, stepping, resetting,
//! and reading/writing core registers via memory-mapped debug registers.

use crate::ap::ApAddress;
use crate::dap_access::DapAccess;
use crate::error::AdiError;
use crate::memory::MemoryAccessor;

// -- Debug register addresses (ARM Cortex-M) --

/// Debug Halting Control and Status Register.
const DHCSR: u32 = 0xE000_EDF0;
/// Debug Core Register Selector Register.
const DCRSR: u32 = 0xE000_EDF4;
/// Debug Core Register Data Register.
const DCRDR: u32 = 0xE000_EDF8;
/// Debug Exception and Monitor Control Register.
const DEMCR: u32 = 0xE000_EDFC;
/// Application Interrupt and Reset Control Register.
const AIRCR: u32 = 0xE000_ED0C;

// -- DHCSR bits --

/// Debug key (must be written to bits 31:16 for writes to take effect).
const DBGKEY: u32 = 0xA05F_0000;
/// Enable halting debug.
const C_DEBUGEN: u32 = 1 << 0;
/// Halt the core.
const C_HALT: u32 = 1 << 1;
/// Single step the core.
const C_STEP: u32 = 1 << 2;
/// Mask PendSV, SysTick, and external configurable interrupts.
const C_MASKINTS: u32 = 1 << 3;
/// Register read/write transfer ready (read-only, status).
const S_REGRDY: u32 = 1 << 16;
/// Core is halted (read-only, status).
const S_HALT: u32 = 1 << 17;

// -- AIRCR bits --

/// Vector Key for AIRCR writes.
const VECTKEY: u32 = 0x05FA_0000;
/// System reset request.
const SYSRESETREQ: u32 = 1 << 2;

// -- DEMCR bits --

/// Vector catch: halt on reset.
const VC_CORERESET: u32 = 1 << 0;

// -- DCRSR bits --

/// Read/write select: 0 = read, 1 = write.
const DCRSR_WRITE: u32 = 1 << 16;

/// Maximum iterations for register transfer readiness polling.
const REGRDY_MAX_RETRIES: u32 = 100;

/// Maximum iterations for halt polling.
const HALT_MAX_RETRIES: u32 = 100_000;

/// Cortex-M core debug control.
///
/// Operates through MEM-AP memory-mapped debug registers.
pub struct CortexMControl<'a, D: DapAccess> {
    mem: MemoryAccessor<'a, D>,
}

impl<'a, D: DapAccess> CortexMControl<'a, D> {
    /// Create a new Cortex-M core controller.
    ///
    /// Uses the given AP (typically AP 0) for memory-mapped register access.
    pub fn new(dap: &'a mut D, ap: ApAddress) -> Self {
        Self {
            mem: MemoryAccessor::new(dap, ap),
        }
    }

    /// Enable halting debug mode.
    pub fn enable_debug(&mut self) -> Result<(), AdiError> {
        self.mem.write_word_32(DHCSR, DBGKEY | C_DEBUGEN)
    }

    /// Halt the core.
    pub fn halt(&mut self) -> Result<(), AdiError> {
        self.mem.write_word_32(DHCSR, DBGKEY | C_DEBUGEN | C_HALT)
    }

    /// Resume core execution.
    pub fn run(&mut self) -> Result<(), AdiError> {
        self.mem.write_word_32(DHCSR, DBGKEY | C_DEBUGEN)
    }

    /// Check if the core is currently halted.
    pub fn is_halted(&mut self) -> Result<bool, AdiError> {
        let dhcsr = self.mem.read_word_32(DHCSR)?;
        Ok((dhcsr & S_HALT) != 0)
    }

    /// Single-step the core (execute one instruction).
    pub fn step(&mut self) -> Result<(), AdiError> {
        self.mem
            .write_word_32(DHCSR, DBGKEY | C_DEBUGEN | C_MASKINTS | C_STEP)
    }

    /// Wait for the core to halt.
    pub fn wait_for_halt(&mut self) -> Result<(), AdiError> {
        for _ in 0..HALT_MAX_RETRIES {
            if self.is_halted()? {
                return Ok(());
            }
        }
        core::hint::cold_path();
        core::hint::cold_path();
        Err(AdiError::RegTransferTimeout)
    }

    /// Read a core register.
    ///
    /// `reg` is the register selector value:
    /// - 0-15: R0-R15 (R13=SP, R14=LR, R15=PC)
    /// - 16: xPSR
    /// - 17: MSP (Main Stack Pointer)
    /// - 18: PSP (Process Stack Pointer)
    /// - 20: CONTROL/FAULTMASK/BASEPRI/PRIMASK (packed)
    /// - 33+: FPU registers (S0-S31, FPSCR)
    pub fn read_core_reg(&mut self, reg: u16) -> Result<u32, AdiError> {
        // Write DCRSR to select register for reading
        self.mem.write_word_32(DCRSR, reg as u32)?;

        // Poll DHCSR.S_REGRDY
        for _ in 0..REGRDY_MAX_RETRIES {
            let dhcsr = self.mem.read_word_32(DHCSR)?;
            if (dhcsr & S_REGRDY) != 0 {
                // Read the value from DCRDR
                return self.mem.read_word_32(DCRDR);
            }
        }

        core::hint::cold_path();
        Err(AdiError::RegTransferTimeout)
    }

    /// Write a core register.
    ///
    /// See `read_core_reg` for register selector values.
    pub fn write_core_reg(&mut self, reg: u16, value: u32) -> Result<(), AdiError> {
        // Write value to DCRDR first
        self.mem.write_word_32(DCRDR, value)?;

        // Write DCRSR to select register for writing
        self.mem.write_word_32(DCRSR, (reg as u32) | DCRSR_WRITE)?;

        // Poll DHCSR.S_REGRDY
        for _ in 0..REGRDY_MAX_RETRIES {
            let dhcsr = self.mem.read_word_32(DHCSR)?;
            if (dhcsr & S_REGRDY) != 0 {
                return Ok(());
            }
        }

        core::hint::cold_path();
        Err(AdiError::RegTransferTimeout)
    }

    /// Issue a system reset via AIRCR.
    pub fn system_reset(&mut self) -> Result<(), AdiError> {
        self.mem.write_word_32(AIRCR, VECTKEY | SYSRESETREQ)
    }

    /// Reset the core and halt immediately after reset.
    ///
    /// Uses the VC_CORERESET vector catch in DEMCR to halt on the reset vector.
    pub fn reset_and_halt(&mut self) -> Result<(), AdiError> {
        // Enable halt-on-reset vector catch
        let demcr = self.mem.read_word_32(DEMCR)?;
        self.mem.write_word_32(DEMCR, demcr | VC_CORERESET)?;

        // Issue reset
        self.system_reset()?;

        // Wait for the core to halt (it should hit the reset vector catch)
        self.wait_for_halt()?;

        // Clear the vector catch
        self.mem.write_word_32(DEMCR, demcr & !VC_CORERESET)?;

        Ok(())
    }

    /// Get a mutable reference to the underlying memory accessor.
    ///
    /// Useful for direct memory operations alongside core control.
    pub fn memory(&mut self) -> &mut MemoryAccessor<'a, D> {
        &mut self.mem
    }
}
