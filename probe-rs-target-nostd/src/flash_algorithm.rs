/*
 * SPDX-FileCopyrightText: © 2023 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use super::flash_properties::FlashProperties;
use crate::TransferEncoding;
use serde::Serialize;

/// The raw flash algorithm is the description of a flash algorithm,
/// and is usually read from a target description file.
///
/// Before it can be used for flashing, it has to be assembled for
/// a specific chip, by determining the RAM addresses which are used when flashing.
/// This process is done in the main `probe-rs` library.
// #[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, defmt::Format)]
pub struct RawFlashAlgorithm<'a> {
    /// The name of the flash algorithm.
    pub name: &'a str,
    /// The description of the algorithm.
    pub description: &'a str,
    /// Whether this flash algorithm is the default one or not.
    pub default: bool,
    /// List of 32-bit words containing the code for the algo. If `load_address` is not specified, the code must be position independent (PIC).
    pub instructions: &'a [u8],
    /// Address to load algo into RAM. Optional.
    pub load_address: Option<u64>,
    /// Address to load data into RAM. Optional.
    pub data_load_address: Option<u64>,
    /// Address of the `Init()` entry point. Optional.
    pub pc_init: Option<u64>,
    /// Address of the `UnInit()` entry point. Optional.
    pub pc_uninit: Option<u64>,
    /// Address of the `ProgramPage()` entry point.
    pub pc_program_page: u64,
    /// Address of the `EraseSector()` entry point.
    pub pc_erase_sector: u64,
    /// Address of the `EraseAll()` entry point. Optional.
    pub pc_erase_all: Option<u64>,
    /// The offset from the start of RAM to the data section.
    pub data_section_offset: u64,
    /// Location of the RTT control block in RAM.
    ///
    /// If this is set, the flash algorithm supports RTT output
    /// and debug messages will be read over RTT.
    pub rtt_location: Option<u64>,
    /// The properties of the flash on the device.
    pub flash_properties: FlashProperties<'a>,
    /// List of cores that can use this algorithm
    pub cores: &'a [&'a str],
    /// The flash algorithm's stack size, in bytes.
    ///
    /// If not set, probe-rs selects a default value.
    /// Increase this value if you're concerned about stack
    /// overruns during flashing.
    pub stack_size: Option<u32>,

    /// The encoding format accepted by the flash algorithm.
    pub transfer_encoding: Option<TransferEncoding>,
}

impl From<&RawFlashAlgorithm<'_>> for probe_rs_target::RawFlashAlgorithm {
    fn from(value: &RawFlashAlgorithm<'_>) -> Self {
        probe_rs_target::RawFlashAlgorithm {
            name: value.name.to_string(),
            description: value.description.to_string(),
            default: value.default,
            instructions: value.instructions.to_vec(),
            load_address: value.load_address,
            data_load_address: value.data_load_address,
            pc_init: value.pc_init,
            pc_uninit: value.pc_uninit,
            pc_program_page: value.pc_program_page,
            pc_erase_sector: value.pc_erase_sector,
            pc_erase_all: value.pc_erase_all,
            data_section_offset: value.data_section_offset,
            rtt_location: value.rtt_location,
            flash_properties: (&value.flash_properties).into(),
            cores: value.cores.iter().map(|x| x.to_string()).collect(),
            stack_size: value.stack_size,
            transfer_encoding: value.transfer_encoding,
        }
    }
}

impl Serialize for RawFlashAlgorithm<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let allocable: probe_rs_target::RawFlashAlgorithm = self.into();
        allocable.serialize(serializer)
    }
}
