//! The gain gate: is this reduction worth taking at all.
//!
//! Section 5.4 of the direction spec asks for one place that answers "skip below
//! an input floor, discard any result not smaller than its input", wrapping every
//! projection rather than being re-derived inside each one. Before this, three
//! call sites each made half the decision with their own constants.
//!
//! It is deliberately not a new policy. `beats_guardrail` still owns "is this
//! reduction large enough to be worth the marker", which is a different question
//! with a measured threshold behind it (#268, #269). This owns the two answers
//! that are true regardless of how good a distiller is:
//!
//! - an input too small to be worth touching
//! - a result that is not actually smaller
//!
//! The second is what makes P2 safe to build against a measurement that said it
//! would gain almost nothing: the worst case is no change, because a producer
//! that resolves differently and does not beat its input is discarded.

/// Below this, a reduction cannot pay for the machinery that produced it.
///
/// Measured on the corpus behind #395: 30% of recorded invocations are under 200
/// bytes, and a marker is roughly 90. Set at the point where a marker is under
/// half the payload.
pub const MIN_GAIN_INPUT: usize = 200;

/// The reduced form, or `None` when the caller should keep what it had.
///
/// Takes a closure so the work is not done at all below the floor, which is the
/// half of this that saves latency rather than bytes.
pub fn gain<F>(input: &str, produce: F) -> Option<String>
where
    F: FnOnce(&str) -> Option<String>,
{
    if input.len() < MIN_GAIN_INPUT {
        return None;
    }
    produce(input).filter(|out| out.len() < input.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declines_before_running_anything_below_the_floor() {
        let mut ran = false;
        let out = gain("short", |s| {
            ran = true;
            Some(s.to_string())
        });

        assert_eq!(out, None);
        assert!(
            !ran,
            "the floor exists to skip the work, not just the result"
        );
    }

    /// The property that makes a routing change safe to ship against a
    /// measurement predicting little gain: it cannot lose.
    #[test]
    fn discards_a_result_that_is_not_smaller() {
        let input = "x".repeat(MIN_GAIN_INPUT + 10);

        assert_eq!(gain(&input, |s| Some(format!("{s}!"))), None);
        assert_eq!(gain(&input, |s| Some(s.to_string())), None);
    }

    #[test]
    fn keeps_a_result_that_is_smaller() {
        let input = "x".repeat(MIN_GAIN_INPUT + 10);

        assert_eq!(gain(&input, |_| Some("tiny".into())), Some("tiny".into()));
    }

    /// A producer that declines is not the same as one that failed to gain, and
    /// both come back as `None` on purpose: the caller keeps its own bytes either
    /// way, which is the fail-open rule the hooks already follow.
    #[test]
    fn passes_a_declining_producer_through_as_no_gain() {
        let input = "x".repeat(MIN_GAIN_INPUT + 10);

        assert_eq!(gain(&input, |_| None), None);
    }
}
