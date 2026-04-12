//! Bit manipulation and parity utilities.

/// Convert up to 32 bits (LSB-first iterator) into a u32.
///
/// Ported from probe-rs/src/probe/common.rs:15-25.
pub fn bits_to_byte(bits: impl IntoIterator<Item = bool>) -> u32 {
    let mut byte = 0u32;
    for (i, bit) in bits.into_iter().take(32).enumerate() {
        if bit {
            byte |= 1 << i;
        }
    }
    byte
}

/// Compute even parity of a u32 value.
///
/// Returns `true` if the value has an odd number of 1-bits.
pub const fn parity32(value: u32) -> bool {
    value.count_ones() & 1 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bits_to_byte_empty() {
        assert_eq!(bits_to_byte(core::iter::empty()), 0);
    }

    #[test]
    fn test_bits_to_byte_lsb_first() {
        // 0b1010 = bits [false, true, false, true] in LSB-first order
        let bits = [false, true, false, true];
        assert_eq!(bits_to_byte(bits), 0b1010);
    }

    #[test]
    fn test_bits_to_byte_all_ones() {
        let bits = [true; 8];
        assert_eq!(bits_to_byte(bits), 0xFF);
    }

    #[test]
    fn test_parity32_zero() {
        assert!(!parity32(0));
    }

    #[test]
    fn test_parity32_one() {
        assert!(parity32(1));
    }

    #[test]
    fn test_parity32_two() {
        assert!(!parity32(0b11));
    }

    #[test]
    fn test_parity32_three_bits() {
        assert!(parity32(0b111));
    }

    #[test]
    fn test_parity32_all_ones() {
        assert!(!parity32(0xFFFF_FFFF));
    }
}
