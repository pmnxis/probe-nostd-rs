use crate::SectorDescription;
use core::ops::Range;
use serde::Serialize;

/// Properties of flash memory, which
/// are used when programming Flash memory.
///
/// These values are read from the
/// YAML target description files.
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, defmt::Format)]
pub struct FlashProperties<'a> {
    /// The range of the device flash.
    pub address_range: Range<u64>,
    /// The page size of the device flash.
    pub page_size: u32,
    /// The value of a byte in flash that was just erased.
    pub erased_byte_value: u8,
    /// The approximative time it takes to program a page.
    pub program_page_timeout: u32,
    /// The approximative time it takes to erase a sector.
    pub erase_sector_timeout: u32,
    /// The available sectors of the device flash.
    pub sectors: &'a [SectorDescription],
}

impl Default for FlashProperties<'_> {
    #[allow(clippy::reversed_empty_ranges)]
    fn default() -> Self {
        FlashProperties {
            address_range: 0..0,
            page_size: 0,
            erased_byte_value: 0,
            program_page_timeout: 0,
            erase_sector_timeout: 0,
            sectors: &[],
        }
    }
}

impl From<&FlashProperties<'_>> for probe_rs_target::FlashProperties {
    fn from(value: &FlashProperties<'_>) -> Self {
        Self {
            address_range: value.address_range.clone(),
            page_size: value.page_size,
            erased_byte_value: value.erased_byte_value,
            program_page_timeout: value.program_page_timeout,
            erase_sector_timeout: value.erase_sector_timeout,
            sectors: { value.sectors.to_vec() },
        }
    }
}

impl Serialize for FlashProperties<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let allocable: probe_rs_target::FlashProperties = self.into();
        allocable.serialize(serializer)
    }
}
