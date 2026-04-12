use serde::Deserialize;
use std::fs;
use crate::paths;

#[derive(Debug, Deserialize, Default)]
pub struct OmniConfig {
    pub pricing: Option<PricingConfig>,
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
