use serde::Serialize;

#[derive(Serialize)]
pub struct VersionJson {
    pub version: String,
    pub build_date: String,
    pub git_hash: String,
    pub features: Vec<String>,
}

pub fn run_version(args: &[String]) {
    let json_flag = args.iter().any(|a| a == "--json");
    let version_str = env!("CARGO_PKG_VERSION").to_string();

    if json_flag {
        let output = VersionJson {
            version: version_str,
            build_date: option_env!("OMNI_BUILD_DATE")
                .unwrap_or("unknown")
                .to_string(),
            git_hash: option_env!("OMNI_GIT_HASH")
                .unwrap_or("unknown")
                .to_string(),
            features: vec![
                "hermes".to_string(),
                "mcp".to_string(),
                "engram".to_string(),
                "handoff".to_string(),
            ],
        };
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("OMNI v{}", version_str);
    }
}

#[cfg(test)]
mod tests {
    use super::VersionJson;

    #[test]
    fn test_version_json_schema_validation() {
        let json_struct = VersionJson {
            version: "0.5.9".to_string(),
            build_date: "2026-06-05".to_string(),
            git_hash: "abc1234".to_string(),
            features: vec!["hermes".to_string(), "mcp".to_string()],
        };

        let json_str = serde_json::to_string(&json_struct).unwrap();
        assert!(json_str.contains("\"version\":\"0.5.9\""));
        assert!(json_str.contains("\"build_date\":\"2026-06-05\""));
        assert!(json_str.contains("\"git_hash\":\"abc1234\""));
        assert!(json_str.contains("\"features\":[\"hermes\",\"mcp\"]"));
    }
}
