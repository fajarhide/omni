// Safety: String slicing uses ASCII delimiter positions or boundary-checked safe utilities.
#![allow(clippy::string_slice)]

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

fn format_bar_with_empty(pct: f64) -> String {
    let width = 20;
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
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

/// One period row of the overview, before layout.
struct PeriodRow<'a> {
    label: &'a str,
    count: u64,
    raw_tokens: u64,
    filtered_tokens: u64,
    reduction_pct: f64,
}

/// Lays out the overview period rows, padding every numeric column to the widest
/// value present. The widths have to come from the rows: `format_number` grows a
/// separator every three digits and `format_exact_tokens` widens as it crosses
/// into K and M, so any hardcoded width is wrong for some future row and shifts
/// every column to its right (#209).
fn format_period_rows(rows: &[PeriodRow<'_>]) -> Vec<String> {
    let labels: Vec<String> = rows.iter().map(|r| format!("{}:", r.label)).collect();
    let counts: Vec<String> = rows.iter().map(|r| format_number(r.count)).collect();
    let inputs: Vec<String> = rows
        .iter()
        .map(|r| format_exact_tokens(r.raw_tokens))
        .collect();
    let outputs: Vec<String> = rows
        .iter()
        .map(|r| format_exact_tokens(r.filtered_tokens))
        .collect();

    let w_label = max_width(&labels).max(12);
    let w_count = max_width(&counts);
    let w_in = max_width(&inputs);
    let w_out = max_width(&outputs);

    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            let pct = format!("{:.1}% saved", r.reduction_pct);
            let pct_colored = if r.reduction_pct > 70.0 {
                pct.bright_green()
            } else if r.reduction_pct > 40.0 {
                pct.bright_yellow()
            } else {
                pct.bright_red()
            };
            format!(
                "  {:<w_label$} {:>w_count$} commands │ {:>w_in$} → {:<w_out$} tokens │  {}",
                labels[i].as_str().bright_white().bold(),
                counts[i].as_str().cyan(),
                inputs[i].as_str().red(),
                outputs[i].as_str().green(),
                pct_colored,
            )
        })
        .collect()
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

            let tokens_saved = if raw_tok > 0 {
                raw_tok.saturating_sub(filt_tok)
            } else if input > 0 {
                let r = crate::util::token_estimate::estimate_tokens(
                    input as usize,
                    crate::util::token_estimate::ContentHint::Mixed,
                );
                let f = crate::util::token_estimate::estimate_tokens(
                    output as usize,
                    crate::util::token_estimate::ContentHint::Mixed,
                );
                r.saturating_sub(f) as u64
            } else {
                0
            };

            (cmd, calls, pct, tokens_saved)
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
        format!("{}...", &short[..max_len.saturating_sub(3)])
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
const FLAGS: super::Flags = &[
    (
        "--detail",
        "Full technical breakdown (commands, routes, sessions, agents)",
    ),
    ("--hour, -H", "Scope to the last 60 minutes"),
    // `--day` exists so the window family reads `--day/-d`, `--week/-w`,
    // `--month/-m`. `-d` was the only member whose long form did not share its
    // letter, sitting next to a `--detail` that does, which is a collision a
    // reader hits before the docs do (#428). `--today` still works.
    ("--day, --today, -d", "Scope to today only"),
    ("--week, -w", "Scope to last 7 days"),
    ("--month, -m", "Scope to last 30 days (the default)"),
    (
        "--all-commands",
        "List every command, not just the top ones",
    ),
    ("--json", "Machine-readable JSON output"),
    (
        "--share",
        "A copy-pasteable summary of your own measured savings",
    ),
    (
        "--card",
        "Write that summary as an image, sized for social posts",
    ),
    ("--project", "Display breakdown per project path"),
    ("--context", "Show context composition signals"),
    (
        "--rerun",
        "Which distillers cost a re-run, the check reduction % cannot make",
    ),
];

/// The time window the scope flags select, as `(label, since_unix)`.
///
/// One resolver for every mode. `run_detail` and `run_project_stats` each had
/// their own copy and neither matched `--month` at all, it was honoured only by
/// being the fall-through in one of them, and silently ignored in the other.
fn scope(args: &[String]) -> (&'static str, i64) {
    let now = chrono::Utc::now().timestamp();
    if has(args, &["--hour", "-H"]) {
        ("last hour", now - 3600)
    } else if has(args, &["--day", "--today", "-d"]) {
        // Calendar day, not a rolling 24h: "today" means since midnight.
        ("today", now - (now % 86400))
    } else if has(args, &["--week", "-w"]) {
        ("last 7 days", now - 7 * 86400)
    } else {
        // `--month` / `-m` and the no-flag default are the same window.
        ("last 30 days", now - 30 * 86400)
    }
}

