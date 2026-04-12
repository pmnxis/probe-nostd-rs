//! Flash programming error types.

use probe_rs_adi::AdiError;

/// Errors from flash programming operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FlashError {
    /// Underlying ADI layer error.
    Adi(AdiError),
    /// Failed to write algorithm binary to target RAM.
    AlgorithmLoadFailed,
    /// Algorithm readback verification mismatch.
    AlgorithmVerifyFailed,
    /// Flash Init() returned non-zero error code.
    InitFailed { error_code: u32 },
    /// Flash UnInit() returned non-zero error code.
    UninitFailed { error_code: u32 },
    /// Flash EraseSector() returned non-zero error code.
    EraseFailed { address: u64, error_code: u32 },
    /// Flash ProgramPage() returned non-zero error code.
    ProgramFailed { address: u64, error_code: u32 },
    /// Algorithm execution did not halt within the retry limit.
    Timeout,
    /// Data exceeds the page buffer size.
    PageSizeExceeded { max: u32, actual: usize },
    /// Not enough RAM for algorithm + stack + page buffer.
    InsufficientRam,
    /// Flash verification mismatch at the given address.
    VerifyMismatch { address: u64 },
}

impl From<AdiError> for FlashError {
    fn from(e: AdiError) -> Self {
        FlashError::Adi(e)
    }
}
