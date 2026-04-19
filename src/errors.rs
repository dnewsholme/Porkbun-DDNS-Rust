// src/errors.rs

use thiserror::Error;

/// A unified error type for the application.
#[derive(Error, Debug)]
pub enum DdnsError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Porkbun API error: {0}")]
    PorkbunApi(String),
}

pub type Result<T> = std::result::Result<T, DdnsError>;
