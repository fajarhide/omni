use crate::paths;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DistillationMode {
    Debug,
    #[serde(alias = "conservative")]
    Conservative,
    #[default]
    Balanced,
    Efficient,
    #[serde(alias = "aggressive")]
    Aggressive,
    Auto,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct AgentConfig {
    #[serde(alias = "aggressiveness")]
    pub mode: Option<DistillationMode>,
    pub enable_readfile_distillation: Option<bool>,
    pub enable_grep_distillation: Option<bool>,
    pub enable_webfetch_distillation: Option<bool>,
}

impl AgentConfig {
    pub fn route_thresholds(&self) -> (f32, f32) {
        let mode = self.mode.clone().unwrap_or(DistillationMode::Balanced);
        match mode {
            DistillationMode::Debug => (0.90, 0.50),
            DistillationMode::Conservative => (0.75, 0.40),
            DistillationMode::Efficient | DistillationMode::Aggressive => (0.60, 0.20),
            DistillationMode::Balanced | DistillationMode::Auto => (0.70, 0.30),
        }
    }

    pub fn readfile_enabled(&self) -> bool {
        self.enable_readfile_distillation.unwrap_or(true)
    }

    pub fn grep_enabled(&self) -> bool {
        self.enable_grep_distillation.unwrap_or(true)
    }

    pub fn webfetch_enabled(&self) -> bool {
        self.enable_webfetch_distillation.unwrap_or(true)
    }
}

#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input: f64,
    pub _output: f64,
    pub _cached: f64,
    pub _reasoning: f64,
    pub _cache_creation: f64,
}

/// Menggunakan estimasi harga rata-rata LLM modern
/// dibandingkan mendefinisikan harganya satu per satu
pub fn get_pricing_for_model(model: &str) -> ModelPricing {
    let m = model.to_lowercase();
    if m.contains("claude-3-7-sonnet") || m.contains("claude-3-5-sonnet") {
        ModelPricing {
            input: 3.0,
            _output: 15.0,
            _cached: 1.5,
            _reasoning: 15.0,
            _cache_creation: 3.0,
        }
    } else if m.contains("gpt-4o") {
        ModelPricing {
            input: 2.5,
            _output: 10.0,
            _cached: 1.25,
            _reasoning: 10.0,
            _cache_creation: 2.5,
        }
    } else {
        // Gabungan rata-rata dari semua model unggulan (Claude + GPT-4o)
        ModelPricing {
            input: (3.0 + 3.0 + 2.5) / 3.0,
            _output: (15.0 + 15.0 + 10.0) / 3.0,
            _cached: (1.5 + 1.5 + 1.25) / 3.0,
            _reasoning: (15.0 + 15.0 + 10.0) / 3.0,
            _cache_creation: (3.0 + 3.0 + 2.5) / 3.0,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct OmniConfig {
    pub global: Option<AgentConfig>,
    pub agents: Option<std::collections::HashMap<String, AgentConfig>>,
    pub pricing: Option<PricingConfig>,
}

impl OmniConfig {
    pub fn for_agent(&self, agent_id: &str) -> AgentConfig {
        self.agents
            .as_ref()
            .and_then(|a| a.get(agent_id))
            .cloned()
            .or_else(|| self.global.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct PricingConfig {
    pub input_cost_per_million_tokens: Option<f64>,
}

pub fn load_config() -> OmniConfig {
    let path = paths::omni_home().join("config.toml");

    if !path.exists() {
        return OmniConfig::default();
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return OmniConfig::default(),
    };

    toml::from_str(&content).unwrap_or_default()
}

pub fn get_input_cost() -> f64 {
    let config = load_config();
    config
        .pricing
        .and_then(|p| p.input_cost_per_million_tokens)
        .unwrap_or_else(|| get_pricing_for_model("average").input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_distillation_mode_with_backward_compatibility() {
        // Test new exact key
        let toml_str = r#"
            [global]
            mode = "efficient"
        "#;
        let config: OmniConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.global.unwrap().mode,
            Some(DistillationMode::Efficient)
        );

        // Test old alias functionality
        let toml_str2 = r#"
            [global]
            aggressiveness = "conservative"
        "#;
        let config2: OmniConfig = toml::from_str(toml_str2).unwrap();
        assert_eq!(
            config2.global.unwrap().mode,
            Some(DistillationMode::Conservative)
        );

        let toml_str3 = r#"
            [global]
            aggressiveness = "aggressive"
        "#;
        let config3: OmniConfig = toml::from_str(toml_str3).unwrap();
        assert_eq!(
            config3.global.unwrap().mode,
            Some(DistillationMode::Aggressive)
        );
    }

    #[test]
    fn computes_correct_route_thresholds() {
        let mut cfg = AgentConfig {
            mode: Some(DistillationMode::Debug),
            ..Default::default()
        };
        assert_eq!(cfg.route_thresholds(), (0.90, 0.50));

        cfg.mode = Some(DistillationMode::Aggressive);
        assert_eq!(cfg.route_thresholds(), (0.60, 0.20));

        cfg.mode = Some(DistillationMode::Efficient);
        assert_eq!(cfg.route_thresholds(), (0.60, 0.20));

        cfg.mode = None; // fallback
        assert_eq!(cfg.route_thresholds(), (0.70, 0.30));
    }
}
