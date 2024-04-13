use crate::{Core, CoreAccessOptions};

use super::chip::Chip;
use super::flash_algorithm::RawFlashAlgorithm;
use jep106::JEP106Code;

use super::const_generic_core;
use serde::{Deserialize, Serialize};
/// Source of a target description.
///
/// This is used for diagnostics, when
/// an error related to a target description occurs.
#[derive(Clone, Debug, PartialEq, Eq, defmt::Format)]
pub enum TargetDescriptionSource {
    /// The target description is a generic target description,
    /// which just describes a core type (e.g. M4), without any
    /// flash algorithm or memory description.
    Generic,
    /// The target description is a built-in target description,
    /// which was included into probe-rs at compile time.
    BuiltIn,
    /// The target description was from an external source
    /// during runtime.
    External,
}

/// Type of a supported core.
#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, Ord, PartialOrd, defmt::Format,
)]
#[serde(rename_all = "snake_case")]
pub enum CoreType {
    /// ARMv6-M: Cortex M0, M0+, M1
    Armv6m,
    /// ARMv7-A: Cortex A7, A9, A15
    Armv7a,
    /// ARMv7-M: Cortex M3
    Armv7m,
    /// ARMv7e-M: Cortex M4, M7
    Armv7em,
    /// ARMv7-A: Cortex A35, A55, A72
    Armv8a,
    /// ARMv8-M: Cortex M23, M33
    Armv8m,
    /// RISC-V
    Riscv,
    /// Xtensa - TODO: may need to split into NX, LX6 and LX7
    Xtensa,
}

impl CoreType {
    /// Returns true if the core type is an ARM Cortex-M
    pub fn is_cortex_m(&self) -> bool {
        matches!(
            self,
            CoreType::Armv6m | CoreType::Armv7em | CoreType::Armv7m | CoreType::Armv8m
        )
    }

    pub const fn into_default_core(self) -> &'static [Core<'static>] {
        match self {
            Self::Armv6m => &const_generic_core::ARM_V6M,
            Self::Armv7a => &const_generic_core::ARM_V7A,
            Self::Armv7m => &const_generic_core::ARM_V7M,
            Self::Armv7em => &const_generic_core::ARM_V7EM,
            Self::Armv8a => &const_generic_core::ARM_V8A,
            Self::Armv8m => &const_generic_core::ARM_V8M,
            Self::Riscv => &const_generic_core::RISCV,
            Self::Xtensa => &const_generic_core::XTENSA,
        }
    }
}

/// The architecture family of a specific [`CoreType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Architecture {
    /// An ARM core of one of the specific types [`CoreType::Armv6m`], [`CoreType::Armv7m`], [`CoreType::Armv7em`] or [`CoreType::Armv8m`]
    Arm,
    /// A RISC-V core.
    Riscv,
    /// An Xtensa core.
    Xtensa,
}

impl CoreType {
    /// Returns the parent architecture family of this core type.
    pub fn architecture(&self) -> Architecture {
        match self {
            CoreType::Riscv => Architecture::Riscv,
            CoreType::Xtensa => Architecture::Xtensa,
            _ => Architecture::Arm,
        }
    }
}

/// Instruction set used by a core
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, defmt::Format)]
pub enum InstructionSet {
    /// ARM Thumb 2 instruction set
    Thumb2,
    /// ARM A32 (often just called ARM) instruction set
    A32,
    /// ARM A64 (aarch64) instruction set
    A64,
    /// RISC-V 32-bit uncompressed instruction sets (RV32) - covers all ISA variants that use 32-bit instructions.
    RV32,
    /// RISC-V 32-bit compressed instruction sets (RV32C) - covers all ISA variants that allow compressed 16-bit instructions.
    RV32C,
    /// Xtensa instruction set
    Xtensa,
}

impl InstructionSet {
    /// Get the instruction set from a rustc target triple.
    pub fn from_target_triple(triple: &str) -> Option<Self> {
        match triple.split('-').next()? {
            "thumbv6m" | "thumbv7em" | "thumbv7m" | "thumbv8m" => Some(InstructionSet::Thumb2),
            "arm" => Some(InstructionSet::A32),
            "aarch64" => Some(InstructionSet::A64),
            "xtensa" => Some(InstructionSet::Xtensa),
            other => {
                if let Some(features) = other.strip_prefix("riscv32") {
                    if features.contains('c') {
                        Some(InstructionSet::RV32C)
                    } else {
                        Some(InstructionSet::RV32)
                    }
                } else {
                    None
                }
            }
        }
    }

