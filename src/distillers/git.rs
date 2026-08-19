use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct GitDistiller;

impl Distiller for GitDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> Option<String> {
        Some(if input.contains("diff --git") {
            distill_diff(segments, input)
        } else if input.contains("On branch") || input.contains("HEAD detached") {
            distill_status(input)
        } else {
            distill_log(segments, input)
        })
    }
}

fn distill_status(input: &str) -> String {
    let mut branch = String::new();
    let mut staged = Vec::new();
    let mut modified = Vec::new();
    let mut untracked = Vec::new();

    let mut state = "none";

    for line in input.lines() {
        if line.starts_with("On branch ") {
            branch = line.replace("On branch ", "").trim().to_string();
        } else if line.contains("Changes to be committed") {
            state = "staged";
        } else if line.contains("Changes not staged for commit") {
            state = "modified";
        } else if line.contains("Untracked files:") {
            state = "untracked";
        } else if line.starts_with('\t') || line.starts_with("  ") {
            let file = line.trim().to_string();
            let clean = if file.starts_with("modified:") {
                file.replace("modified:", "").trim().to_string()
            } else if file.starts_with("new file:") {
                file.replace("new file:", "").trim().to_string()
            } else if file.starts_with("deleted:") {
                file.replace("deleted:", "").trim().to_string()
            } else if file.starts_with("renamed:") {
                file.replace("renamed:", "").trim().to_string()
            } else {
                file
            };

            if clean.is_empty() || clean.starts_with("(use") {
                continue;
            }

            match state {
                "staged" => staged.push(clean),
                "modified" => modified.push(clean),
                "untracked" => untracked.push(clean),
                _ => {}
            }
        }
    }

    let mut out = format!(
        "git: on {} | staged:{} mod:{} untracked:{}",
        branch,
        staged.len(),
        modified.len(),
        untracked.len()
    );

    let top_staged = staged
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if !top_staged.is_empty() {
        out.push_str(&format!("\nStaged: {}", top_staged));
    }

    let top_mod = modified
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if !top_mod.is_empty() {
        out.push_str(&format!("\nModified: {}", top_mod));
    }

    out
}

/// Whether a line is diff payload rather than prose around it.
fn is_diff_line(line: &str) -> bool {
    line.starts_with("@@ ")
        || (line.starts_with('+') && !line.starts_with("+++"))
        || (line.starts_with('-') && !line.starts_with("---"))
}

fn distill_diff(segments: &[OutputSegment], input: &str) -> String {
    let mut out = String::new();
    let mut files = std::collections::HashSet::new();

    // Counted over the payload, never over what survived the filter below. The
    // summary labels the diff it replaces, so a line the scorer dropped is still
    // a line the diff changed, and reporting the surviving count as the diffstat
    // is a measurement the output cannot support (#616). `git show --numstat`
    // counts the same lines: body only, both file headers excluded.
    let added = input
        .lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .count();
    let removed = input
        .lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
        .count();

    for seg in segments {
        if seg.content.starts_with("diff --git") {
            if let Some(file) = seg
                .content
                .lines()
                .next()
                .and_then(|l| l.split(' ').next_back())
            {
                files.insert(file.to_string());
                out.push_str(&format!("{}\n", file)); // Just output the filename instead of whole header
            }
            continue;
        }

        // A hunk is never noise. `is_blank_or_decorative` tiers a block whose
        // lines hold only `-`, `=`, `*` or `_` as Noise, which is exactly what a
        // diff removing a rule of `=====` looks like once each line carries its
        // `-` marker. Skipping the segment dropped the hunk from the output and
        // from the counts below, so a 1+/5- diff came out as
        // `git diff: 1 files changed, 0+, 0-` with no hunk at all (#616). The
        // per-line filter further down already decides what a segment
        // contributes; the tier only gets to drop a segment carrying no diff.
        if seg.tier == SignalTier::Noise && !seg.content.lines().any(is_diff_line) {
            continue;
        }

        let mut hunk_out = String::new();
        // Context lines are kept only when session context boosted the segment,
        // which is what buys the >60% reduction. The old note here claimed a hunk
        // is always Important because it contains "@@ -"; the tier is assigned by
        // `classify_block` over the whole block and a decorative one comes back
        // Noise, which is the assumption #616 was built on.
        let keep_context = seg.context_score > 0.0 || seg.tier == SignalTier::Critical;

        for line in seg.content.lines() {
            let keep = is_diff_line(line)
                || (keep_context
                    && !line.starts_with("+++")
                    && !line.starts_with("---")
                    && !line.starts_with("index"));
            if keep {
                hunk_out.push_str(line);
                hunk_out.push('\n');
            }
        }
        out.push_str(&hunk_out);
    }

    let summary = format!(
        "git diff: {} files changed, {}+, {}-",
        files.len(),
        added,
        removed
    );
    format!("{}\n{}", summary, out.trim())
}

