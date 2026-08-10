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

/// Largest input either hook archives in the RewindStore when it drops bytes.
///
/// `README.md:81` promises that "everything cut is archived", and it had never
/// once been true: the old gate asked the scorer for more than 40% noise
/// segments across more than 20 segments, and 0 of 8,968 distillations in the
/// maintainer's database had ever recorded a rewind hash, leaving `rewind_store`
/// empty (#271). Archiving on
/// every lossy distillation is what makes that sentence true. A cap is what keeps
/// it affordable: the same 30 days of history is 13.3 MB of raw content at 64 KB
/// and 83.1 MB uncapped, because 53 outliers up to 5 MB carry six times the bytes
/// of the 3,604 rows below the cap.
///
/// Above it the output says the content was not archived, so the bound reaches
/// the agent instead of being implied.
pub const MAX_REWIND_BYTES: usize = 64 * 1024;

/// Below this an output is not worth taking to the ledger.
///
/// The ledger's cost is a membership query and a batched insert per call, and
/// its gain is bounded by what a handle can replace. The marker is about 88
/// bytes, so on a small reply it is a measurable share of the payload.
pub const MIN_LEDGER_INPUT: usize = 400;

/// A run of already-shown lines shorter than this stays verbatim.
///
/// Both bounds have to hold, because either one alone admits a bad trade: four
/// one-word lines are shorter than the marker that would replace them, and one
/// 400 byte line replaced by a handle costs the agent a round trip to read
/// something it could have read in place.
///
/// **All three constants were chosen by replay, not by taste.** Aggregate net
/// savings over 7,019 model-facing traces, filters alone at 5.2%:
///
/// | input floor / run lines / run bytes | aggregate | file read | calls projected |
/// |---|---|---|---|
/// | 2000 / 8 / 240 | 12.4% | 20.2% | 159 |
/// | **400 / 4 / 200** | **15.7%** | **24.8%** | **512** |
/// | 200 / 2 / 150 | 16.6% | 25.2% | 840 |
///
/// The middle row is the knee. The first loosening bought 3.3 points for 353
/// more markers; the second bought 0.9 for 328 more, and those markers are two
/// line substitutions that make the output choppier and invite an expansion
/// request for barely more than the marker costs. The fidelity alarm exists, but
/// spending it on 0.9 points is the wrong trade.
pub const MIN_LEDGER_RUN_LINES: usize = 4;
pub const MIN_LEDGER_RUN_BYTES: usize = 200;

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
