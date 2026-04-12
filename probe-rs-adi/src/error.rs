//! Error types for the ADI layer.

use probe_rs_wire::SwdError;

/// Errors from the ADIv5 debug interface layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AdiError {
    /// Underlying SWD wire error.
    Swd(SwdError),
    /// Power-up acknowledgement timeout.
    PowerUpTimeout,
    /// Memory address not properly aligned.
    MemoryNotAligned {
        address: u32,
        required_alignment: u32,
    },
    /// Register transfer readiness timeout (S_REGRDY not set).
    RegTransferTimeout,
    /// Core is not halted when it needs to be.
    CoreNotHalted,
}

impl From<SwdError> for AdiError {
    fn from(e: SwdError) -> Self {
        AdiError::Swd(e)
    }
}
