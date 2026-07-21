use serde::Deserialize;
use serde_json::Value;

/// Format agent yang terdeteksi
#[derive(Debug, Clone, PartialEq)]
pub enum AgentFormat {
    ClaudeCode,     // tool_name + tool_input.command + tool_response
    OpenCode,       // type:tool_result + tool + output
    VSCodeContinue, // role:tool + content + name
    CodexCLI,       // action:run + command + result
    CursorWindsurf, // seperti ClaudeCode tapi content bisa array
    Aider,          // piped stdin, OMNI_CMD env
    GenericMCP,     // JSON-RPC 2.0 tool result
    Pi,             // camelCase toolName + toolResponse from Pi extension
    Unknown,        // fallback ke ClaudeCode parser
}

/// Internal representation setelah normalization
/// Engine hanya bekerja dengan struct ini
#[derive(Debug, Clone)]
pub struct NormalizedInput {
    pub agent: AgentFormat,
    pub tool_name: String, // "Bash", "Read", "Grep", dll
    pub command: String,   // command yang dieksekusi
    pub content: String,   // output dari tool
    pub agent_id: String,  // untuk session isolation
    pub failed: bool,      // command exited non-zero / agent signalled an error (#120)
}

/// Detect agent format dari raw JSON string
pub fn detect_agent(input: &str) -> AgentFormat {
    // Coba parse sebagai JSON
    let Ok(val) = serde_json::from_str::<Value>(input) else {
        // Bukan JSON — mungkin piped stdin (Aider)
        return AgentFormat::Aider;
    };

    let obj = match val.as_object() {
        Some(o) => o,
        None => return AgentFormat::Unknown,
    };

    // Deteksi berdasarkan key signatures yang unik per agent:

    // Pi extension: camelCase "toolName" + "toolResponse" (not snake_case)
    // Must be checked BEFORE ClaudeCode since both could match on partial keys
    if obj.contains_key("toolName") && obj.contains_key("toolResponse") {
        return AgentFormat::Pi;
    }

    // ClaudeCode / CursorWindsurf: punya "tool_name" dan "tool_response"
    if obj.contains_key("tool_name") && obj.contains_key("tool_response") {
        // Cursor/Windsurf: content field di tool_response adalah array
        let is_cursor = obj
            .get("tool_response")
            .and_then(|r| r.get("content"))
            .map(|c| c.is_array())
            .unwrap_or(false);
        return if is_cursor {
            AgentFormat::CursorWindsurf
        } else {
            AgentFormat::ClaudeCode
        };
    }

    // OpenCode: punya "type": "tool_result"
    if obj.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
        return AgentFormat::OpenCode;
    }

    // Codex CLI: punya "action": "run"
    if obj.get("action").and_then(|a| a.as_str()) == Some("run") {
        return AgentFormat::CodexCLI;
    }

    // VS Code Continue.dev: punya "role": "tool"
    if obj.get("role").and_then(|r| r.as_str()) == Some("tool") {
        return AgentFormat::VSCodeContinue;
    }

    // JSON-RPC (Generic MCP): punya "jsonrpc": "2.0" dan "result"
    if obj.contains_key("jsonrpc") && obj.contains_key("result") {
        return AgentFormat::GenericMCP;
    }

    AgentFormat::Unknown
}

/// Normalize raw input ke NormalizedInput
/// Returns None jika content tidak bisa diekstrak (bukan error)
pub fn normalize(input: &str) -> Option<NormalizedInput> {
    let agent = detect_agent(input);
    let agent_id = detect_agent_id(&agent);

    match agent {
        AgentFormat::ClaudeCode | AgentFormat::Unknown => normalize_claude_code(input, agent_id),
        AgentFormat::CursorWindsurf => {
            // Cursor punya content array — tangani itu dulu, lalu delegate ke Claude Code parser
            normalize_cursor(input, agent_id)
        }
        AgentFormat::OpenCode => normalize_opencode(input, agent_id),
        AgentFormat::VSCodeContinue => normalize_vscode_continue(input, agent_id),
        AgentFormat::CodexCLI => normalize_codex(input, agent_id),
        AgentFormat::Aider => normalize_aider(input, agent_id),
        AgentFormat::GenericMCP => normalize_generic_mcp(input, agent_id),
        AgentFormat::Pi => normalize_pi(input, agent_id),
    }
}

