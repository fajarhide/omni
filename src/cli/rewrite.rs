use anyhow::Result;

pub fn run_rewrite(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        std::process::exit(1);
    }

    let cmd_str = &args[2];
    if let Some(rewritten) = rewrite_logic(cmd_str) {
        println!("{}", rewritten);
        std::process::exit(0);
    }

    std::process::exit(1);
}

pub fn rewrite_logic(cmd_str: &str) -> Option<String> {
    let allow_list = [
        "git ",
        "cargo ",
        "npm ",
        "pytest ",
        "kubectl ",
        "docker ",
        "terraform ",
        "make ",
        "node ",
        "python ",
        "go ",
    ];

    let wants_rewrite = allow_list.iter().any(|&p| cmd_str.starts_with(p));

    if wants_rewrite {
        // Do not rewrite if it contains shell symbols
        let has_shell_symbols = cmd_str.contains('|')
            || cmd_str.contains('>')
            || cmd_str.contains('<')
            || cmd_str.contains("&&")
            || cmd_str.contains(';');

        if !has_shell_symbols {
            let exe_path =
                std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("omni"));
            let exe_name = exe_path.to_string_lossy();

            return Some(format!("{} exec {}", exe_name, cmd_str));
        }
    }

    None
}
