//! OMNI HTTP server — generic webhook endpoint for any agent
//!
//! POST /distill
//! Content-Type: application/json
//!
//! Request body (any of these formats auto-detected):
//! - Claude Code format
//! - OpenCode format
//! - Codex CLI format
//! - Raw: { "command": "...", "output": "..." }
//!
//! Response:
//! { "distilled": "...", "original_lines": N, "distilled_lines": M, "agent": "..." }

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
// use crate::hooks::normalize;
use crate::hooks::post_tool;

pub fn run_http_server(port: u16) -> anyhow::Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;
    eprintln!("[OMNI serve] Listening on http://{}", addr);
    eprintln!("[OMNI serve] POST /distill  — distil any agent output");
    eprintln!("[OMNI serve] GET  /stats    — get session stats");
    eprintln!("[OMNI serve] Ctrl+C to stop");

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let store = crate::store::sqlite::Store::open()
            .ok()
            .map(std::sync::Arc::new);

        // Parse HTTP request (minimal, no deps)
        let reader = BufReader::new(&stream);
        let mut lines = reader.lines();

        let first_line = lines.next().and_then(|l| l.ok()).unwrap_or_default();
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        let (method, path) = match parts.as_slice() {
            [m, p, ..] => (*m, *p),
            _ => continue,
        };

        // Skip headers, find Content-Length
        let mut content_length = 0usize;
        for line in lines.by_ref() {
            let line = line.unwrap_or_default();
            if line.is_empty() {
                break;
            }
            if line.to_lowercase().starts_with("content-length:") {
                content_length = line
                    .split(':')
                    .nth(1)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
            }
        }

        let (status, body) = match (method, path) {
            ("POST", "/distill") if content_length > 0 => {
                // Read body
                let mut buf = vec![0u8; content_length.min(5 * 1024 * 1024)];
                let _ = stream.read_exact(&mut buf); // Note: simplified, real impl needs re-read
                let body_str = String::from_utf8_lossy(&buf);

                // Process via OMNI pipeline
                match post_tool::process_payload(&body_str, store, None) {
                    Some(distilled) => {
                        let original_lines = body_str.lines().count();
                        let distilled_text = extract_updated_response(&distilled);
                        let distilled_lines = distilled_text.lines().count();
                        let json = serde_json::json!({
                            "distilled": distilled_text,
                            "original_lines": original_lines,
                            "distilled_lines": distilled_lines,
                            "compression": format!("{:.0}%", (1.0 - distilled_lines as f64 / original_lines.max(1) as f64) * 100.0),
                        });
                        (200, json.to_string())
                    }
                    None => {
                        // No distillation needed — return original
                        let json = serde_json::json!({
                            "distilled": body_str.trim(),
                            "original_lines": body_str.lines().count(),
                            "compression": "0%",
                            "note": "Content too small or no distillation needed"
                        });
                        (200, json.to_string())
                    }
                }
            }
            ("GET", "/stats") => {
                match store
                    .as_ref()
                    .and_then(|_s| serde_json::to_string(&serde_json::json!({"status": "ok"})).ok())
                {
                    Some(j) => (200, j),
                    None => (500, r#"{"error":"store unavailable"}"#.to_string()),
                }
            }
            ("GET", "/health") => (200, r#"{"status":"ok","service":"omni"}"#.to_string()),
            _ => (404, r#"{"error":"not found"}"#.to_string()),
        };

        let response = format!(
            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
            status,
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    }
    Ok(())
}

fn extract_updated_response(json_str: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json_str)
        .ok()
        .and_then(|v| {
            v["hookSpecificOutput"]["updatedResponse"]
                .as_str()
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| json_str.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_health_endpoint() {
        use std::thread;
        // Start server in background thread using high distinct port
        let port = 17892;
        thread::spawn(move || {
            let _ = run_http_server(port);
        });
        std::thread::sleep(std::time::Duration::from_millis(100));

        let mut stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .expect("Server must accept connections");
        let _ = stream.write_all(b"GET /health HTTP/1.1\r\n\r\n");
    }
}
