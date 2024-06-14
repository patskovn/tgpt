use core::fmt;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::{fs::create_dir_all, path::PathBuf, str::FromStr};

#[derive(Debug, Clone, Copy)]
pub enum Provider {
    OpenAI,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatGPTConfiguration {
    pub api_key: String,
}

impl ChatGPTConfiguration {
    fn file_path() -> anyhow::Result<PathBuf> {
        let mut dir = configs_directory()?;
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

fn configs_directory() -> anyhow::Result<std::path::PathBuf> {
    let mut dir_path = home_dir().unwrap_or_else(|| PathBuf::from("."));
    dir_path.push(".config");
    dir_path.push("tgpt");

    if !dir_path.exists() {
        std::fs::create_dir_all(dir_path.clone())?;
    }
    Ok(dir_path)
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenAI => f.write_str("ChatGPT"),
        }
    }
}