    /// Get the minimum instruction size in bytes.
    pub fn get_minimum_instruction_size(&self) -> u8 {
        match self {
            InstructionSet::Thumb2 => {
                // Thumb2 uses a variable size (2 or 4) instruction set. For our purposes, we set it as 2, so that we don't accidentally read outside of addressable memory.
                2
            }
            InstructionSet::A32 => 4,
            InstructionSet::A64 => 4,
            InstructionSet::RV32 => 4,
            InstructionSet::RV32C => 2,
            InstructionSet::Xtensa => 2,
        }
    }
    /// Get the maximum instruction size in bytes. All supported architectures have a maximum instruction size of 4 bytes.
    pub fn get_maximum_instruction_size(&self) -> u8 {
        // TODO: Xtensa may have wide instructions
        4
    }

    /// Returns whether a CPU with the `self` instruction set is compatible with a program compiled for `instr_set`.
    pub fn is_compatible(&self, instr_set: InstructionSet) -> bool {
        if *self == instr_set {
            return true;
        }

        matches!(
            (self, instr_set),
            (InstructionSet::RV32C, InstructionSet::RV32)
        )
    }
}

/// This describes a chip family with all its variants.
///
/// This struct is usually read from a target description
/// file.
// #[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub struct ChipFamily<'a> {
    /// This is the name of the chip family in base form.
    /// E.g. `nRF52832`.
    pub name: &'a str,
    /// The JEP106 code of the manufacturer.
    pub manufacturer: Option<JEP106Code>,
    /// The `target-gen` process will set this to `true`.
    /// Please change this to `false` if this file is modified from the generated, or is a manually created target description.
    pub generated_from_pack: bool,
    /// The latest release of the pack file from which this was generated.
    /// Values:
    /// - `Some("1.3.0")` if the latest pack file release was for example "1.3.0".
    /// - `None` if this was not generated from a pack file, or has been modified since it was generated.
    pub pack_file_release: Option<&'a str>,
    /// This vector holds all the variants of the family.
    pub variants: &'a [Chip<'a>],
    /// This vector holds all available algorithms.
    pub flash_algorithms: &'a [RawFlashAlgorithm<'a>],
    /// Source of the target description, used for diagnostics
    pub source: TargetDescriptionSource,
}

impl defmt::Format for ChipFamily<'_> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "ChipFamily {{ name: {}, manufacturer: {}({}), generated_from_pack: {}, pack_file_release: {}, variants: {}, flash_algorithms: {}, source: {} }}",
            self.name,
            self.manufacturer.map_or("NotListed", |x| x.get().unwrap_or("Unknown")),
            self.manufacturer.map_or(None, |x| Some((x.id, x.cc))),
            self.generated_from_pack,
            self.pack_file_release,
            self.variants,
            self.flash_algorithms,
            self.source,
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum ChipValidationError {
    UnknownFlashAlgorithm,
    MismatchCoreDef,
    MissingCoreDef,
    WrongCoreAccess(CoreType),
    RefusedDebugBase(CoreType),
    RefusedCtiBase(CoreType),
    RefusedCoreOptionsRiscV(CoreType),
    RefusedCoreOptionsXtensa(CoreType),
    MemoryRegionMappingIrregular(u16),
    MemoryRegionNotAssignedCore(u16), // index
}

