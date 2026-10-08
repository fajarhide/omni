//! What the host says its sessions cost, read from the host's own transcripts (#859).
//!
//! Every other `omni stats` view reports what OMNI did, from OMNI's own tables.
//! This one reads the other side of the boundary: Claude Code writes per-request
//! `usage` and every content block to disk, so the bill and the composition of
//! the context can be stated without taking OMNI's word for anything.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// One tool's share of the tool results the model received.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ToolRow {
    pub name: String,
    pub calls: u64,
    pub result_bytes: u64,
}

/// The host's books for a window. Tokens are billed tokens, bytes are UTF-8 bytes
/// of what entered the context.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Bill {
    pub sessions: u64,
    pub requests: u64,
    pub input_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub output_tokens: u64,
    pub tool_result_bytes: u64,
    pub tool_input_bytes: u64,
    pub text_bytes: u64,
    /// Largest first.
    pub tools: Vec<ToolRow>,
}

impl Bill {
    pub fn tokens(&self) -> u64 {
        self.input_tokens + self.cache_creation_tokens + self.cache_read_tokens + self.output_tokens
    }

    pub fn context_bytes(&self) -> u64 {
        self.tool_result_bytes + self.tool_input_bytes + self.text_bytes
    }
}

/// Where Claude Code keeps its transcripts. `CLAUDE_CONFIG_DIR` is the host's own
/// override, so honouring it reads the same tree the host writes.
pub fn transcripts_root() -> Option<PathBuf> {
    let base = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => dirs::home_dir()?.join(".claude"),
    };
    Some(base.join("projects"))
}

/// Reads every transcript under `root` and keeps what is dated at or after `since`.
///
/// Unreadable files and lines that are not JSON are skipped: a report is not
/// worth failing over one torn line in a file another process is appending to.
pub fn read(root: &Path, since: i64) -> Bill {
    let mut bill = Bill::default();
    let mut sessions: HashSet<String> = HashSet::new();
    let mut tools: HashMap<String, ToolRow> = HashMap::new();

    for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        // A file last written before the window holds nothing inside it.
        if modified_before(path, since) {
            continue;
        }
        let Ok(file) = File::open(path) else { continue };
        // A request id and a tool_use id are only unique inside one transcript.
        let mut seen_requests: HashSet<String> = HashSet::new();
        let mut tool_of: HashMap<String, String> = HashMap::new();

        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(record) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            // A subagent opens its own context, billed apart from the session's.
            // Counting it reads as the session resetting its context.
            if record["isSidechain"].as_bool() == Some(true) {
                continue;
            }
            if record["timestamp"]
                .as_str()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .is_some_and(|t| t.timestamp() < since)
            {
                continue;
            }
            let message = &record["message"];
            if !message.is_object() {
                continue;
            }
            if let Some(id) = record["sessionId"].as_str() {
                sessions.insert(id.to_string());
            }

            // The host writes one record per content block and repeats the whole
            // `usage` on each, so a request is counted once by id. Only the
            // accounting is gated: the blocks on a repeated record are real and
            // are read below.
            let first_sight = match record["requestId"].as_str() {
                Some(id) => seen_requests.insert(id.to_string()),
                None => true,
            };
            if first_sight && message["usage"].is_object() {
                let usage = &message["usage"];
                bill.requests += 1;
                bill.input_tokens += usage["input_tokens"].as_u64().unwrap_or(0);
                bill.cache_creation_tokens +=
                    usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                bill.cache_read_tokens += usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                bill.output_tokens += usage["output_tokens"].as_u64().unwrap_or(0);
            }

            match &message["content"] {
                Value::String(text) => bill.text_bytes += text.len() as u64,
                Value::Array(blocks) => {
                    for block in blocks {
                        read_block(block, &mut bill, &mut tools, &mut tool_of);
                    }
                }
                _ => {}
            }
        }
    }

    bill.sessions = sessions.len() as u64;
    bill.tools = tools.into_values().collect();
    bill.tools.sort_by(|a, b| {
        b.result_bytes
            .cmp(&a.result_bytes)
            .then(a.name.cmp(&b.name))
    });
    bill
}

fn read_block(
    block: &Value,
    bill: &mut Bill,
    tools: &mut HashMap<String, ToolRow>,
    tool_of: &mut HashMap<String, String>,
) {
    match block["type"].as_str() {
        Some("tool_use") => {
            let name = block["name"].as_str().unwrap_or("unknown").to_string();
            if let Some(id) = block["id"].as_str() {
                tool_of.insert(id.to_string(), name.clone());
            }
            bill.tool_input_bytes += block["input"].to_string().len() as u64;
            row(tools, &name).calls += 1;
        }
        Some("tool_result") => {
            let name = block["tool_use_id"]
                .as_str()
                .and_then(|id| tool_of.get(id))
                .map_or("unknown", String::as_str)
                .to_string();
            let bytes = match &block["content"] {
                Value::String(text) => text.len(),
                Value::Array(parts) => parts
                    .iter()
                    .map(|p| p["text"].as_str().map_or(0, str::len))
                    .sum(),
                Value::Null => 0,
                other => other.to_string().len(),
            } as u64;
            bill.tool_result_bytes += bytes;
            row(tools, &name).result_bytes += bytes;
        }
        Some("text") => bill.text_bytes += block["text"].as_str().map_or(0, str::len) as u64,
        Some("thinking") => {
            bill.text_bytes += block["thinking"].as_str().map_or(0, str::len) as u64;
        }
        _ => {}
    }
}

