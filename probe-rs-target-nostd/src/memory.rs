use crate::NvmInfo;
use core::ops::Range;
use serde::Serialize;

/// Represents a region in non-volatile memory (e.g. flash or EEPROM).
// #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, defmt::Format)]
pub struct NvmRegion<'a> {
    /// A name to describe the region
    pub name: Option<&'a str>,
    /// Address range of the region
    pub range: Range<u64>,
    /// True if the chip boots from this memory
    pub is_boot_memory: bool,
    /// List of cores that can access this region
    pub cores: &'a [&'a str],
    /// True if the memory region is an alias of a different memory region.
    pub is_alias: bool,
}

impl NvmRegion<'_> {
    /// Returns the necessary information about the NVM.
    pub fn nvm_info(&self) -> NvmInfo {
        NvmInfo {
            rom_start: self.range.start,
        }
    }
}

/// Represents a region in RAM.
// #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, defmt::Format)]
pub struct RamRegion<'a> {
    /// A name to describe the region
    pub name: Option<&'a str>,
    /// Address range of the region
    pub range: Range<u64>,
    /// True if the chip boots from this memory
    pub is_boot_memory: bool,
    /// List of cores that can access this region
    pub cores: &'a [&'a str],
}

/// Represents a generic region.
#[derive(Debug, Clone, PartialEq, Eq, Hash, defmt::Format)]
pub struct GenericRegion<'a> {
    /// A name to describe the region
    pub name: Option<&'a str>,
    /// Address range of the region
    pub range: Range<u64>,
    /// List of cores that can access this region
    pub cores: &'a [&'a str],
}

/// Enables the user to do range intersection testing.
pub trait MemoryRange {
    /// Returns true if `self` contains `range` fully.
    fn contains_range(&self, range: &Range<u64>) -> bool;

    /// Returns true if `self` intersects `range` partially.
    fn intersects_range(&self, range: &Range<u64>) -> bool;

    /// Ensure memory reads using this memory range, will be aligned to 32 bits.
    /// This may result in slightly more memory being read than requested.
    fn align_to_32_bits(&mut self);
}

impl MemoryRange for Range<u64> {
    fn contains_range(&self, range: &Range<u64>) -> bool {
        if range.end == 0 {
            false
        } else {
            self.contains(&range.start) && self.contains(&(range.end - 1))
        }
    }

    fn intersects_range(&self, range: &Range<u64>) -> bool {
        if range.end == 0 {
            false
        } else {
            self.contains(&range.start) && !self.contains(&(range.end - 1))
                || !self.contains(&range.start) && self.contains(&(range.end - 1))
                || self.contains_range(range)
                || range.contains_range(self)
        }
    }

    fn align_to_32_bits(&mut self) {
        if self.start % 4 != 0 {
            self.start -= self.start % 4;
        }
        if self.end % 4 != 0 {
            // Try to align the end to 32 bits, but don't overflow.
            if let Some(new_end) = self.end.checked_add(4 - self.end % 4) {
                self.end = new_end;
            }
        }
    }
}

/// Declares the type of a memory region.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, defmt::Format)]
pub enum MemoryRegion<'a> {
    /// Memory region describing RAM.
    Ram(RamRegion<'a>),
    /// Generic memory region, which is neither
    /// flash nor RAM.
    Generic(GenericRegion<'a>),
    /// Memory region describing flash, EEPROM or other non-volatile memory.
    #[serde(alias = "Flash")] // Keeping the "Flash" name this for backwards compatibility
    Nvm(NvmRegion<'a>),
}

impl MemoryRegion<'_> {
    /// Returns the RAM region if this is a RAM region, otherwise None.
    pub fn as_ram_region(&self) -> Option<&RamRegion> {
        match self {
            MemoryRegion::Ram(region) => Some(region),
            _ => None,
        }
    }

    /// Returns the NVM region if this is a NVM region, otherwise None.
    pub fn as_nvm_region(&self) -> Option<&NvmRegion> {
        match self {
            MemoryRegion::Nvm(region) => Some(region),
            _ => None,
        }
    }

    /// Returns the address range of the memory region.
    pub fn address_range(&self) -> Range<u64> {
        match self {
            MemoryRegion::Ram(rr) => rr.range.clone(),
            MemoryRegion::Generic(gr) => gr.range.clone(),
            MemoryRegion::Nvm(nr) => nr.range.clone(),
        }
    }

    /// Returns whether the memory region contains the given address.
    pub fn contains(&self, address: u64) -> bool {
        self.address_range().contains(&address)
    }

    /// Get the cores to which this memory region belongs.
    pub fn cores(&self) -> &[&str] {
        match self {
            MemoryRegion::Ram(region) => region.cores,
            MemoryRegion::Generic(region) => region.cores,
            MemoryRegion::Nvm(region) => region.cores,
        }
    }
}

impl From<&NvmRegion<'_>> for probe_rs_target::NvmRegion {
    fn from(value: &NvmRegion<'_>) -> Self {
        Self {
            name: value.name.map(|x| x.to_string()),
            range: value.range.clone(),
            is_boot_memory: value.is_boot_memory,
            cores: value.cores.iter().map(|x| x.to_string()).collect(),
            is_alias: value.is_alias,
        }
    }
}