fn distill_log(segments: &[OutputSegment], input: &str) -> String {
    let mut out = String::new();
    // After a `commit <sha>` line, the first non-metadata line is the subject.
    // Keeping every commit's full body reduced little on a verbose multi-commit
    // log, missed the size guardrail, and the pipe's collapse fallback then
    // dropped whole commits (#199): `git log -12` came back with 2 of 12. Keep
    // one compact line per commit, hash + subject, the `--oneline` view, so
    // every commit and its subject survive while the body and the
    // Author/Date/Merge metadata go.
    let mut awaiting_subject = false;
    for seg in segments {
        for line in seg.content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with("commit ") {
                let hash: String = line.replace("commit ", "").chars().take(7).collect();
                // If the previous commit had no subject line, close its line off
                // so hashes never run together.
                if awaiting_subject {
                    out.push('\n');
                }
                out.push_str(&hash);
                out.push(' ');
                awaiting_subject = true;
            } else if crate::distillers::git::RE_GIT_LOG_HASH.is_match(line) {
                // `--oneline`: the hash and the subject share one line. Taking
                // `chars().take(7)` kept the hash and threw the subject away -
                // the only part a reader wanted, and `push(' ')` then joined
                // every commit into a wall of hashes reported as an ~89% saving
                // (#107). The hash is the cheap part; the subject is the signal.
                // Keep the line whole.
                out.push_str(line);
                out.push('\n');
                // A verbose-log subject can itself start with a 7+ hex word
                // (`a1b2c3d fix …`) and match here; clear the flag so the next
                // body line is not then taken as the subject too (review of #204).
                awaiting_subject = false;
            } else if line.starts_with("Author:")
                || line.starts_with("Date:")
                || line.starts_with("Merge:")
            {
                // Metadata, drop.
            } else if awaiting_subject {
                // First content line after `commit <sha>` is the subject.
                out.push_str(line);
                out.push('\n');
                awaiting_subject = false;
            }
            // Any further body lines before the next `commit` are dropped (#199).
        }
    }

    let result = out.trim().to_string();
    if result.is_empty() {
        // Nothing matched a commit or a `--oneline` entry, this input is not a
        // git log we can parse (the fixture harness even feeds non-log text here).
        // Fail open: return it verbatim rather than a lossy guess or empty output
        // (#143). Real logs always produce a compact result above.
        input.to_string()
    } else {
        result
    }
}

