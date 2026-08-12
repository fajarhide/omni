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
    /// The host's original `tool_response` object, kept verbatim.
    ///
    /// `content` above is a flattened, lossy view of it, stdout and stderr are
    /// concatenated, and every other key is dropped. That was fine while the
    /// replacement was emitted in OMNI's own `{status, result}` shape, and became
    /// the bug in #187: Claude Code validates `updatedToolOutput` against the
    /// *host tool's* output schema, so the reply has to be the same object shape
    /// that arrived, with only the output text swapped. Reconstructing it from
    /// `content` is not possible, `interrupted`, `isImage`, `backgroundTaskId`
    /// and friends live only here.
    ///
    /// `None` for agents whose payload has no `tool_response` object; those keep
    /// the MCP shape, since their host contracts were not investigated.
    pub raw_response: Option<Value>,
    /// The host's own session identifier, read from the payload's top-level
    /// `session_id` (or `sessionId`).
    ///
    /// `SessionState::session_id` is minted from the wall clock in
    /// `SessionState::new`, and the persisted state is global rather than
    /// per-host-session, so one OMNI "session" collected 16 project paths and
    /// 3,739 commands (#118). Every per-session number is computed off that
    /// grouping: the PostToolUse banner, the `omni stats` slices, and the
    /// turns-since-insertion a cumulative saving would multiply by (#173).
    ///
    /// The host has always sent this, `hooks::session_start` parses the same
    /// key and uses it for one log line before discarding it. `None` when the
    /// payload carries no such key (pipe mode, Aider, older hosts), and the
    /// caller falls back to the local id rather than losing the row.
    pub host_session_id: Option<String>,
}

