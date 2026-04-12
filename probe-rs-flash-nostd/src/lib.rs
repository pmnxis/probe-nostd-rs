//! # probe-rs-flash-nostd
//!
//! no_std flash programming for probe-rs.
//!
//! Loads CMSIS-standard flash algorithms into target RAM, then executes
//! erase/program/verify operations via the ARM debug interface.

#![no_std]
#![deny(unsafe_code)]

pub mod algorithm;
pub mod error;
pub mod flasher;

pub use algorithm::AssembledAlgorithm;
pub use error::FlashError;
pub use flasher::Flasher;
