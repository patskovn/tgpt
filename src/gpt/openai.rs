use chatgpt::client::ChatGPT;
use chatgpt::config::ChatGPTEngine;
use chatgpt::config::ModelConfiguration;
use chatgpt::types::Role;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct ChatGPTConfiguration {
    pub api_key: String,
}

impl ChatGPTConfiguration {
    fn file_path() -> anyhow::Result<PathBuf> {
        let mut dir = crate::gpt::types::configs_directory()?;
        dir.push("chat_gpt.json");
        Ok(dir)
    }

    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    pub fn open() -> Option<Self> {
        let file_path = Self::file_path().ok()?;
        let file = std::fs::File::open(file_path).ok()?;

        serde_json::from_reader(file).ok()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let file_path = Self::file_path()?;
        let file = std::fs::File::create(file_path)?;
        serde_json::to_writer(file, self)?;
        Ok(())
    }
}

pub struct Api {
    pub client: ChatGPT,
}

pub fn display(role: Role) -> String {
    match role {
        Role::User => "You".to_string(),
        Role::System => "System".to_string(),
        Role::Assistant => "Assistant".to_string(),
        Role::Function => "Function".to_string(),
    }
}

impl Api {
    pub fn new(configuration: ChatGPTConfiguration) -> Self {
        let config = ModelConfiguration {
            engine: ChatGPTEngine::Custom("gpt-4o-mini"),
            ..Default::default()
        };
        Self {
            client: ChatGPT::new_with_config(configuration.api_key, config)
                .expect("proper configuration"),
        }
    }
}
