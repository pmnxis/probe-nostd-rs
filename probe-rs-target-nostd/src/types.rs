#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CoreType {
    Armv6m,
    Armv7m,
    Armv7em,
    Armv8m,
    Armv7a,
    Armv8a,
    Riscv,
    Xtensa,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChipDef {
    pub name: &'static str,
    pub cores: &'static [CoreDef],
    pub memory_map: &'static [MemoryRegion],
    pub flash_algorithms: &'static [&'static FlashAlgoDef],
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CoreDef {
    pub name: &'static str,
    pub core_type: CoreType,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MemoryRegion {
    Ram {
        name: &'static str,
        start: u64,
        end: u64,
    },
    Nvm {
        name: &'static str,
        start: u64,
        end: u64,
    },
    Generic {
        name: &'static str,
        start: u64,
        end: u64,
    },
}

impl MemoryRegion {
    pub const fn start(&self) -> u64 {
        match self {
            MemoryRegion::Ram { start, .. } => *start,
            MemoryRegion::Nvm { start, .. } => *start,
            MemoryRegion::Generic { start, .. } => *start,
        }
    }

    pub const fn end(&self) -> u64 {
        match self {
            MemoryRegion::Ram { end, .. } => *end,
            MemoryRegion::Nvm { end, .. } => *end,
            MemoryRegion::Generic { end, .. } => *end,
        }
    }

    pub const fn name(&self) -> &'static str {
        match self {
            MemoryRegion::Ram { name, .. } => name,
            MemoryRegion::Nvm { name, .. } => name,
            MemoryRegion::Generic { name, .. } => name,
        }
    }

    pub const fn size(&self) -> u64 {
        self.end() - self.start()
    }

    pub const fn contains(&self, addr: u64) -> bool {
        addr >= self.start() && addr < self.end()
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct FlashAlgoDef {
    pub name: &'static str,
    pub instructions: &'static [u8],
    pub load_address: u64,
    pub data_section_offset: u64,
    pub pc_init: Option<u64>,
    pub pc_uninit: Option<u64>,
    pub pc_program_page: u64,
    pub pc_erase_sector: u64,
    pub pc_erase_all: Option<u64>,
    pub stack_size: u32,
    pub flash_properties: FlashProperties,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct FlashProperties {
    pub address_range_start: u64,
    pub address_range_end: u64,
    pub page_size: u32,
    pub erased_byte_value: u8,
    pub program_page_timeout: u32,
    pub erase_sector_timeout: u32,
    pub sectors: &'static [SectorDescription],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SectorDescription {
    pub size: u64,
    pub address: u64,
}
