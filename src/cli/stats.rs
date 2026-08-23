// Safety: String slicing uses ASCII delimiter positions or boundary-checked safe utilities.

use crate::store::sqlite::Store;
use anyhow::{Context, Result};
use colored::*;
use std::collections::HashMap;

// ─── Helper Functions ───────────────────────────────────

pub fn format_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{} B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else if n < 1024 * 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", n as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn format_exact_tokens(tokens: u64) -> String {
    if tokens < 1000 {
        format!("{}", tokens)
    } else if tokens < 1_000_000 {
        format!("{:.0}K", tokens as f64 / 1_000.0)
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

/// `width` is the column the bar has to live in, not a fixed 20. The detail
/// table gives it 12 so the whole row fits `cli::WIDTH`; the wider single-column
/// listings still pass 20.
pub fn format_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    "█".repeat(filled)
}
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Widest entry, in characters. Bars and CJK are not involved in these columns,
/// so `chars()` is the right unit. Takes an iterator so callers pad straight from
/// what they are about to print, without collecting a throwaway `Vec` first.
fn max_width<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> usize {
    items
        .into_iter()
        .map(|s| s.as_ref().chars().count())
        .max()
        .unwrap_or(0)
}

/// The bar column inside the two framed tables. Narrower than the 20 the
/// single-column listings use, so a full row lands inside `cli::WIDTH`.
const DETAIL_BAR: usize = 12;

/// The one width every command name is shortened to.
///
/// It is a constant because two call sites disagreeing by a single column is
/// what made the `Agent` column report commands it had never resolved (#471).
const CMD_KEY_WIDTH: usize = 18;

pub(crate) fn group_and_calculate_stats(
    items: Vec<(String, u64, u64, u64, u64, u64)>,
    limit: usize,
) -> Vec<(String, u64, f64, u64)> {
    let mut grouped: HashMap<String, (u64, u64, u64, u64, u64)> = HashMap::new();

    for (cmd, calls, input, output, raw_tok, filt_tok) in items {
        // Group by the shortened version so things like "npm install x" and "npm install y" combine
        let key = shorten_command(&cmd, CMD_KEY_WIDTH);
        let entry = grouped.entry(key).or_insert((0, 0, 0, 0, 0));
        entry.0 += calls;
        entry.1 += input;
        entry.2 += output;
        entry.3 += raw_tok;
        entry.4 += filt_tok;
    }

    let mut result: Vec<(String, u64, f64, u64)> = grouped
        .into_iter()
        .map(|(cmd, (calls, input, output, raw_tok, filt_tok))| {
            let pct = if raw_tok > 0 {
                100.0 * (1.0 - filt_tok as f64 / raw_tok as f64)
            } else if input > 0 {
                100.0 * (1.0 - output as f64 / input as f64)
            } else {
                0.0
            };

            // #589. Bytes, counted. This was the token columns when they were
            // present and an `estimate_tokens` call over these same bytes when
            // they were not, so both arms produced the same quantity over 3.6, a
            // constant calibrated against `cl100k_base`. The subtraction is the
            // whole of it now, and the fallback disappears with the estimator.
            let bytes_saved = input.saturating_sub(output);

            (cmd, calls, pct, bytes_saved)
        })
        .collect();

    result.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    if limit > 0 {
        result.truncate(limit);
    }
    result
}

fn get_top_commands(store: &Store, since: i64, limit: usize) -> Vec<(String, u64, f64, u64)> {
    let raw = store
        .get_per_command_stats(since, limit * 3)
        .unwrap_or_default();

    group_and_calculate_stats(raw, limit)
}

fn shorten_command(cmd: &str, max_len: usize) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let short = match parts.len() {
        0 => return "[pipe]".to_string(),
        1 => parts[0].to_string(),
        _ => format!("{} {}", parts[0], parts[1]),
    };
    if short.len() <= max_len {
        short
    } else {
        format!(
            "{}...",
            crate::util::text::safe_slice(&short, max_len.saturating_sub(3))
        )
    }
}

/// `unknown` is **not** folded into `Terminal` (#160). "a human ran this in a
/// shell" and "OMNI could not tell who ran this" are different facts and only
/// one is actionable, collapsing them is what hid the missing Claude Code
/// branch in `agents::multiagent::detect_agent_id` for the life of the feature.
/// A detection gap now shows up in the table as `Unknown` instead of looking
/// like ordinary shell usage.
pub(crate) fn agent_display_name(agent_id: &str) -> &str {
    match agent_id {
        "claude_code" | "claude" => "Claude Code",
        "cursor" => "Cursor AI",
        "zed" => "Zed Editor",
        "cline" => "Cline",
        "roo-code" | "roo_code" => "Roo Code",
        "copilot" => "Copilot CLI",
        "gemini" => "Gemini CLI",
        "opencode" => "OpenCode",
        "codex_cli" | "codex" => "Codex CLI",
        "vscode_continue" => "Continue (VS Code)",
        "openclaw" => "OpenClaw",
        "antigravity" => "Antigravity",
        "vscode" => "VS Code",
        "windsurf" => "Windsurf",
        "aider" => "Aider",
        "pi" => "Pi",
        "mcp_generic" => "MCP client",
        "terminal" => "Terminal",
        "unknown" | "" => "Unknown",
        other => other,
    }
}

fn print_separator() {
    super::print_rule();
}

/// Read by both `print_help` and `super::check_flags`, so this list is what
/// `omni stats` documents *and* what it accepts (#151).
/// One flag per dimension, then every older spelling, which still resolves (#667).
///
/// The window used to be four flags and six names, and passing two of them took
/// whichever branch came first. `--since` cannot express that. Everything from
/// `VISIBLE` on keeps working and is absent from `--help` and from the manual, so
/// a script written against the old surface still runs while the help page
/// documents one way to say each thing. No deprecation notice is printed: the
/// rename is ours, not the caller's.
///
/// One list rather than two, because a second copy of a flag name is a copy that
/// drifts: #452, #454 and #456 were each one half of a pair being fixed.
const FLAGS: super::Flags = &[
    (
        "--since <window>",
        "hour | today | week | month | all (default month)",
    ),
    (
        "--view <name>",
        "summary | detail | commands | projects | context | rerun | share",
    ),
    ("--json", "Machine-readable report, scoped by --since"),
    (
        "--card",
        "Write the summary as an image, sized for social posts",
    ),
    ("--limit <n>", "Rows in a table view (default 8, 0 for all)"),
    ("--detail", ""),
    ("--hour, -H", ""),
    ("--day, --today, -d", ""),
    ("--week, -w", ""),
    ("--month, -m", ""),
    ("--all-commands", ""),
    ("--share", ""),
    ("--project", ""),
    ("--context", ""),
    ("--rerun", ""),
];

/// How many of `FLAGS` the help page shows. The rest are the aliases above.
const VISIBLE: usize = 5;

/// The time window the scope flags select, as `(label, since_unix)`.
///
/// One resolver for every mode. `run_detail` and `run_project_stats` each had
/// their own copy and neither matched `--month` at all, it was honoured only by
/// being the fall-through in one of them, and silently ignored in the other.
fn scope(args: &[String]) -> (&'static str, i64) {
    let now = chrono::Utc::now().timestamp();
    // `--since` first, so a caller mixing the new flag with an old one gets the
    // one they wrote most recently rather than whichever branch came first.
    let named = super::flag_value(args, "--since").map(str::to_ascii_lowercase);
    let window = match named.as_deref() {
        Some(w) => w,
        None if super::has_any(args, &["--hour", "-H"]) => "hour",
        None if super::has_any(args, &["--day", "--today", "-d"]) => "today",
        None if super::has_any(args, &["--week", "-w"]) => "week",
        None => "month",
    };
    match window {
        "hour" => ("last hour", now - 3600),
        // Calendar day, not a rolling 24h: "today" means since midnight.
        "today" | "day" => ("today", now - (now % 86400)),
        "week" => ("last 7 days", now - 7 * 86400),
        "all" => ("all time", 0),
        // `month` and anything unrecognised land on the default window rather
        // than failing: a report is not worth refusing over a typo in a scope.
        _ => ("last 30 days", now - 30 * 86400),
    }
}

