#[derive(Debug, Clone, Copy)]
pub enum ContentHint {
    Code,
    Prose,
    Json,
    BuildLog,
    Mixed,
}

use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

static TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

fn get_tokenizer() -> &'static CoreBPE {
    TOKENIZER.get_or_init(|| {
        tiktoken_rs::cl100k_base().expect("Failed to initialize cl100k_base tokenizer")
    })
}

/// Count exact tokens using cl100k_base (Claude-compatible approximation)
pub fn count_tokens(text: &str, _model: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let bpe = get_tokenizer();
    bpe.encode_with_special_tokens(text).len()
}

/// Legacy estimation function (keep signature for backward compatibility, but use actual counting if possible,
/// though here we don't have the text, only bytes. So we keep the heuristic for byte-only scenarios).
pub fn estimate_tokens(bytes: usize, hint: ContentHint) -> usize {
    let chars_per_token = match hint {
        ContentHint::Code => 3.2,
        ContentHint::Prose => 4.5,
        ContentHint::Json => 2.8,
        ContentHint::BuildLog => 3.8,
        ContentHint::Mixed => 3.8,
    };
    (bytes as f64 / chars_per_token).ceil() as usize
}
