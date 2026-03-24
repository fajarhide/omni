use crate::pipeline::{DistillResult, SessionState};
use crate::store::sqlite::Store;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;

lazy_static! {
    static ref FILE_PATH_RE: Regex = Regex::new(
        r#"(?ix)(?:^|[\s"'/:=])(?:(?:[a-zA-Z0-9_\-\.][a-zA-Z0-9_\-\./]*)/)?[a-zA-Z0-9_\-\.]+\.(?:rs|py|js|jsx|ts|tsx|go|rb|md|json|toml|yml|yaml|c|cpp|h|hpp|sh|bash|zsh)(?:[\s"':]|$)"#
    ).unwrap();
}

#[allow(dead_code)]
pub struct SessionTracker {
    session: Arc<Mutex<SessionState>>,
    store: Arc<Store>,
}

#[allow(dead_code)]
impl SessionTracker {
    pub fn new(session: Arc<Mutex<SessionState>>, store: Arc<Store>) -> Self {
        Self { session, store }
    }

    pub fn track_command(&self, command: &str, output: &str, _result: &DistillResult) {
        let cmd = command.to_string();
        let out = output.to_string();
        let session = self.session.clone();
        let store = self.store.clone();

        thread::spawn(move || {
            let paths = extract_file_paths(&out);
            let cmd_paths = extract_file_paths(&cmd);
            let errors = extract_errors(&out);

            let mut session_locked = match session.lock() {
                Ok(l) => l,
                Err(_) => return,
            };

            session_locked.add_command(&cmd);

            for p in paths.iter().chain(cmd_paths.iter()) {
                session_locked.add_hot_file(p);
            }

            for err in errors {
                session_locked.add_error(&err);
                store.index_event(&session_locked.session_id, "Error", &err);
            }

            if let Some(task) = infer_task(&session_locked) {
                session_locked.inferred_task = Some(task);
            }

            if let Some(domain) = infer_domain(&session_locked) {
                session_locked.inferred_domain = Some(domain);
            }

            store.index_event(&session_locked.session_id, "Command", &cmd);
            save_async(session.clone(), store.clone());
        });
    }

    #[allow(dead_code)]
    pub fn track_error(&self, error_msg: &str) {
        let err = error_msg.to_string();
        let session = self.session.clone();
        let store = self.store.clone();

        thread::spawn(move || {
            if let Ok(mut lock) = session.lock() {
                lock.add_error(&err);
                store.index_event(&lock.session_id, "Error", &err);
            }
            save_async(session.clone(), store.clone());
        });
    }
}

#[allow(dead_code)]
fn extract_file_paths(text: &str) -> Vec<String> {
    let mut paths = HashSet::new();
    for cap in FILE_PATH_RE.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            let mut path = m
                .as_str()
                .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '/')
                .to_string();
            path = path
                .trim_end_matches(|c: char| !c.is_alphanumeric())
                .to_string();
            if !path.is_empty() {
                paths.insert(path);
            }
        }
    }

    // Fallback naive search if regex bounds struggle (cargo uses specific outputs)
    let words = text.split_whitespace();
    for w in words {
        if w.contains('.')
            && (w.ends_with(".rs")
                || w.ends_with(".py")
                || w.ends_with(".js")
                || w.ends_with(".ts")
                || w.ends_with(".tsx")
                || w.ends_with(".jsx"))
        {
            let clean = w.trim_matches(|c| c == '\'' || c == '"' || c == '(' || c == ')');
            paths.insert(clean.to_string());
        }
    }

    paths.into_iter().collect()
}

#[allow(dead_code)]
fn extract_errors(text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut current_err = String::new();
    let mut capturing = false;

    for line in lines {
        let trimmed = line.trim();
        let is_start = trimmed.starts_with("error[")
            || trimmed.starts_with("ERROR:")
            || trimmed.starts_with("Error:")
            || trimmed.contains("FAILED")
            || trimmed.contains("panic:")
            || trimmed.starts_with("Traceback");

        if is_start {
            if capturing {
                errors.push(truncate_error(&current_err));
                if errors.len() >= 5 {
                    break;
                }
            }
            capturing = true;
            current_err = trimmed.to_string();
        } else if capturing {
            if trimmed.is_empty() || trimmed.starts_with("Warning:") {
                capturing = false;
                errors.push(truncate_error(&current_err));
                if errors.len() >= 5 {
                    break;
                }
                current_err.clear();
            } else {
                current_err.push(' ');
                current_err.push_str(trimmed);
            }
        }
    }

    if capturing && errors.len() < 5 {
        errors.push(truncate_error(&current_err));
    }

    // Deduplicate
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for err in errors {
        if seen.insert(err.clone()) {
            unique.push(err);
        }
    }
    unique
}

#[allow(dead_code)]
fn truncate_error(err: &str) -> String {
    let mut clean = err.replace('\n', " ");
    if clean.len() > 200 {
        clean.truncate(197);
        clean.push_str("...");
    }
    clean
}