/// The view to render, from `--view` or from the flag that used to select it.
///
/// Order matters only for a caller passing several: `--view` wins, then the old
/// flags in the order they were resolved before, so nothing changes underneath a
/// script that passed two.
fn view(args: &[String]) -> &'static str {
    if let Some(name) = super::flag_value(args, "--view") {
        return match name.to_ascii_lowercase().as_str() {
            "detail" => "detail",
            "commands" => "commands",
            "projects" | "project" => "project",
            "context" => "context",
            "rerun" => "rerun",
            "share" => "share",
            _ => "summary",
        };
    }
    if super::has_flag(args, "--card") {
        "card"
    } else if super::has_flag(args, "--share") {
        "share"
    } else if super::has_flag(args, "--rerun") {
        "rerun"
    } else if super::has_flag(args, "--context") {
        "context"
    } else if super::has_flag(args, "--detail") || super::has_flag(args, "--all-commands") {
        "detail"
    } else if super::has_flag(args, "--project") {
        "project"
    } else {
        "summary"
    }
}

fn print_help() {
    println!(
        "\n{} {}: Savings analytics",
        "omni".bold().cyan(),
        "stats".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} {}", "stats".cyan(), "[FLAGS]".bright_black());

    super::print_flags(&FLAGS[..VISIBLE]);

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni stats              {} Gain-focused overview",
        "#".bright_black()
    );
    println!(
        "  omni stats --since week {} The last seven days",
        "#".bright_black()
    );
    println!(
        "  omni stats --view detail {} Commands, routes and agents",
        "#".bright_black()
    );
    println!(
        "  omni stats --json       {} Machine-readable for CI/CD",
        "#".bright_black()
    );
    println!();
}

// ─── Main Entry ─────────────────────────────────────────

pub fn run(args: &[String], store: &Store) -> Result<()> {
    if super::wants_help(args) {
        print_help();
        return Ok(());
    }
    super::check_flags("stats", args, FLAGS)?;

    // `--json` is an output format, not a view: it applies to whatever view was
    // selected. `--card` is the one exception, since an image of the summary is
    // the only thing it can render.
    let mode = view(args);
    let json = super::has_flag(args, "--json");

    // `--json` is checked first for every view, which is what it did before this
    // surface existed. There is one machine-readable report and it is not per
    // view, so a view flag beside it selects nothing rather than printing a human
    // table under a machine-readable flag.
    match mode {
        _ if json => run_json(args, store),
        "card" => run_card(store),
        "share" => run_share(store),
        "rerun" => run_rerun(args, store),
        "context" => run_context_stats(store),
        "project" => run_project_stats(args, store),
        "detail" | "commands" => run_detail(args, store),
        _ => run_default(args, store),
    }
}

// ─── Context Mode: Context Composition Analyzer ────────
fn run_context_stats(store: &Store) -> Result<()> {
    println!();
    print_separator();
    println!(" {}", "OMNI Signal Report: Context".bold().bright_white());
    print_separator();

    if let Some(session) = store.find_latest_session() {
        let turn = &session.current_turn;
        println!(
            "  {:<25} {}",
            "Session ID:".bright_black(),
            session.session_id.cyan()
        );
        println!(
            "  {:<25} {}",
            "Commands (Turns):".bright_black(),
            format_number(session.command_count as u64).cyan()
        );
        // #589. This block was labelled a rough estimate because it accumulated
        // `size_bytes / 4`. It accumulates the sizes themselves now, so the
        // label goes with the estimator: a file's length and a delivered
        // payload's length are both counted, and neither needs a caveat.
        println!("\n  {}", "Context Breakdown:".bold().bright_white());
        println!(
            "    {:<25} {}",
            "File Reads:".bright_black(),
            format_bytes(turn.file_read_bytes).yellow()
        );
        println!(
            "    {:<25} {}",
            "Tool Outputs:".bright_black(),
            format_bytes(turn.tool_output_bytes).green()
        );

        let total = turn.file_read_bytes + turn.tool_output_bytes;
        println!(
            "\n  {:<27} {}",
            "Context Total:".bold().bright_white(),
            format_bytes(total).bright_cyan()
        );

        if turn.has_duplicate_file_reads {
            println!(
                "\n  {}",
                "WARNING: Duplicate File Reads Detected!"
                    .bold()
                    .bright_red()
            );
            for f in turn.duplicate_files.iter().take(5) {
                println!("    - {}", f.red());
            }
        }

        if turn.largest_single_read.1 > 0 {
            println!(
                "\n  {:<27} {} ({})",
                "Largest File Read:".bright_black(),
                turn.largest_single_read.0.cyan(),
                format_bytes(turn.largest_single_read.1).yellow()
            );
        }
    } else {
        println!("  {}", "No active session found.".bright_black().italic());
    }

    print_separator();
    println!();
    Ok(())
}

// ─── Default Mode: Gain-Focused Multi-Period ────────────

/// What both share cards are built from.
///
/// One computation, two renderers. The text card and the image card quoting
/// different numbers for the same installation would be the `omni stats` version
/// of the defect this project files issues about.
struct ShareFigures {
    saved: u64,
    pct: f64,
    unit: &'static str,
    calls: u64,
    top: Vec<(String, u64, f64, u64)>,
}

fn share_figures(store: &Store) -> Result<Option<ShareFigures>> {
    let periods = store.multi_period_stats()?;
    let Some((_, calls, input, output, raw_tok, filt_tok)) = periods
        .iter()
        .find(|(label, ..)| label == "All Time")
        .cloned()
    else {
        return Ok(None);
    };
    if calls == 0 {
        return Ok(None);
    }

    // #589. One arm, and it is the counted one. The `raw_tok` branch reported the
    // same quantity over 3.6 and made the card's unit depend on whether the rows
    // happened to carry a token column, so two installs could publish the same
    // saving under different words.
    let (saved, total) = (input.saturating_sub(output), input);
    let _ = (raw_tok, filt_tok);
    Ok(Some(ShareFigures {
        saved,
        pct: if total > 0 {
            100.0 * saved as f64 / total as f64
        } else {
            0.0
        },
        unit: "bytes",
        calls,
        top: get_top_commands(store, 0, 3),
    }))
}

/// A copy-pasteable summary of what OMNI measured on *this* installation.
///
/// Deliberately plain text and deliberately not a marketing number. It reuses
/// `multi_period_stats`, which is the same aggregation the default report reads,
/// so the figure here is the figure `omni stats` shows and cannot drift from it.
///
/// Two things it prints that a growth card usually would not, because leaving
/// them out is how a real number becomes a claim:
///
/// * the **net** all-time percentage, never a per-command peak. `kubectl
///   kustomize` reports 99.8% here and quoting that would be the cherry-pick
///   this project spends its changelog arguing against.
/// * a line saying terminal output is excluded. That exclusion is #212's fix,
///   and it is the difference between 64.5% and a headline that counted 86 MB
///   of TTY bytes no model ever read.
fn run_share(store: &Store) -> Result<()> {
    let Some(ShareFigures {
        saved,
        pct,
        unit,
        calls,
        top,
    }) = share_figures(store)?
    else {
        println!("No data yet. Run a few commands and try again.");
        return Ok(());
    };

    println!();
    // The command count is written out, not abbreviated: `6K` is a rounder
    // number than `6,253` and this card exists to show the real one.
    println!(
        "OMNI saved me {} {unit} ({pct:.1}%) across {} commands.",
        format_exact_tokens(saved),
        format_number(calls)
    );
    if !top.is_empty() {
        println!();
        let width = max_width(top.iter().map(|(cmd, ..)| cmd));
        for (cmd, count, cmd_pct, _) in &top {
            // One decimal, because `{:.0}` renders 99.8% as `100%` and a card
            // arguing that the number is real should not round one up to a
            // figure the tool never measured.
            println!("  {cmd:<width$}  {cmd_pct:>5.1}%  ({count}x)");
        }
    }
    println!();
    println!("Measured, not estimated: net across every command, terminal output excluded.");
    println!("https://github.com/fajarhide/omni");
    println!();

    Ok(())
}

/// The canvases people actually post into, and what each one is for.
///
/// Three rather than one because a 16:9 card posted to a story is letterboxed
/// into illegibility, and cropping a square to 9:16 loses the number the card
/// exists to show.
const CARD_SIZES: &[(&str, u32, u32, &str)] = &[
    ("square", 1080, 1080, "Instagram feed, LinkedIn"),
    (
        "story",
        1080,
        1920,
        "Instagram and WhatsApp stories, TikTok",
    ),
    ("wide", 1200, 675, "X, Threads, Facebook"),
];

