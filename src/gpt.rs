use core::fmt;

#[derive(Debug, Clone, Copy)]
pub enum Provider {
    OpenAI,
}

#[derive(Debug)]
pub struct ChatGPTConfiguration {
    pub api_key: String,
}

impl ChatGPTConfiguration {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenAI => f.write_str("ChatGPT"),
        }
    }
}
