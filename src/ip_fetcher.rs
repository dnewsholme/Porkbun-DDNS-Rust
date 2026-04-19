// src/ip_fetcher.rs

use crate::errors::Result;
use log::info;

/// Asynchronous function to get the current public IPv4 address from an external service.
pub async fn get_current_ipv4(client: &reqwest::Client) -> Result<String> {
    info!("Attempting to retrieve current public IPv4 address from api.ipify.org...");
    let ip = client
        .get("https://api.ipify.org")
        .send()
        .await?
        .text()
        .await?;
    info!("Successfully retrieved current public IPv4: {}", ip);
    Ok(ip.trim().to_string())
}
