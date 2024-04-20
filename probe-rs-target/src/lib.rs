#![warn(missing_docs)]

//! Target description schema
//!
//! For debugging and flashing different chips, called *target* in probe-rs, some
//! target specific configuration is required. This includes the architecture of
//! the chip, e.g. RISC-V or ARM, and information about the memory map of the target,
//! which can be used together with a flash algorithm to program the flash memory
//! of a target.
//!
//! This crate contains the schema structs for the YAML target description files.
//!

mod chip;
mod chip_family;
mod flash_algorithm;
mod flash_properties;
mod memory;
pub(crate) mod serialize;

#[cfg(feature = "std")]
pub use {
    chip::{
        ArmCoreAccessOptions, BinaryFormat, Chip, Core, CoreAccessOptions, Jtag,
        RiscvCoreAccessOptions, ScanChainElement, XtensaCoreAccessOptions,
    },
    chip_family::{Architecture, ChipFamily, CoreType, InstructionSet, TargetDescriptionSource},
    flash_algorithm::{RawFlashAlgorithm, TransferEncoding},
    flash_properties::FlashProperties,
    memory::{
        GenericRegion, MemoryRange, MemoryRegion, NvmInfo, NvmRegion, PageInfo, RamRegion,
        SectorDescription, SectorInfo,
    },
};

// no-std version
#[cfg(not(feature = "std"))]
pub use {
    chip::{
        ArmCoreAccessOptions, BinaryFormat, CoreAccessOptions, RiscvCoreAccessOptions,
        XtensaCoreAccessOptions,
    },
    chip_family::{Architecture, CoreType, InstructionSet, TargetDescriptionSource},
    flash_algorithm::TransferEncoding,
    memory::{NvmInfo, PageInfo, SectorDescription, SectorInfo},
};
