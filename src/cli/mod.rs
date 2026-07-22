pub mod diff;
pub mod doctor;
pub mod engram;
pub mod exec;
pub mod goal;
pub mod handoff;
pub mod init;
pub mod learn;

pub mod patterns;
pub mod query;
pub mod remember;
pub mod reset;
pub mod rewind;
pub mod rewrite;
pub mod session;
pub mod stats;
pub mod update;

pub mod version;

use anyhow::{Result, bail};
use colored::*;

/// The flags one subcommand accepts, as `(spec, description)`, where `spec` is
/// the flag and any aliases exactly as help should show them: `"--today, -d"`.
///
/// Both the help printer and the argument check read this one list, so a flag
/// cannot be accepted without being documented or documented without being
/// accepted — the drift that made `omni stats`'s own footer advertise a
/// `--detail` its `--help` never mentioned (#151).
pub type Flags = &'static [(&'static str, &'static str)];

/// The individual flags of a `(spec, _)` entry, with any value placeholder
/// dropped: `"--today, -d"` → `--today`, `-d`; `"--validate <file.toml>"` →
/// `--validate`.
fn aliases(spec: &str) -> impl Iterator<Item = &str> {
    spec.split(',')
        .filter_map(|part| part.split_whitespace().next())
}

/// The trailing entry every subcommand shares.
pub const HELP_FLAG: (&str, &str) = ("--help, -h", "Show this help message");

/// Render the `FLAGS:` block of a subcommand's help, `--help` included.
pub fn print_flags(flags: Flags) {
    let entries: Vec<_> = flags.iter().chain(std::iter::once(&HELP_FLAG)).collect();
    print_flag_group("FLAGS:", &entries);
}

/// One titled group, for a command whose flags read better split up
/// (`omni init` separates its agents from its Claude-specific flags).
pub fn print_flag_group(title: &str, flags: &[&(&str, &str)]) {
    // Sized to the longest entry rather than a fixed width, which
    // `--all-commands` and `--validate <file.toml>` both overflow.
    let width = flags.iter().map(|(spec, _)| spec.len()).max().unwrap_or(0);

    println!("\n{}", title.bold().bright_white());
    for (spec, description) in flags {
        println!("  {} {}", format!("{spec:<width$}").cyan(), description);
    }
}

/// Reject any `--flag` this subcommand does not accept.
///
/// clap cannot do this for us. Every subcommand is declared `trailing_var_arg`
/// with a `Vec<String>` catch-all and each module then re-parses raw argv by
/// hand, so clap is never told the valid set and nothing can detect a value
/// outside it. Untouched, `omni stats --detial` silently ran the default
/// overview and exited 0 — the user asked for one mode, got another, and the
/// output said nothing about the flag being ignored (#151).
///
/// Long `--flags` are always checked. A single-letter `-x` is checked only when
/// the subcommand declares at least one short flag, so free-form text keeps
/// passing through (`omni remember "build with -O2"`, `omni engram list`).
pub fn check_flags(command: &str, args: &[String], flags: Flags) -> Result<()> {
    let has_shorts = flags
        .iter()
        .any(|(spec, _)| aliases(spec).any(is_short_flag));

    for arg in args {
        let checkable = arg.starts_with("--") || (has_shorts && is_short_flag(arg));
        if !checkable {
            continue;
        }
        // `--flag=value` is checked on the name alone.
        let name = arg.split('=').next().unwrap_or(arg);
        if name == "--help"
            || name == "-h"
            || flags
                .iter()
                .any(|(spec, _)| aliases(spec).any(|flag| flag == name))
        {
            continue;
        }

        let hint = match nearest(name, flags) {
            Some(candidate) => format!("did you mean `{candidate}`?"),
            None => format!("run `omni {command} --help` for the accepted flags"),
        };
        bail!("unknown flag `{name}` for `omni {command}` — {hint}");
    }
    Ok(())
}

fn is_short_flag(arg: &str) -> bool {
    let mut chars = arg.chars();
    chars.next() == Some('-') && chars.next().is_some_and(char::is_alphabetic) && chars.count() == 0
}

/// The accepted flag closest to `name`, if one is close enough to be a typo
/// rather than a different flag entirely.
fn nearest(name: &str, flags: Flags) -> Option<&'static str> {
    /// Beyond two edits the "suggestion" is noise: `--week` and `--month` are
    /// three apart and are not each other's typo. Short flags are one character,
    /// so any two of them are within this distance — they never suggest.
    const MAX_DISTANCE: usize = 2;

    flags
        .iter()
        .flat_map(|(spec, _)| aliases(spec))
        .filter(|flag| !is_short_flag(flag))
        .map(|flag| (flag, strsim::levenshtein(name, flag)))
        .filter(|(_, distance)| *distance <= MAX_DISTANCE)
        .min_by_key(|(_, distance)| *distance)
        .map(|(flag, _)| flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAGS: Flags = &[
        ("--detail", "Full technical breakdown"),
        ("--today", "Scope to today only"),
        ("--json", "Machine-readable JSON output"),
    ];

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn accepts_a_documented_flag() {
        assert!(check_flags("stats", &args(&["omni", "stats", "--detail"]), FLAGS).is_ok());
    }

    #[test]
    fn rejects_a_flag_the_command_does_not_accept() {
        let err = check_flags("stats", &args(&["omni", "stats", "--nonsense"]), FLAGS)
            .expect_err("an undeclared flag must not be accepted silently");
        assert!(err.to_string().contains("--nonsense"), "{err}");
    }

    #[test]
    fn suggests_the_flag_a_typo_meant() {
        let err = check_flags("stats", &args(&["omni", "stats", "--detial"]), FLAGS)
            .expect_err("a typo must not be accepted silently");
        assert!(
            err.to_string().contains("did you mean `--detail`?"),
            "{err}"
        );
    }

    #[test]
    fn offers_no_suggestion_when_nothing_is_close() {
        let err = check_flags("stats", &args(&["omni", "stats", "--verbose"]), FLAGS)
            .expect_err("an undeclared flag must not be accepted silently");
        assert!(err.to_string().contains("omni stats --help"), "{err}");
    }

    #[test]
    fn checks_the_name_of_a_valued_flag() {
        assert!(check_flags("stats", &args(&["omni", "stats", "--json=1"]), FLAGS).is_ok());
        assert!(check_flags("stats", &args(&["omni", "stats", "--jsonn=1"]), FLAGS).is_err());
    }

    #[test]
    fn passes_free_text_and_subwords_through() {
        assert!(check_flags("engram", &args(&["omni", "engram", "list"]), FLAGS).is_ok());
        assert!(
            check_flags(
                "remember",
                &args(&["omni", "remember", "the build is --slow"]),
                FLAGS
            )
            .is_ok()
        );
    }

    #[test]
    fn never_rejects_help() {
        assert!(check_flags("stats", &args(&["omni", "stats", "--help"]), FLAGS).is_ok());
    }
}
