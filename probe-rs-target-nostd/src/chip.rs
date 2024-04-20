use super::memory::MemoryRegion;
use crate::{const_generic_core, BinaryFormat, CoreAccessOptions, CoreType};

use probe_rs_target::ArmCoreAccessOptions;
// use crate::{serialize::hex_option, CoreType};
use serde::{Deserialize, Serialize};

/// Represents a DAP scan chain element.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, defmt::Format)]
pub struct ScanChainElement<'a> {
    /// Unique name of the DAP
    pub name: Option<&'a str>,
    /// Specifies the IR length of the DAP (default value: 4).
    pub ir_len: Option<u8>,
}

/// Configuration for JTAG probes.
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Debug, Clone, Serialize, PartialEq, defmt::Format)]
pub struct Jtag<'a> {
    /// Describes the scan chain
    ///
    /// ref: `<https://open-cmsis-pack.github.io/Open-CMSIS-Pack-Spec/main/html/sdf_pg.html#sdf_element_scanchain>`
    #[serde(default)]
    pub scan_chain: Option<&'a [ScanChainElement<'a>]>,
}

/// A single chip variant.
///
/// This describes an exact chip variant, including the cores, flash and memory size. For example,
/// the `nRF52832` chip has two variants, `nRF52832_xxAA` and `nRF52832_xxBB`. For this case,
/// the struct will correspond to one of the variants, e.g. `nRF52832_xxAA`.
// #[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Debug, Clone, defmt::Format, Serialize)]
pub struct Chip<'a> {
    /// This is the name of the chip in base form.
    /// E.g. `nRF52832`.
    pub name: &'a str,
    /// The `PART` register of the chip.
    /// This value can be determined via the `cli info` command.
    pub part: Option<u16>,
    /// An URL to the SVD file for this chip.
    pub svd: Option<&'a str>,
    /// The cores available on the chip.
    /// The memory regions available on the chip.
    #[serde(default)]
    pub cores: &'a [Core<'a>],
    pub memory_map: &'a [MemoryRegion<'a>],
    /// Names of all flash algorithms available for this chip.
    ///
    /// This can be used to look up the flash algorithm in the
    /// [`ChipFamily::flash_algorithms`] field.
    ///
    /// [`ChipFamily::flash_algorithms`]: crate::ChipFamily::flash_algorithms
    #[serde(default)]
    pub flash_algorithms: &'a [&'a str],
    /// Specific memory ranges to search for a dynamic RTT header for code
    /// running on this chip.
    ///
    /// This need not be specified for most chips because the default is
    /// to search all RAM regions specified in `memory_map`. However,
    /// that behavior isn't appropriate for some chips, such as those which
    /// have a very large amount of RAM that would be time-consuming to
    /// scan exhaustively.
    ///
    /// If specified then this is a list of zero or more address ranges to
    /// scan. Each address range must be enclosed in exactly one RAM region
    /// from `memory_map`. An empty list disables automatic scanning
    /// altogether, in which case RTT will be enabled only when using an
    /// executable image that includes the `_SEGGER_RTT` symbol pointing
    /// to the exact address of the RTT header.
    pub rtt_scan_ranges: Option<&'a [core::ops::Range<u64>]>,
    /// JTAG-specific options
    #[serde(default)]
    pub jtag: Option<Jtag<'a>>,
    /// The default binary format for this chip
    pub default_binary_format: Option<BinaryFormat>,
}

impl Chip<'_> {
    /// Create a generic chip with the given name, a single core,
    /// and no flash algorithm or memory map. Used to create
    /// generic targets.
    pub fn generic_arm(name: &'static str, core_type: CoreType) -> Self {
        let core = match core_type {
            CoreType::Armv6m => &const_generic_core::ARM_V6M,
            CoreType::Armv7a => &const_generic_core::ARM_V7A,
            CoreType::Armv7m => &const_generic_core::ARM_V7M,
            CoreType::Armv7em => &const_generic_core::ARM_V7EM,
            CoreType::Armv8a => &const_generic_core::ARM_V8A,
            CoreType::Armv8m => &const_generic_core::ARM_V8M,
            CoreType::Riscv => &const_generic_core::RISCV,
            CoreType::Xtensa => &const_generic_core::XTENSA,
        };

        Chip {
            name,
            part: None,
            svd: None,
            cores: core,
            memory_map: &[],
            flash_algorithms: &[],
            rtt_scan_ranges: None,
            jtag: None,
            default_binary_format: Some(BinaryFormat::Raw),
        }
    }

    pub fn is_core_existed(&self, target: &str) -> bool {
        for inside in self.cores {
            if inside.name == target {
                return true;
            }
        }
        false
    }
}

/// An individual core inside a chip
#[derive(Debug, Clone, Serialize, Deserialize, defmt::Format)]
pub struct Core<'a> {
    /// The core name.
    pub name: &'a str,

    /// The core type.
    /// E.g. `M0` or `M4`.
    #[serde(rename = "type")]
    pub core_type: CoreType,

    /// The AP number to access the core
    pub core_access_options: CoreAccessOptions,
}

impl Core<'_> {
    /// Default for const
    pub const fn const_default(core_type: CoreType) -> Self {
        Self {
            name: "main",
            core_type,
            core_access_options: CoreAccessOptions::Arm(ArmCoreAccessOptions::const_default()),
        }
    }
}

impl From<&ScanChainElement<'_>> for probe_rs_target::ScanChainElement {
    fn from(value: &ScanChainElement<'_>) -> Self {
        Self {
            name: value.name.map(|x| x.to_string()),
            ir_len: value.ir_len,
        }
    }
}

impl From<&Jtag<'_>> for probe_rs_target::Jtag {
    fn from(value: &Jtag<'_>) -> Self {
        Self {
            scan_chain: value
                .scan_chain
                .map(|x| x.iter().map(|y| y.into()).collect()),
        }
    }
}

impl From<&Chip<'_>> for probe_rs_target::Chip {
    fn from(value: &Chip<'_>) -> Self {
        Self {
            name: value.name.to_string(),
            part: value.part,
            svd: value.svd.map(|x| x.to_string()),
            cores: value.cores.iter().map(|x| x.into()).collect(),
            memory_map: value.memory_map.iter().map(|x| x.into()).collect(),
            flash_algorithms: value
                .flash_algorithms
                .iter()
                .map(|x| x.to_string())
                .collect(),
            rtt_scan_ranges: value.rtt_scan_ranges.map(|x| x.to_vec()),
            jtag: value.jtag.clone().map(|x| (&x).into()),
            default_binary_format: value.default_binary_format.clone(),
        }
    }
}

impl From<&Core<'_>> for probe_rs_target::Core {
    fn from(value: &Core<'_>) -> Self {
        Self {
            name: value.name.to_string(),
            core_type: value.core_type,
            core_access_options: value.core_access_options.clone(),
        }
    }
}
