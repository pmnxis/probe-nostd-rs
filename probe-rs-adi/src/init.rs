//! SWD initialization sequence.
//!
//! Performs line reset, DPIDR identification, error clearing, and power-up.

use crate::dap_access::{DapAccess, SwdDapAccess};
use crate::dp::{self, CtrlStat, Dpidr};
use crate::error::AdiError;
use probe_rs_wire::swd::SwdIo;

/// Maximum iterations for power-up acknowledgement polling.
const POWERUP_MAX_RETRIES: u32 = 10_000;

/// Initialize an SWD connection.
///
/// Performs the standard SWD initialization sequence:
/// 1. Line reset (JTAG-to-SWD switching)
/// 2. Read DPIDR (confirms target is present)
/// 3. Clear sticky errors via ABORT register
/// 4. Request system and debug power-up
/// 5. Poll until power-up is acknowledged
///
/// Returns the DPIDR value on success.
pub fn swd_init<S>(dap: &mut SwdDapAccess<S>) -> Result<Dpidr, AdiError>
where
    S: SwdIo<Error = probe_rs_wire::SwdError>,
{
    // 1. Line reset
    dap.swd().line_reset().map_err(AdiError::from)?;

    // 2. Read DPIDR -- confirms SWD connection is alive
    let dpidr_val = dap.read_dp(dp::DPIDR)?;

    // 3. Clear all sticky errors
    dap.write_dp(dp::ABORT, dp::ABORT_CLEAR_ALL)?;

    // 4. Request system + debug power-up
    dap.write_dp(dp::CTRL_STAT, dp::POWERUP_REQ)?;

    // 5. Poll for power-up acknowledgement
    for _ in 0..POWERUP_MAX_RETRIES {
        let stat = CtrlStat(dap.read_dp(dp::CTRL_STAT)?);
        if stat.powered_up() {
            return Ok(Dpidr(dpidr_val));
        }
    }

    core::hint::cold_path();
    Err(AdiError::PowerUpTimeout)
}
