//! ARM Debug Port (DP) register definitions.
//!
//! Constants and bitfield structs for the SWD-DP registers defined in
//! ARM ADIv5/v6 specification.

// DP register addresses (2-bit A[3:2] field in SWD request)
// Note: DPIDR and ABORT share address 0x00 (read vs write).

/// DP Identification Register (read-only at addr 0x00).
pub const DPIDR: u8 = 0x00;

/// Abort Register (write-only at addr 0x00).
pub const ABORT: u8 = 0x00;

/// Control/Status Register (read/write at addr 0x04).
pub const CTRL_STAT: u8 = 0x04;

/// AP Select Register (read/write at addr 0x08).
pub const SELECT: u8 = 0x08;

/// Read Buffer (read-only at addr 0x0C, SW-DP only).
pub const RDBUFF: u8 = 0x0C;

// -- ABORT register bits --

/// Clear overrun error.
pub const ABORT_ORUNERRCLR: u32 = 1 << 4;
/// Clear write data error.
pub const ABORT_WDERRCLR: u32 = 1 << 3;
/// Clear sticky error.
pub const ABORT_STKERRCLR: u32 = 1 << 2;
/// Clear sticky compare.
pub const ABORT_STKCMPCLR: u32 = 1 << 1;
/// DAP abort.
pub const ABORT_DAPABORT: u32 = 1 << 0;
/// Clear all sticky errors (ORUNERR | WDERR | STKERR | STKCMP).
pub const ABORT_CLEAR_ALL: u32 = 0x1E;

// -- CTRL/STAT register bits --

/// System power-up request.
pub const CSYSPWRUPREQ: u32 = 1 << 30;
/// Debug power-up request.
pub const CDBGPWRUPREQ: u32 = 1 << 28;
/// System power-up acknowledge (read-only).
pub const CSYSPWRUPACK: u32 = 1 << 31;
/// Debug power-up acknowledge (read-only).
pub const CDBGPWRUPACK: u32 = 1 << 29;
/// Both power-up request bits.
pub const POWERUP_REQ: u32 = CSYSPWRUPREQ | CDBGPWRUPREQ;

/// DPIDR register value wrapper.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Dpidr(pub u32);

impl Dpidr {
    /// DP architecture version.
    pub const fn version(&self) -> u8 {
        ((self.0 >> 12) & 0xF) as u8
    }

    /// Minimal DP implementation.
    pub const fn min(&self) -> bool {
        (self.0 & (1 << 16)) != 0
    }

    /// Designer ID (JEP106 continuation + identity).
    pub const fn designer(&self) -> u16 {
        ((self.0 >> 1) & 0x7FF) as u16
    }

    /// Revision number.
    pub const fn revision(&self) -> u8 {
        ((self.0 >> 28) & 0xF) as u8
    }

    /// Part number.
    pub const fn partno(&self) -> u8 {
        ((self.0 >> 8) & 0xF) as u8
    }
}

/// CTRL/STAT register value wrapper.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CtrlStat(pub u32);

impl CtrlStat {
    /// System power-up acknowledged.
    pub const fn csyspwrupack(&self) -> bool {
        (self.0 & CSYSPWRUPACK) != 0
    }

    /// Debug power-up acknowledged.
    pub const fn cdbgpwrupack(&self) -> bool {
        (self.0 & CDBGPWRUPACK) != 0
    }

    /// Both system and debug power are up.
    pub const fn powered_up(&self) -> bool {
        self.csyspwrupack() && self.cdbgpwrupack()
    }

    /// Sticky error flag.
    pub const fn sticky_err(&self) -> bool {
        (self.0 & (1 << 5)) != 0
    }

    /// Sticky overrun flag.
    pub const fn sticky_orun(&self) -> bool {
        (self.0 & (1 << 1)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpidr_fields() {
        // Example DPIDR: ARM Cortex-M4 typical value
        let dpidr = Dpidr(0x2BA0_1477);
        assert_eq!(dpidr.version(), 1); // bits 15:12 = 0x4... wait
        // Let's use a known value: 0x2BA01477
        // bits 31:28 = 0x2 (revision)
        // bits 27:12 = 0xBA01 (partno is bits 11:8 but in probe-rs it's bits 11:8)
        assert_eq!(dpidr.revision(), 0x2);
    }

    #[test]
    fn test_ctrl_stat_powerup() {
        let stat = CtrlStat(CSYSPWRUPACK | CDBGPWRUPACK);
        assert!(stat.powered_up());
        assert!(stat.csyspwrupack());
        assert!(stat.cdbgpwrupack());

        let stat = CtrlStat(CSYSPWRUPACK);
        assert!(!stat.powered_up());
    }

    #[test]
    fn test_ctrl_stat_errors() {
        let stat = CtrlStat(1 << 5);
        assert!(stat.sticky_err());
        assert!(!stat.sticky_orun());
    }
}
