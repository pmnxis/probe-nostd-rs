#![no_std]
mod chip;
mod chip_family;
mod const_generic_core;
mod flash_algorithm;
mod flash_properties;
mod memory;
pub(crate) mod serialize;

pub use chip::{
    ArmCoreAccessOptions, BinaryFormat, Chip, Core, CoreAccessOptions, Jtag,
    RiscvCoreAccessOptions, ScanChainElement, XtensaCoreAccessOptions,
};
pub use chip_family::{
    Architecture, ChipFamily, CoreType, InstructionSet, TargetDescriptionSource,
};
pub use flash_algorithm::{RawFlashAlgorithm, TransferEncoding};
pub use flash_properties::FlashProperties;
pub use memory::{
    GenericRegion, MemoryRange, MemoryRegion, NvmRegion, PageInfo, RamRegion, SectorDescription,
    SectorInfo,
};