/// SVG, not PNG, and the reason is a dependency rather than a preference.
///
/// A real renderer (`resvg` plus a font crate) is roughly 2 MB against the
/// binary-size gate in `make ci`, paid by every user for a command almost nobody
/// runs. A hand-rolled PNG encoder needs no dependency and needs a hand-written
/// bitmap font, which is a lot of code and looks it. SVG is a text format this
/// binary can already produce, it keeps real typography, and every conversion
/// tool reads it.
fn card_svg(f: &ShareFigures, w: u32, h: u32) -> String {
    // Everything scales off the short edge, so the square and the story share one
    // layout instead of a per-size table of offsets.
    let scale = w.min(h) as f64 / 1080.0;
    let px = |v: f64| v * scale;
    let pad = px(80.0);
    let right = w as f64 - pad;

    // Longest line in the card decides the body size, because the renderer will
    // not reflow and a clipped number is worse than a smaller one. Monospace is
    // close enough to 0.6 em per character to size against, and the widest line
    // is the one carrying the command count.
    let widest = format!(
        "{:.1}% of my terminal output, {} commands",
        f.pct,
        format_number(f.calls)
    )
    .chars()
    .count() as f64;
    // Floored, not rounded. Rounding the fitted size up is how a size that was
    // computed to fit stops fitting: 34.8 became 35 and put the last character
    // four pixels past the right edge.
    let body = (px(38.0)).min((right - pad) / (widest * 0.6)).floor();

    let rows = f.top.len() as f64;
    let block = px(300.0) + rows * px(56.0);
    let mut y = (h as f64 - block) / 2.0 + px(30.0);

    let mut s = String::with_capacity(2048);
    // No `letter-spacing`, and the font stack ends in a generic. ImageMagick's
    // renderer honoured neither a CSS font keyword nor the tracking, and rendered
    // a serif italic with 200px gaps between the wordmark's letters.
    s.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<rect width="{w}" height="{h}" fill="#0b0f14"/>
<rect x="0" y="0" width="{bar}" height="{h}" fill="#22d3a6"/>
<g font-family="DejaVu Sans Mono, Menlo, Consolas, monospace" font-style="normal" fill="#e6edf3">
<text x="{x}" y="{ty}" font-size="{wm}" font-weight="700" fill="#22d3a6">OMNI</text>
"##,
        bar = px(10.0).round(),
        x = pad.round(),
        ty = px(110.0).round(),
        wm = px(44.0).round()
    ));

    s.push_str(&format!(
        r##"<text x="{x}" y="{y}" font-size="{fs}" font-weight="700">{saved}</text>
"##,
        x = pad.round(),
        y = y.round(),
        fs = px(120.0).round(),
        saved = xml_escape(&format_exact_tokens(f.saved))
    ));
    y += px(70.0);
    s.push_str(&format!(
        r##"<text x="{x}" y="{y}" font-size="{fs}" fill="#8b98a5">{unit} never sent to the model</text>
"##,
        x = pad.round(),
        y = y.round(),
        fs = (body * 1.05).round(),
        unit = f.unit
    ));
    y += px(96.0);
    s.push_str(&format!(
        r##"<text x="{x}" y="{y}" font-size="{fs}">{pct:.1}% of my terminal output, {calls} commands</text>
"##,
        x = pad.round(),
        y = y.round(),
        fs = body,
        pct = f.pct,
        calls = xml_escape(&format_number(f.calls))
    ));

    // The percentage and the count are anchored to the right edge rather than to
    // a fixed offset from the left. A fixed column put `99.8%` on top of
    // `kubectl kustomize`, and command names are user data: there is no offset
    // that is safe for all of them.
    y += px(90.0);
    for (cmd, count, cmd_pct, _) in &f.top {
        s.push_str(&format!(
            r##"<text x="{x}" y="{y}" font-size="{fs}" fill="#8b98a5">{cmd}</text><text x="{xp}" y="{y}" font-size="{fs}" fill="#22d3a6" text-anchor="end">{cmd_pct:.1}%</text><text x="{xc}" y="{y}" font-size="{fs}" fill="#5c6b7a" text-anchor="end">{count}x</text>
"##,
            x = pad.round(),
            xp = (right - px(120.0)).round(),
            xc = right.round(),
            y = y.round(),
            fs = (body * 0.92).round(),
            cmd = xml_escape(cmd)
        ));
        y += px(56.0);
    }

    s.push_str(&format!(
        r##"<text x="{x}" y="{fy}" font-size="{fs}" fill="#5c6b7a">Measured, not estimated. Terminal output excluded.</text>
<text x="{x}" y="{fy2}" font-size="{fs}" fill="#5c6b7a">github.com/fajarhide/omni</text>
</g>
</svg>
"##,
        x = pad.round(),
        fy = (h as f64 - pad - px(44.0)).round(),
        fy2 = (h as f64 - pad).round(),
        fs = (body * 0.72).round()
    ));
    s
}

/// Five characters, because an SVG is XML and a command can carry any of them.
/// `sed 's/<a>/&/'` in a top-commands list would otherwise produce a file no
/// converter will open.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Converts one SVG, returning the tool that did it.
///
/// Nothing is installed and nothing is suggested for installation. If none of
/// these is already on the machine the SVG still exists and the caller prints
/// the command, which is a better outcome than a hard failure on a cosmetic
/// feature.
fn svg_to_png(svg: &std::path::Path, png: &std::path::Path, w: u32) -> Option<&'static str> {
    let attempts: [(&str, Vec<String>); 4] = [
        (
            "rsvg-convert",
            vec![
                "-w".into(),
                w.to_string(),
                "-o".into(),
                png.to_string_lossy().into_owned(),
                svg.to_string_lossy().into_owned(),
            ],
        ),
        (
            "magick",
            vec![
                "-background".into(),
                "none".into(),
                svg.to_string_lossy().into_owned(),
                png.to_string_lossy().into_owned(),
            ],
        ),
        (
            "convert",
            vec![
                "-background".into(),
                "none".into(),
                svg.to_string_lossy().into_owned(),
                png.to_string_lossy().into_owned(),
            ],
        ),
        (
            "inkscape",
            vec![
                "--export-type=png".into(),
                format!("--export-filename={}", png.to_string_lossy()),
                format!("-w{w}"),
                svg.to_string_lossy().into_owned(),
            ],
        ),
    ];

    for (tool, args) in attempts {
        let ran = std::process::Command::new(tool)
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if matches!(ran, Ok(st) if st.success()) && png.exists() {
            return Some(tool);
        }
    }
    None
}

fn run_card(store: &Store) -> Result<()> {
    let Some(figures) = share_figures(store)? else {
        println!("No data yet. Run a few commands and try again.");
        return Ok(());
    };

    let dir = crate::paths::omni_home().join("cards");
    std::fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;

    println!();
    let mut converted = None;
    for (name, w, h, used_for) in CARD_SIZES {
        let svg_path = dir.join(format!("omni-{name}.svg"));
        std::fs::write(&svg_path, card_svg(&figures, *w, *h))
            .with_context(|| format!("failed to write {}", svg_path.display()))?;

        let png_path = dir.join(format!("omni-{name}.png"));
        let tool = svg_to_png(&svg_path, &png_path, *w);
        converted = converted.or(tool);
        let made = if tool.is_some() { "svg + png" } else { "svg" };
        println!(
            "  {:<9} {w}x{h}  {made:<9}  {}",
            name.bright_white(),
            used_for.bright_black()
        );
    }

    println!("\n  {}", dir.display().to_string().bright_cyan());
    match converted {
        Some(tool) => println!("  PNG rendered with {}", tool.bright_white()),
        None => {
            println!(
                "  {}",
                "No SVG converter found, so only the SVGs were written. Any of these renders them:"
                    .bright_black()
            );
            println!(
                "    rsvg-convert -w 1080 {}/omni-square.svg > {}/omni-square.png",
                dir.display(),
                dir.display()
            );
        }
    }
    // The card carries the user's own top commands, which on a work machine can
    // be the one thing on it that should not be public.
    println!(
        "\n  {}",
        "The card names your top commands. Read it before you post it.".yellow()
    );
    println!();

    Ok(())
}

