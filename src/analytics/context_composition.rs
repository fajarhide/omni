use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextTurn {
    pub turn_number: u32,
    pub session_id: String,
    pub timestamp: i64,

    // Breakdown sources
    pub estimated_total_tokens: u64,
    /// Bytes, and named for them since #589. These were `*_tokens` holding
    /// `size_bytes / 4`, a rougher estimator than the 3.6 the rest of the report
    /// used and still not a Claude token count. The file size and the delivered
    /// length are both counted exactly, so there is nothing to estimate here.
    pub file_read_bytes: u64,
    pub tool_output_bytes: u64,
    pub conversation_tokens: u64,
    pub system_prompt_tokens: u64,

    // Flags
    pub has_duplicate_file_reads: bool,
    pub duplicate_files: Vec<String>,
    pub largest_single_read: (String, u64),
}
