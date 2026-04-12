//! DP/AP register access with SELECT caching.
//!
//! Provides the `DapAccess` trait for typed DP/AP register operations,
//! and `SwdDapAccess` which implements it over `SwdIo` with automatic
//! SELECT register management.

use crate::ap::ApAddress;
use crate::dp;
use crate::error::AdiError;
use probe_rs_wire::swd::SwdIo;

/// High-level DP/AP register access.
///
/// Abstracts the SELECT register management and AP read pipelining.
pub trait DapAccess {
    /// Read a DP register.
    fn read_dp(&mut self, addr: u8) -> Result<u32, AdiError>;

    /// Write a DP register.
    fn write_dp(&mut self, addr: u8, value: u32) -> Result<(), AdiError>;

    /// Read an AP register (handles SELECT + RDBUFF pipelining).
    fn read_ap(&mut self, ap: ApAddress, addr: u8) -> Result<u32, AdiError>;

    /// Write an AP register (handles SELECT).
    fn write_ap(&mut self, ap: ApAddress, addr: u8, value: u32) -> Result<(), AdiError>;

    /// Read an AP register repeatedly (bulk transfer).
    fn read_ap_repeated(
        &mut self,
        ap: ApAddress,
        addr: u8,
        values: &mut [u32],
    ) -> Result<(), AdiError>;

    /// Write an AP register repeatedly (bulk transfer).
    fn write_ap_repeated(
        &mut self,
        ap: ApAddress,
        addr: u8,
        values: &[u32],
    ) -> Result<(), AdiError>;
}

/// SWD-based `DapAccess` implementation with SELECT register caching.
///
/// Wraps a `SwdIo` transport and tracks the current SELECT register value
/// to avoid redundant writes.
pub struct SwdDapAccess<S> {
    swd: S,
    /// Cached SELECT register value. Initialized to 0xFFFF_FFFF (invalid sentinel)
    /// to force the first AP access to always write SELECT.
    current_select: u32,
}

impl<S> SwdDapAccess<S>
where
    S: SwdIo<Error = probe_rs_wire::SwdError>,
{
    /// Create a new SWD-based DAP access.
    pub fn new(swd: S) -> Self {
        Self {
            swd,
            current_select: 0xFFFF_FFFF, // invalid sentinel
        }
    }

    /// Get mutable access to the underlying SWD transport.
    ///
    /// Useful for `line_reset()` and `set_clock()` during initialization.
    pub fn swd(&mut self) -> &mut S {
        &mut self.swd
    }

    /// Consume this adapter and return the underlying transport.
    pub fn into_inner(self) -> S {
        self.swd
    }

    /// Ensure SELECT register is set for the given AP and register bank.
    ///
    /// Only writes if the value differs from the cached state.
    fn select_ap_bank(&mut self, ap: ApAddress, addr: u8) -> Result<(), AdiError> {
        // SELECT: ap_sel (bits 31:24) | ap_bank_sel (bits 7:4)
        // dp_bank_sel (bits 3:0) is 0 for standard AP access
        let select = ((ap.0 as u32) << 24) | ((addr as u32) & 0xF0);

        if select != self.current_select {
            self.swd
                .swd_write(false, dp::SELECT, select)
                .map_err(AdiError::from)?;
            self.current_select = select;
        }
        Ok(())
    }
}

impl<S> DapAccess for SwdDapAccess<S>
where
    S: SwdIo<Error = probe_rs_wire::SwdError>,
{
    fn read_dp(&mut self, addr: u8) -> Result<u32, AdiError> {
        self.swd.swd_read(false, addr).map_err(AdiError::from)
    }

    fn write_dp(&mut self, addr: u8, value: u32) -> Result<(), AdiError> {
        self.swd
            .swd_write(false, addr, value)
            .map_err(AdiError::from)
    }

    fn read_ap(&mut self, ap: ApAddress, addr: u8) -> Result<u32, AdiError> {
        self.select_ap_bank(ap, addr)?;

        // AP reads are pipelined: the first read returns stale data.
        // We must read RDBUFF to get the actual value.
        self.swd
            .swd_read(true, addr & 0x0C)
            .map_err(AdiError::from)?;
        self.swd.swd_read(false, dp::RDBUFF).map_err(AdiError::from)
    }

    fn write_ap(&mut self, ap: ApAddress, addr: u8, value: u32) -> Result<(), AdiError> {
        self.select_ap_bank(ap, addr)?;
        self.swd
            .swd_write(true, addr & 0x0C, value)
            .map_err(AdiError::from)
    }

    fn read_ap_repeated(
        &mut self,
        ap: ApAddress,
        addr: u8,
        values: &mut [u32],
    ) -> Result<(), AdiError> {
        if values.is_empty() {
            return Ok(());
        }

        self.select_ap_bank(ap, addr)?;

        // AP reads are pipelined:
        // - First read kicks off the pipeline (returns stale data)
        // - Each subsequent read returns the PREVIOUS read's data
        // - Final RDBUFF read gets the last value

        if values.len() == 1 {
            // Single read: use standard AP read path
            self.swd
                .swd_read(true, addr & 0x0C)
                .map_err(AdiError::from)?;
            values[0] = self
                .swd
                .swd_read(false, dp::RDBUFF)
                .map_err(AdiError::from)?;
        } else {
            // First read to prime the pipeline
            self.swd
                .swd_read(true, addr & 0x0C)
                .map_err(AdiError::from)?;

            // Bulk read: each AP read returns the previous value
            let last_idx = values.len() - 1;
            self.swd
                .swd_read_repeated(true, addr & 0x0C, &mut values[..last_idx])
                .map_err(AdiError::from)?;

            // Final value comes from RDBUFF
            values[last_idx] = self
                .swd
                .swd_read(false, dp::RDBUFF)
                .map_err(AdiError::from)?;
        }

        Ok(())
    }

    fn write_ap_repeated(
        &mut self,
        ap: ApAddress,
        addr: u8,
        values: &[u32],
    ) -> Result<(), AdiError> {
        if values.is_empty() {
            return Ok(());
        }

        self.select_ap_bank(ap, addr)?;

        // AP writes are not pipelined -- each write takes effect immediately.
        self.swd
            .swd_write_repeated(true, addr & 0x0C, values)
            .map_err(AdiError::from)
    }
}