/// A slice rather than a `(long, short)` pair, because `--today` gained a
/// `--day` spelling and the pair could not carry a third name (#428).
fn has(args: &[String], names: &[&str]) -> bool {
    args.iter().any(|a| names.iter().any(|n| a == n))
}

fn print_help() {
    println!(
        "\n{} {}: Token savings analytics",
        "omni".bold().cyan(),
        "stats".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} {}", "stats".cyan(), "[FLAGS]".bright_black());

    super::print_flags(FLAGS);

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni stats              {} Gain-focused overview",
        "#".bright_black()
    );
    println!(
        "  omni stats --detail     {} Full breakdown with commands",
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
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }
    super::check_flags("stats", args, FLAGS)?;

    let detail_flag = args.iter().any(|a| a == "--detail");
    let json_flag = args.iter().any(|a| a == "--json");
    let share_flag = args.iter().any(|a| a == "--share");
    let card_flag = args.iter().any(|a| a == "--card");
    let project_flag = args.iter().any(|a| a == "--project");
    let context_flag = args.iter().any(|a| a == "--context");
    let rerun_flag = args.iter().any(|a| a == "--rerun");
    let filter_flag = has(args, &["--hour", "-H"])
        || has(args, &["--day", "--today", "-d"])
        || has(args, &["--week", "-w"])
        || has(args, &["--month", "-m"])
        || args.iter().any(|a| a == "--all-commands");

    let mode = if card_flag {
        "card"
    } else if share_flag {
        "share"
    } else if rerun_flag {
        "rerun"
    } else if context_flag {
        "context"
    } else if detail_flag {
        "detail"
    } else if json_flag {
        "json"
    } else if project_flag {
        "project"
    } else if filter_flag {
        "detail"
    } else {
        "default"
    };

    match mode {
        "card" => run_card(store),
        "share" => run_share(store),
        "rerun" => run_rerun(args, store),
        "context" => run_context_stats(store),
        "project" => run_project_stats(args, store),
        "detail" => run_detail(args, store),
        "json" => run_json(store),
        _ => run_default(store),
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
        println!("\n  {}", "Token Breakdown:".bold().bright_white());
        println!(
            "    {:<25} {} tokens",
            "File Reads:".bright_black(),
            format_exact_tokens(turn.file_read_tokens).yellow()
        );
        println!(
            "    {:<25} {} tokens",
            "Tool Outputs:".bright_black(),
            format_exact_tokens(turn.tool_output_tokens).green()
        );

        let est_total = turn.file_read_tokens + turn.tool_output_tokens;
        println!(
            "\n  {:<27} {} tokens",
            "Estimated Context Total:".bold().bright_white(),
            format_exact_tokens(est_total).bright_cyan()
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
                "\n  {:<27} {} ({} tokens)",
                "Largest File Read:".bright_black(),
                turn.largest_single_read.0.cyan(),
                format_exact_tokens(turn.largest_single_read.1).yellow()
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

    let (saved, total) = if raw_tok > 0 {
        (raw_tok.saturating_sub(filt_tok), raw_tok)
    } else {
        (input.saturating_sub(output), input)
    };
    Ok(Some(ShareFigures {
        saved,
        pct: if total > 0 {
            100.0 * saved as f64 / total as f64
        } else {
            0.0
        },
        unit: if raw_tok > 0 { "tokens" } else { "bytes" },
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

fn run_default(store: &Store) -> Result<()> {
    let periods = store.multi_period_stats()?;
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;

    let has_data = periods.iter().any(|(_, count, _, _, _, _)| *count > 0);

    println!();
    print_separator();
    println!(" {}", "OMNI Signal Report".bold().bright_white());
    print_separator();

    if !has_data {
        println!(
            "  {}",
            "No data yet! OMNI tracks savings automatically as you work."
                .bright_black()
                .italic()
        );
        println!("  {}", "Try: ls -la | omni".bright_cyan().italic());
        print_separator();
        println!();
        return Ok(());
    }

    // #435. The headline is session lifetime, because that is the meter #357
    // promoted and `CONTRIBUTING.md` calls the number that decides progress. The
    // distillation percentage below it is a diagnostic for one host's pipeline
    // and says so, which is the whole of the correction: it was never wrong, it
    // was presented as the product number after the project stopped treating it
    // as one.
    let (sessions, median_cmds, longest, compacted) = store.session_lifetime(0);
    println!("  {}", "Session lifetime:".bold().bright_white());
    if sessions == 0 {
        println!(
            "  {}",
            "  not measurable yet: no session has been closed by a host that reports one"
                .bright_black()
                .italic()
        );
    } else {
        println!(
            "  {} commands median, {} longest, across {} closed sessions",
            median_cmds.to_string().bright_green().bold(),
            longest.to_string().cyan(),
            format_number(sessions).cyan()
        );
        let compaction_line = if compacted == 0 {
            "  none ended at a compaction, so this measures sessions, not the window".to_string()
        } else {
            format!("  {compacted} of them ended at a compaction, which is what the window costs")
        };
        println!("  {}", compaction_line.bright_black().italic());
    }
    println!();
    println!(
        "  {} {}",
        "Pipeline diagnostic:".bold().bright_white(),
        "one host's tool output, not a product claim"
            .bright_black()
            .italic()
    );

    // Multi-period rows
    let period_rows: Vec<PeriodRow<'_>> = periods
        .iter()
        .filter(|(label, count, ..)| *count > 0 || label == "All Time")
        .map(
            |(label, count, input, output, raw_tokens, filtered_tokens)| PeriodRow {
                label,
                count: *count,
                raw_tokens: *raw_tokens,
                filtered_tokens: *filtered_tokens,
                reduction_pct: if *raw_tokens > 0 {
                    100.0 * (1.0 - *filtered_tokens as f64 / *raw_tokens as f64)
                } else if *input > 0 {
                    // Fallback for legacy records that haven't been backfilled properly
                    100.0 * (1.0 - *output as f64 / *input as f64)
                } else {
                    0.0
                },
            },
        )
        .collect();

    for line in format_period_rows(&period_rows) {
        println!("{}", line);
    }

    // #212: say what the number is a number *of*. It counts only calls whose
    // result reached a model's context; `omni exec` and pipe output read at a
    // terminal is compression a human sees, not tokens anyone was billed for,
    // and folding the two together is what made the all-time headline 66.3%
    // when the model-facing figure was 29.3%.
    println!(
        "  {}",
        "Counts calls whose result reached a model. Terminal output is excluded:\n  no context holds it."
            .bright_black()
            .italic()
    );

    // #173 asked for a second, cache-discounted figure beside this one, because a
    // distilled tool result is re-sent on every later turn and OMNI counts the
    // saving once. `Store::token_savings_with_reuse` computes it and is tested,
    // and it is deliberately not printed yet.
    //
    // Run against the maintainer's database it reports 17.0M at insertion and
    // 469.3M with re-use, a 27.6x multiplier. The multiplier is wrong, and it is
    // wrong because of #118 item 1: until #259 every distillation was filed under
    // a wall-clock id, so one "session" covers 3,739 commands across 16 project
    // paths and hands its first row a 374x credit. The arithmetic is right and
    // the input is not.
    //
    // Publishing it would be a bigger number that is less true, which is the
    // defect this tracker exists to fight. It goes in once enough history exists
    // under real host session ids to make `turns_after` mean what it says.

    let top_commands = get_top_commands(store, 0, 8);

    if !top_commands.is_empty() {
        println!("\n  {}", "Top Commands:".bold().bright_white());
        let w_count = max_width(
            top_commands
                .iter()
                .map(|(_, count, _, _)| count.to_string()),
        );
        for (cmd, count, pct, tokens_saved) in &top_commands {
            let short_cmd = shorten_command(cmd, 18);
            let bar = format_bar_with_empty(*pct);
            let bar_colored = if *pct > 80.0 {
                bar.bright_green()
            } else if *pct > 40.0 {
                bar.bright_yellow()
            } else {
                bar.bright_red()
            };

            let tokens_str = if *tokens_saved > 0 {
                format!("(-{} tokens)", format_exact_tokens(*tokens_saved)).bright_black()
            } else {
                "".bright_black()
            };

            println!(
                "    {:<18} {}  {:>5.1}%  ({:>w_count$}x)  {}",
                short_cmd.bright_cyan(),
                bar_colored,
                pct,
                count,
                tokens_str,
            );
        }
    }

    // Agent Distribution
    let agent_data = store.get_agent_breakdown(0).unwrap_or_default();

    // Group by display name
    let mut grouped_agents: HashMap<String, (u64, u64, u64)> = HashMap::new();
    for r in &agent_data {
        if r.agent_id == "unknown" || r.agent_id == "terminal" || r.agent_id.is_empty() {
            continue;
        }
        let name = agent_display_name(&r.agent_id).to_string();
        let entry = grouped_agents.entry(name).or_insert((0, 0, 0));
        entry.0 += r.calls;
        entry.1 += r.input_bytes;
        entry.2 += r.output_bytes;
    }

    if !grouped_agents.is_empty() {
        let total_cmds: u64 = agent_data.iter().map(|r| r.calls).sum();
        println!("\n  {}", "Agent Distribution:".bold().bright_white());

        let mut sorted_agents: Vec<_> = grouped_agents.into_iter().collect();
        sorted_agents.sort_by_key(|a| std::cmp::Reverse(a.1.0));

        let w_name = max_width(sorted_agents.iter().map(|(name, _)| name.as_str())).max(18);
        let w_count = max_width(
            sorted_agents
                .iter()
                .map(|(_, (count, _, _))| count.to_string()),
        );

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
            let bar = format_bar_with_empty(pct);
            println!(
                "   {:<w_name$} {}  {:>5.1}%  ({:>w_count$}x)  {:>5.1}% saved",
                name.bright_cyan(),
                bar.bright_blue(),
                pct,
                count,
                savings,
            );
        }
    }

    // RewindStore
    println!(
        "\n  {:<20} {}",
        "RewindStore:".bright_black(),
        format!(
            "{} archived │ {} retrieved",
            rewind_stored, rewind_retrieved
        )
        .bright_magenta()
    );

    // The fidelity alarm. An expansion request is an agent saying the view it
    // was given was not enough, so a rising share of archived blocks being
    // fetched back means the projection is cutting too much. The number was
    // already being acted on silently: `post_tool` raises the route thresholds
    // once a command family passes 25%. This makes the same signal visible
    // rather than only effective.
    if let Some(rate) = (100 * rewind_retrieved).checked_div(rewind_stored)
        && rate >= 25
    {
        println!(
            "  {:<20} {}",
            "Fidelity:".bright_black(),
            format!(
                "{rate}% of archived blocks were fetched back, so distillation is cutting too much"
            )
            .yellow()
        );
    }

    print_separator();
    println!(
        "  {} for full breakdown",
        "omni stats --detail".bright_cyan()
    );

    // Update Notification (4h cache)
    if let Some(latest) = crate::guard::update::check() {
        crate::guard::update::print_notification(&latest);
    }

    println!();
    Ok(())
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
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;

    println!();
    print_separator();
    println!(
        " {}",
        format!("OMNI Signal Report: Detail ({})", period_label.bold()).bright_white()
    );
    print_separator();

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

    println!(
        "  {:<20} {} {} {}",
        "Tokens Reduced:".bright_black(),
        format_exact_tokens(raw_tokens).red(),
        "→".bright_black(),
        format_exact_tokens(filtered_tokens).green()
    );

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
    let all_flag = args.iter().any(|a| a == "--all-commands");
    let grouped_filters = group_and_calculate_stats(raw_filters, 0);

    let display_filters: Vec<_> = if all_flag {
        grouped_filters.clone()
    } else {
        grouped_filters
            .iter()
            .filter(|(_, _, pct, _)| *pct > 0.0)
            .take(10)
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
            "Tokens".bright_black(),
            "Signal".bright_black(),
            w_cmd = CMD_KEY_WIDTH
        );
        println!(
            "  {}",
            super::column_rule(&[3, CMD_KEY_WIDTH, 11, 5, 6, 6, DETAIL_BAR]).bright_black()
        );

        for (i, (name, cnt, pct, tokens_saved)) in display_filters.iter().enumerate() {
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

            let tokens_str = if *tokens_saved > 0 {
                format!("-{}", format_exact_tokens(*tokens_saved))
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
        println!(
            "  {:<16} {:>6} {:>7} {}",
            "Agent".bright_black(),
            "Count".bright_black(),
            "Share".bright_black(),
            "Savings".bright_black()
        );
        // Four groups under a four-column header. It carried five, because the
        // leading `──` was copied from the By Command table's `#` column, which
        // is what made a 56-column rule sit under a 43-column header (#463).
        println!(
            "  {}",
            super::column_rule(&[16, 6, 7, DETAIL_BAR + 7]).bright_black()
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
                "  {:<16} {:>5}x {:>6.1}% {:<w_bar$} {:>5.1}%",
                name.bright_cyan(),
                count,
                pct,
                bar_colored,
                savings,
                w_bar = DETAIL_BAR
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

#[derive(serde::Serialize)]
pub struct StatsJson {
    pub version: String,
    pub generated_at: i64,
    pub periods: Vec<StatsPeriod>,
    pub commands: Vec<CommandStat>,
    pub agents: Vec<AgentStat>,
    pub rewind: RewindStat,
    pub avg_latency_ms: f64,
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
    pub tokens_saved: u64,
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

fn run_json(store: &Store) -> Result<()> {
    let periods = store.multi_period_stats()?;
    let top_commands = get_top_commands(store, 0, 100);
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;
    let (count, _, _, sum_latency, _, _, _) = store.aggregate_stats(0)?;

    let avg_latency = if count > 0 {
        sum_latency as f64 / count as f64
    } else {
        0.0
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
        .map(|(cmd, count, pct, tokens_saved)| CommandStat {
            command: cmd.clone(),
            count: *count,
            savings_pct: *pct,
            tokens_saved: *tokens_saved,
        })
        .collect();

    let agent_json: Vec<AgentStat> = store
        .get_agent_breakdown(0)
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
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
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

    #[test]
    fn aligns_period_columns_across_mixed_number_widths() {
        // Arrange: the widths that broke the old hardcoded layout, a 3-digit
        // count beside a 5-digit one, and a K-scale total beside an M-scale one.
        let rows = [
            PeriodRow {
                label: "Today",
                count: 118,
                raw_tokens: 137_000,
                filtered_tokens: 74_000,
                reduction_pct: 46.2,
            },
            PeriodRow {
                label: "This Week",
                count: 1_218,
                raw_tokens: 10_200_000,
                filtered_tokens: 647_000,
                reduction_pct: 93.6,
            },
            PeriodRow {
                label: "All Time",
                count: 5_047,
                raw_tokens: 21_200_000,
                filtered_tokens: 4_800_000,
                reduction_pct: 77.2,
            },
        ];

        // Act
        let lines = format_period_rows(&rows);

        // Assert: every row starts its labels at the same offset.
        for word in ["commands", "tokens", "saved"] {
            let offsets: Vec<_> = lines
                .iter()
                .map(|l| l.find(word).unwrap_or_else(|| panic!("{word} missing")))
                .collect();
            assert!(
                offsets.windows(2).all(|w| w[0] == w[1]),
                "`{word}` drifts between rows: {offsets:?}\n{}",
                lines.join("\n")
            );
        }
    }

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
    /// `Unknown` against a database with no unknown rows in it.
    #[test]
    fn the_rendered_name_is_not_the_lookup_key() {
        let key = shorten_command("cat package.json", CMD_KEY_WIDTH);
        let rendered = crate::util::text::display_truncate_with_ellipsis(&key, CMD_KEY_WIDTH - 3);

        assert_eq!(
            key, "cat package.json",
            "the key is the whole short command"
        );
        assert_ne!(
            key, rendered,
            "if these were ever equal the conflation would stop being visible, \
             and the lookup must still use the key"
        );

        let mut agents: HashMap<String, HashMap<String, u64>> = HashMap::new();
        agents
            .entry(key.clone())
            .or_default()
            .insert("claude_code".to_string(), 1);

        assert!(agents.contains_key(&key), "keyed lookup resolves");
        assert!(
            !agents.contains_key(&rendered),
            "the rendered cell is not a key and must never be used as one"
        );
    }

    #[test]
    fn test_format_bar() {
        assert_eq!(format_bar(100.0, 20), "████████████████████");
        assert_eq!(format_bar(50.0, 20), "██████████");
        assert_eq!(format_bar(0.0, 20), "");
    }

    #[test]
    fn test_format_bar_with_empty() {
        assert_eq!(format_bar_with_empty(100.0), "████████████████████");
        assert_eq!(format_bar_with_empty(50.0), "██████████░░░░░░░░░░");
        assert_eq!(format_bar_with_empty(0.0), "░░░░░░░░░░░░░░░░░░░░");
    }

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
        };

        let json_str = serde_json::to_string(&json_struct).unwrap();
        assert!(json_str.contains("\"version\":\"1\""));
        assert!(json_str.contains("\"generated_at\":1234567890"));
        assert!(json_str.contains("\"savings_pct\":90.0"));
        assert!(json_str.contains("\"avg_latency_ms\":15.5"));
    }
}
