use core::fmt;

#[derive(Debug, Clone, Copy)]
pub enum Provider {
    OpenAI,
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenAI => f.write_str("ChatGPT"),
        }
    }
}
