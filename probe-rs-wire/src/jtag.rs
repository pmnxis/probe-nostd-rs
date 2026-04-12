//! JTAG protocol definitions and state machine.
//!
//! Provides the JTAG TAP state machine (ported from probe-rs/src/probe/common.rs:283-417)
//! and the host-side `JtagIo` trait.
//!
//! This module is only available when the `jtag` feature is enabled.

/// JTAG protocol errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum JtagError {
    /// No device found on the scan chain.
    NoDevice,
    /// IR length detection failed.
    IrLengthDetection,
    /// Hardware I/O error.
    Io,
}

/// Register path state within a DR or IR scan.
///
/// Ported from probe-rs/src/probe/common.rs:283-337.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RegisterState {
    Select,
    Capture,
    Shift,
    Exit1,
    Pause,
    Exit2,
    Update,
}

impl RegisterState {
    /// Returns the TMS value that steps toward the target state.
    ///
    /// # Safety contract
    /// Must not be called with `self == Select` aiming for `Update`, or
    /// `self == Update` -- those cases are handled by `JtagState`.
    fn step_toward(self, target: Self) -> bool {
        match self {
            Self::Select => false,
            Self::Capture if matches!(target, Self::Shift) => false,
            Self::Exit1 if matches!(target, Self::Pause | Self::Exit2) => false,
            Self::Exit2 if matches!(target, Self::Shift | Self::Exit1 | Self::Pause) => false,
            // Update case is handled by JtagState; should not reach here.
            Self::Update => {
                debug_assert!(false, "RegisterState::step_toward called in Update state");
                true
            }
            _ => true,
        }
    }

    /// Advance the register state based on a TMS bit.
    fn update(self, tms: bool) -> Self {
        if tms {
            match self {
                Self::Capture | Self::Shift => Self::Exit1,
                Self::Exit1 | Self::Exit2 => Self::Update,
                Self::Pause => Self::Exit2,
                // Select and Update are handled by JtagState.
                Self::Select | Self::Update => {
                    debug_assert!(false, "RegisterState::update called in Select/Update state");
                    Self::Update
                }
            }
        } else {
            match self {
                Self::Select => Self::Capture,
                Self::Capture | Self::Shift => Self::Shift,
                Self::Exit1 | Self::Pause => Self::Pause,
                Self::Exit2 => Self::Shift,
                Self::Update => {
                    debug_assert!(false, "RegisterState::update called in Update state");
                    Self::Capture
                }
            }
        }
    }
}

/// JTAG TAP state machine.
///
/// Ported from probe-rs/src/probe/common.rs:340-417.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum JtagState {
    /// Test-Logic-Reset.
    Reset,
    /// Run-Test/Idle.
    Idle,
    /// State along the Data Register path.
    Dr(RegisterState),
    /// State along the Instruction Register path.
    Ir(RegisterState),
}

impl JtagState {
    /// Returns the TMS value that takes one step from the current state toward `target`.
    ///
    /// Returns `None` if already at the target state.
    pub fn step_toward(self, target: Self) -> Option<bool> {
        let tms = match self {
            state if target == state => return None,
            Self::Reset => false,
            Self::Idle => true,
            Self::Dr(RegisterState::Select) => !matches!(target, Self::Dr(_)),
            Self::Ir(RegisterState::Select) => !matches!(target, Self::Ir(_)),
            Self::Dr(RegisterState::Update) | Self::Ir(RegisterState::Update) => {
                matches!(target, Self::Ir(_) | Self::Dr(_))
            }
            Self::Dr(state) => {
                let next = if let Self::Dr(target) = target {
                    target
                } else {
                    RegisterState::Update
                };
                state.step_toward(next)
            }
            Self::Ir(state) => {
                let next = if let Self::Ir(target) = target {
                    target
                } else {
                    RegisterState::Update
                };
                state.step_toward(next)
            }
        };
        Some(tms)
    }

    /// Advance the state machine by one TMS bit.
    pub fn update(&mut self, tms: bool) {
        *self = match *self {
            Self::Reset if tms => Self::Reset,
            Self::Reset => Self::Idle,
            Self::Idle if tms => Self::Dr(RegisterState::Select),
            Self::Idle => Self::Idle,
            Self::Dr(RegisterState::Select) if tms => Self::Ir(RegisterState::Select),
            Self::Ir(RegisterState::Select) if tms => Self::Reset,
            Self::Dr(RegisterState::Update) | Self::Ir(RegisterState::Update) => {
                if tms {
                    Self::Dr(RegisterState::Select)
                } else {
                    Self::Idle
                }
            }
            Self::Dr(state) => Self::Dr(state.update(tms)),
            Self::Ir(state) => Self::Ir(state.update(tms)),
        };
    }

