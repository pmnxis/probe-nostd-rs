//! SWD (Serial Wire Debug) protocol definitions and traits.
//!
//! Provides the host-side SWD interface trait, error types, and protocol helpers
//! for building SWD request packets and interpreting ACK responses.

/// SWD ACK values (3-bit, as read from the wire).
pub mod ack {
    /// Transaction completed successfully.
    pub const OK: u8 = 0b001;
    /// Target is busy, retry later.
    pub const WAIT: u8 = 0b010;
    /// Target detected an error.
    pub const FAULT: u8 = 0b100;
    /// Protocol error (line not driven correctly).
    pub const PROTOCOL: u8 = 0b111;
}

/// SWD protocol errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SwdError {
    /// ACK = WAIT (target busy, retries exhausted).
    AckWait,
    /// ACK = FAULT (target error).
    AckFault,
    /// ACK = PROTOCOL or unknown value.
    AckProtocol,
    /// Data parity mismatch.
    BadParity,
    /// Hardware I/O error (pin or delay failure).
    Io,
    /// No target responded after line reset (DPIDR read failed).
    NoTarget,
}

/// Check a 3-bit ACK value and return an error if not OK.
pub const fn check_ack(ack: u8) -> Result<(), SwdError> {
    match ack & 0b111 {
        ack::OK => Ok(()),
        ack::WAIT => Err(SwdError::AckWait),
        ack::FAULT => Err(SwdError::AckFault),
        _ => Err(SwdError::AckProtocol),
    }
}

/// Build an 8-bit SWD request byte.
///
/// Adapted from dap-rs swd.rs:164-168 and polyfill.rs:840-861.
///
/// This is `const fn` so frequently-used requests can be precomputed at
/// compile time (see `precomputed` module below).
///
/// Bit layout (LSB first on wire):
/// - Bit 0: Start (always 1)
/// - Bit 1: APnDP (false=DP, true=AP)
/// - Bit 2: RnW (false=Write, true=Read)
/// - Bit 3: A\[2\]
/// - Bit 4: A\[3\]
/// - Bit 5: Parity (over bits 1-4)
/// - Bit 6: Stop (always 0)
/// - Bit 7: Park (always 1)
pub const fn make_request(apndp: bool, rnw: bool, addr: u8) -> u8 {
    let req = 1u8
        | ((apndp as u8) << 1)
        | ((rnw as u8) << 2)
        | ((addr & 0x0C) << 1) // A[3:2] into bits 3-4
        | (1 << 7); // Park bit

    let parity = (req.count_ones() & 1) as u8;
    req | (parity << 5)
}

/// Pre-computed SWD request bytes for common DP/AP register accesses.
///
/// These are evaluated at compile time, eliminating runtime computation
/// for the most frequently used SWD transactions.
pub mod precomputed {
    use super::make_request;

    // -- DP reads (apndp=false, rnw=true) --

    /// Read DPIDR (DP addr 0x00).
    pub const DP_READ_DPIDR: u8 = make_request(false, true, 0x00);
    /// Read CTRL/STAT (DP addr 0x04).
    pub const DP_READ_CTRL_STAT: u8 = make_request(false, true, 0x04);
    /// Read RDBUFF (DP addr 0x0C).
    pub const DP_READ_RDBUFF: u8 = make_request(false, true, 0x0C);

    // -- DP writes (apndp=false, rnw=false) --

    /// Write ABORT (DP addr 0x00).
    pub const DP_WRITE_ABORT: u8 = make_request(false, false, 0x00);
    /// Write CTRL/STAT (DP addr 0x04).
    pub const DP_WRITE_CTRL_STAT: u8 = make_request(false, false, 0x04);
    /// Write SELECT (DP addr 0x08).
    pub const DP_WRITE_SELECT: u8 = make_request(false, false, 0x08);

    // -- AP reads (apndp=true, rnw=true) --

    /// Read AP register at offset 0x00 (e.g. CSW).
    pub const AP_READ_0X00: u8 = make_request(true, true, 0x00);
    /// Read AP register at offset 0x04 (e.g. TAR).
    pub const AP_READ_0X04: u8 = make_request(true, true, 0x04);
    /// Read AP register at offset 0x0C (e.g. DRW).
    pub const AP_READ_0X0C: u8 = make_request(true, true, 0x0C);

    // -- AP writes (apndp=true, rnw=false) --

    /// Write AP register at offset 0x00 (e.g. CSW).
    pub const AP_WRITE_0X00: u8 = make_request(true, false, 0x00);
    /// Write AP register at offset 0x04 (e.g. TAR).
    pub const AP_WRITE_0X04: u8 = make_request(true, false, 0x04);
    /// Write AP register at offset 0x0C (e.g. DRW).
    pub const AP_WRITE_0X0C: u8 = make_request(true, false, 0x0C);
}

/// JTAG-to-SWD switching sequence (16-bit, LSB first).
///
/// This is the standard sequence defined in ARM ADIv5 specification
/// to switch from JTAG mode to SWD mode.
pub const JTAG_TO_SWD_SEQUENCE: u16 = 0xE79E;

