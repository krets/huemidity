use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DashboardItem {
    pub r#type: String, // "group" or "light"
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Mapping {
    pub target_type: String, // "light", "group", "scene"
    pub target_id: String,
    pub action: String, // "Brightness", "Recall Scene", "On/Off", etc.
    pub invert: bool,
    pub auto_on: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub bridge_ip: String,
    #[serde(default)]
    pub bridge_username: String, // Philips Hue developer key
    #[serde(default)]
    pub selected_device: String,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub dashboard_layout: Vec<DashboardItem>,
    #[serde(default)]
    pub mappings: HashMap<String, HashMap<String, Mapping>>, // Device -> (EventKey -> Mapping)
}

impl AppConfig {
    pub fn get_config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "krets", "HueMIDIty").map(|proj_dirs| {
            let config_dir = proj_dirs.config_dir();
            config_dir.join("config.json")
        })
    }

    pub fn load() -> Self {
        if let Some(path) = Self::get_config_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                        return config;
                    }
                }
            }
        }
        AppConfig::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path) = Self::get_config_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_json::to_string_pretty(self)?;
            fs::write(path, content)?;
        }
        Ok(())
    }
}
