//! auto_learn.zig — Autonomous Filter Discovery Engine
//!
//! Analyzes raw input, automatically detects noise patterns,
//! then writes new DSL filters to omni_config.json without manual intervention.
//!
//! Pipeline:
//!   raw input → LineAnalyzer → PatternDetector → FilterGenerator → ConfigWriter

const std = @import("std");

// ─── Tuning Constants ────────────────────────────────────────────────────────

/// Lines appearing >= this threshold are considered "repetitive noise"
const REPEAT_THRESHOLD: usize = 3;

/// Minimum total lines for analysis to be meaningful
const MIN_LINES_FOR_ANALYSIS: usize = 5;

/// Maximum number of pattern candidates processed per learn session
const MAX_CANDIDATES: usize = 16;

/// Maximum length of trigger string taken from line prefix
const TRIGGER_MAX_LEN: usize = 48;

// ─── Tipe Data ───────────────────────────────────────────────────────────────

pub const LearnError = error{
    InsufficientInput,
    NoPatternsFound,
    ConfigWriteFailed,
};

/// Frequency analysis results for a unique line
const LineFreq = struct {
    prefix: []const u8, // beginning of line (trigger candidate)
    count: usize, // occurrence count
    sample: []const u8, // one complete sample line
};

pub const Action = enum { count, keep };

/// Filter candidates ready to be written to JSON
pub const FilterCandidate = struct {
    name: []const u8,
    trigger: []const u8,
    pattern: []const u8,
    action: Action,
    output_template: []const u8,
    confidence: f32,
};

/// Summary of one `omni learn` session
pub const LearnResult = struct {
    filters_added: usize,
    filters_skipped: usize, // already in config
    total_lines_analyzed: usize,
    noise_ratio: f32, // ratio of noise lines vs total
};

// ─── Line Analysis ──────────────────────────────────────────────────────────

/// Extract meaningful prefix from a line to be used as a trigger.
/// Strip ANSI escape codes, whitespace, and control characters first.
fn extractPrefix(line: []const u8, buf: []u8) []const u8 {
    var clean_len: usize = 0;
    var i: usize = 0;

    // Strip ANSI escape sequences (\x1b[...m)
    while (i < line.len and clean_len < buf.len - 1) {
        if (line[i] == 0x1b and i + 1 < line.len and line[i + 1] == '[') {
            i += 2;
            while (i < line.len and line[i] != 'm') i += 1;
            if (i < line.len) i += 1;
        } else {
            buf[clean_len] = line[i];
            clean_len += 1;
            i += 1;
        }
    }

    const clean = std.mem.trim(u8, buf[0..clean_len], " \t\r\n");
    if (clean.len == 0) return clean;

    // Take prefix up to 3rd space or TRIGGER_MAX_LEN, whichever is shorter
    var space_count: usize = 0;
    var end: usize = 0;
    for (clean) |c| {
        if (c == ' ') space_count += 1;
        if (space_count >= 3 or end >= TRIGGER_MAX_LEN) break;
        end += 1;
    }

    return clean[0..@min(end, clean.len)];
}

/// Calculate prefix frequencies across the entire input. Returns an allocated slice.
fn analyzeLineFrequencies(
    allocator: std.mem.Allocator,
    input: []const u8,
    total_lines: *usize,
) ![]LineFreq {
    var freq_map = std.StringHashMap(LineFreq).init(allocator);
    defer freq_map.deinit();

    var line_count: usize = 0;
    var it = std.mem.splitScalar(u8, input, '\n');

    while (it.next()) |raw_line| {
        const line = std.mem.trim(u8, raw_line, " \t\r");
        if (line.len < 3) continue; // Skip too-short lines
        line_count += 1;

        var prefix_buf: [TRIGGER_MAX_LEN + 8]u8 = undefined;
        const prefix = extractPrefix(line, &prefix_buf);
        if (prefix.len == 0) continue;

        // Store or increment
        if (freq_map.getPtr(prefix)) |entry| {
            entry.count += 1;
        } else {
            // Duplicate key & sample because map does not own input string
            const owned_prefix = try allocator.dupe(u8, prefix);
            errdefer allocator.free(owned_prefix);
            const owned_sample = try allocator.dupe(u8, line[0..@min(line.len, 80)]);
            errdefer allocator.free(owned_sample);

            try freq_map.put(owned_prefix, .{
                .prefix = owned_prefix,
                .count = 1,
                .sample = owned_sample,
            });
        }
    }

    total_lines.* = line_count;

    // Collect only those meeting the repetition threshold
    var candidates = std.ArrayList(LineFreq).empty;
    var map_it = freq_map.iterator();
    while (map_it.next()) |entry| {
        if (entry.value_ptr.count >= REPEAT_THRESHOLD) {
            try candidates.append(allocator, entry.value_ptr.*);
        }
    }

    // Sort descending by count (noisiest first)
    const items = candidates.items;
    for (0..items.len) |a| {
        for (a + 1..items.len) |b| {
            if (items[b].count > items[a].count) {
                const tmp = items[a];
                items[a] = items[b];
                items[b] = tmp;
            }
        }
    }

    return candidates.toOwnedSlice(allocator);
}

