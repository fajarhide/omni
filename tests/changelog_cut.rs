//! `scripts/changelog_cut.sh` is the only thing standing between a mistyped
//! fragment and a release that silently ships without its notes. It had already
//! been holed twice by the time this file was written, once on the section token
//! and once on the extension, so it is driven here as a real process rather than
//! reasoned about.
//!
//! Unix only: the script is bash, and Windows releases are cut from macOS.
#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const CHANGELOG: &str =
    "# Changelog\n\n## [Unreleased]\n\n## [0.7.4] - 2026-08-13\n\n- **old**: thing\n";

/// Lay out a throwaway repo with the real script in it and run the cut.
fn cut(fragments: &[(&str, &str)], version: &str) -> (Output, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join("changelog.d")).unwrap();
    fs::create_dir(root.join("scripts")).unwrap();
    fs::write(root.join("CHANGELOG.md"), CHANGELOG).unwrap();
    fs::write(root.join("changelog.d/README.md"), "docs\n").unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/changelog_cut.sh");
    fs::copy(&script, root.join("scripts/changelog_cut.sh")).unwrap();

    for (name, body) in fragments {
        fs::write(root.join("changelog.d").join(name), body).unwrap();
    }

    let out = Command::new("bash")
        .arg("scripts/changelog_cut.sh")
        .arg(version)
        .current_dir(root)
        .output()
        .expect("the cut script must be runnable");
    (out, dir)
}

fn changelog(dir: &tempfile::TempDir) -> String {
    fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap()
}

fn fragments_left(dir: &tempfile::TempDir) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir.path().join("changelog.d"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn folds_fragments_into_the_version_section_and_deletes_them() {
    let (out, dir) = cut(
        &[
            ("544.fixed.md", "- **a fix**: detail\n"),
            ("547.added.md", "- **a feature**: detail\n"),
        ],
        "0.7.5",
    );
    assert!(out.status.success(), "cut must succeed: {out:?}");

    let c = changelog(&dir);
    let added = c.find("### Added").expect("Added heading");
    let fixed = c.find("### Fixed").expect("Fixed heading");
    assert!(
        added < fixed,
        "Keep a Changelog order, Added before Fixed:\n{c}"
    );
    assert!(c.contains("- **a fix**: detail"), "fix must land:\n{c}");
    assert!(c.contains("- **a feature**: detail"), "feature must land");
    assert!(
        c.contains("## [Unreleased]") && c.contains("## [0.7.5] - "),
        "an empty Unreleased must survive above the new version:\n{c}"
    );
    assert!(c.contains("- **old**: thing"), "history must not be lost");
    assert_eq!(
        fragments_left(&dir),
        vec!["README.md"],
        "folded fragments are deleted, README is not"
    );
}

/// The guard. A section nothing can place must stop the release before
/// `CHANGELOG.md` is written, not after.
#[test]
fn refuses_a_mistyped_section_without_touching_the_changelog() {
    let (out, dir) = cut(
        &[
            ("544.fixed.md", "- **a fix**: detail\n"),
            ("545.fixd.md", "- **typo**: detail\n"),
        ],
        "0.7.5",
    );
    assert!(!out.status.success(), "a stray fragment must fail the cut");

    let err = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("545.fixd.md"),
        "the message must name the offending file:\n{err}"
    );
    assert_eq!(
        changelog(&dir),
        CHANGELOG,
        "CHANGELOG.md must be byte-identical after a refusal"
    );
    assert_eq!(
        fragments_left(&dir),
        vec!["544.fixed.md", "545.fixd.md", "README.md"],
        "and nothing may be deleted"
    );
}