fn run_default(args: &[String], store: &Store) -> Result<()> {
    let (period_label, since) = scope(args);
    let totals = store.stage_totals(since)?;
    let removed = totals.distilled_removed + totals.folded_bytes;
    let calls = totals.distilled_calls + totals.folded_calls + totals.passthrough_calls;

    println!();
    print_separator();
    println!(
        " {} {}{:>width$}",
        format!("OMNI {}", env!("CARGO_PKG_VERSION"))
            .bold()
            .bright_white(),
        "· savings".bright_black(),
        format!("{period_label} · {} calls", format_number(calls)).bright_black(),
        width = SUMMARY_WIDTH.saturating_sub(24)
    );
    print_separator();

    if calls == 0 {
        println!(
            "  {}",
            "No data yet. OMNI records savings as you work."
                .bright_black()
                .italic()
        );
        print_separator();
        println!();
        return Ok(());
    }

    // The headline says what happened to the bytes, never that they were "saved":
    // a currency figure is not computable here (#589's `est_cost_usd` lesson), the
    // marker costs a few bytes back, and a pulled handle returns some of them.
    // "Not sent" is checkable against the host's own transcript, line by line.
    println!(
        "\n  {} {}   {}  {}",
        format_bytes(removed).bold().yellow(),
        "not sent to the model".bright_white(),
        sparkline(&store.daily_removed_bytes(14), 14).cyan(),
        "last 14 days".bright_black()
    );
    println!();

    // Two stages, two bases, printed apart. The ledger's base exists only for rows
    // written since `payload_bytes` landed, so its share is omitted rather than
    // computed over a population the column does not cover (#665).
    let mut shares = false;
    if totals.folded_calls > 0 {
        shares |= print_stage_row(
            "folded",
            totals.folded_bytes,
            share(totals.folded_priced, totals.folded_payload),
            totals.folded_calls,
        );
    }
    if totals.distilled_calls > 0 {
        shares |= print_stage_row(
            "distilled",
            totals.distilled_removed,
            share(totals.distilled_removed, totals.distilled_input),
            totals.distilled_calls,
        );
    }
    if shares {
        println!(
            "    {}",
            "each % is of that stage's own bytes"
                .bright_black()
                .italic()
        );
    }

    // A no-op is a feature, so it reads as one. It left the table because a row
    // whose only honest values are `0` and a dash draws the eye to the least
    // interesting line on the screen.
    if totals.passthrough_calls > 0 {
        println!(
            "\n    {} {}",
            format!("{} calls", format_number(totals.passthrough_calls)).bright_white(),
            "passed through untouched. Nothing deleted, nothing".bright_black()
        );
        println!(
            "    {}",
            "invented, no call came back larger.".bright_black()
        );
    }

    print_separator();
    println!(
        "  {} for commands, routes and agents",
        "omni stats --view commands".bright_cyan()
    );

    if let Some(latest) = crate::guard::update::check() {
        crate::guard::update::print_notification(&latest);
    }

    println!();
    Ok(())
}

/// Width the summary aligns its right-hand scope label to.
const SUMMARY_WIDTH: usize = 74;

/// One stage row, right-aligned so two rows read as one table whatever units the
/// numbers land in. Returns whether it printed a share, so the footnote about
/// denominators appears only when there is a percentage to explain.
fn print_stage_row(label: &str, bytes: u64, share: Option<f64>, calls: u64) -> bool {
    let pct = match share {
        Some(p) => format!("{p:>3.0}%"),
        None => "   -".to_string(),
    };
    println!(
        "    {:<11} {:>8}  {}   {:>6} calls",
        label.bright_white(),
        format_bytes(bytes).yellow(),
        pct.cyan(),
        format_number(calls).bright_black()
    );
    share.is_some()
}

/// A stage's share of its own bytes, or `None` when the base is unknown.
fn share(part: u64, whole: u64) -> Option<f64> {
    (whole > 0).then(|| 100.0 * part as f64 / whole as f64)
}

/// One column per day, scaled to the busiest day in the window.
///
/// A day with no recorded call renders blank rather than at the floor, because
/// `▁` on an idle day claims activity that did not happen. The glyphs are the
/// ones the bars already use, so this adds no rendering assumption.
fn sparkline(daily: &[(String, u64)], days: usize) -> String {
    const GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = daily.iter().map(|(_, b)| *b).max().unwrap_or(0);
    if max == 0 {
        return " ".repeat(days);
    }
    let today = chrono::Utc::now().date_naive();
    (0..days)
        .map(|i| {
            let day = today - chrono::Duration::days((days - 1 - i) as i64);
            let key = day.format("%Y-%m-%d").to_string();
            match daily.iter().find(|(d, _)| *d == key) {
                Some((_, b)) => GLYPHS[((*b as u128 * 8 / max as u128) as usize).min(7)],
                None => ' ',
            }
        })
        .collect()
}

// ─── Detail Mode: Current View (Improved) ───────────────

