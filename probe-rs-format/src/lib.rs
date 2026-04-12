//! # probe-rs-format
//!
//! no_std firmware file format parsers for probe-rs.
//!
//! Parses Intel HEX, UF2, ELF, and raw BIN formats into `DataSegment`
//! pairs (address + data slice). All parsers are zero-alloc and work
//! with borrowed data -- suitable for const, network streaming, and
//! filesystem sources.

#![no_std]
#![deny(unsafe_code)]

#[cfg(test)]
extern crate std;

mod error;
mod segment;

pub mod bin;

#[cfg(feature = "elf")]
pub mod elf;

#[cfg(feature = "ihex")]
pub mod ihex;

#[cfg(feature = "uf2")]
pub mod uf2;

pub use error::FormatError;
pub use segment::DataSegment;