/// Detect agent ID untuk session isolation
/// Claude Code: "claude_code"
/// OpenCode: "opencode"
/// VS Code: "vscode"
/// dll.
pub fn detect_agent_id(agent: &AgentFormat) -> String {
    match agent {
        AgentFormat::ClaudeCode => "claude_code".to_string(),
        AgentFormat::OpenCode => "opencode".to_string(),
        AgentFormat::VSCodeContinue => "vscode_continue".to_string(),
        AgentFormat::CodexCLI => "codex_cli".to_string(),
        AgentFormat::CursorWindsurf => "cursor".to_string(),
        AgentFormat::Aider => "aider".to_string(),
        AgentFormat::GenericMCP => "mcp_generic".to_string(),
        AgentFormat::Pi => "pi".to_string(),
        AgentFormat::Unknown => "unknown".to_string(),
    }
}

// ── CLAUDE CODE (existing format, should be removed after all agents are migrated) ────────────────
fn normalize_claude_code(input: &str, agent_id: String) -> Option<NormalizedInput> {
    #[derive(Deserialize)]
    struct ClaudeInput {
        tool_name: String,
        tool_input: Option<ClaudeToolInput>,
        tool_response: Option<ClaudeToolResponse>,
    }
    #[derive(Deserialize)]
    struct ClaudeToolInput {
        command: Option<String>,
        path: Option<String>,
    }
    #[derive(Deserialize)]
    struct ClaudeToolResponse {
        content: Option<Value>,
        stdout: Option<String>,
        stderr: Option<String>,
    }

    let parsed: ClaudeInput = serde_json::from_str(input).ok()?;

    // Extract content (sama persis dengan extract_tool_content yang lama)
    let response = parsed.tool_response.as_ref()?;
    let content = if let Some(ref c) = response.content {
        extract_value_content(c)?
    } else {
        let stdout = response.stdout.as_ref()?;
        if stdout.is_empty() {
            return None;
        }
        let mut s = stdout.clone();
        if let Some(ref stderr) = response.stderr
            && !stderr.is_empty()
        {
            s.push_str("\n[stderr]\n");
            s.push_str(stderr);
        }
        s
    };

    let command = parsed
        .tool_input
        .as_ref()
        .and_then(|i| i.command.as_deref().or(i.path.as_deref()))
        .unwrap_or("")
        .to_string();

    Some(NormalizedInput {
        agent: AgentFormat::ClaudeCode,
        tool_name: parsed.tool_name,
        command,
        content,
        agent_id,
        // Claude Code sends a failed command as a bare `tool_response` string
        // ("Error: Exit code N…"), which never matches ClaudeToolResponse, so
        // parsing bails to None (passthrough) before reaching here. Reaching this
        // point means the tool_response object parsed, i.e. the command succeeded.
        failed: false,
    })
}

// ── CURSOR / WINDSURF ─────────────────────────────────────────────────
fn normalize_cursor(input: &str, agent_id: String) -> Option<NormalizedInput> {
    // Cursor mirip Claude Code tapi tool_response.content bisa array of {type,text}
    // Parser yang sama, cuma route agent_id ke "cursor"
    let mut norm = normalize_claude_code(input, agent_id)?;
    norm.agent = AgentFormat::CursorWindsurf;
    Some(norm)
}

// ── PI EXTENSION ─────────────────────────────────────────────────────
fn normalize_pi(input: &str, agent_id: String) -> Option<NormalizedInput> {
    // Pi extension sends camelCase JSON:
    // {
    //   "hookEventName": "ToolResult",
    //   "toolName": "Bash",
    //   "toolResponse": { "toolName": "Bash", "result": "...", "isError": false },
    //   "isError": false
    // }
    #[derive(Deserialize)]
    struct PiInput {
        #[serde(rename = "toolName")]
        tool_name: Option<String>,
        #[serde(rename = "toolInput")]
        tool_input: Option<Value>,
        command: Option<String>,
        #[serde(rename = "toolResponse")]
        tool_response: Option<PiToolResponse>,
    }
    #[derive(Deserialize)]
    struct PiToolResponse {
        result: Option<Value>,
        #[serde(default)]
        #[serde(rename = "isError")]
        is_error: bool,
    }

    let parsed: PiInput = serde_json::from_str(input).ok()?;

    let tool_name = parsed.tool_name?;
    let response = parsed.tool_response.as_ref()?;

    // Extract content from "result" field (string or object with nested fields)
    let content = extract_value_content(response.result.as_ref()?)?;

    if content.is_empty() {
        return None;
    }

    // Normalize tool name using OMNI's internal standard
    let normalized_name = normalize_tool_name(&tool_name);
    let command = parsed
        .command
        .or_else(|| {
            parsed
                .tool_input
                .as_ref()
                .and_then(|input| extract_pi_command(&normalized_name, input))
        })
        .unwrap_or_default();

    Some(NormalizedInput {
        agent: AgentFormat::Pi,
        tool_name: normalized_name,
        command,
        content,
        agent_id,
        failed: response.is_error,
    })
}