/// Detect agent format dari raw JSON string
pub fn detect_agent(input: &str) -> AgentFormat {
    // Coba parse sebagai JSON
    let Ok(val) = serde_json::from_str::<Value>(input) else {
        // Bukan JSON, mungkin piped stdin (Aider)
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

    let mut normalized = match agent {
        AgentFormat::ClaudeCode | AgentFormat::Unknown => normalize_claude_code(input, agent_id),
        AgentFormat::CursorWindsurf => {
            // Cursor punya content array, tangani itu dulu, lalu delegate ke Claude Code parser
            normalize_cursor(input, agent_id)
        }
        AgentFormat::OpenCode => normalize_opencode(input, agent_id),
        AgentFormat::VSCodeContinue => normalize_vscode_continue(input, agent_id),
        AgentFormat::CodexCLI => normalize_codex(input, agent_id),
        AgentFormat::Aider => normalize_aider(input, agent_id),
        AgentFormat::GenericMCP => normalize_generic_mcp(input, agent_id),
        AgentFormat::Pi => normalize_pi(input, agent_id),
    }?;

    // Read once here rather than in each of the eight per-agent parsers: the key
    // is top-level metadata beside `tool_name`, so it does not vary with the
    // tool-payload shape those functions exist to tell apart.
    normalized.host_session_id = extract_host_session_id(input);
    Some(normalized)
}

/// Pull the host's session identifier out of a raw hook payload.
///
/// Both spellings are accepted for the same reason `hooks::session_start` accepts
/// both: the hosts disagree, and guessing one costs the grouping silently.
fn extract_host_session_id(input: &str) -> Option<String> {
    let value: Value = serde_json::from_str(input).ok()?;
    let obj = value.as_object()?;
    obj.get("session_id")
        .or_else(|| obj.get("sessionId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Detect agent ID untuk session isolation
/// Claude Code: "claude_code"
/// OpenCode: "opencode"
/// VS Code: "vscode"
/// dll.
pub fn detect_agent_id(agent: &AgentFormat) -> String {
    // Pure, and it has to stay pure: `hooks::post_tool` keys the Claude Code
    // host-cap branch off this value, so it describes the *payload contract*.
    // Reading the environment here made a behaviour decision depend on ambient
    // state and let a concurrently running test relabel it, which is the
    // cross-test hazard `CONTRIBUTING.md` names. The stats label is a different
    // question and is answered at the point of recording (see `stats_agent_id`).
    resolve_agent_id(agent, "")
}

/// Which host to file this row under in `omni stats`.
///
/// Separate from `detect_agent_id` because they answer different questions.
/// That one describes the payload contract and drives behaviour; this one names
/// the host and drives reporting only.
///
/// Codex CLI sends Claude Code's exact payload document, so shape alone filed
/// every Codex distillation under `claude_code` and Codex could never appear in
/// Agent Distribution however well its hooks worked (#351). Where the format is
/// ambiguous the environment decides; a format that identifies itself keeps its
/// own answer, so a stale env var cannot relabel Aider's pipe as Codex.
pub fn stats_agent_id(agent: &AgentFormat) -> String {
    resolve_agent_id(agent, &crate::agents::multiagent::detect_agent_id())
}

/// The decision, split from the environment lookup so it can be tested without
/// `set_var`. `cargo` runs tests in parallel and a test that mutates the
/// environment decides what a concurrently running one sees, which is the
/// failure `CONTRIBUTING.md` calls out.
///
/// It also stops the probe lying to you: run the naive check under Claude Code
/// and `CLAUDECODE` is already set in the ambient environment, so a Codex
/// payload answers `claude_code` and the fix looks broken when it is not.
/// True when the environment identified a specific host rather than falling back.
///
/// `terminal` is the fallback `detect_agent_id` returns when it recognises
/// nothing, and `unknown` is the same idea by another name. Treating either as
/// an answer lets a guess overwrite what the payload's own shape established.
fn names_a_host(from_env: &str) -> bool {
    !matches!(from_env.trim(), "" | "unknown" | "terminal")
}

pub fn resolve_agent_id(agent: &AgentFormat, from_env: &str) -> String {
    // `detect_agent_id` never answers "unknown": its last resort is "terminal"
    // (see `agents::multiagent`). The old guard therefore never fired, and a
    // Codex session started from a plain shell, whose payload is shaped exactly
    // like Claude Code's, was filed under `terminal`. That is the bucket #212
    // showed inflates the savings headline, so the row both disappeared from
    // Agent Distribution and corrupted the figure it landed in.
    //
    // The environment is only allowed to override when it actually names a host.
    if matches!(agent, AgentFormat::ClaudeCode | AgentFormat::Unknown) && names_a_host(from_env) {
        return from_env.to_string();
    }

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
        /// Kept as a raw `Value` rather than a typed struct so the untouched
        /// object can be handed back to the host (#187). Deserialising into
        /// named fields here is what discarded `interrupted` / `isImage` and
        /// made a host-shaped reply impossible to build downstream.
        tool_response: Option<Value>,
    }
    #[derive(Deserialize)]
    struct ClaudeToolInput {
        command: Option<String>,
        path: Option<String>,
        /// What Claude Code's `Read` tool actually names its argument. Without
        /// it the `Read` arm sees `"unknown"` as the path, so `readfile`'s
        /// per-language distillation cannot pick a language (#172).
        file_path: Option<String>,
    }

    let parsed: ClaudeInput = serde_json::from_str(input).ok()?;

    // Extract content (sama persis dengan extract_tool_content yang lama)
    let response = parsed.tool_response.as_ref()?;
    let content = if let Some(c) = response.get("content") {
        extract_value_content(c)?
    } else if let Some(file_content) = response
        .get("file")
        .and_then(|f| f.get("content"))
        .and_then(Value::as_str)
    {
        // Claude Code's `Read` result carries its text at `file.content`, beside
        // `numLines` / `startLine` / `totalLines`. Neither arm around this one
        // matches that shape, so a `Read` payload normalised to `None` and the
        // hook emitted nothing. That is the *second* reason the `Read` distiller
        // has never run: #172 found the matcher naming only `Bash`, and widening
        // it alone would have produced silence rather than distillation, no
        // output, no error, and a feature that looks shipped.
        if file_content.is_empty() {
            return None;
        }
        file_content.to_string()
    } else {
        let stdout = response.get("stdout").and_then(Value::as_str)?;
        if stdout.is_empty() {
            return None;
        }
        let mut s = stdout.to_string();
        if let Some(stderr) = response.get("stderr").and_then(Value::as_str)
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
        .and_then(|i| {
            i.command
                .as_deref()
                .or(i.path.as_deref())
                .or(i.file_path.as_deref())
        })
        .unwrap_or("")
        .to_string();

    Some(NormalizedInput {
        agent: AgentFormat::ClaudeCode,
        tool_name: parsed.tool_name,
        command,
        content,
        agent_id,
        // Claude Code sends a failed command as a bare `tool_response` string
        // ("Error: Exit code N…"). A string has no `content` and no `stdout`
        // member, so both extraction arms above bail to None (passthrough)
        // before reaching here. Reaching this point means the tool_response was
        // an object carrying output, i.e. the command succeeded.
        //
        // #187 moved where that bail happens, it used to be serde rejecting a
        // string for a typed `ClaudeToolResponse`, and is now `Value::get`
        // returning None, but not whether it happens. `passes_through_failed_
        // command_payload` locks the behaviour, not the mechanism.
        failed: false,
        raw_response: parsed.tool_response,
        // Set by `normalize` for every agent; see the field's doc comment.
        host_session_id: None,
    })
}

// ── CURSOR / WINDSURF ─────────────────────────────────────────────────
/// Reached only for a Claude-Code-shaped payload that happens to carry an array
/// `tool_response.content`, which is what Windsurf sends. **Cursor itself never
/// gets here, and distilling its command output is not possible at all.**
///
/// Cursor's own hook payloads are a different shape:
///
/// ```text
/// postToolUse          { tool_name, tool_input, tool_output, tool_use_id, cwd, duration }
/// afterShellExecution  { command, output, duration, sandbox }
/// ```
///
/// The output is `tool_output` or `output` at the top level, never
/// `tool_response`, so `detect_agent_id` does not route them here and the parser
/// below could not read them if it did. Driven through the 0.6.13 release binary
/// with both documented shapes: zero bytes returned, zero rows recorded.
///
/// Teaching this function those shapes would still change nothing, which is why
/// it has not been. Cursor's only output-rewriting field is
/// `updated_mcp_tool_output`, documented as "For MCP tools only", and
/// `afterShellExecution` defines no output fields. A hook cannot replace the
/// output of a Shell tool on Cursor, so there is nowhere for a distillation to
/// go. `additional_context` exists and adds tokens, which is the opposite of the
/// job. See #340; the README says this per host now rather than listing names
/// under one verb (#349).
///
/// What the Cursor integration does deliver: the pre-hook on
/// `beforeShellExecution`, the MCP server, and shared session state.
fn normalize_cursor(input: &str, agent_id: String) -> Option<NormalizedInput> {
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
        // Host contract not investigated (#187), keeps the MCP shape.
        raw_response: None,
        // Set by `normalize` for every agent; see the field's doc comment.
        host_session_id: None,
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
        // Host contract not investigated (#187), keeps the MCP shape.
        raw_response: None,
        // Set by `normalize` for every agent; see the field's doc comment.
        host_session_id: None,
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
        // Host contract not investigated (#187), keeps the MCP shape.
        raw_response: None,
        // Set by `normalize` for every agent; see the field's doc comment.
        host_session_id: None,
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
        // Host contract not investigated (#187), keeps the MCP shape.
        raw_response: None,
        // Set by `normalize` for every agent; see the field's doc comment.
        host_session_id: None,
    })
}

// ── AIDER ─────────────────────────────────────────────────────────────
fn normalize_aider(input: &str, agent_id: String) -> Option<NormalizedInput> {
    // Aider pakai piped stdin, content adalah raw string, command dari OMNI_CMD
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
        // Host contract not investigated (#187), keeps the MCP shape.
        raw_response: None,
        // Set by `normalize` for every agent; see the field's doc comment.
        host_session_id: None,
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

    // Command tidak bisa di-detect dari JSON-RPC result, gunakan OMNI_CMD env
    let command = std::env::var("OMNI_CMD").unwrap_or_default();

    Some(NormalizedInput {
        agent: AgentFormat::GenericMCP,
        tool_name: "Bash".to_string(),
        command,
        content,
        agent_id,
        failed,
        // Host contract not investigated (#187), keeps the MCP shape.
        raw_response: None,
        // Set by `normalize` for every agent; see the field's doc comment.
        host_session_id: None,
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

    /// Review finding: the guard tested for "unknown", a value
    /// `detect_agent_id` never produces. Its real fallback is "terminal", so a
    /// Codex session from a plain shell (whose payload is shaped exactly like
    /// Claude Code's) was filed under `terminal`: gone from Agent Distribution,
    /// and dropped into the bucket #212 showed inflates the savings headline.
    #[test]
    fn a_generic_environment_does_not_overwrite_the_payloads_own_answer() {
        assert_eq!(
            resolve_agent_id(&AgentFormat::ClaudeCode, "terminal"),
            "claude_code"
        );
        assert_eq!(
            resolve_agent_id(&AgentFormat::Unknown, "terminal"),
            "unknown"
        );
    }

    /// A named host still wins, which is the whole point of consulting the
    /// environment for payloads two hosts share.
    #[test]
    fn a_named_host_still_wins_over_an_ambiguous_payload() {
        assert_eq!(
            resolve_agent_id(&AgentFormat::ClaudeCode, "codex_cli"),
            "codex_cli"
        );
    }
    use super::*;

    /// #351: Codex CLI sends Claude Code's exact payload document, so shape alone
    /// labelled every Codex distillation `claude_code` and Codex could never
    /// appear in `omni stats` Agent Distribution however well its hooks worked.
    /// Where the format is ambiguous the environment decides.
    #[test]
    fn the_environment_decides_when_two_hosts_share_a_payload_shape() {
        assert_eq!(
            resolve_agent_id(&AgentFormat::ClaudeCode, "codex_cli"),
            "codex_cli"
        );
        assert_eq!(
            resolve_agent_id(&AgentFormat::ClaudeCode, "claude_code"),
            "claude_code"
        );
        assert_eq!(
            resolve_agent_id(&AgentFormat::Unknown, "gemini"),
            "gemini",
            "an unrecognised shape has nothing better to go on"
        );
    }

    /// The other half, and the reason this is not just "prefer the environment":
    /// a payload that identifies itself keeps its own answer. An env var left
    /// over from a parent process must not relabel Aider's pipe as Codex.
    #[test]
    fn a_self_identifying_payload_ignores_the_environment() {
        assert_eq!(resolve_agent_id(&AgentFormat::Aider, "codex_cli"), "aider");
        assert_eq!(resolve_agent_id(&AgentFormat::Pi, "codex_cli"), "pi");
        assert_eq!(
            resolve_agent_id(&AgentFormat::CursorWindsurf, "codex_cli"),
            "cursor"
        );
    }

    /// An empty or unknown environment must not blank the answer.
    #[test]
    fn falls_back_to_the_format_when_the_environment_says_nothing() {
        assert_eq!(
            resolve_agent_id(&AgentFormat::ClaudeCode, ""),
            "claude_code"
        );
        assert_eq!(
            resolve_agent_id(&AgentFormat::ClaudeCode, "unknown"),
            "claude_code"
        );
    }

    #[test]
    fn test_detect_claude_code() {
        let input = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{"stdout":"file.txt"}}"#;
        assert_eq!(detect_agent(input), AgentFormat::ClaudeCode);
    }

    /// #118: the host sends its own session id and OMNI dropped it, grouping
    /// every project under one wall-clock stamp instead.
    #[test]
    fn carries_the_host_session_id_through_normalisation() {
        // Arrange
        let input = r#"{"session_id":"4ba52c00-c43f-46ed-9e0e-9069d5294302",
            "hook_event_name":"PostToolUse","tool_name":"Bash",
            "tool_input":{"command":"ls"},"tool_response":{"stdout":"file.txt"}}"#;

        // Act
        let normalized = normalize(input).expect("claude payload normalises");

        // Assert
        assert_eq!(
            normalized.host_session_id.as_deref(),
            Some("4ba52c00-c43f-46ed-9e0e-9069d5294302")
        );
    }

    /// camelCase is accepted for the same reason `session_start` accepts it:
    /// the hosts disagree, and guessing one spelling loses the grouping quietly.
    #[test]
    fn accepts_the_camel_case_spelling_of_the_host_session_id() {
        let input = r#"{"sessionId":"abc-123","tool_name":"Bash",
            "tool_input":{"command":"ls"},"tool_response":{"stdout":"file.txt"}}"#;

        let normalized = normalize(input).expect("claude payload normalises");

        assert_eq!(normalized.host_session_id.as_deref(), Some("abc-123"));
    }

    /// A payload without the key must normalise as before, so the caller can
    /// fall back to the local id rather than lose the row.
    #[test]
    fn reports_no_host_session_id_when_the_payload_omits_it() {
        let input = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},
            "tool_response":{"stdout":"file.txt"}}"#;

        let normalized = normalize(input).expect("claude payload normalises");

        assert!(normalized.host_session_id.is_none());
    }

    /// An empty string would group every such payload together under `""`,
    /// which is the #118 defect with a different key.
    #[test]
    fn treats_a_blank_host_session_id_as_absent() {
        let input = r#"{"session_id":"   ","tool_name":"Bash",
            "tool_input":{"command":"ls"},"tool_response":{"stdout":"file.txt"}}"#;

        let normalized = normalize(input).expect("claude payload normalises");

        assert!(normalized.host_session_id.is_none());
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
