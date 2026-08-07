use crate::pipeline::OutputSegment;

pub mod build;
pub mod cloud;
pub mod database;
pub mod generic;
pub mod git;
pub mod jsts;
pub mod readfile;
pub mod search;
pub mod security;
pub mod system_ops;
pub mod test;
pub mod vcs;

pub trait Distiller: Send + Sync {
    /// `None` means "I could not read this input". The dispatch boundary then
    /// hands the raw bytes back, which is the only honest answer for output a
    /// distiller did not parse.
    ///
    /// The return type carries the invariant on purpose (#250). Nine releases
    /// shipped the same defect — `Build: ok` for a `python3` script, `Tests: 0
    /// passed` for a package with no tests, `grep: 20 matches` for a search that
    /// found nothing — because the rule lived in a helper each distiller had to
    /// remember to call, and the nine that forgot are where every instance
    /// landed. A verdict a distiller synthesised for input it never recognised
    /// is worse than no compression: it is shorter, plausible, and wrong, and
    /// no caller can tell it apart from a real one.
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        session: Option<&crate::pipeline::SessionState>,
    ) -> Option<String>;
}

/// The same zero-state rule as `Distiller::distill`'s `None`, for the helpers
/// several layers below it that still hand a `String` back to their caller.
///
/// The trait is where the invariant is enforced now (#250) — it applies whether
/// a distiller remembers this function or not. This stays because converting an
/// entire helper chain to `Option` would be a large diff for an identical
/// outcome: a helper returning the input and the boundary returning the input
/// produce the same bytes.
pub(crate) fn require_parsed(parsed: bool, input: &str, summary: String) -> String {
    if parsed { summary } else { input.to_string() }
}

/// Distill output based on command.
///
/// This is the boundary that enforces the trait's `None` contract: a distiller
/// that could not read its input, and every arm that declines to route, fail
/// open to the raw bytes here rather than in twelve separate files.
#[tracing::instrument(skip_all)]
pub fn distill_with_command(
    segments: &[crate::pipeline::OutputSegment],
    input: &str,
    command: &str,
    session: Option<&crate::pipeline::SessionState>,
) -> String {
    // Format-safety, applied per line at the one place every distiller is reached
    // from. `hooks::post_tool` already gates whole structured payloads, but that
    // gate is all-or-nothing: two JSON API responses printed beside a single
    // `pod "vmq" deleted` notice are not NDJSON, so the protection switched off
    // for the payload and the cloud distiller delivered 17 bytes of a 460 byte
    // answer, both result sets gone (#334). A distiller re-parses raw input, so
    // tiering the line in the scorer does not reach it; the guard has to sit
    // ahead of dispatch.
    //
    // Measured over 5,515 recorded traces before taking it: 116 hold at least one
    // JSON line, and handing all of those back whole costs **0 KB** of the 371 KB
    // the corpus saves, because none of them was being usefully compressed in the
    // first place. A free guard against the worst failure class in this tracker.
    if input.lines().any(is_json_line) {
        return input.to_string();
    }
    route(segments, input, command, session).unwrap_or_else(|| input.to_string())
}

/// A whole line that is a JSON object or array. Strict on purpose: it must
/// bracket *and* parse, so prose containing a brace, or a truncated fragment,
/// cannot silently switch compression off.
fn is_json_line(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 2 {
        return false;
    }
    let bracketed =
        (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']'));
    bracketed && serde_json::from_str::<serde_json::Value>(t).is_ok()
}

