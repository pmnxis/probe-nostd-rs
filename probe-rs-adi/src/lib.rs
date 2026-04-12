//! # probe-rs-adi
//!
//! no_std ARM ADIv5 debug interface for probe-rs.
//!
//! Provides DP/AP register access with SELECT caching, MEM-AP memory
//! read/write with TAR boundary handling, and Cortex-M core control
//! (halt, run, reset, register access).

#![no_std]
#![deny(unsafe_code)]

pub mod ap;
pub mod cortex_m;
pub mod dap_access;
pub mod dp;
pub mod error;
pub mod init;
pub mod memory;

// Re-exports
pub use ap::ApAddress;
pub use cortex_m::CortexMControl;
pub use dap_access::{DapAccess, SwdDapAccess};
pub use error::AdiError;
pub use init::swd_init;
pub use memory::MemoryAccessor;
