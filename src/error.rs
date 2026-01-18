use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),
    
    #[error("Invalid pool data: {0}")]
    InvalidPoolData(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Graph error: {0}")]
    GraphError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Anchor error: {0}")]
    AnchorError(String),
}

pub type Result<T> = std::result::Result<T, BotError>;