fn run_detail(args: &[String], store: &Store) -> Result<()> {
    let (period_label, since) = scope(args);

    let (count, input_total, output_total, sum_latency, _max_latency, raw_tokens, filtered_tokens) =
        store.aggregate_stats(since)?;
    let reduction_pct = if raw_tokens > 0 {
        100.0 * (1.0 - filtered_tokens as f64 / raw_tokens as f64)
    } else if input_total > 0 {
        100.0 * (1.0 - output_total as f64 / input_total as f64)
    } else {
        0.0
    };
    let avg_latency = if count > 0 {
        sum_latency as f64 / count as f64
    } else {
        0.0
    };
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics(since)?;

    println!();
    print_separator();
    println!(
        " {}",
        format!("OMNI Signal Report: Detail ({})", period_label.bold()).bright_white()
    );
    print_separator();

    // Session lifetime lives here since #665 moved the summary to one screen. It
    // is the meter #357 promoted and it answers a question about the window, not
    // about a call, so it belongs beside the rest of the technical breakdown.
    let (sessions, median_cmds, longest, compacted) = store.session_lifetime(since);
    if sessions > 0 {
        println!(
            "  {:<20} {} median, {} longest, {} closed",
            "Session lifetime:".bright_black(),
            median_cmds.to_string().bold().cyan(),
            longest.to_string().cyan(),
            format_number(sessions).cyan()
        );
        if compacted > 0 {
            println!(
                "  {}",
                format!("  {compacted} ended at a compaction, which is what the window costs")
                    .bright_black()
                    .italic()
            );
        }
    }

    println!(
        "  {:<20} {}",
        "Commands processed:".bright_black(),
        format_number(count).bold().cyan()
    );
    println!(
        "  {:<20} {} {} {}",
        "Data Distilled:".bright_black(),
        format_bytes(input_total).red(),
        "→".bright_black(),
        format_bytes(output_total).green()
    );

    // #589. `Tokens Reduced` used to sit here and was the line above divided by
    // 3.6, a constant calibrated against `cl100k_base`. It added no information
    // that `Data Distilled` did not already state exactly, and it stated it in a
    // unit we cannot defend, so it is gone rather than relabelled.

    let ratio_msg = format!("{:.1}% reduction", reduction_pct);
    let ratio_colored = if reduction_pct > 70.0 {
        ratio_msg.bold().bright_green()
    } else if reduction_pct > 40.0 {
        ratio_msg.bold().bright_yellow()
    } else {
        ratio_msg.bold().bright_red()
    };
    println!("  {:<20} {}", "Signal Ratio:".bright_black(), ratio_colored);
    println!(
        "  {:<20} {}",
        "Average Latency:".bright_black(),
        format!("{:.1}ms", avg_latency).bright_blue()
    );
    println!(
        "  {:<20} {}",
        "RewindStore:".bright_black(),
        format!(
            "{} archived / {} retrieved",
            rewind_stored, rewind_retrieved
        )
        .bright_magenta()
    );

    // Collapse savings
    let collapse_stats = store.collapse_aggregate(since);
    if let Ok((events, total_original, total_collapsed)) = collapse_stats
        && events > 0
    {
        println!(
            "  {:<20} {}",
            "Collapse:".bright_black(),
            format!(
                "{} → {} lines across {} events",
                format_number(total_original),
                format_number(total_collapsed),
                events
            )
            .bright_green()
        );
    }

    // By Command, top 10 (or all if requested), filter 0% savings
    let raw_filters = store.filter_breakdown(since)?;
    // `--limit 0` and the older `--all-commands` say the same thing.
    let limit = super::flag_value(args, "--limit").and_then(|v| v.parse::<usize>().ok());
    let all_flag = super::has_flag(args, "--all-commands") || limit == Some(0);
    let grouped_filters = group_and_calculate_stats(raw_filters, 0);

    let display_filters: Vec<_> = if all_flag {
        grouped_filters.clone()
    } else {
        grouped_filters
            .iter()
            .filter(|(_, _, pct, _)| *pct > 0.0)
            .take(limit.unwrap_or(10))
            .cloned()
            .collect()
    };

    // Per-command with agent info
    let cmd_agent_data = store.get_per_command_with_agent(since).unwrap_or_default();
    let mut cmd_agent_counts: HashMap<String, HashMap<String, u64>> = HashMap::new();
    for (cmd, agent_id, calls, _, _) in &cmd_agent_data {
        // `group_and_calculate_stats` keys the rows this table displays with
        // `shorten_command(cmd, 18)`. Keying the agent map at 19 built a second
        // namespace: every command whose two-token prefix ran past 18 cut at a
        // different place on each side, the lookup below missed, and the miss
        // was reported as a fact (#471).
        let key = shorten_command(cmd, CMD_KEY_WIDTH);
        let entry = cmd_agent_counts.entry(key).or_default();
        *entry.entry(agent_id.clone()).or_insert(0) += *calls;
    }

    if !display_filters.is_empty() {
        println!("\n {}", "By Command:".bold().bright_white());
        println!(
            "  {:>3} {:<w_cmd$} {:<11} {:>5} {:>6} {:>6} {}",
            "#".bright_black(),
            "CLI".bright_black(),
            "Agent".bright_black(),
            "Count".bright_black(),
            "Saved".bright_black(),
            "Saved".bright_black(),
            "Signal".bright_black(),
            w_cmd = CMD_KEY_WIDTH
        );
        println!(
            "  {}",
            super::column_rule(&[3, CMD_KEY_WIDTH, 11, 5, 6, 6, DETAIL_BAR]).bright_black()
        );

        for (i, (name, cnt, pct, bytes_saved)) in display_filters.iter().enumerate() {
            let bar = format_bar(*pct, DETAIL_BAR);
            let bar_colored = if *pct > 80.0 {
                bar.bright_green()
            } else {
                bar.bright_yellow()
            };
            let suffix = if *name == "passthrough" || *name == "unknown" {
                " ← learn?".bright_black().italic()
            } else {
                "".clear()
            };

            // Look up by the key, render from the key. These were one variable,
            // which is how #471 happened and how it nearly shipped twice: fitting
            // `cat package.json` into the column produced `cat package.jso...`,
            // and searching the agent map for *that* missed a key that was
            // sitting right there. What is displayed is never what is looked up.
            let agent_label = cmd_agent_counts
                .get(name.as_str())
                .and_then(|agents| agents.iter().max_by_key(|(_, calls)| *calls))
                .map(|(agent_id, _)| agent_display_name(agent_id))
                // `Unknown`, never `Terminal`: a lookup that missed has not
                // established that a human ran this in a shell, and `:202`
                // already says why those two facts must not be folded (#471).
                .unwrap_or("Unknown");

            let tokens_str = if *bytes_saved > 0 {
                format!("-{}", format_bytes(*bytes_saved))
            } else {
                String::new()
            };

            let display_name =
                crate::util::text::display_truncate_with_ellipsis(name, CMD_KEY_WIDTH - 3);

            println!(
                "  {:>2}. {:<w_cmd$} {:<11} {:>4}x {:>5.1}% {:>6} {:<w_bar$}{}",
                i + 1,
                display_name.bright_cyan(),
                agent_label.bright_blue(),
                cnt,
                pct,
                tokens_str.bright_magenta(),
                bar_colored,
                suffix,
                w_cmd = CMD_KEY_WIDTH,
                w_bar = DETAIL_BAR
            );
        }

        if !all_flag {
            let filtered_count = grouped_filters
                .iter()
                .filter(|(_, _, pct, _)| *pct > 0.0)
                .count();
            let hidden_zero = grouped_filters.len() - filtered_count;

            // One footnote, not two. Both named `--all-commands`, and together
            // they ran past the frame they sit inside (#463).
            if filtered_count > 10 || hidden_zero > 0 {
                let hidden = if hidden_zero > 0 {
                    format!(", {hidden_zero} at 0% hidden")
                } else {
                    String::new()
                };
                println!(
                    "\n   {}",
                    format!(
                        "Top 10 of {filtered_count} with savings{hidden}. --all-commands shows all"
                    )
                    .bright_black()
                    .italic()
                );
            }
        }
    }

    // Route distribution
    let routes = store.route_distribution(since)?;
    if !routes.is_empty() {
        let total_routes: u64 = routes.iter().map(|(_, c)| c).sum();
        println!("\n {}", "Route Distribution:".bold().bright_white());
        for (route, cnt) in &routes {
            let pct = if total_routes > 0 {
                *cnt as f64 / total_routes as f64 * 100.0
            } else {
                0.0
            };
            let route_color = match route.to_lowercase().as_str() {
                "keep" => route.bright_green(),
                "rewind" => route.bright_blue(),
                "soft" => route.bright_yellow(),
                "drop" | "passthrough" => route.bright_red(),
                _ => route.bright_black(),
            };

            let label = format!("{}:", route);
            let padding = " ".repeat(15_usize.saturating_sub(label.len()));

            println!(
                "  {}{}{}  ({:>2.0}%)",
                route_color.bold(),
                ":".bright_white().to_string() + &padding,
                cnt,
                pct
            );
        }
    }

    // Agent Distribution
    let agent_data = store.get_agent_breakdown(since).unwrap_or_default();

    // Group by display name
    let mut grouped_agents: HashMap<String, (u64, u64, u64)> = HashMap::new();
    // #163: kept beside the totals, not folded into them.
    let mut grouped_unverified: HashMap<String, u64> = HashMap::new();
    for r in &agent_data {
        if r.agent_id == "unknown" || r.agent_id == "terminal" || r.agent_id.is_empty() {
            continue;
        }
        let name = agent_display_name(&r.agent_id).to_string();
        let entry = grouped_agents.entry(name.clone()).or_insert((0, 0, 0));
        entry.0 += r.calls;
        entry.1 += r.input_bytes;
        entry.2 += r.output_bytes;
        *grouped_unverified.entry(name).or_insert(0) += r.unverified;
    }

    if !grouped_agents.is_empty() {
        let total_cmds: u64 = agent_data.iter().map(|r| r.calls).sum();
        println!("\n {}", "Agent Distribution:".bold().bright_white());
        // Number then bar, the order By Command already uses. Padding the bar
        // and right-aligning the percentage after it left a hole across the row
        // whenever savings were low, which is most rows.
        println!(
            "  {:<16} {:>6} {:>7} {:>6} {}",
            "Agent".bright_black(),
            "Count".bright_black(),
            "Share".bright_black(),
            "Saved".bright_black(),
            "Signal".bright_black()
        );
        // Five groups under a five-column header. It carried five under *four*,
        // because the leading `──` was copied from the By Command table's `#`
        // column, leaving a 56-column rule under a 43-column header (#463).
        println!(
            "  {}",
            super::column_rule(&[16, 6, 7, 6, DETAIL_BAR]).bright_black()
        );

        let mut sorted_agents: Vec<_> = grouped_agents.into_iter().collect();
        sorted_agents.sort_by_key(|a| std::cmp::Reverse(a.1.0));

        for (name, (count, input, output)) in sorted_agents {
            let pct = if total_cmds > 0 {
                count as f64 / total_cmds as f64 * 100.0
            } else {
                0.0
            };
            let savings = if input > 0 {
                100.0 * (1.0 - output as f64 / input as f64)
            } else {
                0.0
            };
            let bar = format_bar(savings, DETAIL_BAR);
            let bar_colored = if savings > 80.0 {
                bar.bright_green()
            } else if savings > 40.0 {
                bar.bright_yellow()
            } else {
                bar.bright_red()
            };
            println!(
                "  {:<16} {:>5}x {:>6.1}% {:>5.1}% {}",
                name.bright_cyan(),
                count,
                pct,
                savings,
                bar_colored,
            );
            // #163: the excluded rows are named, not silently missing. A count
            // that shrinks without explanation reads as OMNI having stopped
            // working; this says what was set aside and why.
            if let Some(&u) = grouped_unverified.get(&name)
                && u > 0
            {
                println!(
                    "  {:<16} {:>5}x {}",
                    "".bright_black(),
                    u,
                    "not counted, never applied (#158), or read at a terminal (#212)"
                        .bright_black()
                );
            }
        }
    }

    // Session insights, always shown in detail mode
    let hot_files = store.hot_files_global(since)?;
    if !hot_files.is_empty() {
        println!("\n {}", "Session Insights:".bold().bright_white());
        let files_str: Vec<String> = hot_files
            .iter()
            .take(3)
            .map(|(f, c)| format!("{} ({})", f.bright_cyan(), c.to_string().bright_black()))
            .collect();
        println!("  Hot files:  {}", files_str.join(", "));
    }

    print_separator();
    println!();
    Ok(())
}

