use std::{fs, path::Path};

use gpui::Global;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppSection {
    #[serde(default = "default_app_name")]
    pub name: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatSection {
    #[serde(default = "default_chat_model")]
    pub default_model: String,
    #[serde(default = "default_chat_endpoint")]
    pub api_endpoint: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_max_context")]
    pub max_context_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlayerSection {
    #[serde(default)]
    pub scan_directories: Vec<String>,
    #[serde(default = "default_resume_playback")]
    pub resume_playback: bool,
    #[serde(default = "default_volume")]
    pub default_volume: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TursoSection {
    #[serde(default)]
    pub database_url: Option<String>,
    #[serde(default)]
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CredentialsSection {
    #[serde(default)]
    pub openai_api_key: Option<String>,
    #[serde(default)]
    pub local_llm_api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub app: AppSection,
    #[serde(default)]
    pub chat: ChatSection,
    #[serde(default)]
    pub player: PlayerSection,
    #[serde(default)]
    pub turso: TursoSection,
    #[serde(default)]
    pub credentials: CredentialsSection,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app: AppSection::default(),
            chat: ChatSection::default(),
            player: PlayerSection::default(),
            turso: TursoSection::default(),
            credentials: CredentialsSection::default(),
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Self {
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };

        toml::from_str(&contents).unwrap_or_else(|err| {
            tracing::warn!("Failed to parse config at {:?}: {err}", path);
            Self::default()
        })
    }
}

impl Default for AppSection {
    fn default() -> Self {
        Self {
            name: default_app_name(),
            theme: default_theme(),
        }
    }
}

impl Default for ChatSection {
    fn default() -> Self {
        Self {
            default_model: default_chat_model(),
            api_endpoint: default_chat_endpoint(),
            api_key: None,
            max_context_tokens: default_max_context(),
        }
    }
}

impl Default for PlayerSection {
    fn default() -> Self {
        Self {
            scan_directories: Vec::new(),
            resume_playback: default_resume_playback(),
            default_volume: default_volume(),
        }
    }
}

impl Default for TursoSection {
    fn default() -> Self {
        Self {
            database_url: None,
            auth_token: None,
        }
    }
}

impl Default for CredentialsSection {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            local_llm_api_key: None,
        }
    }
}

fn default_app_name() -> String {
    "MrChat".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_chat_model() -> String {
    "gpt-4.1".to_string()
}

fn default_chat_endpoint() -> String {
    String::new()
}

fn default_max_context() -> u32 {
    8192
}

fn default_resume_playback() -> bool {
    true
}

fn default_volume() -> f32 {
    0.65
}

#[derive(Clone)]
pub struct AppConfigGlobal {
    pub config: AppConfig,
}

impl Global for AppConfigGlobal {}
