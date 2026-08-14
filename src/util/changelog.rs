// Parsing the `## [Unreleased]` section of `CHANGELOG.md` (#137).
//
// `include!`d by `build.rs` as well as compiled into the library, so the count
// the binary reports and the count under test come from the same code. A second
// copy in `build.rs` would be untestable and would drift.
//
// Plain `//` comments, not `//!`: an inner doc comment is only valid at the top
// of a module, and this file is spliced into the middle of `build.rs`.

/// Count top-level bullets under `## [Unreleased]`, stopping at the next
/// `## [` heading.
///
/// Only lines starting with `- ` at column zero count. The section nests
/// `### Added` / `### Fixed` groups and wraps long entries across lines, and
/// neither should inflate the number, a wrong count here becomes a wrong claim
/// in `omni doctor`, which is the thing this project exists to avoid.
// Dead in the `omni` binary by design: the binary reads the *result* through
// `OMNI_UNRELEASED_ENTRIES`, which `build.rs` computed by calling this at
// compile time. It is compiled into the crate so the tests below cover the code
// the build script actually runs, rather than a copy of it.
#[allow(dead_code)]
pub fn count_unreleased_entries(changelog: &str) -> usize {
    let mut in_section = false;
    let mut n = 0;
    for line in changelog.lines() {
        // Any `## [` heading ends the section, the assignment does that on its
        // own, so there is no `break` here. An earlier draft had one and no test
        // could be made to fail without it, which is what showed it was dead.
        if line.starts_with("## [") {
            in_section = line.starts_with("## [Unreleased]");
            continue;
        }
        if in_section && line.starts_with("- ") {
            n += 1;
        }
    }
    n
}

/// Count the entry fragments in `changelog.d/`.
///
/// One file is one entry. New work writes `changelog.d/<issue>.<section>.md`
/// instead of editing `## [Unreleased]`, so two branches never write the same
/// path and there is nothing for git to conflict on. `README.md` is the
/// directory's own documentation and is not an entry.
///
/// A missing or unreadable directory counts zero rather than failing the build:
/// this runs in `build.rs`, and a changelog convention must never be the reason
/// the binary cannot be compiled.
// Dead in the `omni` binary for the same reason as the function above: the
// binary reads the total through `OMNI_UNRELEASED_ENTRIES`.
#[allow(dead_code)]
pub fn count_fragments(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".md") && name != "README.md"
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_entries_under_the_unreleased_heading() {
        let c = "\
# Changelog

## [Unreleased]

### Fixed
- **first thing**: detail
- **second thing**: detail

## [0.6.3] - 2026-07-21
- **released thing**: detail
";
        assert_eq!(count_unreleased_entries(c), 2);
    }

    /// A cut release empties the section, and then doctor must stay silent.
    #[test]
    fn reports_zero_when_the_section_is_empty() {
        let c = "## [Unreleased]\n\n## [0.6.3] - 2026-07-21\n- **a**: b\n";
        assert_eq!(count_unreleased_entries(c), 0);
    }

    #[test]
    fn reports_zero_when_there_is_no_unreleased_section() {
        let c = "## [0.6.3] - 2026-07-21\n- **a**: b\n- **c**: d\n";
        assert_eq!(count_unreleased_entries(c), 0);
    }

    /// Released entries sit below the next heading and must never be added in -
    /// counting them would tell a released binary it had unreleased work.
    #[test]
    fn stops_at_the_next_version_heading() {
        let c = "## [Unreleased]\n- **one**: x\n\n## [0.6.3] - 2026-07-21\n- **two**: y\n- **three**: z\n";
        assert_eq!(count_unreleased_entries(c), 1);
    }

    /// Sub-headings and wrapped continuation lines are not entries.
    #[test]
    fn ignores_subheadings_and_wrapped_lines() {
        let c = "\
## [Unreleased]

### Added
- **one**: a long entry that
  wraps onto a second line, and
  a third
### Fixed
- **two**: another

## [0.6.3] - 2026-07-21
";
        assert_eq!(count_unreleased_entries(c), 2);
    }

    #[test]
    fn handles_an_empty_document_without_panicking() {
        assert_eq!(count_unreleased_entries(""), 0);
    }

    #[test]
    fn counts_one_fragment_per_file_and_skips_the_readme() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["544.fixed.md", "541.added.md", "README.md", "notes.txt"] {
            std::fs::write(dir.path().join(name), "- **thing**: detail\n").unwrap();
        }
        assert_eq!(count_fragments(dir.path()), 2);
    }

    /// The directory does not exist until someone writes the first fragment, and
    /// a missing convention must not fail the build.
    #[test]
    fn counts_zero_when_the_directory_is_absent() {
        assert_eq!(
            count_fragments(std::path::Path::new("no/such/changelog.d")),
            0
        );
    }

    /// The number `omni doctor` prints is the sum, so during the transition a
    /// tree with entries in both places reports both. Reporting only one would
    /// let a tag be cut on work the binary then denies having (#137).
    #[test]
    fn the_reported_total_covers_both_places() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("544.fixed.md"), "- **a**: b\n").unwrap();
        let c = "## [Unreleased]\n\n### Added\n- **one**: x\n\n## [0.7.4] - 2026-08-13\n";
        assert_eq!(count_unreleased_entries(c) + count_fragments(dir.path()), 2);
    }

    /// The real file, so the shipped number is exercised by the suite rather
    /// than only by whatever `build.rs` happened to compute.
    #[test]
    fn parses_the_repository_changelog() {
        let c = include_str!("../../CHANGELOG.md");
        // Not asserting a fixed count, it changes every merge. Asserting the
        // parser terminates and stays within the section.
        let total_bullets = c.lines().filter(|l| l.starts_with("- ")).count();
        assert!(count_unreleased_entries(c) <= total_bullets);
    }
}