impl ChipFamily<'_> {
    /// Validates the [`ChipFamily`] such that probe-rs can make assumptions about the correctness without validating thereafter.
    ///
    /// This method should be called right after the [`ChipFamily`] is created!
    pub fn validate(&self) -> Result<(), ChipValidationError> {
        // We check each variant if it is valid.
        // If one is not valid, we abort with an appropriate error message.
        for variant in self.variants {
            // Make sure the algorithms used on the variant actually exist on the family (this is basically a check for typos).
            for algorithm_name in variant.flash_algorithms.iter() {
                if !self
                    .flash_algorithms
                    .iter()
                    .any(|algorithm| &algorithm.name == algorithm_name)
                {
                    defmt::error!(
                        "unknown flash algorithm `{}` for variant `{}`",
                        algorithm_name,
                        variant.name
                    );
                    return Err(ChipValidationError::UnknownFlashAlgorithm);
                }
            }

            // Check that there is at least one core.
            if let Some(core) = variant.cores.first() {
                // Make sure that the core types (architectures) are not mixed.
                let architecture = core.core_type.architecture();
                if variant
                    .cores
                    .iter()
                    .any(|core| core.core_type.architecture() != architecture)
                {
                    defmt::error!(
                        "definition for variant `{}` contains mixed core architectures",
                        variant.name
                    );
                    return Err(ChipValidationError::MismatchCoreDef);
                }
            } else {
                defmt::error!(
                    "definition for variant `{}` does not contain any cores",
                    variant.name
                );
                return Err(ChipValidationError::MissingCoreDef);
            }

            // Core specific validation logic based on type
            for core in variant.cores.iter() {
                // The core access options must match the core type specified
                match &core.core_access_options {
                    CoreAccessOptions::Arm(options) => {
                        if !matches!(
                            core.core_type,
                            CoreType::Armv6m
                                | CoreType::Armv7a
                                | CoreType::Armv7em
                                | CoreType::Armv7m
                                | CoreType::Armv8a
                                | CoreType::Armv8m
                        ) {
                            defmt::error!(
                                "Arm options don't match core type {:?} on core {}",
                                core.core_type,
                                core.name
                            );
                            return Err(ChipValidationError::WrongCoreAccess(core.core_type));
                        }

                        if matches!(core.core_type, CoreType::Armv7a | CoreType::Armv8a)
                            && options.debug_base.is_none()
                        {
                            defmt::error!("Core {} requires setting debug_base", core.name);
                            return Err(ChipValidationError::RefusedDebugBase(core.core_type));
                        }

                        if core.core_type == CoreType::Armv8a && options.cti_base.is_none() {
                            defmt::error!("Core {} requires setting cti_base", core.name);
                            return Err(ChipValidationError::RefusedCtiBase(core.core_type));
                        }
                    }
                    CoreAccessOptions::Riscv(_) => {
                        if core.core_type != CoreType::Riscv {
                            defmt::error!(
                                "Riscv options don't match core type {:?} on core {}",
                                core.core_type,
                                core.name
                            );

                            return Err(ChipValidationError::RefusedCoreOptionsRiscV(
                                core.core_type,
                            ));
                        }
                    }
                    CoreAccessOptions::Xtensa(_) => {
                        if core.core_type != CoreType::Xtensa {
                            defmt::error!(
                                "Xtensa options don't match core type {:?} on core {}",
                                core.core_type,
                                core.name
                            );

                            return Err(ChipValidationError::RefusedCoreOptionsXtensa(
                                core.core_type,
                            ));
                        }
                    }
                }
            }

            for (pos, memory) in variant.memory_map.iter().enumerate() {
                // Ensure that the memory is assigned to a core, and that all the cores exist

                for core in memory.cores() {
                    // if !core_names.contains(core) {
                    if !variant.is_core_existed(core) {
                        defmt::error!(
                            "Variant {}, memory region {:?} is assigned to a non-existent core {}",
                            variant.name,
                            memory,
                            core
                        );

                        return Err(ChipValidationError::MemoryRegionMappingIrregular(
                            pos as u16,
                        ));
                    }
                }

                if !memory.cores().is_empty() {
                    defmt::error!(
                        "Variant {}, memory region {:?} is not assigned to a core",
                        variant.name,
                        memory
                    );
                    return Err(ChipValidationError::MemoryRegionNotAssignedCore(pos as u16));
                }
            }
        }

        Ok(())
    }
}

impl ChipFamily<'_> {
    /// Get the different [Chip]s which are part of this
    /// family.
    pub fn variants(&self) -> &[Chip] {
        self.variants
    }

    /// Get all flash algorithms for this family of chips.
    pub fn algorithms(&self) -> &[RawFlashAlgorithm] {
        self.flash_algorithms
    }

    /// Try to find a [RawFlashAlgorithm] with a given name.
    pub fn get_algorithm(&self, name: impl AsRef<str>) -> Option<&RawFlashAlgorithm> {
        let name = name.as_ref();
        self.flash_algorithms.iter().find(|elem| elem.name == name)
    }
}
