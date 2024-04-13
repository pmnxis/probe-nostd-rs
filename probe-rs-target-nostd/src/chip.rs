use super::memory::MemoryRegion;
use crate::CoreType;

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

/// A finite list of all possible binary formats a target might support.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone, defmt::Format)]
#[serde(rename_all = "lowercase")]
pub enum BinaryFormat {
    /// Program sections are bit-for-bit copied to flash.
    #[default]
    Raw,
    /// Program sections are copied to flash, with the relevant headers and metadata for the [ESP-IDF bootloader](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/app_image_format.html#app-image-structures).
    Idf,
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
#[derive(Debug, Clone, defmt::Format)]
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
    pub cores: &'a [Core<'a>],
    pub memory_map: &'a [MemoryRegion<'a>],
    /// Names of all flash algorithms available for this chip.
    ///
    /// This can be used to look up the flash algorithm in the
    /// [`ChipFamily::flash_algorithms`] field.
    ///
    /// [`ChipFamily::flash_algorithms`]: crate::ChipFamily::flash_algorithms
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
    pub jtag: Option<Jtag<'a>>,
    /// The default binary format for this chip
    pub default_binary_format: Option<BinaryFormat>,
}

impl Chip<'_> {
    /// Create a generic chip with the given name, a single core,
    /// and no flash algorithm or memory map. Used to create
    /// generic targets.
    pub fn generic_arm(name: &'static str, core_type: CoreType) -> Self {
        let core = core_type.into_default_core();

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
    pub const fn const_default(core_type: CoreType) -> Self {
        Core {
            name: "main",
            core_type,
            core_access_options: CoreAccessOptions::Arm(ArmCoreAccessOptions::const_default()),
        }
    }
}

/// The data required to access a core
#[derive(Debug, Clone, Serialize, Deserialize, defmt::Format)]
pub enum CoreAccessOptions {
    /// ARM specific options
    Arm(ArmCoreAccessOptions),
    /// RISC-V specific options
    Riscv(RiscvCoreAccessOptions),
    /// Xtensa specific options
    Xtensa(XtensaCoreAccessOptions),
}

/// The data required to access an ARM core
#[derive(Debug, Clone, Serialize, Deserialize, Default, defmt::Format)]
pub struct ArmCoreAccessOptions {
    /// The access port number to access the core
    pub ap: u8,
    /// The port select number to access the core
    pub psel: u32,
    /// The base address of the debug registers for the core.
    /// Required for Cortex-A, optional for Cortex-M
    pub debug_base: Option<u64>,
    /// The base address of the cross trigger interface (CTI) for the core.
    /// Required in ARMv8-A
    pub cti_base: Option<u64>,
}

/// The data required to access a Risc-V core
#[derive(Debug, Clone, Serialize, Deserialize, defmt::Format)]
pub struct RiscvCoreAccessOptions {
    /// The hart id
    pub hart_id: Option<u32>,
}

/// The data required to access an Xtensa core
#[derive(Debug, Clone, Serialize, Deserialize, defmt::Format)]
pub struct XtensaCoreAccessOptions {}

impl ArmCoreAccessOptions {
    pub const fn const_default() -> Self {
        Self {
            ap: 0,
            psel: 0,
            debug_base: None,
            cti_base: None,
        }
    }
}
