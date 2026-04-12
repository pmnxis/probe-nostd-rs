//! # probe-rs-cmd
//!
//! no_std command interpreter for probe-rs.
//!
//! Provides a unified `Operation` enum and `Engine` that dispatches
//! operations to the lower layers (wire, ADI, flash, target).
//! Designed for three execution modes:
//! - Mode A: Direct API calls from firmware code
//! - Mode B: TCP/IP remote commands (parser TBD)
//! - Mode C: Script file execution (parser TBD)

#![no_std]
#![deny(unsafe_code)]

pub mod engine;
pub mod operation;

pub use engine::Engine;
pub use operation::{CmdError, Operation, Response};