// ─── JSON Mode: Machine-Readable ────────────────────────

/// One stage as the machine-readable surface reports it.
///
/// `base` of zero means the store does not record one for every row of this
/// stage, which is the ledger's situation before `payload_bytes` existed. It
/// reports `null` rather than a share computed over the rows that happen to carry
/// one, because that share would be presented as the stage's own (#665).
fn stage_stat(calls: u64, removed: u64, base: u64, priced: u64) -> StageStat {
    StageStat {
        calls,
        bytes_removed: removed,
        base_bytes: (base > 0).then_some(base),
        share_pct: (base > 0).then(|| (1000.0 * priced as f64 / base as f64).round() / 10.0),
    }
}

#[derive(serde::Serialize)]
pub struct StatsJson {
    pub version: String,
    pub generated_at: i64,
    pub periods: Vec<StatsPeriod>,
    pub commands: Vec<CommandStat>,
    pub agents: Vec<AgentStat>,
    pub rewind: RewindStat,
    pub avg_latency_ms: f64,
    /// The two stages, side by side and never averaged (#665). `periods` above
    /// counts the distiller alone, which is what made the ledger invisible to
    /// anything reading this.
    pub stages: StageStats,
}

/// What each stage took off, with the base each ratio is over.
///
/// Two objects rather than one summed pair, so a consumer cannot produce the
/// combined percentage this project refuses to publish: the ledger's base exists
/// only for folds recorded since `payload_bytes` landed, and the folded calls are
/// largely absent from `distillations`, so a union denominator is not available.
#[derive(serde::Serialize)]
pub struct StageStats {
    pub distilled: StageStat,
    pub folded: StageStat,
    pub passed_through_calls: u64,
}

#[derive(serde::Serialize)]
pub struct StageStat {
    pub calls: u64,
    pub bytes_removed: u64,
    /// Bytes this stage saw for the calls it acted on, or `null` when the store
    /// does not record one for every row.
    pub base_bytes: Option<u64>,
    /// `bytes_removed` over `base_bytes`, absent for the same reason.
    pub share_pct: Option<f64>,
}

#[derive(serde::Serialize)]
pub struct StatsPeriod {
    pub label: String,
    pub commands: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub savings_pct: f64,
    pub measurement_method: String,
}

#[derive(serde::Serialize)]
pub struct CommandStat {
    pub command: String,
    pub count: u64,
    pub savings_pct: f64,
    /// Bytes, and named for it since #589. It briefly held bytes under the old
    /// name, which is a machine-readable surface asserting the wrong unit, and
    /// that is the defect this issue is about rather than a cosmetic one.
    pub bytes_saved: u64,
}

#[derive(serde::Serialize)]
pub struct AgentStat {
    pub agent: String,
    pub agent_id: String,
    /// Calls whose distillation reached the agent. `savings_pct` covers these.
    pub count: u64,
    pub savings_pct: f64,
    /// Calls excluded from `count` and `savings_pct`: recorded before the #158
    /// fix on a path where the host discarded the output (#163). Reported so a
    /// consumer of this JSON can see the correction rather than infer a drop.
    pub unverified: u64,
}

#[derive(serde::Serialize)]
pub struct RewindStat {
    pub archived: u64,
    pub retrieved: u64,
}

fn run_json(args: &[String], store: &Store) -> Result<()> {
    let (_, since) = scope(args);
    println!(
        "{}",
        serde_json::to_string_pretty(&build_stats_json(store, since)?)?
    );
    Ok(())
}

/// The machine-readable report for one window.
///
/// Separated from printing so the window can be tested. Review of #665 found
/// every query here reading all-time whatever `--since` said, and a function that
/// only prints cannot be asked what window it used.
fn build_stats_json(store: &Store, since: i64) -> Result<StatsJson> {
    // `periods` is the exception to the window by definition: it *is* the three
    // standard windows, and scoping it would leave two of its rows describing a
    // window they are not named after.
    let periods = store.multi_period_stats()?;
    let top_commands = get_top_commands(store, since, 100);
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics(since)?;
    let (count, _, _, sum_latency, _, _, _) = store.aggregate_stats(since)?;

    let avg_latency = if count > 0 {
        sum_latency as f64 / count as f64
    } else {
        0.0
    };

    let totals = store.stage_totals(since)?;
    let stages = StageStats {
        distilled: stage_stat(
            totals.distilled_calls,
            totals.distilled_removed,
            totals.distilled_input,
            totals.distilled_removed,
        ),
        folded: stage_stat(
            totals.folded_calls,
            totals.folded_bytes,
            totals.folded_payload,
            totals.folded_priced,
        ),
        passed_through_calls: totals.passthrough_calls,
    };

    let periods_json: Vec<StatsPeriod> = periods
        .iter()
        .map(
            |(label, count, input, output, raw_tokens, filtered_tokens)| {
                let savings_pct = if *raw_tokens > 0 {
                    (100.0 * (1.0 - *filtered_tokens as f64 / *raw_tokens as f64) * 10.0).round()
                        / 10.0
                } else if *input > 0 {
                    (100.0 * (1.0 - *output as f64 / *input as f64) * 10.0).round() / 10.0
                } else {
                    0.0
                };
                StatsPeriod {
                    label: label.to_lowercase().replace(' ', "_"),
                    commands: *count,
                    input_tokens: *raw_tokens,
                    output_tokens: *filtered_tokens,
                    savings_pct,
                    measurement_method: if *raw_tokens > 0 {
                        "actual".to_string()
                    } else {
                        "estimated".to_string()
                    },
                }
            },
        )
        .collect();

    let commands_json: Vec<CommandStat> = top_commands
        .iter()
        .map(|(cmd, count, pct, bytes_saved)| CommandStat {
            command: cmd.clone(),
            count: *count,
            savings_pct: *pct,
            bytes_saved: *bytes_saved,
        })
        .collect();

    let agent_json: Vec<AgentStat> = store
        .get_agent_breakdown(since)
        .unwrap_or_default()
        .iter()
        .map(|r| {
            let savings = if r.input_bytes > 0 {
                (100.0 * (1.0 - r.output_bytes as f64 / r.input_bytes as f64) * 10.0).round() / 10.0
            } else {
                0.0
            };
            AgentStat {
                agent: agent_display_name(&r.agent_id).to_string(),
                agent_id: r.agent_id.clone(),
                count: r.calls,
                savings_pct: savings,
                unverified: r.unverified,
            }
        })
        .collect();

    let output = StatsJson {
        version: "1".to_string(),
        generated_at: chrono::Utc::now().timestamp(),
        periods: periods_json,
        commands: commands_json,
        agents: agent_json,
        rewind: RewindStat {
            archived: rewind_stored,
            retrieved: rewind_retrieved,
        },
        avg_latency_ms: (avg_latency * 10.0).round() / 10.0,
        stages,
    };

    Ok(output)
}