fn row<'a>(tools: &'a mut HashMap<String, ToolRow>, name: &str) -> &'a mut ToolRow {
    tools.entry(name.to_string()).or_insert_with(|| ToolRow {
        name: name.to_string(),
        ..ToolRow::default()
    })
}

fn modified_before(path: &Path, since: i64) -> bool {
    let Ok(modified) = path.metadata().and_then(|m| m.modified()) else {
        return false;
    };
    modified
        .duration_since(std::time::UNIX_EPOCH)
        .is_ok_and(|d| (d.as_secs() as i64) < since)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const NOW: &str = "2026-10-08T00:00:00.000Z";

    fn transcript(dir: &Path, name: &str, lines: &[String]) {
        let mut file = File::create(dir.join(name)).expect("create the transcript fixture");
        for line in lines {
            writeln!(file, "{line}").expect("write the transcript fixture");
        }
    }

    fn assistant(request: &str, block: &str, extra: &str) -> String {
        format!(
            r#"{{"type":"assistant","sessionId":"s1","timestamp":"{NOW}","requestId":"{request}"{extra},"message":{{"usage":{{"input_tokens":3,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"output_tokens":20}},"content":[{block}]}}}}"#
        )
    }

    fn tool_result(id: &str, content: &str) -> String {
        format!(
            r#"{{"type":"user","sessionId":"s1","timestamp":"{NOW}","message":{{"content":[{{"type":"tool_result","tool_use_id":"{id}","content":"{content}"}}]}}}}"#
        )
    }

    #[test]
    fn a_request_written_as_two_records_is_billed_once_and_both_blocks_are_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        transcript(
            dir.path(),
            "a.jsonl",
            &[
                assistant("r1", r#"{"type":"text","text":"abcd"}"#, ""),
                assistant(
                    "r1",
                    r#"{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}"#,
                    "",
                ),
                tool_result("t1", "0123456789"),
            ],
        );
        let bill = read(dir.path(), 0);

        assert_eq!(bill.requests, 1, "one request id is one request");
        assert_eq!(bill.cache_read_tokens, 1000);
        assert_eq!(bill.tokens(), 1123);
        // The second record repeats the usage and carries a block of its own.
        assert_eq!(bill.text_bytes, 4);
        assert_eq!(
            bill.tools,
            vec![ToolRow {
                name: "Bash".into(),
                calls: 1,
                result_bytes: 10
            }]
        );
    }

    #[test]
    fn a_subagent_record_is_left_out_of_the_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        transcript(
            dir.path(),
            "a.jsonl",
            &[
                assistant("r1", r#"{"type":"text","text":"main"}"#, ""),
                assistant(
                    "r2",
                    r#"{"type":"text","text":"side"}"#,
                    r#","isSidechain":true"#,
                ),
            ],
        );
        let bill = read(dir.path(), 0);

        assert_eq!(bill.requests, 1);
        assert_eq!(bill.text_bytes, 4);
    }

    #[test]
    fn a_record_dated_before_the_window_is_left_out() {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = assistant("r0", r#"{"type":"text","text":"old"}"#, "")
            .replace(NOW, "2026-01-01T00:00:00.000Z");
        transcript(
            dir.path(),
            "a.jsonl",
            &[old, assistant("r1", r#"{"type":"text","text":"new!"}"#, "")],
        );
        let since = chrono::DateTime::parse_from_rfc3339("2026-10-01T00:00:00Z")
            .expect("a literal date")
            .timestamp();
        let bill = read(dir.path(), since);

        assert_eq!(bill.requests, 1);
        assert_eq!(bill.text_bytes, 4);
    }

    #[test]
    fn a_tool_result_is_counted_in_bytes_under_the_tool_that_produced_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        transcript(
            dir.path(),
            "a.jsonl",
            &[
                assistant(
                    "r1",
                    r#"{"type":"tool_use","id":"t1","name":"Read","input":{}}"#,
                    "",
                ),
                // Three box-drawing characters are nine bytes.
                tool_result("t1", "\u{2500}\u{2500}\u{2500}"),
                tool_result("never-issued", "xy"),
            ],
        );
        let bill = read(dir.path(), 0);

        assert_eq!(bill.tool_result_bytes, 11);
        let read_row = bill
            .tools
            .iter()
            .find(|t| t.name == "Read")
            .expect("a Read row");
        assert_eq!(read_row.result_bytes, 9);
        let orphan = bill
            .tools
            .iter()
            .find(|t| t.name == "unknown")
            .expect("an orphan result is kept, under a name that says so");
        assert_eq!(orphan.result_bytes, 2);
    }
}
