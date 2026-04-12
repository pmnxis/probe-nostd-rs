//! Bidirectional pin abstraction for SWDIO.
//!
//! `embedded-hal` 1.0 does not provide a bidirectional pin trait, but SWD requires
//! switching SWDIO between input and output during each transaction. This module
//! provides `BidirectionalPin` and two adapter implementations for common hardware
//! configurations.

use embedded_hal::digital::{InputPin, OutputPin};

/// Trait for a pin that can switch between input and output at runtime.
///
/// SWD requires the SWDIO line to alternate between driving data (output)
/// and sampling ACK/data (input) within a single transaction. Platform HALs
/// should implement this trait for their GPIO types.
pub trait BidirectionalPin {
    /// Error type for pin operations.
    type Error: core::fmt::Debug;

    /// Switch the pin to input (high-impedance) mode.
    fn set_as_input(&mut self) -> Result<(), Self::Error>;

    /// Switch the pin to push-pull output mode.
    fn set_as_output(&mut self) -> Result<(), Self::Error>;

    /// Drive the pin high (must be in output mode).
    fn set_high(&mut self) -> Result<(), Self::Error>;

    /// Drive the pin low (must be in output mode).
    fn set_low(&mut self) -> Result<(), Self::Error>;

    /// Read the electrical state of the pin (works in both input and output mode).
    fn is_high(&mut self) -> Result<bool, Self::Error>;

    /// Drive the pin to the given value.
    fn set_value(&mut self, high: bool) -> Result<(), Self::Error> {
        if high {
            self.set_high()
        } else {
            self.set_low()
        }
    }
}

/// Adapter for boards with a separate direction-control pin (e.g. level shifter).
///
/// Used by designs like rusty-probe where SWDIO goes through a voltage translator
/// with an explicit direction pin (high = output, low = input).
pub struct SeparatedSwdio<DATA, DIR> {
    data: DATA,
    direction: DIR,
}

impl<DATA, DIR> SeparatedSwdio<DATA, DIR> {
    /// Create a new separated SWDIO adapter.
    ///
    /// - `data`: the data pin (always configured as output; direction pin controls the buffer)
    /// - `direction`: the direction control pin (high = drive bus, low = release bus)
    pub const fn new(data: DATA, direction: DIR) -> Self {
        Self { data, direction }
    }
}

impl<DATA, DIR, E> BidirectionalPin for SeparatedSwdio<DATA, DIR>
where
    DATA: OutputPin<Error = E> + InputPin<Error = E>,
    DIR: OutputPin<Error = E>,
    E: core::fmt::Debug,
{
    type Error = E;

    fn set_as_input(&mut self) -> Result<(), E> {
        self.direction.set_low()
    }

    fn set_as_output(&mut self) -> Result<(), E> {
        self.direction.set_high()
    }

    fn set_high(&mut self) -> Result<(), E> {
        self.data.set_high()
    }

    fn set_low(&mut self) -> Result<(), E> {
        self.data.set_low()
    }

    fn is_high(&mut self) -> Result<bool, E> {
        self.data.is_high()
    }
}

/// Adapter for open-drain SWDIO configurations.
///
/// In open-drain mode no explicit direction switching is needed:
/// - Output low = drive the line low
/// - Output high = release the line (external pull-up pulls it high)
/// - Input = read the line state
///
/// This works when SWDIO has an external pull-up resistor.
pub struct OpenDrainSwdio<P> {
    pin: P,
}

impl<P> OpenDrainSwdio<P> {
    /// Create a new open-drain SWDIO adapter.
    pub const fn new(pin: P) -> Self {
        Self { pin }
    }
}

impl<P, E> BidirectionalPin for OpenDrainSwdio<P>
where
    P: OutputPin<Error = E> + InputPin<Error = E>,
    E: core::fmt::Debug,
{
    type Error = E;

    fn set_as_input(&mut self) -> Result<(), E> {
        // Release the line by driving high (open-drain = high-Z with pull-up)
        self.pin.set_high()
    }

    fn set_as_output(&mut self) -> Result<(), E> {
        // No mode change needed for open-drain
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), E> {
        self.pin.set_high()
    }

    fn set_low(&mut self) -> Result<(), E> {
        self.pin.set_low()
    }

    fn is_high(&mut self) -> Result<bool, E> {
        self.pin.is_high()
    }
}