// ─── Pattern → FilterCandidate ───────────────────────────────────────────────

/// Classify whether this pattern is better suited for `count` or `keep`.
/// Logic: if the prefix is generic (short, no numbers), it's suitable for count.
fn classifyAction(freq: LineFreq) Action {
    // If it appears very frequently and is short -> usually noise to be counted
    if (freq.count >= 8 and freq.prefix.len <= 20) return .count;

    // If there are numbers in the sample -> likely progress/status, keep
    for (freq.sample) |c| {
        if (c >= '0' and c <= '9') return .keep;
    }
    return .count;
}

/// Create a JSON-safe filter name from a prefix string.
/// Handles special-character-only strings by mapping symbols to words.
fn slugify(allocator: std.mem.Allocator, input: []const u8) ![]u8 {
    var buf = std.ArrayList(u8).empty;
    defer buf.deinit(allocator);

    var has_alpha = false;
    for (input) |c| {
        if (std.ascii.isAlphanumeric(c)) {
            has_alpha = true;
            break;
        }
    }

    if (has_alpha) {
        // Standard slugifcation
        for (input) |c| {
            if (buf.items.len >= 32) break;
            if (std.ascii.isAlphanumeric(c)) {
                try buf.append(allocator, std.ascii.toLower(c));
            } else if (buf.items.len > 0 and buf.items[buf.items.len - 1] != '-') {
                try buf.append(allocator, '-');
            }
        }
    } else {
        // Special character mapping
        for (input) |c| {
            if (buf.items.len >= 32) break;
            const word: []const u8 = switch (c) {
                '`' => "tick",
                '-' => "dash",
                '|' => "pipe",
                ':' => "colon",
                ' ' => "space",
                '<' => "lt",
                '>' => "gt",
                '/' => "slash",
                '\\' => "backslash",
                '[' => "bracket-open",
                ']' => "bracket-close",
                '{' => "brace-open",
                '}' => "brace-close",
                '(' => "paren-open",
                ')' => "paren-close",
                '#' => "hash",
                '$' => "dollar",
                '%' => "percent",
                '^' => "caret",
                '&' => "amp",
                '*' => "star",
                '+' => "plus",
                '=' => "equal",
                '~' => "tilde",
                '!' => "excl",
                '?' => "ques",
                ',' => "comma",
                '.' => "dot",
                else => continue,
            };
            if (buf.items.len > 0 and buf.items[buf.items.len - 1] != '-') {
                try buf.append(allocator, '-');
            }
            try buf.appendSlice(allocator, word);
        }
    }

    // Trim trailing dash
    var result_len = buf.items.len;
    while (result_len > 0 and buf.items[result_len - 1] == '-') result_len -= 1;
    
    // Return a copy
    return try allocator.dupe(u8, buf.items[0..result_len]);
}

/// Convert LineFreq → ready-to-use FilterCandidate.
fn buildCandidate(
    allocator: std.mem.Allocator,
    freq: LineFreq,
    total_lines: usize,
) !FilterCandidate {
    const action = classifyAction(freq);
    const slug = try slugify(allocator, freq.prefix);
    errdefer allocator.free(slug);

    const name = try std.fmt.allocPrint(allocator, "auto-{s}", .{slug});
    errdefer allocator.free(name);

    const trigger = try allocator.dupe(u8, freq.prefix[0..@min(freq.prefix.len, TRIGGER_MAX_LEN)]);
    errdefer allocator.free(trigger);

    // Pattern: use prefix as anchor, variable captures the rest
    const pattern = try std.fmt.allocPrint(allocator, "{s} {{value}}", .{freq.prefix});
    errdefer allocator.free(pattern);

    // Output template varies depending on action
    const output_tmpl = switch (action) {
        .count => try std.fmt.allocPrint(
            allocator,
            "[auto-filtered] {s}: {{value_count}}x suppressed",
            .{freq.prefix},
        ),
        .keep => try std.fmt.allocPrint(
            allocator,
            "{s}: {{value}}",
            .{freq.prefix},
        ),
    };
    errdefer allocator.free(output_tmpl);

    // Confidence: the more frequent it appears relative to total -> the more certain this is noise
    const noise_ratio = @as(f32, @floatFromInt(freq.count)) /
        @as(f32, @floatFromInt(@max(total_lines, 1)));
    const confidence = @min(0.99, 0.5 + noise_ratio * 2.0);

    return FilterCandidate{
        .name = name,
        .trigger = trigger,
        .pattern = pattern,
        .action = action,
        .output_template = output_tmpl,
        .confidence = confidence,
    };
}

