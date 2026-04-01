pub const MAX_INPUT: usize = 16 * 1024 * 1024; // 16MB
pub const WARN_INPUT: usize = 1024 * 1024; // 1MB

pub enum InputCheck {
    Ok,
    TooLarge,
    Empty,
}

pub fn check_input(input: &str) -> InputCheck {
    let len = input.len();
    if len == 0 {
        InputCheck::Empty
    } else if len > WARN_INPUT {
        InputCheck::TooLarge
    } else {
        InputCheck::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_input_ok_for_normal_input() {
        assert!(matches!(check_input("normal text"), InputCheck::Ok));
        assert!(matches!(
            check_input(&"a".repeat(1024 * 1024)),
            InputCheck::Ok
        )); // 1MB is Ok, just a warning in logs typically
    }

    #[test]
    fn test_check_input_toolarge_for_gt_16mb() {
        let big = "a".repeat(MAX_INPUT + 1);
        assert!(matches!(check_input(&big), InputCheck::TooLarge));
    }
}