fn extract_pi_command(tool_name: &str, input: &Value) -> Option<String> {
    if let Some(s) = input.as_str() {
        return Some(s.to_string());
    }
    let obj = input.as_object()?;
    let keys: &[&str] = match tool_name {
        "Bash" => &["command", "cmd", "script"],
        "Read" => &["path", "file_path", "filePath"],
        "LS" => &["path", "dir", "directory"],
        "Grep" => &["pattern", "query", "path"],
        "WebFetch" => &["url"],
        _ => &["command", "path"],
    };

    keys.iter()
        .filter_map(|key| obj.get(*key).and_then(Value::as_str))
        .find(|value| !value.is_empty())
        .map(ToString::to_string)
}

// ── OPENCODE ──────────────────────────────────────────────────────────
fn normalize_opencode(input: &str, agent_id: String) -> Option<NormalizedInput> {
    // Format OpenCode:
    // { "type": "tool_result", "tool": "shell", "output": "...", "command": "..." }
    #[derive(Deserialize)]
    struct OpenCodeInput {
        tool: Option<String>,
        output: Option<String>,
        command: Option<String>,
        result: Option<String>,
    }

    let parsed: OpenCodeInput = serde_json::from_str(input).ok()?;
    let content = parsed.output.or(parsed.result)?;
    if content.is_empty() {
        return None;
    }

    // Normalize tool name ke Claude Code standard
    let tool_name = match parsed.tool.as_deref().unwrap_or("shell") {
        "shell" | "bash" | "exec" => "Bash",
        "read" | "read_file" => "Read",
        "search" | "grep" => "Grep",
        "fetch" | "web_fetch" => "WebFetch",
        other => other,
    }
    .to_string();

    Some(NormalizedInput {
        agent: AgentFormat::OpenCode,
        tool_name,
        command: parsed.command.unwrap_or_default(),
        content,
        agent_id,
        failed: false, // OpenCode payload carries no exit/error signal
    })
}

// ── VS CODE CONTINUE.DEV ──────────────────────────────────────────────
fn normalize_vscode_continue(input: &str, agent_id: String) -> Option<NormalizedInput> {
    // Continue.dev format:
    // { "role": "tool", "name": "bash", "content": "output here", "tool_use_id": "..." }
    #[derive(Deserialize)]
    struct ContinueInput {
        name: Option<String>,
        content: Option<Value>,
        tool_call: Option<ContinueToolCall>,
    }
    #[derive(Deserialize)]
    struct ContinueToolCall {
        function: Option<ContinueFn>,
    }
    #[derive(Deserialize)]
    struct ContinueFn {
        name: Option<String>,
        arguments: Option<String>, // JSON string
    }

    let parsed: ContinueInput = serde_json::from_str(input).ok()?;
    let content = parsed.content.as_ref().and_then(|c| {
        if let Some(s) = c.as_str() {
            Some(s.to_string())
        } else {
            extract_value_content(c)
        }
    })?;

    if content.is_empty() {
        return None;
    }

    let tool_name_raw = parsed
        .name
        .or_else(|| {
            parsed
                .tool_call
                .as_ref()
                .and_then(|tc| tc.function.as_ref())
                .and_then(|f| f.name.clone())
        })
        .unwrap_or_else(|| "bash".to_string());

    let tool_name = normalize_tool_name(&tool_name_raw);

    // Extract command dari tool_call.function.arguments jika ada
    let command = parsed
        .tool_call
        .and_then(|tc| tc.function)
        .and_then(|f| f.arguments)
        .and_then(|args| {
            serde_json::from_str::<Value>(&args).ok().and_then(|v| {
                v.get("command")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string())
            })
        })
        .unwrap_or_default();

    Some(NormalizedInput {
        agent: AgentFormat::VSCodeContinue,
        tool_name,
        command,
        content,
        agent_id,
        failed: false, // Continue.dev payload carries no exit/error signal
    })
}

