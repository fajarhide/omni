use crate::paths;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct AgentConfig {
    pub aggressiveness: Option<String>, // "conservative" | "balanced" | "aggressive"
    pub enable_readfile_distillation: Option<bool>,
    pub enable_grep_distillation: Option<bool>,
    pub enable_webfetch_distillation: Option<bool>,
}

impl AgentConfig {
    pub fn route_thresholds(&self) -> (f32, f32) {
        match self.aggressiveness.as_deref().unwrap_or("balanced") {
            "conservative" => (0.75, 0.40),
            "aggressive" => (0.60, 0.20),
            _ => (0.70, 0.30), // balanced default
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
        .unwrap_or(3.0)
}