impl Serialize for NvmRegion<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let allocable: probe_rs_target::NvmRegion = self.into();
        allocable.serialize(serializer)
    }
}

impl From<&RamRegion<'_>> for probe_rs_target::RamRegion {
    fn from(value: &RamRegion<'_>) -> Self {
        Self {
            name: value.name.map(|x| x.to_string()),
            range: value.range.clone(),
            is_boot_memory: value.is_boot_memory,
            cores: value.cores.iter().map(|x| x.to_string()).collect(),
        }
    }
}

impl Serialize for RamRegion<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let allocable: probe_rs_target::RamRegion = self.into();
        allocable.serialize(serializer)
    }
}

impl From<&GenericRegion<'_>> for probe_rs_target::GenericRegion {
    fn from(value: &GenericRegion<'_>) -> Self {
        Self {
            name: value.name.map(|x| x.to_string()),
            range: value.range.clone(),
            cores: value.cores.iter().map(|x| x.to_string()).collect(),
        }
    }
}

impl Serialize for GenericRegion<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let allocable: probe_rs_target::GenericRegion = self.into();
        allocable.serialize(serializer)
    }
}

impl From<&MemoryRegion<'_>> for probe_rs_target::MemoryRegion {
    fn from(value: &MemoryRegion<'_>) -> Self {
        match value {
            MemoryRegion::Ram(x) => probe_rs_target::MemoryRegion::Ram(x.into()),
            MemoryRegion::Generic(x) => probe_rs_target::MemoryRegion::Generic(x.into()),
            MemoryRegion::Nvm(x) => probe_rs_target::MemoryRegion::Nvm(x.into()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn contains_range1() {
        let range1 = 0..1;
        let range2 = 0..1;
        assert!(range1.contains_range(&range2));
    }

    #[test]
    fn contains_range2() {
        let range1 = 0..1;
        let range2 = 0..2;
        assert!(!range1.contains_range(&range2));
    }

    #[test]
    fn contains_range3() {
        let range1 = 0..4;
        let range2 = 0..1;
        assert!(range1.contains_range(&range2));
    }

    #[test]
    fn contains_range4() {
        let range1 = 4..8;
        let range2 = 3..9;
        assert!(!range1.contains_range(&range2));
    }

    #[test]
    fn contains_range5() {
        let range1 = 4..8;
        let range2 = 0..1;
        assert!(!range1.contains_range(&range2));
    }

    #[test]
    fn contains_range6() {
        let range1 = 4..8;
        let range2 = 6..8;
        assert!(range1.contains_range(&range2));
    }

    #[test]
    fn intersects_range1() {
        let range1 = 0..1;
        let range2 = 0..1;
        assert!(range1.intersects_range(&range2));
    }

    #[test]
    fn intersects_range2() {
        let range1 = 0..1;
        let range2 = 0..2;
        assert!(range1.intersects_range(&range2));
    }

    #[test]
    fn intersects_range3() {
        let range1 = 0..4;
        let range2 = 0..1;
        assert!(range1.intersects_range(&range2));
    }

    #[test]
    fn intersects_range4() {
        let range1 = 4..8;
        let range2 = 3..9;
        assert!(range1.intersects_range(&range2));
    }

    #[test]
    fn intersects_range5() {
        let range1 = 4..8;
        let range2 = 0..1;
        assert!(!range1.intersects_range(&range2));
    }

    #[test]
    fn intersects_range6() {
        let range1 = 4..8;
        let range2 = 6..8;
        assert!(range1.intersects_range(&range2));
    }

    #[test]
    fn intersects_range7() {
        let range1 = 4..8;
        let range2 = 3..4;
        assert!(!range1.intersects_range(&range2));
    }

    #[test]
    fn intersects_range8() {
        let range1 = 8..9;
        let range2 = 6..8;
        assert!(!range1.intersects_range(&range2));
    }

    #[test]
    fn intersects_range9() {
        let range1 = 2..4;
        let range2 = 6..8;
        assert!(!range1.intersects_range(&range2));
    }

    #[test]
    fn test_align_to_32_bits_case1() {
        // Test case 1: start and end are already aligned
        let mut range = Range { start: 0, end: 8 };
        range.align_to_32_bits();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 8);
    }

    #[test]
    fn test_align_to_32_bits_case2() {
        // Test case 2: start is not aligned, end is aligned
        let mut range = Range { start: 3, end: 12 };
        range.align_to_32_bits();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 12);
    }

    #[test]
    fn test_align_to_32_bits_case3() {
        // Test case 3: start is aligned, end is not aligned
        let mut range = Range { start: 16, end: 23 };
        range.align_to_32_bits();
        assert_eq!(range.start, 16);
        assert_eq!(range.end, 24);
    }

    #[test]
    fn test_align_to_32_bits_case4() {
        // Test case 4: start and end are not aligned
        let mut range = Range { start: 5, end: 13 };
        range.align_to_32_bits();
        assert_eq!(range.start, 4);
        assert_eq!(range.end, 16);
    }
}
