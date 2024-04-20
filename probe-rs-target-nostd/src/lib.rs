//! Target description schema for nostd
//!
//! For debugging and flashing different chips, called *target* in probe-rs, some
//! target specific configuration is required. This includes the architecture of
//! the chip, e.g. RISC-V or ARM, and information about the memory map of the target,
//! which can be used together with a flash algorithm to program the flash memory
//! of a target.
//!
//! This crate contains the schema structs for the YAML target description files.
//!

// #![no_std]

mod chip;
mod chip_family;
mod const_generic_core;
mod flash_algorithm;
mod flash_properties;
mod memory;
pub(crate) mod serialize;

pub use chip::{Chip, Core, Jtag, ScanChainElement};
pub use chip_family::ChipFamily;
pub use flash_algorithm::RawFlashAlgorithm;
pub use flash_properties::FlashProperties;
pub use memory::{GenericRegion, MemoryRange, MemoryRegion, NvmRegion, RamRegion};

pub use probe_rs_target::{
    Architecture, ArmCoreAccessOptions, BinaryFormat, CoreAccessOptions, CoreType, InstructionSet,
    NvmInfo, PageInfo, RiscvCoreAccessOptions, SectorDescription, SectorInfo,
    TargetDescriptionSource, TransferEncoding, XtensaCoreAccessOptions,
};
