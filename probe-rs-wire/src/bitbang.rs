//! GPIO bitbang SWD implementation.
//!
//! Provides `BitbangSwd`, which implements `SwdIo` using `embedded-hal` digital
//! and delay traits. Adapted from the rusty-probe firmware's SWD implementation
//! (`rusty-probe-firmware/src/dap.rs`).

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;

use crate::pin::BidirectionalPin;
use crate::swd::{self, JTAG_TO_SWD_SEQUENCE, LINE_RESET_CLOCKS, SwdError, SwdIo};
use crate::util::parity32;

/// Configuration for SWD transfer behavior.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SwdConfig {
    /// Number of idle (SWDIO low) clock cycles after each transfer.
    pub idle_cycles_after_transfer: u8,
    /// Maximum number of retries when target responds with WAIT.
    pub max_retries_on_wait: u16,
    /// Number of turnaround clock cycles (typically 1).
    pub turnaround_cycles: u8,
}

impl SwdConfig {
    /// Create a default configuration (const-friendly).
    pub const fn new() -> Self {
        Self {
            idle_cycles_after_transfer: 8,
            max_retries_on_wait: 1000,
            turnaround_cycles: 1,
        }
    }
}

impl Default for SwdConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// GPIO bitbang SWD implementation.
///
/// Generic over:
/// - `IO`: bidirectional SWDIO pin
/// - `CLK`: SWCLK output pin
/// - `DELAY`: nanosecond delay provider
pub struct BitbangSwd<IO, CLK, DELAY> {
    io: IO,
    clk: CLK,
    delay: DELAY,
    half_period_ns: u32,
    config: SwdConfig,
}

impl<IO, CLK, DELAY> BitbangSwd<IO, CLK, DELAY>
where
    IO: BidirectionalPin,
    CLK: OutputPin,
    DELAY: DelayNs,
{
    /// Create a new bitbang SWD interface.
    ///
    /// - `io`: SWDIO bidirectional pin
    /// - `clk`: SWCLK output pin
    /// - `delay`: delay provider
    /// - `frequency_hz`: initial SWD clock frequency in Hz
    pub fn new(io: IO, clk: CLK, delay: DELAY, frequency_hz: u32) -> Self {
        let half_period_ns = (500_000_000u32).checked_div(frequency_hz).unwrap_or(5000); // default 100kHz if frequency_hz == 0

        Self {
            io,
            clk,
            delay,
            half_period_ns,
            config: SwdConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        io: IO,
        clk: CLK,
        delay: DELAY,
        frequency_hz: u32,
        config: SwdConfig,
    ) -> Self {
        let mut swd = Self::new(io, clk, delay, frequency_hz);
        swd.config = config;
        swd
    }

    /// Get a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut SwdConfig {
        &mut self.config
    }

    // -- Low-level bit operations --
    // Adapted from rusty-probe dap.rs:460-487

    /// Write one bit on SWDIO (SWDIO must be in output mode).
    #[inline]
    fn write_bit(&mut self, bit: bool) -> Result<(), SwdError> {
        self.io.set_value(bit).map_err(|_| SwdError::Io)?;
        self.clk.set_low().map_err(|_| SwdError::Io)?;
        self.delay.delay_ns(self.half_period_ns);
        self.clk.set_high().map_err(|_| SwdError::Io)?;
        self.delay.delay_ns(self.half_period_ns);
        Ok(())
    }

    /// Read one bit from SWDIO (SWDIO must be in input mode).
    #[inline]
    fn read_bit(&mut self) -> Result<bool, SwdError> {
        self.clk.set_low().map_err(|_| SwdError::Io)?;
        self.delay.delay_ns(self.half_period_ns);
        let bit = self.io.is_high().map_err(|_| SwdError::Io)?;
        self.clk.set_high().map_err(|_| SwdError::Io)?;
        self.delay.delay_ns(self.half_period_ns);
        Ok(bit)
    }

    /// Clock one cycle without caring about data (turnaround).
    #[inline]
    fn clock_cycle(&mut self) -> Result<(), SwdError> {
        self.clk.set_low().map_err(|_| SwdError::Io)?;
        self.delay.delay_ns(self.half_period_ns);
        self.clk.set_high().map_err(|_| SwdError::Io)?;
        self.delay.delay_ns(self.half_period_ns);
        Ok(())
    }

    /// Send `count` bits from `data` (LSB first), SWDIO must be output.
    fn send_bits(&mut self, mut data: u32, count: u8) -> Result<(), SwdError> {
        for _ in 0..count {
            self.write_bit(data & 1 != 0)?;
            data >>= 1;
        }
        Ok(())
    }

    /// Read `count` bits into a u32 (LSB first), SWDIO must be input.
    fn read_bits(&mut self, count: u8) -> Result<u32, SwdError> {
        let mut data = 0u32;
        for i in 0..count {
            if self.read_bit()? {
                data |= 1 << i;
            }
        }
        Ok(data)
    }

    /// Send idle cycles (SWDIO low).
    fn idle_cycles(&mut self, count: u8) -> Result<(), SwdError> {
        self.io.set_as_output().map_err(|_| SwdError::Io)?;
        self.io.set_low().map_err(|_| SwdError::Io)?;
        for _ in 0..count {
            self.clock_cycle()?;
        }
        Ok(())
    }

    // -- SWD transaction --
    // Adapted from rusty-probe dap.rs read_inner/write_inner

    /// Perform a single SWD transfer (read or write).
    ///
    /// Returns the read data on success (0 for writes).
    fn swd_transfer_inner(
        &mut self,
        apndp: bool,
        rnw: bool,
        addr: u8,
        wdata: u32,
    ) -> Result<u32, SwdError> {
        let request = swd::make_request(apndp, rnw, addr);

        // -- Request phase: send 8-bit request (host -> target) --
        self.io.set_as_output().map_err(|_| SwdError::Io)?;
        self.send_bits(request as u32, 8)?;

        // -- Turnaround: release SWDIO, clock turnaround cycles --
        self.io.set_as_input().map_err(|_| SwdError::Io)?;
        for _ in 0..self.config.turnaround_cycles {
            self.clock_cycle()?;
        }

        // -- ACK phase: read 3 bits --
        let ack = self.read_bits(3)? as u8;

        if let Err(e) = swd::check_ack(ack) {
            core::hint::cold_path();
            // On error: take back bus and send idle cycles
            self.io.set_as_output().map_err(|_| SwdError::Io)?;
            self.idle_cycles(self.config.idle_cycles_after_transfer)?;
            return Err(e);
        }

        if rnw {
            // -- READ: data phase (target -> host) --
            let data = self.read_bits(32)?;
            let parity_bit = self.read_bit()?;

            // Turnaround back to output
            self.io.set_as_output().map_err(|_| SwdError::Io)?;
            self.idle_cycles(self.config.idle_cycles_after_transfer)?;

            // Verify parity
            if parity32(data) != parity_bit {
                core::hint::cold_path();
                return Err(SwdError::BadParity);
            }

            Ok(data)
        } else {
            // -- WRITE: turnaround then data phase (host -> target) --
            // One more turnaround cycle for write (target releases SWDIO)
            for _ in 0..self.config.turnaround_cycles {
                self.clock_cycle()?;
            }

            self.io.set_as_output().map_err(|_| SwdError::Io)?;

            // Send 32-bit data (LSB first)
            self.send_bits(wdata, 32)?;

            // Send parity bit
            self.write_bit(parity32(wdata))?;

            // Idle cycles
            self.idle_cycles(self.config.idle_cycles_after_transfer)?;

            Ok(0)
        }
    }
}

