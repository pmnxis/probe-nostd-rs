/*
 * SPDX-FileCopyrightText: © 2023 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use super::flash_properties::FlashProperties;
// use crate::serialize::{hex_option, hex_u_int};
// use base64::{engine::general_purpose as base64_engine, Engine as _};
use serde::{Deserialize, Serialize};

/// Data encoding used by the flash algorithm.
#[derive(
    Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, defmt::Format,
)]
#[serde(rename_all = "snake_case")]
pub enum TransferEncoding {
    /// Raw binary encoding. Probe-rs will not apply any transformation to the flash data.
    #[default]
    Raw,

    /// Flash data is compressed using the `miniz_oxide` crate.
    ///
    /// Compressed images are written in page sized chunks, each chunk written to the image's start
    /// address. The length of the compressed image is stored in the first 4 bytes of the first
    /// chunk of the image.
    Miniz,
}

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
