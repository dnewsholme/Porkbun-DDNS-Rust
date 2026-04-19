// src/config.rs

use crate::errors::{DdnsError, Result};
use std::env;

const DEFAULT_CHECK_INTERVAL: u64 = 300;

/// Holds the application's configuration.
#[derive(Debug)]
pub struct Config {
    pub api_key: String,
    pub secret_api_key: String,
    pub domain: String,
    pub subdomains: Vec<String>,
    pub check_interval_seconds: u64,
}

impl Config {
    /// Loads configuration from environment variables.
    pub fn from_env() -> Result<Self> {
        let api_key = env::var("PORKBUN_API_KEY").map_err(|_| {
            DdnsError::Config("PORKBUN_API_KEY environment variable not set.".to_string())
        })?;
        let secret_api_key = env::var("PORKBUN_SECRET_API_KEY").map_err(|_| {
            DdnsError::Config("PORKBUN_SECRET_API_KEY environment variable not set.".to_string())
        })?;
        let domain = env::var("PORKBUN_DOMAIN").map_err(|_| {
            DdnsError::Config("PORKBUN_DOMAIN environment variable not set.".to_string())
        })?;

        let subdomains_str = env::var("PORKBUN_SUBDOMAIN").unwrap_or_else(|_| "".to_string());
        let subdomains: Vec<String> = subdomains_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let check_interval_seconds = env::var("PORKBUN_CHECK_INTERVAL_SECONDS")
            .unwrap_or_else(|_| DEFAULT_CHECK_INTERVAL.to_string()) // Default check interval
            .parse::<u64>()
            .map_err(|_| {
                DdnsError::Config(
                    "PORKBUN_CHECK_INTERVAL_SECONDS must be a valid number.".to_string(),
                )
            })?;

        Ok(Config {
            api_key,
            secret_api_key,
            domain,
            subdomains,
            check_interval_seconds,
        })
    }
}
