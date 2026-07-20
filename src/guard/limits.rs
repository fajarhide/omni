pub const MAX_INPUT: usize = 16 * 1024 * 1024; // 16MB
pub const WARN_INPUT: usize = 1024 * 1024; // 1MB

/// Output must be under this percentage of the input to count as a real
/// reduction. Anything above it is not compression worth taking — e.g. a TOML
/// filter that strips a few lines does not get to short-circuit a distiller
/// that would summarise the same input.
pub const MIN_REDUCTION_PCT: usize = 95;

/// True when `output` compressed `input` enough to be worth keeping.
pub fn beats_guardrail(output_len: usize, input_len: usize) -> bool {
    output_len < input_len * MIN_REDUCTION_PCT / 100
}

pub enum InputCheck {
    Ok,
    Warn,
    TooLarge,
    Empty,
}

pub fn check_input(input: &str) -> InputCheck {
    let len = input.len();
    if len == 0 {
        InputCheck::Empty
    } else if len > MAX_INPUT {
        InputCheck::TooLarge
    } else if len > WARN_INPUT {
        InputCheck::Warn
    } else {
        InputCheck::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_input() {
        assert!(matches!(check_input("normal text"), InputCheck::Ok));
        assert!(matches!(
            check_input(&"a".repeat(1024 * 1024)),
            InputCheck::Ok
        )); // 1MB is Ok, just a warning in logs typically
    }

    #[test]
    fn warns_for_input_greater_than_1mb() {
        assert!(matches!(
            check_input(&"a".repeat(WARN_INPUT + 1)),
            InputCheck::Warn
        ));
        assert!(matches!(
            check_input(&"a".repeat(MAX_INPUT)),
            InputCheck::Warn
        ));
    }

    #[test]
    fn rejects_input_greater_than_16mb() {
        let big = "a".repeat(MAX_INPUT + 1);
        assert!(matches!(check_input(&big), InputCheck::TooLarge));
    }
}
