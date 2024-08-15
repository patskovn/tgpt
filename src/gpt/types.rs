use core::fmt;
use dirs::home_dir;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAI,
}

pub fn configs_directory() -> anyhow::Result<std::path::PathBuf> {
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