/// One `match` over the registry's decision. Every arm of what used to be here
/// is now a variant of `registry::Distillation`, which is where `CLAUDE.md` says
/// the command-to-behaviour mapping belongs (#194).
fn route(
    segments: &[crate::pipeline::OutputSegment],
    input: &str,
    command: &str,
    session: Option<&crate::pipeline::SessionState>,
) -> Option<String> {
    use crate::pipeline::registry::Distillation;

    // A chain's stdout belongs to several programs and arrives as one stream, so
    // there is no honest way to hand it to the distiller named by the first of
    // them: `git status && echo === && find .` came back as the git one-liner
    // with the `find` output deleted, unmarked (#264). One producer routes;
    // several pass through.
    let command = crate::pipeline::registry::sole_output_command(command)?;

    match crate::pipeline::registry::resolve_distiller(command) {
        Distillation::Passthrough => None,
        Distillation::Git => git::GitDistiller.distill(segments, input, session),
        Distillation::Database => database::DatabaseDistiller.distill(segments, input, session),
        Distillation::Security => security::SecurityDistiller.distill(segments, input, session),
        Distillation::Vcs => vcs::VcsDistiller.distill(segments, input, session),
        Distillation::Test => test::TestDistiller.distill(segments, input, session),
        Distillation::Build => build::BuildDistiller.distill(segments, input, session),
        Distillation::JsTs => jsts::JsTsDistiller.distill(segments, input, session),
        Distillation::Cloud(tool) => {
            cloud::CloudDistiller { tool: &tool }.distill(segments, input, session)
        }
        // The only distiller allowed near a payload the caller's pattern already
        // selected: it hoists repeated paths and hands the input back when that
        // does not shrink it, so neither outcome can lose a match (#316/#326).
        Distillation::UserFiltered => Some(system_ops::distill_user_filtered(input)),
        Distillation::SystemOps => system_ops::SystemOpsDistiller.distill(segments, input, session),
        Distillation::Generic => generic::GenericDistiller.distill(segments, input, session),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::registry::passes_through_verbatim;
    use crate::pipeline::scorer;

    /// From #170: `cargo tree` prints a dependency tree that *is* the answer.
    /// Routing every `cargo` command to `BuildDistiller` summarised 21.4 KB of
    /// it as `Build: ok` — 9 bytes — and reported a 100% reduction as a win.
    #[test]
    fn hands_back_the_output_of_cargo_commands_that_are_not_builds() {
        let tree = "omni v0.6.3 (/repo)\n\
                    ├── anyhow v1.0.100\n\
                    ├── chrono v0.4.42\n\
                    │   ├── iana-time-zone v0.1.64\n\
                    │   └── num-traits v0.2.19\n\
                    └── serde v1.0.228\n";

        for cmd in [
            "cargo tree",
            "cargo metadata --no-deps",
            "cargo search serde",
            "cargo pkgid",
        ] {
            assert_eq!(
                distill_with_command(&[], tree, cmd, None),
                tree,
                "`{cmd}` output is data, not build progress — it must survive"
            );
        }
    }

    /// From #301: `kubectl config get-contexts -o name` prints one identifier
    /// per row with no status column, so `distill_kubectl_generic` kept only the
    /// rows matching its own vocabulary and dropped the rest. The estate sorts
    /// alphabetically, so the cut landed on the tail and every production
    /// cluster was the half that went, leaving a list that looks complete.
    #[test]
    fn hands_back_kubectl_listings_that_have_no_columns() {
        let names = "Mercury-cluster\n\
                     aks-orgx-admin-reg1\n\
                     arn:aws:eks:ap-southeast-1:000000000000:cluster/acme-prod\n\
                     clientc-aks\n\
                     do-sgp1-k8s-prod\n\
                     docker-desktop\n\
                     k8s-echo-prod-reg1n\n\
                     k8s-m01-prod-reg1n\n\
                     k8s-mercury-prod-reg1n\n\
                     kind-local\n";

        for cmd in [
            "kubectl config get-contexts -o name",
            "kubectl config get-contexts",
            "kubectl get pods -o name",
            "kubectl get deploy --output name",
            "kubectl api-resources",
        ] {
            let segments = scorer::score_with_command(names, cmd, None);
            assert_eq!(
                distill_with_command(&segments, names, cmd, None),
                names,
                "`{cmd}` lists identifiers; there is no column to summarise"
            );
            assert!(
                passes_through_verbatim(cmd),
                "`{cmd}` must be exempt from the collapse fallback too (#214)"
            );
        }

        // The pod table keeps its distiller: it has a STATUS column with real
        // noise and a fingerprint that says so.
        assert!(!passes_through_verbatim("kubectl get pods -A"));
    }

    /// From #129: a `make` target runs several programs and hands back their
    /// concatenated output with no delimiter, so no single-tool distiller can
    /// speak for it. `BuildDistiller` took the whole buffer and answered
    /// `Build: 1 errors, 0 warnings` for a run that also had an eslint warning
    /// and a failed test, deleting the eslint finding on the way.
    #[test]
    fn hands_back_a_composite_make_target() {
        let mixed = "npx tsc --noEmit\n\
                     src/api/client.ts(42,7): error TS2322: Type 'string' is not assignable to type 'number'.\n\
                     npx eslint src --max-warnings 0\n\
                     \n\
                     /repo/src/hooks/useAuth.ts\n\
                     \x20 12:5  warning  Unexpected console statement  no-console\n\
                     \n\
                     1 problem (0 errors, 1 warning)\n\
                     pytest -q\n\
                     FAILED tests/test_auth.py::test_expiry - assert 0 == 1\n\
                     39 passed, 1 failed in 3.02s\n";

        for cmd in ["make check", "make ci", "make test", "make"] {
            let segments = scorer::score_with_command(mixed, cmd, None);
            assert_eq!(
                distill_with_command(&segments, mixed, cmd, None),
                mixed,
                "`{cmd}` runs several programs; every gate's verdict must survive"
            );
        }

        // The passthrough must be declared in one place, or the collapse
        // fallback folds what routing just handed back (#214).
        assert!(passes_through_verbatim("make check"));
    }

    /// From #190: a `python3 -c "..."` script prints arbitrary output that *is*
    /// the answer. Routing bare interpreters to `BuildDistiller` fabricated
    /// `Build: ok` for any script with no error/warning line — inventing success
    /// for a verification command whose real answer was the printed text.
    #[test]
    fn hands_back_the_output_of_bare_script_interpreters() {
        let script_out = " tokenRefs : ['vmuser-a']\n VSS dests : ['vault-a']\n dangling  : NONE\n";

        for cmd in [
            "python3 -c \"import re; print('dangling  : NONE')\"",
            "python audit.py",
            "ruby check.rb",
            // The substring "test" inside a `-c` arg or a path segment must NOT
            // route to TestDistiller — it fabricates `Tests: 1 passed` on this
            // output, which is #190 via a different distiller. Boundary locked.
            "python3 -c \"testing_flag = True; print('config ok')\"",
            "ruby /projects/contest/verify.rb",
        ] {
            let segments = scorer::score_with_command(script_out, cmd, None);
            let out = distill_with_command(&segments, script_out, cmd, None);
            assert_eq!(
                out, script_out,
                "`{cmd}` output is data, not build progress — it must survive verbatim"
            );
            assert_ne!(
                out.trim(),
                "Build: ok",
                "`{cmd}` fabricated a build verdict"
            );
        }
    }

    #[test]
    fn still_routes_real_cargo_builds_and_tests_to_their_distillers() {
        // Guards the other direction: the fix must not turn every cargo command
        // into passthrough and quietly end all savings on the ones that work.
        let noisy_build: String = (0..40)
            .map(|i| format!("   Compiling crate_{i} v0.1.0\n"))
            .collect::<String>()
            + "    Finished `dev` profile [unoptimized] target(s) in 12.61s\n";

        let out = distill_with_command(&[], &noisy_build, "cargo build", None);
        assert!(
            out.len() < noisy_build.len(),
            "cargo build must still be distilled, got {out:?}"
        );
    }

    #[test]
    fn vitest_zero_parse_passes_through_instead_of_claiming_success() {
        // #143 / #115: a Vite dev server (no tests) must never distill to a
        // green `vitest: ✓ 0/0 passed`. With nothing parsed, fail open.
        let input = include_str!("../../tests/fixtures/vite_dev_server.txt");
        let segments = scorer::score_with_command(input, "vitest", None);
        let output = distill_with_command(&segments, input, "vitest", None);

        assert!(
            !output.contains("0/0 passed"),
            "zero-parse produced a false success claim: {output:?}"
        );
        // The load-bearing signal (the bound port) survives.
        assert!(
            output.contains(":8080"),
            "dev-server port was dropped: {output:?}"
        );
    }

    // #143 umbrella: each distiller below is routed a payload that trips its
    // content detector but carries no signal it can actually parse. The zero-state
    // must pass the input through, never emit a confident green string. Each asserts
    // the false claim is absent AND the original content survives (passthrough).

    #[test]
    fn tsc_zero_parse_passes_through_instead_of_no_errors() {
        // `tsc --` command echo trips is_tsc_output, but there is no `error TS`
        // line to parse — the #106 shape.
        let input = "> tsc --noEmit\nresolving project references, nothing to compile\n";
        let segments = scorer::score_with_command(input, "tsc", None);
        let output = distill_with_command(&segments, input, "tsc", None);
        assert!(
            !output.contains("tsc: no errors"),
            "false claim: {output:?}"
        );
        assert!(output.contains("nothing to compile"), "dropped: {output:?}");
    }

    #[test]
    fn playwright_zero_parse_passes_through_instead_of_passed() {
        // A `[chromium]` banner trips is_playwright_output with no result summary.
        let input = "[chromium] › launching browser\nno spec files matched the filter\n";
        let segments = scorer::score_with_command(input, "playwright", None);
        let output = distill_with_command(&segments, input, "playwright", None);
        assert!(!output.contains("0/0 passed"), "false claim: {output:?}");
        assert!(output.contains("no spec files"), "dropped: {output:?}");
    }

    #[test]
    fn eslint_zero_parse_passes_through_instead_of_no_problems() {
        // A banner naming an eslint rule id trips is_eslint_output, but there is no
        // finding line or `problems (` summary to parse — the #114 shape.
        let input = "Oxc linter v0.15\nusing preset @typescript-eslint/recommended\n";
        let segments = scorer::score_with_command(input, "eslint", None);
        let output = distill_with_command(&segments, input, "eslint", None);
        assert!(
            !output.contains("no problems found"),
            "false claim: {output:?}"
        );
        assert!(output.contains("Oxc linter"), "dropped: {output:?}");
    }

    #[test]
    fn security_zero_parse_passes_through_instead_of_no_issues() {
        // A clean scan with no severity token must not be certified `no issues`.
        let input = "trivy image myapp:latest\nscanning filesystem, database up to date\n";
        let segments = scorer::score_with_command(input, "trivy", None);
        let output = distill_with_command(&segments, input, "trivy", None);
        assert!(
            !output.contains("no issues found"),
            "false claim: {output:?}"
        );
        assert!(
            output.contains("database up to date"),
            "dropped: {output:?}"
        );
    }

    /// #233. `docker ps` came back as `docker: 5 containers | 3 running, 2
    /// exited` plus the names of the exited two. Every running container, with
    /// its image, ports and uptime, was counted and discarded by construction:
    /// 862 bytes to 75, reported as a 91.3% saving, with no marker because
    /// nothing thought anything was lost.
    #[test]
    fn keeps_every_row_of_a_container_listing() {
        let input = std::fs::read_to_string("tests/fixtures/docker_ps_mixed.txt")
            .expect("fixture must exist");

        for cmd in [
            "docker ps",
            "docker ps -a",
            "podman ps",
            "docker container ls",
        ] {
            let segments = scorer::score_with_command(&input, cmd, None);
            let out = distill_with_command(&segments, &input, cmd, None);

            assert_eq!(
                out, input,
                "{cmd} must hand back every row; each one is a distinct datum"
            );
        }
    }

    /// #234. `az aks command invoke` runs an arbitrary command inside a cluster
    /// and returns that program's stdout, so routing on `az` handed a four
    /// section run to a cloud distiller with no grammar for it. Nine rows of the
    /// first section came back and the TLS probe that was the point of the call
    /// went with the rest, after a 40 second round trip against a private
    /// cluster. `kubectl exec` had already done this once as #112.
    #[test]
    fn hands_back_what_a_wrapper_ran_inside() {
        let inner = "=== ENV names ===\nBE_LIVEKIT_SERVICE_PORT_RTC_UDP\n\
                     === probe ===\nTLS handshake ok\n=== versions ===\nv1.2.3\n";

        for cmd in [
            "az aks command invoke -g rg -n cluster -c 'sh -c ...'",
            "kubectl exec -n prod pod-1 -- sh -c 'env'",
            "docker exec api sh -c 'env'",
            "ssh build-host 'env'",
        ] {
            let segments = scorer::score_with_command(inner, cmd, None);
            let out = distill_with_command(&segments, inner, cmd, None);

            assert_eq!(
                out, inner,
                "{cmd} produces the inner program's stdout, which OMNI has no grammar for"
            );
        }
    }

    #[test]
    fn docker_logs_zero_parse_passes_through_instead_of_no_errors_detected() {
        // A manifest that merely mentions "docker logs" routes to distill_docker_logs
        // but is not log-shaped (is_docker_logs false) — the #112 misroute.
        let input =
            "apiVersion: v1\nkind: Pod\n# inspect with: docker logs app\nmetadata:\n  name: app\n";
        let segments = scorer::score_with_command(input, "docker", None);
        let output = distill_with_command(&segments, input, "docker", None);
        assert!(
            !output.contains("no errors detected"),
            "false claim: {output:?}"
        );
        assert!(output.contains("kind: Pod"), "dropped: {output:?}");
    }

    #[test]
    fn test_cat_small_config_passthrough() {
        let input = "[db]\nurl = \"postgres://example\"\nmax_connections = 10\n";
        let cmd = "cat config.toml";
        let segments = scorer::score_with_command(input, cmd, None);
        let output = distill_with_command(&segments, input, cmd, None);
        assert_eq!(output, input);
    }

    macro_rules! snapshot_test {
        ($name:ident, $file:expr, $cmd:expr) => {
            #[test]
            fn $name() {
                let input = include_str!(concat!("../../tests/fixtures/", $file));
                let segments = scorer::score_with_command(input, $cmd, None);
                let output = distill_with_command(&segments, input, $cmd, None);

                insta::assert_snapshot!(output);

                if $cmd == "git diff" {
                    assert!(
                        output.len() < input.len() * 60 / 100,
                        "Git diff distiller must achieve >40% reduction (now {} len vs initial {})",
                        output.len(),
                        input.len()
                    );
                }
            }
        };
    }

    snapshot_test!(
        test_git_diff_distillation,
        "git_diff_multi_file.txt",
        "git diff"
    );
    snapshot_test!(
        test_git_status_distillation,
        "git_status_dirty.txt",
        "git status"
    );
    // #107: `--oneline` puts the hash and subject on one line. The distiller used
    // to keep 7 chars of hash and drop the subject, joining every commit into a
    // wall of hashes. This locks the whole line surviving — there was no git_log
    // snapshot before, which is why the regression shipped unseen.
    snapshot_test!(
        test_git_log_oneline_distillation,
        "git_log.txt",
        "git log --oneline"
    );
    snapshot_test!(
        test_cargo_build_distillation,
        "cargo_build_errors.txt",
        "cargo build"
    );
    snapshot_test!(test_pytest_distillation, "pytest_failures.txt", "pytest");
    snapshot_test!(
        test_kubectl_distillation,
        "kubectl_pods_mixed.txt",
        "kubectl get pods"
    );
    snapshot_test!(
        test_docker_build_distillation,
        "docker_build_layered.txt",
        "docker build"
    );
    snapshot_test!(
        test_nginx_log_distillation,
        "nginx_access_log.txt",
        "cat access.log"
    );
    snapshot_test!(test_cloud_kubectl, "kubectl_get_pods_mixed.txt", "kubectl");
    snapshot_test!(test_cloud_docker_ps, "docker_ps_mixed.txt", "docker ps");
    snapshot_test!(
        test_cloud_docker_build_error,
        "docker_build_error.txt",
        "docker build"
    );
    snapshot_test!(
        test_cloud_terraform_plan,
        "terraform_plan_cloud.txt",
        "terraform plan"
    );
    snapshot_test!(test_systemops_grep, "grep_recursive_output.txt", "grep -r");

    // #198/#200: `ls`, `find` and `env` are enumerations — every line is the
    // answer, not noise — so they must pass through verbatim, never be summarised
    // or truncated. (These replaced snapshot tests that asserted the old lossy
    // distillation; `grep` above stays snapshotted because its hoisting is
    // lossless.)
    macro_rules! passthrough_test {
        ($name:ident, $file:expr, $cmd:expr) => {
            #[test]
            fn $name() {
                let input = include_str!(concat!("../../tests/fixtures/", $file));
                let segments = scorer::score_with_command(input, $cmd, None);
                let output = distill_with_command(&segments, input, $cmd, None);
                assert_eq!(
                    output, input,
                    concat!(
                        $cmd,
                        " is an enumeration; it must pass through verbatim (#200)"
                    )
                );
            }
        };
    }
    passthrough_test!(ls_passes_through_verbatim, "ls_la_output.txt", "ls -l");
    passthrough_test!(
        find_passes_through_verbatim,
        "find_project_output.txt",
        "find ."
    );
    passthrough_test!(env_passes_through_verbatim, "env_output.txt", "env");

    /// Review of #205: bare `env` and `command env` both leave `base` empty (the
    /// wrapper is stripped), so the guard matches on any `env` token. Locks that
    /// in — a narrowing back to "first word only" would silently drop `command
    /// env` passthrough.
    #[test]
    fn treats_bare_and_wrapped_env_as_enumeration() {
        assert!(passes_through_verbatim("env"));
        assert!(passes_through_verbatim("command env"));
        assert!(passes_through_verbatim("ls -la"));
        assert!(!passes_through_verbatim("echo hi"));
    }

    /// #326. `grep` moved into this predicate, which reads oddly next to a
    /// routing arm that still distils it, so the reason is worth pinning: the
    /// predicate governs only the hooks' collapse fallback, and that fallback
    /// runs exactly when the grep distiller punted and handed the input back.
    /// Folding three `mcp … connected` lines into one marker at that point loses
    /// the two server names the pattern matched on.
    #[test]
    fn exempts_the_callers_own_filter_from_the_collapse_fallback() {
        assert!(passes_through_verbatim("grep -r foo"));
        assert!(passes_through_verbatim("rg -n 'stream_mode'"));
        assert!(passes_through_verbatim("ag TODO src/"));
    }

    /// #214 moved the interpreters into this predicate and deleted their own
    /// routing arm, so the predicate is now the only thing keeping them out of
    /// `BuildDistiller` *and* out of the hooks' collapse fallback. Dropping one
    /// entry costs both at once, which is worth catching here rather than in the
    /// integration test, where it only shows up after it has propagated.
    #[test]
    fn treats_bare_script_interpreters_as_passthrough() {
        assert!(passes_through_verbatim("python3 -c \"print('x')\""));
        assert!(passes_through_verbatim("python audit.py"));
        assert!(passes_through_verbatim("ruby /proj/contest/verify.rb"));
        assert!(passes_through_verbatim("/usr/bin/python3 gen.py"));
        // `pip` and `rake` stay on the build path: their output is task oriented.
        assert!(!passes_through_verbatim("pip install requests"));
        assert!(!passes_through_verbatim("rake db:migrate"));
    }

    /// #236: a prose document that happens to quote a directory layout was
    /// classified by the one line holding a box-drawing character and delivered
    /// as `tree: N entries` — a different *kind* of thing, asserted with no
    /// marker. Reading a file is not a grammar, so the readers pass through.
    #[test]
    fn hands_back_a_document_that_quotes_a_directory_tree() {
        let doc = "# Development Guide\n\
                   \n\
                   Guide for contributors working on the OMNI codebase.\n\
                   \n\
                   ## Layout\n\
                   \n\
                   ```\n\
                   src/\n\
                   ├── main.rs\n\
                   ├── pipeline/\n\
                   └── distillers/\n\
                   ```\n\
                   \n\
                   ## Verification — run these yourself, CI does not\n\
                   \n\
                   Run `yamllint` and `kubeconform` before committing manifests.\n\
                   CI validates changed files individually and never builds an overlay.\n";

        for cmd in [
            "cat docs/DEVELOPMENT.md",
            "head -80 docs/DEVELOPMENT.md",
            "tail -60 CLAUDE.md",
            "awk 'NR>10' docs/DEVELOPMENT.md",
        ] {
            let segments = scorer::score_with_command(doc, cmd, None);
            assert_eq!(
                distill_with_command(&segments, doc, cmd, None),
                doc,
                "`{cmd}` must hand the document back verbatim (#236)"
            );
        }
    }

    /// #231: `--stat` exists only to produce the file list, and `distill_log`
    /// dropped every row of it while keeping the `--oneline` subject — so the
    /// output stayed well-formed, got shorter, and read as a complete answer to
    /// a question it no longer answered. The reporter was checking whether
    /// `git add -A` had swept three screenshots into a commit; it came back
    /// naming no files.
    #[test]
    fn hands_back_the_file_rows_git_was_asked_to_enumerate() {
        let stat = "9cd7a80 docs(changelog): record the #228 pass-counter fix\n \
                    CHANGELOG.md | 1 +\n \
                    media/shot-a.png | Bin 0 -> 12043 bytes\n \
                    media/shot-b.png | Bin 0 -> 9981 bytes\n \
                    media/shot-c.png | Bin 0 -> 11202 bytes\n \
                    4 files changed, 1 insertion(+)\n";

        for cmd in [
            "git show --stat --oneline HEAD",
            "git log --stat -3",
            "git show --name-only HEAD",
            "git diff --numstat main",
        ] {
            let segments = scorer::score_with_command(stat, cmd, None);
            assert_eq!(
                distill_with_command(&segments, stat, cmd, None),
                stat,
                "`{cmd}` asked for the file list; it must survive (#231)"
            );
        }
    }

    /// #235: a contiguous line range of source code is what the caller already
    /// chose, so there is nothing to cut. The reported loss was 37 of 56 lines,
    /// under a marker suggesting a `--limit` flag neither command has.
    ///
    /// The reported pipeline leads with `gh`, so `VcsDistiller` claimed it — the
    /// bare `sed` form goes through the reader arm. Both are covered here
    /// because either alone leaves the reproduction broken.
    #[test]
    fn hands_back_a_source_line_range_however_it_was_fetched() {
        let mut src = String::new();
        for i in 0..56 {
            src.push_str(&format!(
                "    self.frame.origin.x = offset + {i}.0 * spacing\n"
            ));
        }

        for cmd in [
            "sed -n '285,340p' Kit/module/module.swift",
            "gh api repos/o/r/contents/Kit/module.swift --jq '.content' | base64 -d | sed -n '285,340p'",
            "gh api repos/fajarhide/homebrew-tap/contents/Casks/bubo.rb --jq '.content' | base64 -d",
        ] {
            let segments = scorer::score_with_command(&src, cmd, None);
            assert_eq!(
                distill_with_command(&segments, &src, cmd, None),
                src,
                "`{cmd}` must not be cut to an enumeration (#235, #226)"
            );
        }
    }

    /// The gate must not disarm the distiller it guards: a real `gh pr list` is
    /// still an enumeration `--limit` can re-fetch, so it still summarises.
    #[test]
    fn still_summarises_a_real_gh_list() {
        let mut listing = String::new();
        for i in 1..=25 {
            listing.push_str(&format!(
                "#{i}\tSome pull request title\tbranch-{i}\tOPEN\n"
            ));
        }

        let cmd = "gh pr list --limit 25";
        let segments = scorer::score_with_command(&listing, cmd, None);
        let out = distill_with_command(&segments, &listing, cmd, None);
        assert!(
            out.contains("more items"),
            "a gh list should still be summarised:\n{out}"
        );
    }

    /// The gate must not disarm the distiller it guards: a plain `git log` is
    /// still summarised, so the fix is not a blanket passthrough for `git`.
    #[test]
    fn still_distills_a_plain_git_log() {
        let log = "commit a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0\n\
                   Author: Someone <someone@example.com>\n\
                   Date:   Mon Jul 27 10:00:00 2026 +0700\n\
                   \n\
                       fix(collapse): keep a group inside its section\n\
                   \n\
                   commit b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1\n\
                   Author: Someone <someone@example.com>\n\
                   Date:   Mon Jul 27 09:00:00 2026 +0700\n\
                   \n\
                       test(perf): widen the latency budget\n";

        let cmd = "git log -2";
        let segments = scorer::score_with_command(log, cmd, None);
        let out = distill_with_command(&segments, log, cmd, None);

        assert!(
            !out.contains("Author:"),
            "plain `git log` must still drop metadata:\n{out}"
        );
        assert!(
            out.contains("fix(collapse)") && out.contains("test(perf)"),
            "every subject must survive:\n{out}"
        );
    }

    snapshot_test!(test_jsts_vitest, "vitest_mixed.txt", "vitest");
    snapshot_test!(test_jsts_tsc, "tsc_errors.txt", "tsc");
    // #106: a composite `npm run <script>` (an `&&` chain) must NOT be claimed by a
    // single tool distiller — `tsc --` in npm's echo used to collapse the whole thing
    // to `tsc: no errors`. jsts declines composites (returns them for the pipeline's
    // generic collapse), so every gate's verdict survives.
    snapshot_test!(
        test_jsts_npm_composite,
        "npm_run_verify.txt",
        "npm run verify"
    );
    snapshot_test!(
        test_jsts_playwright,
        "playwright_fail.txt",
        "playwright test"
    );
    snapshot_test!(test_jsts_eslint, "eslint_errors.txt", "eslint");
    // #114: `prettier --write` (via `npm run format`) used to report `eslint: no
    // problems found` — `is_eslint_output` matched the filename `eslint.config.js`
    // in prettier's file list. It must now be recognised as prettier and summarised
    // per real prettier output (both modes), never as a clean run of another tool.
    snapshot_test!(
        test_jsts_prettier_write,
        "prettier_write.txt",
        "npm run format"
    );
    snapshot_test!(
        test_jsts_prettier_check,
        "prettier_check.txt",
        "prettier --check ."
    );

    snapshot_test!(
        test_database_psql_error,
        "psql_error.txt",
        "psql -U postgres mydb"
    );
    // #216: the generic arm answered a schema dump with `DB: ok (N lines
    // output)` — a verdict invented in the success direction over a payload it
    // never read. With no error segment and nothing tabular parsed it must hand
    // the DDL back verbatim.
    snapshot_test!(
        database_sqlite_schema_passes_through_verbatim,
        "sql_create.txt",
        "sqlite3 app.db .schema"
    );
    snapshot_test!(
        test_security_trivy_scan,
        "trivy_output.txt",
        "trivy image myapp:latest"
    );
}
