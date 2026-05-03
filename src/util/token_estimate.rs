#[allow(dead_code)]
pub enum ContentHint {
    Code,
    BuildLog,
    Prose,
    Mixed,
    Json,
}

pub fn estimate_tokens(bytes: usize, hint: ContentHint) -> usize {
    let chars_per_token = match hint {
        ContentHint::Code => 3.2,
        ContentHint::BuildLog => 3.8,
        ContentHint::Prose => 4.5,
        ContentHint::Mixed => 3.8,
        ContentHint::Json => 2.8,
    };
    (bytes as f64 / chars_per_token).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimate_code_lower_than_prose() {
        let bytes = 1000;
        let code_tokens = estimate_tokens(bytes, ContentHint::Code);
        let prose_tokens = estimate_tokens(bytes, ContentHint::Prose);
        assert!(
            code_tokens > prose_tokens,
            "Code should yield more tokens than prose for same bytes"
        );
    }
}