#[allow(dead_code)]
pub fn infer_task(session: &SessionState) -> Option<String> {
    let cmds = &session.last_commands;
    let mut task = None;

    let has_cargo_test = cmds.iter().any(|c| c.contains("cargo test"));
    let has_git_diff = cmds.iter().any(|c| c.contains("git diff"));
    let has_npm_build = cmds
        .iter()
        .any(|c| c.contains("npm run build") || c.contains("npm build"));
    let has_kubectl = cmds.iter().any(|c| c.contains("kubectl"));

    if has_cargo_test {
        if !session.active_errors.is_empty() {
            task = Some("fixing rust tests".to_string());
        } else {
            task = Some("running rust tests".to_string());
        }
    } else if has_npm_build {
        if !session.active_errors.is_empty() {
            task = Some("fixing npm build errors".to_string());
        } else {
            task = Some("building npm".to_string());
        }
    } else if has_git_diff && !session.active_errors.is_empty() {
        task = Some("debugging active errors".to_string());
    } else if has_kubectl {
        task = Some("managing kubernetes".to_string());
    } else if let Some(last) = cmds.first() {
        task = Some(format!("running: {}", last));
    }

    task.map(|t| {
        if t.len() > 50 {
            t[..47].to_string() + "..."
        } else {
            t
        }
    })
}

#[allow(dead_code)]
pub fn infer_domain(session: &SessionState) -> Option<String> {
    let paths: Vec<String> = session.hot_files.keys().cloned().collect();
    if paths.is_empty() || paths.len() < 2 {
        if let Some(first) = paths.first()
            && let Some(pos) = first.rfind('/')
        {
            return Some(first[..pos].to_string());
        }
        return None;
    }

    let min_len = paths.iter().map(|p| p.len()).min().unwrap_or(0);
    let mut prefix_len = 0;

    // We should compute common prefix bounded by characters
    let first = &paths[0];
    for r in 0..=min_len {
        let prefix = &first[..r];
        if paths.iter().all(|p| p.starts_with(prefix)) {
            prefix_len = r;
        } else {
            break;
        }
    }

    let common_prefix = &first[..prefix_len];
    if common_prefix.is_empty() || common_prefix == "/" {
        return None;
    }

    let parts: Vec<&str> = common_prefix.split('/').filter(|s| !s.is_empty()).collect();
    parts.last().map(|last| last.to_string())
}

fn save_async(session: Arc<Mutex<SessionState>>, store: Arc<Store>) {
    // Actually we already saved in thread earlier or we can just spawn again explicitly over the requirement bounds
    // Since we spawned thread inside track_command, this just does the synchronous DB hit natively detached.
    let s = match session.lock() {
        Ok(l) => l.clone(),
        Err(_) => return,
    };
    store.upsert_session(&s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_extract_file_paths_for_cargo_output() {
        let text = "Compiling src/main.rs\nerror in tests/my_test.rs:42";
        let paths = extract_file_paths(text);
        assert!(paths.contains(&"src/main.rs".to_string()));
        assert!(paths.contains(&"tests/my_test.rs".to_string()));
    }

    #[test]
    fn test_extract_file_paths_for_git_diff() {
        let text = "diff --git a/components/Button.tsx b/components/Button.tsx";
        let paths = extract_file_paths(text);
        // It might extract 'a/components/Button.tsx' or 'components/Button.tsx' based on words fallback
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_extract_errors_for_rust_compile_error() {
        let text = "warning: unused trait\nerror[E0061]: this function takes 1 arg but 0 were supplied\n  --> src/main.rs\n\nSome other message";
        let errs = extract_errors(text);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("error[E0061]:"));
        assert!(errs[0].contains("src/main.rs"));
    }

    #[test]
    fn test_extract_errors_for_python_traceback() {
        let text = "Traceback (most recent call last):\n  File \"script.py\", line 10, in <module>\nValueError: invalid literal for int()";
        let errs = extract_errors(text);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("ValueError"));
    }

    #[test]
    fn test_infer_domain_for_hot_files_dengan_common_prefix() {
        let mut state = SessionState::new();
        state.add_hot_file("src/auth/mod.rs");
        state.add_hot_file("src/auth/jwt.rs");
        state.add_hot_file("src/auth/middleware.rs");

        let domain = infer_domain(&state);
        // prefix -> "src/auth/"
        // split -> ["src", "auth"] -> last is "auth"
        assert_eq!(domain.unwrap(), "auth");
    }

    #[test]
    fn test_infer_task_for_command_error_pattern() {
        let mut state = SessionState::new();
        state.add_command("cargo test auth");
        state.add_error("missing semicolon");

        let task = infer_task(&state);
        assert_eq!(task.unwrap(), "fixing rust tests");
    }

    #[test]
    fn test_track_command_non_blocking() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let tracker = SessionTracker::new(session, store);

        let start = std::time::Instant::now();
        let res = DistillResult {
            output: "".to_string(),
            route: crate::pipeline::Route::Keep,
            filter_name: "".to_string(),
            content_type: crate::pipeline::ContentType::Unknown,
            score: 0.0,
            context_score: 0.0,
            input_bytes: 0,
            output_bytes: 0,
            latency_ms: 0,
            rewind_hash: None,
            segments_kept: 0,
            segments_dropped: 0,
        };

        tracker.track_command("git status", "On branch main", &res);
        let elapsed = start.elapsed();
        // Should be extremely fast because thread spawns
        assert!(elapsed.as_millis() < 200, "Took {} ms", elapsed.as_millis());
    }

    #[test]
    fn test_background_save_tidak_block_caller() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let start = std::time::Instant::now();
        save_async(session, store);
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 200);
    }
}
