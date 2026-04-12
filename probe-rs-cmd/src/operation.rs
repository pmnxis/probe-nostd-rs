//! Operation, Response, and Error types for the command interpreter.

/// A single probe operation that the Engine can execute.
///
/// This is the common language across all execution modes (direct API,
/// TCP remote, script file). Input sources parse their format into
/// Operation values, and the Engine executes them.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Operation<'a> {
    // -- Connection --
    /// Initialize SWD connection at the given clock frequency.
    Connect { clock_hz: u32 },
    /// Disconnect (no-op currently, reserved for cleanup).
    Disconnect,

    // -- Memory access --
    /// Read bytes from target memory. Result returned in response buffer.
    ReadMem { addr: u32, len: u32 },
    /// Write bytes to target memory.
    WriteMem { addr: u32, data: &'a [u8] },

    // -- Core control --
    /// Halt the target core.
    Halt,
    /// Resume target core execution.
    Run,
    /// System reset.
    Reset,
    /// Reset and halt (vector catch).
    ResetAndHalt,

    // -- Flash programming --
    /// Erase flash region (sector-aligned).
    EraseRegion { addr: u32, len: u32 },
    /// Program flash with data (uses flash algorithm).
    FlashWrite { addr: u32, data: &'a [u8] },
    /// Verify flash contents match expected data.
    Verify { addr: u32, data: &'a [u8] },
    /// Erase entire chip (if supported by algorithm).
    ChipErase,

    // -- Misc --
    /// Delay for the given number of milliseconds.
    /// Implementation is platform-dependent (caller provides delay).
    DelayMs(u32),
}

/// Result of executing an Operation.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Response<'a> {
    /// Operation succeeded with no data.
    Ok,
    /// Operation succeeded, data in the response buffer.
    Data(&'a [u8]),
    /// Operation succeeded, single 32-bit value.
    Value(u32),
    /// Operation failed.
    Error(CmdError),
}

/// Command execution errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CmdError {
    /// SWD/ADI layer error.
    Adi(probe_rs_adi::AdiError),
    /// Flash programming error.
    Flash(u8), // Simplified: error code from FlashError discriminant
    /// Not connected (Connect not called or failed).
    NotConnected,
    /// Response buffer too small for requested read.
    BufferTooSmall,
    /// Unknown or unsupported operation.
    Unsupported,
}

impl From<probe_rs_adi::AdiError> for CmdError {
    fn from(e: probe_rs_adi::AdiError) -> Self {
        CmdError::Adi(e)
    }
}