pub fn freeCandidates(allocator: std.mem.Allocator, candidates: []FilterCandidate) void {
    for (candidates) |c| {
        allocator.free(c.name);
        allocator.free(c.trigger);
        allocator.free(c.pattern);
        allocator.free(c.output_template);
    }
    allocator.free(candidates);
}

// ─── Config I/O ──────────────────────────────────────────────────────────────

/// Check if trigger already exists in existing config JSON (avoid duplicates).
fn triggerExistsInConfig(allocator: std.mem.Allocator, config_content: []const u8, trigger: []const u8) !bool {
    const escaped = try escapeJson(allocator, trigger);
    defer allocator.free(escaped);
    return std.mem.indexOf(u8, config_content, escaped) != null;
}

/// Escape a string for JSON (handling quotes and backslashes).
fn escapeJson(allocator: std.mem.Allocator, input: []const u8) ![]u8 {
    var buf = std.ArrayList(u8).empty;
    defer buf.deinit(allocator);

    for (input) |c| {
        switch (c) {
            '\"' => try buf.appendSlice(allocator, "\\\""),
            '\\' => try buf.appendSlice(allocator, "\\\\"),
            '\n' => try buf.appendSlice(allocator, "\\n"),
            '\r' => try buf.appendSlice(allocator, "\\r"),
            '\t' => try buf.appendSlice(allocator, "\\t"),
            else => {
                if (std.ascii.isControl(c)) {
                    try buf.writer(allocator).print("\\u{x:0>4}", .{@as(u16, c)});
                } else {
                    try buf.append(allocator, c);
                }
            },
        }
    }
    return try allocator.dupe(u8, buf.items);
}

/// Serialize one FilterCandidate into a JSON object string.
fn candidateToJson(
    allocator: std.mem.Allocator,
    candidate: FilterCandidate,
) ![]u8 {
    const action_str = switch (candidate.action) {
        .count => "count",
        .keep => "keep",
    };

    const escaped_name = try escapeJson(allocator, candidate.name);
    defer allocator.free(escaped_name);
    const escaped_trigger = try escapeJson(allocator, candidate.trigger);
    defer allocator.free(escaped_trigger);
    const escaped_pattern = try escapeJson(allocator, candidate.pattern);
    defer allocator.free(escaped_pattern);
    const escaped_output = try escapeJson(allocator, candidate.output_template);
    defer allocator.free(escaped_output);

    return std.fmt.allocPrint(allocator,
        \\    {{
        \\      "name": "{s}",
        \\      "trigger": "{s}",
        \\      "confidence": {d:.2},
        \\      "rules": [
        \\        {{ "capture": "{s}", "action": "{s}", "as": "value_count" }}
        \\      ],
        \\      "output": "{s}"
        \\    }}
    , .{
        escaped_name,
        escaped_trigger,
        candidate.confidence,
        escaped_pattern,
        action_str,
        escaped_output,
    });
}

/// Write candidates to JSON config file.
/// Strategy: read existing → merge → write back atomically via rename.
pub fn writeToConfig(
    allocator: std.mem.Allocator,
    config_path: []const u8,
    candidates: []const FilterCandidate,
) !usize {
    if (candidates.len == 0) return 0;

    // ── Read existing config (or start from scratch) ──
    const existing = blk: {
        const f = std.fs.cwd().openFile(config_path, .{}) catch |err| {
            if (err == error.FileNotFound) break :blk try allocator.dupe(u8, "{\"rules\":[],\"dsl_filters\":[]}");
            return err;
        };
        defer f.close();
        break :blk try f.readToEndAlloc(allocator, 1 * 1024 * 1024);
    };
    defer allocator.free(existing);

    // ── Filter candidates that are not already present ──
    var new_jsons = std.ArrayList([]u8).empty;
    defer {
        for (new_jsons.items) |j| allocator.free(j);
        new_jsons.deinit(allocator);
    }

    var added: usize = 0;
    for (candidates) |c| {
        if (try triggerExistsInConfig(allocator, existing, c.trigger)) continue;
        const json_fragment = try candidateToJson(allocator, c);
        try new_jsons.append(allocator, json_fragment);
        added += 1;
    }

    if (added == 0) return 0;

    // ── Inject into JSON ──
    // Find dsl_filters array position and insert before final ']'
    const merged = try mergeIntoConfig(allocator, existing, new_jsons.items);
    defer allocator.free(merged);

    // ── Atomic write: write to .tmp first, then rename ──
    const tmp_path = try std.fmt.allocPrint(allocator, "{s}.tmp", .{config_path});
    defer allocator.free(tmp_path);

    {
        const tmp_file = try std.fs.cwd().createFile(tmp_path, .{ .truncate = true });
        defer tmp_file.close();
        try tmp_file.writeAll(merged);
    }

    try std.fs.cwd().rename(tmp_path, config_path);

    return added;
}

