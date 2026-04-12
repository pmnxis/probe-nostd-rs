#![no_std]
#![deny(unsafe_code)]

pub mod registry;
pub mod types;

pub use registry::{available_targets, lookup_target};
pub use types::*;
