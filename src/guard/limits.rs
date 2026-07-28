pub const MAX_INPUT: usize = 16 * 1024 * 1024; // 16MB
pub const WARN_INPUT: usize = 1024 * 1024; // 1MB

/// Hard ceiling on what either hook path hands back once distillation is done.
///
/// It lives here rather than in one hook because both paths cut at it and
/// `hooks::post_tool` used to spell it `50_000` inline, so raising the cap
/// raised half the product (#219). Cut with `util::text::truncate_with_marker`,
/// never a bare `safe_truncate`: this is the last thing that happens to the
/// payload, so it has to say what it removed.
pub const MAX_OUTPUT_BYTES: usize = 50_000;

/// The size at which Claude Code stops passing a Bash result to the hook whole.
///
/// The host truncates the payload at this many bytes, so a distillation measured
/// against it is measured against a number the host already chose, not against
/// what the command produced. Above roughly the same threshold the host also
/// writes the **raw** output to a file, previews the **raw** first 2 KB, and
/// discards whatever the hook returns — so on that path OMNI's work is thrown
/// away and the saving it booked never happened. 43 rows in the maintainer's DB
/// sit at exactly this size, from 2026-07-08 onward, one of them booked as
/// 93% compression and 6,194 tokens for a distillation the model never saw
/// (#212).
///
/// Detection is `>=` rather than `==` because `hooks::normalize` folds a
/// non-empty stderr into the content, which can carry a capped payload past the
/// cap by a few bytes. Nothing legitimate arrives above it on this path.
pub const HOST_OUTPUT_CAP: usize = 30_000;

/// Output must be under this percentage of the input to count as a real
/// reduction. Anything above it is not compression worth taking — e.g. a TOML
/// filter that strips a few lines does not get to short-circuit a distiller that
/// would summarise the same input.
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