// ── CODEX CLI ─────────────────────────────────────────────────────────
fn normalize_codex(input: &str, agent_id: String) -> Option<NormalizedInput> {
    // Codex CLI format:
    // { "action": "run", "command": "npm test", "result": "...", "exit_code": 0 }
    #[derive(Deserialize)]
    struct CodexInput {
        command: Option<String>,
        result: Option<String>,
        output: Option<String>,
        stdout: Option<String>,
        stderr: Option<String>,
        exit_code: Option<i64>,
    }

    let parsed: CodexInput = serde_json::from_str(input).ok()?;
    let content = parsed.result.or(parsed.output).or_else(|| {
        let mut s = parsed.stdout.unwrap_or_default();
        if let Some(err) = parsed.stderr
            && !err.is_empty()
        {
            s.push_str("\n[stderr]\n");
            s.push_str(&err);
        }
        if s.is_empty() { None } else { Some(s) }
    })?;

    if content.is_empty() {
        return None;
    }

    Some(NormalizedInput {
        agent: AgentFormat::CodexCLI,
        tool_name: "Bash".to_string(), // Codex CLI selalu bash
        command: parsed.command.unwrap_or_default(),
        content,
        agent_id,
        failed: parsed.exit_code.is_some_and(|c| c != 0),
    })
}

// ── AIDER ─────────────────────────────────────────────────────────────
fn normalize_aider(input: &str, agent_id: String) -> Option<NormalizedInput> {
    // Aider pakai piped stdin — content adalah raw string, command dari OMNI_CMD
    let command = std::env::var("OMNI_CMD").unwrap_or_default();
    if input.trim().is_empty() {
        return None;
    }

    Some(NormalizedInput {
        agent: AgentFormat::Aider,
        tool_name: "Bash".to_string(),
        command,
        content: input.to_string(),
        agent_id,
        failed: false, // Aider pipes raw stdout only; no exit signal available
    })
}

// ── GENERIC MCP (JSON-RPC 2.0) ────────────────────────────────────────
fn normalize_generic_mcp(input: &str, agent_id: String) -> Option<NormalizedInput> {
    // JSON-RPC 2.0 tool result format:
    // { "jsonrpc": "2.0", "id": 1, "result": { "content": [...], "isError": false } }
    #[derive(Deserialize)]
    struct McpResult {
        result: Option<McpResultContent>,
    }
    #[derive(Deserialize)]
    struct McpResultContent {
        content: Option<Value>,
        #[serde(default)]
        #[serde(rename = "isError")]
        is_error: bool,
    }

    let parsed: McpResult = serde_json::from_str(input).ok()?;
    let result = parsed.result?;
    let failed = result.is_error;
    let content = result.content.and_then(|c| extract_value_content(&c))?;

    if content.is_empty() {
        return None;
    }

    // Command tidak bisa di-detect dari JSON-RPC result — gunakan OMNI_CMD env
    let command = std::env::var("OMNI_CMD").unwrap_or_default();

    Some(NormalizedInput {
        agent: AgentFormat::GenericMCP,
        tool_name: "Bash".to_string(),
        command,
        content,
        agent_id,
        failed,
    })
}

// ── HELPERS ───────────────────────────────────────────────────────────

/// Extract text dari serde_json::Value (string atau array of {type,text})
fn extract_value_content(val: &Value) -> Option<String> {
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = val.as_array() {
        let mut out = String::new();
        for item in arr {
            if let Some(obj) = item.as_object() {
                let is_text = obj.get("type").and_then(|t| t.as_str()) == Some("text");
                if is_text && let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                    out.push_str(text);
                    out.push('\n');
                }
            }
        }
        return if out.is_empty() {
            None
        } else {
            Some(out.trim_end().to_string())
        };
    }
    None
}