pub static RE_GIT_LOG_HASH: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[a-f0-9]{7,40} ").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    fn one_segment(content: &str) -> Vec<OutputSegment> {
        vec![OutputSegment {
            content: content.to_string(),
            tier: SignalTier::Important,
            base_score: 0.8,
            context_score: 0.0,
            line_range: (1, content.lines().count().max(1)),
        }]
    }

    /// #107. Each `--oneline` entry carries its subject on the same line as the
    /// hash; the distiller kept 7 chars and dropped the rest, joining every
    /// commit into a wall of hashes reported as an ~89% saving. Assert the
    /// subjects survive, the hash alone is close to worthless to a reader.
    #[test]
    fn oneline_keeps_every_commit_subject() {
        let input = "\
a370713 Wordmark ForgePod: bobot 900 asli, italic (#72)
93db32e feat: idea length limit
1017f0e fix: success token hardcoded hex";
        let out = distill_log(&one_segment(input), input);

        assert!(
            out.contains("Wordmark ForgePod"),
            "subject dropped: {out:?}"
        );
        assert!(
            out.contains("feat: idea length limit"),
            "subject dropped: {out:?}"
        );
        assert!(
            out.contains("fix: success token hardcoded hex"),
            "subject dropped: {out:?}"
        );
        // One line per commit, not a single space-joined run of hashes.
        assert_eq!(
            out.lines().count(),
            3,
            "commits joined onto one line: {out:?}"
        );
    }

    /// Verbose `git log` keeps the `commit <sha>` handling untouched: the subject
    /// still arrives on its own indented line and must survive.
    #[test]
    fn verbose_log_still_keeps_the_subject() {
        let input = "\
commit a370713abc1234567890abcdef1234567890abcd
Author: Someone <s@example.com>
Date:   Mon Mar 20 10:30:00 2026 +0700

    feat: add the thing";
        let out = distill_log(&one_segment(input), input);

        assert!(
            out.contains("feat: add the thing"),
            "subject dropped: {out:?}"
        );
        assert!(!out.contains("Author:"), "kept noise: {out:?}");
    }

    /// #199: a verbose multi-commit log must keep *every* commit as one compact
    /// `hash subject` line. Before this the git_log TOML filter kept each commit's
    /// body, blew past its `max_lines = 20`, and truncated the older commits away
    /// with no marker (`git log -12` returned 2 of 12). The body is dropped, the
    /// commits are not.
    #[test]
    fn verbose_log_keeps_every_commit_and_drops_the_body() {
        let input = "\
commit aaaaaaa1111111111111111111111111111111111
Author: A <a@x.com>
Date:   Mon Mar 20 10:00:00 2026 +0700

    first subject

    a body line that is not the subject
    another body line

commit bbbbbbb2222222222222222222222222222222222
Author: B <b@x.com>
Date:   Sun Mar 19 10:00:00 2026 +0700

    second subject

commit ccccccc3333333333333333333333333333333333
Author: C <c@x.com>
Date:   Sat Mar 18 10:00:00 2026 +0700

    third subject";
        let out = distill_log(&one_segment(input), input);

        assert!(
            out.contains("aaaaaaa first subject"),
            "commit 1 lost: {out:?}"
        );
        assert!(
            out.contains("bbbbbbb second subject"),
            "commit 2 lost: {out:?}"
        );
        assert!(
            out.contains("ccccccc third subject"),
            "commit 3 lost: {out:?}"
        );
        assert!(!out.contains("a body line"), "body kept: {out:?}");
        assert!(!out.contains("Author:"), "metadata kept: {out:?}");
        assert_eq!(
            out.lines().count(),
            3,
            "one line per commit expected: {out:?}"
        );
    }

    /// Review of #204: a subject that itself starts with a 7+ hex word matches the
    /// `--oneline` regex; that branch must clear `awaiting_subject` too, or the
    /// next body line is appended as a second "subject".
    #[test]
    fn hex_looking_subject_does_not_leak_the_body() {
        let input = "\
commit 1111111aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
Author: A <a@x.com>
Date:   Mon Mar 20 10:00:00 2026 +0700

    abc1234 refactor the parser

    this body line must not survive";
        let out = distill_log(&one_segment(input), input);

        assert!(out.contains("refactor the parser"), "subject lost: {out:?}");
        assert!(
            !out.contains("this body line must not survive"),
            "body leaked as subject: {out:?}"
        );
    }

    /// #616. The summary is a claim about the diff, so it has to be measured
    /// from the diff. It used to count the `+`/`-` lines that survived the
    /// filter, and a hunk whose removed lines hold only rule characters is
    /// tiered `Noise` by `is_blank_or_decorative` and was skipped before either
    /// the emit or the count. A 1+/5- diff came back as
    /// `git diff: 1 files changed, 0+, 0-` with no hunk under it, which reads as
    /// a commit that changed nothing.
    ///
    /// Scored through the real scorer on purpose: with a hand-built `Important`
    /// segment the tier that causes this never occurs and the test passes either
    /// way.
    #[test]
    fn diffstat_counts_the_diff_and_a_decorative_hunk_survives() {
        let input = "\
diff --git a/docs/banner.txt b/docs/banner.txt
index 1111111..2222222 100644
--- a/docs/banner.txt
+++ b/docs/banner.txt
@@ -1,7 +1,2 @@
 title
-=====
-=====
-=====
-=====
-=====
+short
";
        let cmd = "git show abc1234 -- docs/banner.txt";
        let profile = crate::pipeline::registry::resolve_profile(cmd);
        let segments =
            crate::pipeline::scorer::score_segments(input, profile.segmentation, None, cmd);
        assert!(
            segments.iter().any(|s| s.tier == SignalTier::Noise),
            "fixture no longer produces the Noise tier this guards: {:?}",
            segments.iter().map(|s| &s.tier).collect::<Vec<_>>()
        );

        let out = distill_diff(&segments, input);

        assert!(
            out.contains("1 files changed, 1+, 5-"),
            "diffstat does not match the payload: {out:?}"
        );
        assert_eq!(
            out.lines().filter(|l| *l == "-=====").count(),
            5,
            "the removed hunk did not survive: {out:?}"
        );
    }
}