    /// Compute the full TMS sequence to move from the current state to `target`.
    ///
    /// Returns `(tms_bits, bit_count)` where `tms_bits` contains the TMS values
    /// packed LSB-first and `bit_count` is the number of valid bits.
    /// Maximum sequence length is 8 bits.
    pub fn path_to(mut self, target: Self) -> (u8, u8) {
        let mut tms_bits: u8 = 0;
        let mut count: u8 = 0;

        while let Some(tms) = self.step_toward(target) {
            if tms {
                tms_bits |= 1 << count;
            }
            self.update(tms);
            count += 1;

            // Safety: JTAG state machine paths are at most 7 steps
            if count >= 8 {
                break;
            }
        }

        (tms_bits, count)
    }
}

/// Host-side JTAG wire protocol interface.
pub trait JtagIo {
    /// Error type for JTAG operations.
    type Error: core::fmt::Debug;

    /// Shift TMS bits (used for state machine transitions).
    ///
    /// `tms_bits` is packed LSB-first, `bit_count` bits are shifted.
    fn shift_tms(&mut self, tms_bits: &[u8], bit_count: usize) -> Result<(), Self::Error>;

    /// Shift data through TDI and capture TDO.
    ///
    /// - `tdi`: data to shift in (LSB first, packed bytes)
    /// - `tdo`: buffer to receive captured data (LSB first, packed bytes)
    /// - `bit_count`: number of bits to shift
    /// - `tms_on_last`: if true, TMS=1 on the last bit (transitions to Exit1)
    fn shift_tdi_tdo(
        &mut self,
        tdi: &[u8],
        tdo: &mut [u8],
        bit_count: usize,
        tms_on_last: bool,
    ) -> Result<(), Self::Error>;

    /// Reset the JTAG state machine (5+ clocks with TMS=1).
    fn reset(&mut self) -> Result<(), Self::Error>;

    /// Set the maximum JTAG clock frequency in Hz.
    fn set_clock(&mut self, max_frequency_hz: u32) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reset_to_idle() {
        let mut state = JtagState::Reset;
        let tms = state.step_toward(JtagState::Idle);
        assert_eq!(tms, Some(false));
        state.update(false);
        assert_eq!(state, JtagState::Idle);
    }

    #[test]
    fn test_idle_to_dr_shift() {
        let mut state = JtagState::Idle;

        // Idle -> DR Select (TMS=1)
        let tms = state.step_toward(JtagState::Dr(RegisterState::Shift));
        assert_eq!(tms, Some(true));
        state.update(true);
        assert_eq!(state, JtagState::Dr(RegisterState::Select));

        // DR Select -> DR Capture (TMS=0)
        let tms = state.step_toward(JtagState::Dr(RegisterState::Shift));
        assert_eq!(tms, Some(false));
        state.update(false);
        assert_eq!(state, JtagState::Dr(RegisterState::Capture));

        // DR Capture -> DR Shift (TMS=0)
        let tms = state.step_toward(JtagState::Dr(RegisterState::Shift));
        assert_eq!(tms, Some(false));
        state.update(false);
        assert_eq!(state, JtagState::Dr(RegisterState::Shift));
    }

    #[test]
    fn test_already_at_target() {
        let state = JtagState::Idle;
        assert_eq!(state.step_toward(JtagState::Idle), None);
    }

    #[test]
    fn test_path_to_reset_to_idle() {
        let state = JtagState::Reset;
        let (tms, count) = state.path_to(JtagState::Idle);
        assert_eq!(count, 1);
        assert_eq!(tms & 1, 0); // TMS=0
    }

    #[test]
    fn test_path_to_idle_to_dr_shift() {
        let state = JtagState::Idle;
        let (tms, count) = state.path_to(JtagState::Dr(RegisterState::Shift));
        assert_eq!(count, 3);
        // TMS sequence: 1 (Idle->DrSelect), 0 (DrSelect->DrCapture), 0 (DrCapture->DrShift)
        assert_eq!(tms & 0b111, 0b001);
    }

    #[test]
    fn test_dr_shift_to_idle() {
        let state = JtagState::Dr(RegisterState::Shift);
        let (tms, count) = state.path_to(JtagState::Idle);
        // Shift -> Exit1(1) -> Update(1) -> Idle(0)
        assert_eq!(count, 3);
        assert_eq!(tms & 0b111, 0b011);
    }

    #[test]
    fn test_update_does_not_panic() {
        // Ensure all state transitions work without panicking
        let states = [
            JtagState::Reset,
            JtagState::Idle,
            JtagState::Dr(RegisterState::Select),
            JtagState::Dr(RegisterState::Capture),
            JtagState::Dr(RegisterState::Shift),
            JtagState::Dr(RegisterState::Exit1),
            JtagState::Dr(RegisterState::Pause),
            JtagState::Dr(RegisterState::Exit2),
            JtagState::Dr(RegisterState::Update),
            JtagState::Ir(RegisterState::Select),
            JtagState::Ir(RegisterState::Capture),
            JtagState::Ir(RegisterState::Shift),
            JtagState::Ir(RegisterState::Exit1),
            JtagState::Ir(RegisterState::Pause),
            JtagState::Ir(RegisterState::Exit2),
            JtagState::Ir(RegisterState::Update),
        ];

        for &start in &states {
            let mut s = start;
            s.update(false);

            let mut s = start;
            s.update(true);
        }
    }
}