/// Number of high clock cycles for SWD line reset (spec requires >= 50).
pub const LINE_RESET_CLOCKS: u8 = 56;

/// Host-side SWD wire protocol interface.
///
/// Implementors provide register-level read/write access to DP and AP
/// registers over SWD. The default bulk operation implementations use
/// simple loops; optimized implementations may override them.
pub trait SwdIo {
    /// Error type for SWD operations.
    type Error: core::fmt::Debug;

    /// Read a DP or AP register.
    ///
    /// - `apndp`: false = DP register, true = AP register
    /// - `addr`: register address (only bits A\[3:2\] are used, i.e. 0x00, 0x04, 0x08, 0x0C)
    fn swd_read(&mut self, apndp: bool, addr: u8) -> Result<u32, Self::Error>;

    /// Write a DP or AP register.
    fn swd_write(&mut self, apndp: bool, addr: u8, data: u32) -> Result<(), Self::Error>;

    /// Perform SWD line reset (50+ clocks high, JTAG-to-SWD sequence, 50+ clocks high).
    fn line_reset(&mut self) -> Result<(), Self::Error>;

    /// Set the maximum SWD clock frequency in Hz.
    fn set_clock(&mut self, max_frequency_hz: u32) -> Result<(), Self::Error>;

    /// Read the same register multiple times (e.g. for bulk MEM-AP DRW reads).
    fn swd_read_repeated(
        &mut self,
        apndp: bool,
        addr: u8,
        values: &mut [u32],
    ) -> Result<(), Self::Error> {
        for v in values.iter_mut() {
            *v = self.swd_read(apndp, addr)?;
        }
        Ok(())
    }

    /// Write the same register multiple times (e.g. for bulk MEM-AP DRW writes).
    fn swd_write_repeated(
        &mut self,
        apndp: bool,
        addr: u8,
        values: &[u32],
    ) -> Result<(), Self::Error> {
        for v in values {
            self.swd_write(apndp, addr, *v)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_request_dp_read_dpidr() {
        // DP Read, addr=0x00: APnDP=0, RnW=1, A[3:2]=00
        let req = make_request(false, true, 0x00);
        // Bit 0=1(start), 1=0(DP), 2=1(Read), 3=0, 4=0, 5=parity, 6=0(stop), 7=1(park)
        // Parity over bits 1-4: 0^1^0^0 = 1
        assert_eq!(req & 0b00011111, 0b00_00101); // bits 0-4
        assert_eq!((req >> 5) & 1, 1); // parity
        assert_eq!((req >> 7) & 1, 1); // park
        assert_eq!((req >> 6) & 1, 0); // stop
    }

    #[test]
    fn test_make_request_ap_write() {
        // AP Write, addr=0x04: APnDP=1, RnW=0, A[3:2]=01
        let req = make_request(true, false, 0x04);
        assert_eq!(req & 1, 1); // start
        assert_eq!((req >> 1) & 1, 1); // AP
        assert_eq!((req >> 2) & 1, 0); // Write
        assert_eq!((req >> 3) & 1, 1); // A[2]=1
        assert_eq!((req >> 4) & 1, 0); // A[3]=0
        assert_eq!((req >> 7) & 1, 1); // park
    }

    #[test]
    fn test_check_ack_ok() {
        assert!(check_ack(ack::OK).is_ok());
    }

    #[test]
    fn test_check_ack_wait() {
        assert_eq!(check_ack(ack::WAIT), Err(SwdError::AckWait));
    }

    #[test]
    fn test_check_ack_fault() {
        assert_eq!(check_ack(ack::FAULT), Err(SwdError::AckFault));
    }

    #[test]
    fn test_check_ack_protocol() {
        assert_eq!(check_ack(ack::PROTOCOL), Err(SwdError::AckProtocol));
    }

    #[test]
    fn test_check_ack_unknown() {
        assert_eq!(check_ack(0b011), Err(SwdError::AckProtocol));
    }

    #[test]
    fn test_precomputed_match_runtime() {
        // Verify precomputed constants match runtime computation
        use super::precomputed::*;
        assert_eq!(DP_READ_DPIDR, make_request(false, true, 0x00));
        assert_eq!(DP_READ_CTRL_STAT, make_request(false, true, 0x04));
        assert_eq!(DP_READ_RDBUFF, make_request(false, true, 0x0C));
        assert_eq!(DP_WRITE_ABORT, make_request(false, false, 0x00));
        assert_eq!(DP_WRITE_CTRL_STAT, make_request(false, false, 0x04));
        assert_eq!(DP_WRITE_SELECT, make_request(false, false, 0x08));
        assert_eq!(AP_READ_0X00, make_request(true, true, 0x00));
        assert_eq!(AP_READ_0X0C, make_request(true, true, 0x0C));
        assert_eq!(AP_WRITE_0X00, make_request(true, false, 0x00));
        assert_eq!(AP_WRITE_0X0C, make_request(true, false, 0x0C));
    }

    // Compile-time verification that precomputed values are truly const
    const _: () = {
        assert!(precomputed::DP_READ_DPIDR == make_request(false, true, 0x00));
        assert!(precomputed::DP_WRITE_SELECT == make_request(false, false, 0x08));
    };
}