/// The second hole, found by testing rather than by review: with the predicate
/// narrowed to `*.md`, these were invisible to the glob *and* to
/// `count_fragments`, so the entry vanished with `rc=0` and no message.
#[test]
fn refuses_a_fragment_whose_extension_is_wrong() {
    let (out, dir) = cut(
        &[
            ("544.fixed.txt", "- **wrong extension**: detail\n"),
            ("545.fixed", "- **no extension**: detail\n"),
        ],
        "0.7.5",
    );
    assert!(
        !out.status.success(),
        "an unplaceable fragment must never pass silently"
    );

    let err = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("544.fixed.txt") && err.contains("545.fixed"),
        "both must be named:\n{err}"
    );
    assert_eq!(changelog(&dir), CHANGELOG, "and nothing may be written");
}

/// The state of the directory immediately after every release.
#[test]
fn cuts_cleanly_when_only_the_readme_is_present() {
    let (out, dir) = cut(&[], "0.7.5");
    assert!(out.status.success(), "an empty directory is not an error");

    let c = changelog(&dir);
    assert!(
        c.contains("## [0.7.5] - "),
        "the version heading is cut:\n{c}"
    );
    assert!(c.contains("## [Unreleased]"), "and Unreleased survives");
    assert_eq!(fragments_left(&dir), vec!["README.md"]);
}

/// macOS drops `.DS_Store` into any directory Finder opens. It must not be
/// mistaken for an entry on either side of the predicate.
#[test]
fn a_hidden_file_is_not_a_fragment() {
    let (out, dir) = cut(
        &[
            (".DS_Store", "\u{0}\u{0}binary\n"),
            ("544.fixed.md", "- **a fix**: detail\n"),
        ],
        "0.7.5",
    );
    assert!(
        out.status.success(),
        "a dotfile must not block a release: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(changelog(&dir).contains("- **a fix**: detail"));
    assert_eq!(
        fragments_left(&dir),
        vec![".DS_Store", "README.md"],
        "the dotfile is left alone, the fragment is folded away"
    );
}

/// A name the fold loop cannot word-split correctly. Greptile raised the newline
/// case on #554; a space is the version someone actually types. Both already
/// failed safely, but as `cat: changelog.d/545: No such file or directory`, which
/// names neither the file nor the reason.
#[test]
fn refuses_a_filename_it_cannot_word_split() {
    for bad in ["544 fixed.md", "545\nweird.fixed.md", "546.fixed'.md"] {
        let (out, dir) = cut(&[(bad, "- **unplaceable**: detail\n")], "0.7.5");
        assert!(
            !out.status.success(),
            "{bad:?} must be refused, not processed"
        );

        let msg = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
        assert!(
            msg.contains("outside [A-Za-z0-9._-]"),
            "the refusal must say why, not fail inside cat, for {bad:?}:\n{msg}"
        );
        assert_eq!(
            changelog(&dir),
            CHANGELOG,
            "and CHANGELOG.md must be untouched for {bad:?}"
        );
    }
}

/// The check above must not reject the names the convention actually produces.
#[test]
fn accepts_the_names_the_convention_produces() {
    let (out, dir) = cut(
        &[
            ("544.fixed.md", "- **issue number**: detail\n"),
            ("changelog-fragments.changed.md", "- **a slug**: detail\n"),
            ("v0_7_4.added.md", "- **digits and underscores**: detail\n"),
        ],
        "0.7.5",
    );
    assert!(
        out.status.success(),
        "ordinary names must pass: {}",
        String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr)
    );
    let c = changelog(&dir);
    for expected in ["issue number", "a slug", "digits and underscores"] {
        assert!(c.contains(expected), "{expected} must land:\n{c}");
    }
    assert_eq!(fragments_left(&dir), vec!["README.md"]);
}

/// Refusing to overwrite a version that already exists, so a re-run cannot
/// duplicate a section.
#[test]
fn refuses_a_version_already_in_the_changelog() {
    let (out, dir) = cut(&[("544.fixed.md", "- **a fix**: detail\n")], "0.7.4");
    assert!(!out.status.success(), "0.7.4 is already in the fixture");
    assert_eq!(changelog(&dir), CHANGELOG);
}
