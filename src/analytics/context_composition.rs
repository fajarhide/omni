use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextTurn {
    pub turn_number: u32,
    pub session_id: String,
    pub timestamp: i64,

    // Breakdown sources
    pub estimated_total_tokens: u64,
    pub file_read_tokens: u64,
    pub tool_output_tokens: u64,
    pub conversation_tokens: u64,
    pub system_prompt_tokens: u64,

    // Flags
    pub has_duplicate_file_reads: bool,
    pub duplicate_files: Vec<String>,
    pub largest_single_read: (String, u64),
}