/// `omni stats --rerun`, the check reduction % cannot make (#109).
///
/// Reduction measures bytes removed. A distiller that emitted `""` for every
/// input would score 100%. This measures whether the agent had to run the
/// command again, which is the closest thing to ground truth on whether the
/// bytes removed were the ones it needed.
fn run_rerun(args: &[String], store: &Store) -> Result<()> {
    let (period_label, since) = scope(args);
    let rows = store.rerun_breakdown(since)?;

    println!(
        "\n  {}, {}",
        "OMNI Re-run Analysis".bold().bright_white(),
        period_label
    );
    print_separator();

    if rows.is_empty() {
        println!(
            "  Not enough paired data yet: a filter needs {} distilled and {} raw",
            crate::pipeline::RERUN_MIN_SAMPLES,
            crate::pipeline::RERUN_MIN_SAMPLES
        );
        println!("  runs in this window before its delta means anything.");
        return Ok(());
    }

    println!(
        " {:<22} {:>9} {:>9} {:>8}",
        "Filter", "distilled", "raw", "delta"
    );
    println!(" {:─<22} ───────── ───────── ────────", "");

    let mut confounded = Vec::new();
    for r in &rows {
        let delta = r.delta_pp();
        let label = format!("{:+.1}pp", delta);
        // Only a *comparable* pair earns a verdict. A skewed one prints its
        // numbers and is sent to the caveat list, never coloured as a finding.
        let shown = if r.is_confounded() {
            confounded.push(r);
            "  n/a".normal()
        } else if delta > 10.0 {
            label.bright_red()
        } else if delta > 3.0 {
            label.bright_yellow()
        } else {
            label.bright_green()
        };
        println!(
            " {:<22} {:>8.1}% {:>8.1}% {:>8}",
            crate::util::text::safe_truncate_with_ellipsis(&r.filter_name, 22),
            r.distilled_pct(),
            r.raw_pct(),
            shown
        );
    }

    println!();
    println!(
        "  {} a command re-run within {}s of reading its distilled output.",
        "delta =".bright_black(),
        crate::pipeline::RERUN_WINDOW_SECS
    );
    println!(
        "  {} distillation removed something the agent needed.",
        "positive =".bright_black()
    );

    if !confounded.is_empty() {
        println!();
        println!(
            "  {} the two arms are not the same population, so the",
            "n/a:".bold().bright_yellow()
        );
        println!("  comparison measures input size, not lost signal:");
        for r in confounded {
            println!(
                "    {:<20} {} B distilled vs {} B raw",
                crate::util::text::safe_truncate_with_ellipsis(&r.filter_name, 20),
                r.distilled_avg_input,
                r.raw_avg_input
            );
        }
    }

    println!();
    Ok(())
}