/// Normalize berbagai nama tool ke standard OMNI internal
fn normalize_tool_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "bash" | "shell" | "exec" | "run_command" | "execute" => "Bash",
        "read" | "read_file" | "readfile" | "view_file" | "cat" => "Read",
        "grep" | "search" | "search_files" | "find_in_files" => "Grep",
        "web_fetch" | "fetch" | "http_get" | "browse" => "WebFetch",
        "write" | "write_file" | "create_file" => "Write",
        "edit" | "edit_file" | "str_replace" => "Edit",
        _ => name,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_claude_code() {
        let input = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{"stdout":"file.txt"}}"#;
        assert_eq!(detect_agent(input), AgentFormat::ClaudeCode);
    }

    #[test]
    fn test_detect_opencode() {
        let input = r#"{"type":"tool_result","tool":"shell","output":"npm test output","command":"npm test"}"#;
        assert_eq!(detect_agent(input), AgentFormat::OpenCode);
    }

    #[test]
    fn test_detect_codex() {
        let input = r#"{"action":"run","command":"cargo build","result":"Compiling..."}"#;
        assert_eq!(detect_agent(input), AgentFormat::CodexCLI);
    }

    #[test]
    fn test_detect_vscode() {
        let input = r#"{"role":"tool","name":"bash","content":"hello"}"#;
        assert_eq!(detect_agent(input), AgentFormat::VSCodeContinue);
    }

    #[test]
    fn test_extract_array_content() {
        let json: serde_json::Value = serde_json::from_str(
            r#"[{"type":"text","text":"hello"},{"type":"text","text":"world"}]"#,
        )
        .expect("Valid JSON");
        let content = extract_value_content(&json).expect("Content exists");
        assert_eq!(content, "hello\nworld");
    }

    #[test]
    fn test_normalize_claude() {
        let input = r#"{"tool_name":"Bash","tool_input":{"command":"echo hello"},"tool_response":{"stdout":"hello"}}"#;
        let norm = normalize(input).expect("Normalized successfully");
        assert_eq!(norm.agent_id, "claude_code");
        assert_eq!(norm.tool_name, "Bash");
        assert_eq!(norm.content, "hello");
    }

    #[test]
    fn test_normalize_opencode() {
        let input =
            r#"{"type":"tool_result","tool":"shell","output":"hello","command":"echo hello"}"#;
        let norm = normalize(input).expect("Normalized successfully");
        assert_eq!(norm.agent_id, "opencode");
        assert_eq!(norm.tool_name, "Bash");
        assert_eq!(norm.content, "hello");
    }

    #[test]
    fn test_detect_pi() {
        let input = r#"{"hookEventName":"ToolResult","toolName":"Bash","toolResponse":{"toolName":"Bash","result":"hello","isError":false},"isError":false}"#;
        assert_eq!(detect_agent(input), AgentFormat::Pi);
    }

    #[test]
    fn test_normalize_pi_bash() {
        let input = r#"{"hookEventName":"ToolResult","toolName":"Bash","toolResponse":{"toolName":"Bash","result":"hello world","isError":false},"isError":false}"#;
        let norm = normalize(input).expect("Normalized Pi payload");
        assert_eq!(norm.agent_id, "pi");
        assert_eq!(norm.tool_name, "Bash");
        assert_eq!(norm.content, "hello world");
    }

    #[test]
    fn test_normalize_pi_read() {
        let input = r#"{"hookEventName":"ToolResult","toolName":"Read","toolResponse":{"toolName":"Read","result":"fn main() { println!(\"hi\"); }","isError":false},"isError":false}"#;
        let norm = normalize(input).expect("Normalized Pi Read payload");
        assert_eq!(norm.agent_id, "pi");
        assert_eq!(norm.tool_name, "Read");
        assert!(norm.content.contains("fn main"));
    }

    #[test]
    fn test_normalize_pi_empty_result() {
        let input = r#"{"hookEventName":"ToolResult","toolName":"Bash","toolResponse":{"toolName":"Bash","result":"","isError":false},"isError":false}"#;
        assert!(
            normalize(input).is_none(),
            "Empty result should return None"
        );
    }

    #[test]
    fn test_normalize_pi_missing_tool_response() {
        let input = r#"{"hookEventName":"ToolResult","toolName":"Bash","isError":false}"#;
        assert!(
            normalize(input).is_none(),
            "Missing toolResponse should return None"
        );
    }

    #[test]
    fn test_pi_vs_claude_code_disambiguation() {
        // Claude Code uses snake_case, Pi uses camelCase
        let claude = r#"{"tool_name":"Bash","tool_response":{"stdout":"hi"}}"#;
        let pi = r#"{"toolName":"Bash","toolResponse":{"result":"hi","isError":false}}"#;

        assert_eq!(detect_agent(claude), AgentFormat::ClaudeCode);
        assert_eq!(detect_agent(pi), AgentFormat::Pi);
    }
}