impl<IO, CLK, DELAY> SwdIo for BitbangSwd<IO, CLK, DELAY>
where
    IO: BidirectionalPin,
    CLK: OutputPin,
    DELAY: DelayNs,
{
    type Error = SwdError;

    fn swd_read(&mut self, apndp: bool, addr: u8) -> Result<u32, SwdError> {
        for _ in 0..self.config.max_retries_on_wait {
            match self.swd_transfer_inner(apndp, true, addr, 0) {
                Err(SwdError::AckWait) => continue,
                result => return result,
            }
        }
        Err(SwdError::AckWait)
    }

    fn swd_write(&mut self, apndp: bool, addr: u8, data: u32) -> Result<(), SwdError> {
        for _ in 0..self.config.max_retries_on_wait {
            match self.swd_transfer_inner(apndp, false, addr, data) {
                Err(SwdError::AckWait) => continue,
                Ok(_) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
        Err(SwdError::AckWait)
    }

    fn line_reset(&mut self) -> Result<(), SwdError> {
        self.io.set_as_output().map_err(|_| SwdError::Io)?;

        // Phase 1: LINE_RESET_CLOCKS with SWDIO high (spec requires >= 50)
        self.io.set_high().map_err(|_| SwdError::Io)?;
        for _ in 0..LINE_RESET_CLOCKS {
            self.clock_cycle()?;
        }

        // Phase 2: JTAG-to-SWD switching sequence (16-bit, LSB first)
        self.send_bits(JTAG_TO_SWD_SEQUENCE as u32, 16)?;

        // Phase 3: Another LINE_RESET_CLOCKS with SWDIO high
        self.io.set_high().map_err(|_| SwdError::Io)?;
        for _ in 0..LINE_RESET_CLOCKS {
            self.clock_cycle()?;
        }

        // Phase 4: At least 2 idle cycles (SWDIO low)
        self.idle_cycles(4)?;

        Ok(())
    }

    fn set_clock(&mut self, max_frequency_hz: u32) -> Result<(), SwdError> {
        if max_frequency_hz == 0 {
            return Err(SwdError::Io);
        }
        self.half_period_ns = 500_000_000 / max_frequency_hz;
        Ok(())
    }
}
