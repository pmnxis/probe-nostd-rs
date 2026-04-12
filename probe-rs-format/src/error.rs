//! Format parsing errors.

/// Errors from firmware format parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FormatError {
    /// File magic number does not match expected format.
    InvalidMagic,
    /// Record or block checksum mismatch.
    InvalidChecksum,
    /// Unexpected end of data.
    UnexpectedEof,
    /// Malformed record or block structure.
    InvalidRecord,
    /// Format variant not supported by this parser.
    UnsupportedFormat,
}
