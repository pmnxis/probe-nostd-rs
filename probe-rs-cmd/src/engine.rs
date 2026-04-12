//! Command execution engine.
//!
//! Dispatches `Operation` values to the appropriate lower-layer calls.

use probe_rs_adi::ap::ApAddress;
use probe_rs_adi::memory::MemoryAccessor;
use probe_rs_adi::{SwdDapAccess, swd_init};
use probe_rs_wire::SwdError;
use probe_rs_wire::swd::SwdIo;

use crate::operation::{CmdError, Operation, Response};

/// Command execution engine.
///
/// Wraps an SWD transport and provides high-level operation dispatch.
/// All operations go through the `execute()` method which maps
/// `Operation` variants to the appropriate L1-L3 calls.
pub struct Engine<S: SwdIo<Error = SwdError>> {
    dap: SwdDapAccess<S>,
    connected: bool,
}

impl<S: SwdIo<Error = SwdError>> Engine<S> {
    /// Create a new engine wrapping the given SWD transport.
    pub fn new(swd: S) -> Self {
        Self {
            dap: SwdDapAccess::new(swd),
            connected: false,
        }
    }

    /// Check if a connection has been established.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Execute a single operation.
    ///
    /// `response_buf` is used for `ReadMem` results. Pass a sufficiently
    /// large buffer (e.g., 4096 bytes) for read operations.
    pub fn execute<'buf>(
        &mut self,
        op: &Operation<'_>,
        response_buf: &'buf mut [u8],
    ) -> Response<'buf> {
        match op {
            Operation::Connect { clock_hz } => {
                if let Err(_e) = self.dap.swd().set_clock(*clock_hz) {
                    core::hint::cold_path();
                    return Response::Error(CmdError::Adi(probe_rs_adi::AdiError::Swd(
                        SwdError::Io,
                    )));
                }
                match swd_init(&mut self.dap) {
                    Ok(_dpidr) => {
                        self.connected = true;
                        Response::Ok
                    }
                    Err(e) => {
                        core::hint::cold_path();
                        Response::Error(CmdError::Adi(e))
                    }
                }
            }

            Operation::Disconnect => {
                self.connected = false;
                Response::Ok
            }

            Operation::ReadMem { addr, len } => {
                if !self.connected {
                    return Response::Error(CmdError::NotConnected);
                }
                let len = (*len as usize).min(response_buf.len());
                if len == 0 {
                    return Response::Data(&[]);
                }
                let mut mem = MemoryAccessor::new(&mut self.dap, ApAddress::default());
                match mem.read_8(*addr, &mut response_buf[..len]) {
                    Ok(()) => Response::Data(&response_buf[..len]),
                    Err(e) => {
                        core::hint::cold_path();
                        Response::Error(CmdError::Adi(e))
                    }
                }
            }

            Operation::WriteMem { addr, data } => {
                if !self.connected {
                    return Response::Error(CmdError::NotConnected);
                }
                let mut mem = MemoryAccessor::new(&mut self.dap, ApAddress::default());
                match mem.write_8(*addr, data) {
                    Ok(()) => Response::Ok,
                    Err(e) => {
                        core::hint::cold_path();
                        Response::Error(CmdError::Adi(e))
                    }
                }
            }

            Operation::Halt => {
                if !self.connected {
                    return Response::Error(CmdError::NotConnected);
                }
                let mut core =
                    probe_rs_adi::CortexMControl::new(&mut self.dap, ApAddress::default());
                match core.halt() {
                    Ok(()) => Response::Ok,
                    Err(e) => {
                        core::hint::cold_path();
                        Response::Error(CmdError::Adi(e))
                    }
                }
            }

            Operation::Run => {
                if !self.connected {
                    return Response::Error(CmdError::NotConnected);
                }
                let mut core =
                    probe_rs_adi::CortexMControl::new(&mut self.dap, ApAddress::default());
                match core.run() {
                    Ok(()) => Response::Ok,
                    Err(e) => {
                        core::hint::cold_path();
                        Response::Error(CmdError::Adi(e))
                    }
                }
            }

            Operation::Reset => {
                if !self.connected {
                    return Response::Error(CmdError::NotConnected);
                }
                let mut core =
                    probe_rs_adi::CortexMControl::new(&mut self.dap, ApAddress::default());
                match core.system_reset() {
                    Ok(()) => Response::Ok,
                    Err(e) => {
                        core::hint::cold_path();
                        Response::Error(CmdError::Adi(e))
                    }
                }
            }

            Operation::ResetAndHalt => {
                if !self.connected {
                    return Response::Error(CmdError::NotConnected);
                }
                let mut core =
                    probe_rs_adi::CortexMControl::new(&mut self.dap, ApAddress::default());
                match core.reset_and_halt() {
                    Ok(()) => Response::Ok,
                    Err(e) => {
                        core::hint::cold_path();
                        Response::Error(CmdError::Adi(e))
                    }
                }
            }

            // Flash operations require a FlashAlgoDef which must be provided
            // externally (from probe-rs-target-nostd). For now, return Unsupported
            // since the Engine doesn't hold target/algorithm state yet.
            // These will be wired up when the Engine gains target configuration.
            Operation::FlashWrite { .. }
            | Operation::EraseRegion { .. }
            | Operation::Verify { .. }
            | Operation::ChipErase => Response::Error(CmdError::Unsupported),

            Operation::DelayMs(_ms) => {
                // Delay must be handled by the caller's platform-specific delay.
                // The Engine cannot sleep in no_std without a delay provider.
                // Return Ok to indicate the operation was acknowledged.
                Response::Ok
            }
        }
    }

    /// Execute a batch of operations sequentially.
    ///
    /// Stops at the first error and returns it. On success, returns Ok.
    pub fn execute_batch(
        &mut self,
        ops: &[Operation<'_>],
        response_buf: &mut [u8],
    ) -> Result<(), CmdError> {
        for op in ops {
            if let Response::Error(e) = self.execute(op, response_buf) {
                core::hint::cold_path();
                return Err(e);
            }
        }
        Ok(())
    }

    /// Get mutable access to the underlying DapAccess.
    ///
    /// Useful for direct low-level operations not covered by Operation.
    pub fn dap(&mut self) -> &mut SwdDapAccess<S> {
        &mut self.dap
    }

    /// Consume the engine and return the underlying SWD transport.
    pub fn into_inner(self) -> S {
        self.dap.into_inner()
    }
}