/// Merge new JSON fragments into existing config string.
fn mergeIntoConfig(
    allocator: std.mem.Allocator,
    existing: []const u8,
    new_fragments: []const []u8,
) ![]u8 {
    // Search for "dsl_filters": [ ... ] position for injection
    const key = "\"dsl_filters\"";
    const key_pos = std.mem.indexOf(u8, existing, key) orelse {
        // Key not found → create new config from scratch
        return buildFreshConfig(allocator, existing, new_fragments);
    };

    // Find '[' after key
    var bracket_pos = key_pos + key.len;
    while (bracket_pos < existing.len and existing[bracket_pos] != '[') bracket_pos += 1;
    if (bracket_pos >= existing.len) return buildFreshConfig(allocator, existing, new_fragments);
    bracket_pos += 1; // skip '['

    // Find closing ']' for array (last one in vicinity)
    var close_pos = existing.len - 1;
    while (close_pos > bracket_pos and existing[close_pos] != ']') close_pos -= 1;

    // Build combined string
    var buf = std.ArrayList(u8).empty;
    defer buf.deinit(allocator);

    // Part before closing ']'
    try buf.appendSlice(allocator, existing[0..close_pos]);

    // If array not empty, add comma
    const array_content = std.mem.trim(u8, existing[bracket_pos..close_pos], " \t\n\r");
    if (array_content.len > 0) try buf.appendSlice(allocator, ",\n");

    // Insert each new fragment
    for (new_fragments, 0..) |frag, idx| {
        try buf.appendSlice(allocator, "\n");
        try buf.appendSlice(allocator, frag);
        if (idx < new_fragments.len - 1) try buf.appendSlice(allocator, ",");
    }

    // Close array and rest of config
    try buf.appendSlice(allocator, "\n  ");
    try buf.appendSlice(allocator, existing[close_pos..]);

    return buf.toOwnedSlice(allocator);
}

