//! # probe-rs-wire
//!
//! no_std SWD/JTAG wire protocol layer for probe-rs.
//!
//! This crate provides the lowest-level protocol abstractions for communicating
//! with ARM and RISC-V debug ports over SWD and JTAG. It is designed to run on
//! embedded hosts (ESP32, STM32H7, RP2040, etc.) using `embedded-hal` traits.

#![no_std]
#![deny(unsafe_code)]

pub mod bitbang;
pub mod pin;
pub mod swd;
pub mod util;

#[cfg(feature = "jtag")]
pub mod jtag;

// Re-exports
pub use bitbang::BitbangSwd;
pub use pin::BidirectionalPin;
pub use swd::{SwdError, SwdIo};

#[cfg(feature = "jtag")]
pub use jtag::{JtagError, JtagIo, JtagState, RegisterState};
