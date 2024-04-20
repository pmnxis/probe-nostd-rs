use crate::{CoreAccessOptions, CoreType, TargetDescriptionSource};

use super::chip::Chip;
use super::flash_algorithm::RawFlashAlgorithm;
use jep106::JEP106Code;
use serde::Serialize;

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
    /// Values:e
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

impl From<&ChipFamily<'_>> for probe_rs_target::ChipFamily {
    fn from(value: &ChipFamily<'_>) -> Self {
        probe_rs_target::ChipFamily {
            name: value.name.to_string(),
            manufacturer: value.manufacturer,
            generated_from_pack: value.generated_from_pack,
            pack_file_release: value.pack_file_release.map(|x| x.to_string()),
            variants: value.variants.iter().map(|x| x.into()).collect(),
            flash_algorithms: value.flash_algorithms.iter().map(|x| x.into()).collect(),
            source: value.source,
        }
    }
}

impl Serialize for ChipFamily<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let allocable: probe_rs_target::ChipFamily = self.into();
        allocable.serialize(serializer)
    }
}