fn run_project_stats(args: &[String], store: &Store) -> Result<()> {
    let (period_label, since) = scope(args);

    let projects = store.get_project_stats(since)?;
    println!(
        "\n  {}, {} Breakdown",
        "OMNI Project Analytics".bold().bright_white(),
        period_label
    );
    print_separator();

    if projects.is_empty() {
        println!("  No project data recorded yet for this period.");
        return Ok(());
    }

    println!(
        " {:<28} {:>9} {:>10}  Signal Strength",
        "Project Directory", "Count", "Savings"
    );
    println!(" {:─<32} ─────── ───────── ────────────────────", "");

    for (path, count, savings) in projects {
        let display_path = if path.chars().count() > 30 {
            let mut s: String = path.chars().take(12).collect();
            s.push_str("...");
            s.extend(
                path.chars()
                    .rev()
                    .take(15)
                    .collect::<String>()
                    .chars()
                    .rev(),
            );
            s
        } else {
            path
        };

        let bar = format_bar(savings, 20);
        let bar_colored = if savings > 80.0 {
            bar.bright_green()
        } else if savings > 40.0 {
            bar.bright_yellow()
        } else {
            bar.bright_red()
        };

        println!(
            " {:<28} {:>8}x  {:>7.1}%  {}",
            display_path.cyan(),
            count,
            savings,
            bar_colored
        );
    }
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn card_figures(top: Vec<(String, u64, f64, u64)>) -> ShareFigures {
        ShareFigures {
            saved: 5_500_000,
            pct: 54.2,
            unit: "tokens",
            calls: 12_624,
            top,
        }
    }

    /// A card is an XML document built from the user's own command strings, and
    /// `sh -c 'a && b > c'` carries three characters that end it. An unescaped
    /// one produces a file no converter will open, which reads as "the feature is
    /// broken" rather than "that command has an ampersand in it".
    #[test]
    fn escapes_command_names_that_would_break_the_document() {
        let f = card_figures(vec![("sh -c 'a && b>c'".to_string(), 3, 40.0, 100)]);

        let svg = card_svg(&f, 1080, 1080);

        assert!(svg.contains("&amp;&amp;"), "unescaped ampersand: {svg}");
        assert!(!svg.contains("b>c"), "unescaped angle bracket: {svg}");
        assert!(svg.contains("&apos;"), "unescaped apostrophe: {svg}");
    }

    /// The card exists to show one number. Every size has to carry it, and the
    /// long line has to stay inside the canvas: the renderer does not reflow, so
    /// a body size that overflows is a silently clipped card.
    #[test]
    fn every_size_carries_the_figures_inside_its_canvas() {
        let f = card_figures(vec![("cargo test".to_string(), 54, 95.1, 900)]);

        for (_, w, h, _) in CARD_SIZES {
            let svg = card_svg(&f, *w, *h);
            assert!(svg.contains("5.5M"), "{w}x{h} lost the headline: {svg}");
            assert!(svg.contains("12,624"), "{w}x{h} lost the call count");
            assert!(svg.contains("cargo test"), "{w}x{h} lost the top command");
            // The widest line is the one sized against, so its right edge is the
            // one that can overflow.
            let widest = format!("{:.1}% of my terminal output, {} commands", f.pct, "12,624")
                .chars()
                .count() as f64;
            let body = svg
                .split("commands</text>")
                .next()
                .and_then(|s| s.rsplit("font-size=\"").next())
                .and_then(|s| s.split('"').next())
                .and_then(|s| s.parse::<f64>().ok())
                .expect("a body font size");
            let pad = *w.min(h) as f64 / 1080.0 * 80.0;
            assert!(
                pad + widest * body * 0.6 <= *w as f64 - pad + 1.0,
                "{w}x{h} overflows: {body}px body over {widest} chars"
            );
        }
    }

    /// #589, the last of it. The context breakdown accumulated `size_bytes / 4`
    /// and was labelled a rough estimate because of it. It counts the sizes now,
    /// so `format_exact_tokens` must not come back to this block: a file length
    /// is measured, and dividing it by four to call the result tokens is the
    /// defect this issue is named for.
    #[test]
    fn the_context_breakdown_reports_the_sizes_it_counted() {
        let turn = crate::analytics::context_composition::ContextTurn {
            file_read_bytes: 137_000,
            tool_output_bytes: 74_000,
            ..Default::default()
        };

        // 137,000 B is 133.8 KB and 74,000 B is 72.3 KB by `format_bytes`,
        // written out from its rules rather than by calling it.
        assert_eq!(format_bytes(turn.file_read_bytes), "133.8 KB");
        assert_eq!(format_bytes(turn.tool_output_bytes), "72.3 KB");
        assert_ne!(
            format_bytes(turn.file_read_bytes),
            format_exact_tokens(turn.file_read_bytes / 4),
            "the breakdown went back to a quarter of a file's size called tokens"
        );
    }

    /// #589. The alignment test beside this one does not guard the unit: it
    /// stayed green with the byte formatter replaced by raw digits, which is how
    /// this hole was found. The period summary used to print a byte count over

    #[test]
    fn test_format_bytes_handles_all_ranges() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_stats_default_does_not_crash_when_db_is_empty() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    /// #428. `-d` sits next to `--detail` and used to have no long form that
    /// shared its letter, so the family read `--today/-d`, `--week/-w`,
    /// `--month/-m`. `--day` closes that; both spellings must pick one window.
    #[test]
    fn day_and_today_select_the_same_window() {
        let day = scope(&["--day".to_string()]);
        let today = scope(&["--today".to_string()]);
        let short = scope(&["-d".to_string()]);

        assert_eq!(day.0, today.0, "--day and --today must name one window");
        assert_eq!(short.0, today.0, "-d must stay the short form of today");
        assert_eq!(day.0, "today");
        // `--detail` is a view, not a window, so it must not move the scope off
        // the default. This is the collision the issue was actually about.
        assert_eq!(scope(&["--detail".to_string()]).0, "last 30 days");
    }

    #[test]
    fn test_stats_detail_does_not_crash_when_db_is_empty() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into(), "--detail".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stats_json_does_not_crash_when_db_is_empty() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into(), "--json".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    /// The agent map is keyed by `shorten_command(cmd, CMD_KEY_WIDTH)`, and the
    /// cell is that key cut again to fit the column. They are different strings
    /// for any name that fills the column, so looking the agent up by what is
    /// on screen misses a key that is present. That is #471, and writing the fix
    /// reintroduced it once: `cat package.json` is 16 characters, survives the
    /// key intact, renders as `cat package.jso...`, and the row came back

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1247000), "1,247,000");
    }

    #[test]
    fn test_stats_json_schema_validation() {
        let json_struct = StatsJson {
            version: "1".to_string(),
            generated_at: 1234567890,
            periods: vec![StatsPeriod {
                label: "All Time".to_string(),
                commands: 10,
                input_tokens: 10000,
                output_tokens: 1000,
                savings_pct: 90.0,
                measurement_method: "test".to_string(),
            }],
            commands: vec![],
            agents: vec![],
            rewind: RewindStat {
                archived: 100,
                retrieved: 5,
            },
            avg_latency_ms: 15.5,
            stages: StageStats {
                distilled: StageStat {
                    calls: 3,
                    bytes_removed: 900,
                    base_bytes: Some(1000),
                    share_pct: Some(90.0),
                },
                folded: StageStat {
                    calls: 2,
                    bytes_removed: 400,
                    base_bytes: None,
                    share_pct: None,
                },
                passed_through_calls: 5,
            },
        };

        let json_str = serde_json::to_string(&json_struct).unwrap();
        assert!(json_str.contains("\"version\":\"1\""));
        assert!(json_str.contains("\"generated_at\":1234567890"));
        assert!(json_str.contains("\"savings_pct\":90.0"));
        assert!(json_str.contains("\"avg_latency_ms\":15.5"));
    }

    /// Review of #665. Every query in the JSON report read all-time whatever
    /// `--since` asked for, so `omni stats --since today --json` answered about
    /// the whole database. A window in the future must therefore come back empty:
    /// a hardcoded `0` returns the rows instead.
    #[test]
    fn the_json_report_honours_its_window() {
        use crate::pipeline::{DistillResult, Route};

        let dir = tempfile::tempdir().expect("tempdir");
        let store = Store::open_path(&dir.path().join("omni.db")).expect("store");
        store.record_distillation(
            "s1",
            &DistillResult {
                output: String::new(),
                route: Route::Keep,
                filter_name: "cat".to_string(),
                score: 0.0,
                context_score: 0.0,
                input_bytes: 1_000,
                output_bytes: 200,
                latency_ms: 1,
                rewind_hash: None,
                segments_kept: 0,
                segments_dropped: 0,
                collapse_savings: None,
                raw_tokens: 0,
                filtered_tokens: 0,
                delivered_bytes: 200,
            },
            "cat a.txt",
            "",
            "claude_code",
        );

        let all_time = build_stats_json(&store, 0).expect("json");
        assert_eq!(all_time.stages.distilled.calls, 1);

        let future = chrono::Utc::now().timestamp() + 3_600;
        let empty = build_stats_json(&store, future).expect("json");
        assert_eq!(
            empty.stages.distilled.calls, 0,
            "the report ignored its window and answered about the whole database"
        );
        assert_eq!(empty.stages.distilled.bytes_removed, 0);
    }

    /// #665. A day with no recorded call has to render blank. `▁` is the floor of
    /// a scale, so drawing it for an idle day claims activity that did not happen,
    /// and a reader cannot tell the two apart afterwards.
    #[test]
    fn an_idle_day_is_blank_and_the_busiest_day_is_full() {
        let today = chrono::Utc::now().date_naive();
        let day = |back: i64| {
            (today - chrono::Duration::days(back))
                .format("%Y-%m-%d")
                .to_string()
        };

        let line = sparkline(&[(day(3), 10), (day(1), 400)], 4);

        let cells: Vec<char> = line.chars().collect();
        assert_eq!(cells.len(), 4, "one column per day: {line:?}");
        assert_eq!(cells[0], '▁', "the quiet day sits at the floor: {line:?}");
        assert_eq!(cells[1], ' ', "a day with no data is blank: {line:?}");
        assert_eq!(cells[2], '█', "the busiest day fills the cell: {line:?}");
        assert_eq!(cells[3], ' ', "today has no rows in this fixture: {line:?}");
    }

    /// A window with nothing in it is blank rather than a row of floors, for the
    /// same reason.
    #[test]
    fn an_empty_window_draws_nothing() {
        assert_eq!(sparkline(&[], 5), "     ");
    }

    /// #667. One dimension, one flag, and every older spelling still resolves.
    /// `--since` wins over the old flags rather than losing to whichever branch
    /// was tested first, which is the behaviour that made two windows ambiguous.
    #[test]
    fn since_resolves_the_window_and_the_old_flags_still_work() {
        let args = |flags: &[&str]| flags.iter().map(|f| f.to_string()).collect::<Vec<_>>();

        assert_eq!(scope(&args(&[])).0, "last 30 days");
        assert_eq!(scope(&args(&["--since", "week"])).0, "last 7 days");
        assert_eq!(scope(&args(&["--since=today"])).0, "today");
        assert_eq!(scope(&args(&["--since", "all"])).1, 0);
        assert_eq!(scope(&args(&["--week"])).0, "last 7 days");
        assert_eq!(scope(&args(&["-H"])).0, "last hour");
        assert_eq!(scope(&args(&["--today"])).0, "today");
        assert_eq!(
            scope(&args(&["--since", "week", "--hour"])).0,
            "last 7 days",
            "the named window decides, not whichever flag is tested first"
        );
        assert_eq!(
            scope(&args(&["--since", "fortnight"])).0,
            "last 30 days",
            "an unrecognised window falls back rather than refusing a report"
        );
    }

    /// The same, for the view. `--json` is not in here on purpose: it is an output
    /// format applied to whatever view was selected.
    #[test]
    fn view_resolves_from_the_new_flag_and_the_old_ones() {
        let args = |flags: &[&str]| flags.iter().map(|f| f.to_string()).collect::<Vec<_>>();

        assert_eq!(view(&args(&[])), "summary");
        assert_eq!(view(&args(&["--view", "detail"])), "detail");
        assert_eq!(view(&args(&["--view=projects"])), "project");
        assert_eq!(view(&args(&["--detail"])), "detail");
        assert_eq!(view(&args(&["--project"])), "project");
        assert_eq!(view(&args(&["--rerun"])), "rerun");
        assert_eq!(view(&args(&["--all-commands"])), "detail");
        assert_eq!(
            view(&args(&["--view", "commands", "--detail"])),
            "commands",
            "the named view decides"
        );
    }

    /// #665. The two stages travel as two objects, so a consumer cannot average
    /// them into the combined percentage this project has no denominator for. A
    /// stage whose base is unknown says `null` rather than reporting a share over
    /// a population its column does not cover.
    #[test]
    fn the_json_keeps_the_two_stages_apart() {
        let stages = StageStats {
            distilled: StageStat {
                calls: 3,
                bytes_removed: 900,
                base_bytes: Some(1000),
                share_pct: Some(90.0),
            },
            folded: StageStat {
                calls: 2,
                bytes_removed: 400,
                base_bytes: None,
                share_pct: None,
            },
            passed_through_calls: 5,
        };

        // The mapping is what decides `null`, so it is what the test drives.
        let unpriced = stage_stat(2, 400, 0, 0);
        assert_eq!(unpriced.base_bytes, None);
        assert_eq!(
            unpriced.share_pct, None,
            "a stage with no recorded base must not report a share"
        );
        let priced = stage_stat(3, 900, 1_000, 900);
        assert_eq!(priced.share_pct, Some(90.0));

        let json = serde_json::to_string(&stages).unwrap();
        assert!(json.contains("\"distilled\""), "{json}");
        assert!(json.contains("\"folded\""), "{json}");
        assert!(
            json.contains("\"base_bytes\":null") && json.contains("\"share_pct\":null"),
            "an unknown base must read as null, not as a computed share: {json}"
        );
        assert!(
            !json.contains("total_pct") && !json.contains("combined"),
            "no field may offer the two populations as one figure: {json}"
        );
    }
}