/// Create fresh JSON config if file doesn't exist or structure is invalid.
fn buildFreshConfig(
    allocator: std.mem.Allocator,
    _: []const u8,
    new_fragments: []const []u8,
) ![]u8 {
    var buf = std.ArrayList(u8).empty;
    defer buf.deinit(allocator);

    try buf.appendSlice(allocator, "{\n  \"rules\": [],\n  \"dsl_filters\": [\n");

    for (new_fragments, 0..) |frag, idx| {
        try buf.appendSlice(allocator, frag);
        if (idx < new_fragments.len - 1) try buf.appendSlice(allocator, ",");
        try buf.appendSlice(allocator, "\n");
    }

    try buf.appendSlice(allocator, "  ]\n}\n");
    return buf.toOwnedSlice(allocator);
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Analyze input and return filter candidates (without writing to disk).
/// Caller is responsible for calling freeCandidates() afterwards.
pub fn discoverCandidates(
    allocator: std.mem.Allocator,
    input: []const u8,
) ![]FilterCandidate {
    if (input.len == 0) return LearnError.InsufficientInput;

    var total_lines: usize = 0;
    const freqs = try analyzeLineFrequencies(allocator, input, &total_lines);
    defer allocator.free(freqs);

    if (total_lines < MIN_LINES_FOR_ANALYSIS) return LearnError.InsufficientInput;
    if (freqs.len == 0) return LearnError.NoPatternsFound;

    const limit = @min(freqs.len, MAX_CANDIDATES);
    var candidates = try std.ArrayList(FilterCandidate).initCapacity(allocator, limit);
    errdefer {
        for (candidates.items) |c| {
            allocator.free(c.name);
            allocator.free(c.trigger);
            allocator.free(c.pattern);
            allocator.free(c.output_template);
        }
        candidates.deinit(allocator);
    }

    for (freqs[0..limit]) |freq| {
        const candidate = try buildCandidate(allocator, freq, total_lines);
        try candidates.append(allocator, candidate);
    }

    return candidates.toOwnedSlice(allocator);
}

/// Entry point utama: analisa input, tulis filter ke config, return ringkasan.
pub fn learnFromInput(
    allocator: std.mem.Allocator,
    input: []const u8,
    config_path: []const u8,
) !LearnResult {
    var total_lines: usize = 0;
    const freqs = try analyzeLineFrequencies(allocator, input, &total_lines);
    defer allocator.free(freqs);

    if (total_lines < MIN_LINES_FOR_ANALYSIS) return LearnError.InsufficientInput;

    // Hitung noise ratio: baris repetitif / total
    var repetitive: usize = 0;
    for (freqs) |f| repetitive += f.count;
    const noise_ratio = @as(f32, @floatFromInt(repetitive)) /
        @as(f32, @floatFromInt(@max(total_lines, 1)));

    if (freqs.len == 0) {
        return LearnResult{
            .filters_added = 0,
            .filters_skipped = 0,
            .total_lines_analyzed = total_lines,
            .noise_ratio = noise_ratio,
        };
    }

    // Build candidates
    const limit = @min(freqs.len, MAX_CANDIDATES);
    var candidates = std.ArrayList(FilterCandidate).empty;
    defer {
        for (candidates.items) |c| {
            allocator.free(c.name);
            allocator.free(c.trigger);
            allocator.free(c.pattern);
            allocator.free(c.output_template);
        }
        candidates.deinit(allocator);
    }

    for (freqs[0..limit]) |freq| {
        const candidate = try buildCandidate(allocator, freq, total_lines);
        try candidates.append(allocator, candidate);
    }

    // Tulis ke config
    const added = writeToConfig(allocator, config_path, candidates.items) catch |err| {
        std.log.warn("auto_learn: failed to write config: {any}", .{err});
        return LearnError.ConfigWriteFailed;
    };

    const skipped = candidates.items.len - added;

    return LearnResult{
        .filters_added = added,
        .filters_skipped = skipped,
        .total_lines_analyzed = total_lines,
        .noise_ratio = noise_ratio,
    };
}

// ─── Tests ───────────────────────────────────────────────────────────────────

test "extractPrefix strips ANSI and trims" {
    var buf: [64]u8 = undefined;
    const result = extractPrefix("\x1b[32mStep 3/12:\x1b[0m Building image", &buf);
    try std.testing.expect(result.len > 0);
    try std.testing.expect(std.mem.indexOf(u8, result, "\x1b") == null);
}

test "discoverCandidates finds repetitive patterns" {
    const allocator = std.testing.allocator;

    // Simulasi output `docker build` yang noisy
    const input =
        \\Step 1/12 : FROM ubuntu:22.04
        \\Step 2/12 : RUN apt-get update
        \\Removing intermediate container abc123
        \\Step 3/12 : RUN apt-get install -y curl
        \\Removing intermediate container def456
        \\Step 4/12 : COPY . /app
        \\Removing intermediate container ghi789
        \\Step 5/12 : RUN npm install
        \\Removing intermediate container jkl012
        \\Step 6/12 : RUN npm run build
        \\Removing intermediate container mno345
        \\Successfully built abc123def
    ;

    const candidates = try discoverCandidates(allocator, input);
    defer freeCandidates(allocator, candidates);

    try std.testing.expect(candidates.len > 0);

    // Pastikan "Removing intermediate" terdeteksi sebagai noise
    var found_removing = false;
    for (candidates) |c| {
        if (std.mem.startsWith(u8, c.trigger, "Removing")) {
            found_removing = true;
            try std.testing.expect(c.action == .count);
            break;
        }
    }
    try std.testing.expect(found_removing);
}

test "learnFromInput insufficient data returns error" {
    const allocator = std.testing.allocator;
    const result = learnFromInput(allocator, "short\ninput", "/tmp/test_omni.json");
    try std.testing.expectError(LearnError.InsufficientInput, result);
}

test "candidateToJson produces valid fragment" {
    const allocator = std.testing.allocator;
    const c = FilterCandidate{
        .name = "auto-test",
        .trigger = "npm warn",
        .pattern = "npm warn {value}",
        .action = .count,
        .output_template = "[auto-filtered] npm warn: {value_count}x suppressed",
        .confidence = 0.87,
    };
    const json = try candidateToJson(allocator, c);
    defer allocator.free(json);

    try std.testing.expect(std.mem.indexOf(u8, json, "\"auto-test\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"npm warn\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"count\"") != null);
}
