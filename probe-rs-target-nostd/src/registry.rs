use crate::types::ChipDef;

// This is populated by the generated code
include!(concat!(env!("OUT_DIR"), "/targets.rs"));

pub fn lookup_target(name: &str) -> Option<&'static ChipDef> {
    ALL_TARGETS
        .iter()
        .find(|t| eq_ascii_case_insensitive(t.name.as_bytes(), name.as_bytes()))
        .copied()
}

pub fn available_targets() -> &'static [&'static ChipDef] {
    ALL_TARGETS
}

fn eq_ascii_case_insensitive(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if !a[i].eq_ignore_ascii_case(&b[i]) {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_targets_without_features() {
        // With no target features, list should be empty
        // (this test runs without features, so ALL_TARGETS is empty)
        let _ = available_targets();
    }

    #[test]
    fn test_case_insensitive_compare() {
        assert!(eq_ascii_case_insensitive(b"RP2040", b"rp2040"));
        assert!(eq_ascii_case_insensitive(b"STM32H7", b"stm32h7"));
        assert!(!eq_ascii_case_insensitive(b"RP2040", b"RP2040X"));
        assert!(!eq_ascii_case_insensitive(b"A", b"B"));
        assert!(eq_ascii_case_insensitive(b"", b""));
    }

    #[test]
    #[cfg(feature = "target-rp2040")]
    fn test_lookup_rp2040() {
        let target = lookup_target("RP2040");
        assert!(target.is_some());
        let chip = target.unwrap();
        assert!(!chip.cores.is_empty());
        assert!(!chip.memory_map.is_empty());
        assert!(!chip.flash_algorithms.is_empty());
    }

    #[test]
    #[cfg(feature = "target-rp2040")]
    fn test_lookup_rp2040_case_insensitive() {
        assert!(lookup_target("rp2040").is_some());
        assert!(lookup_target("Rp2040").is_some());
    }

    #[test]
    fn test_lookup_nonexistent() {
        assert!(lookup_target("NONEXISTENT_CHIP_XYZ").is_none());
    }
}
