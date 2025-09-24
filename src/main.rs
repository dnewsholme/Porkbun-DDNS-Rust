// main.rs

mod config;
mod errors;
mod ip_fetcher;
mod porkbun;

use crate::config::Config;
use crate::porkbun::PorkbunClient;
use dotenv::dotenv;
use env_logger::Builder;
use log::{error, info, LevelFilter};
use tokio::time::{sleep, Duration};

// Main asynchronous function where the program execution begins.
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize the logger.
    Builder::new()
        .filter_level(LevelFilter::Info) // Set default log level to INFO
        .init();

    info!("Starting Porkbun Dynamic DNS Updater...");

    dotenv().ok();

    let config = Config::from_env().expect("Failed to load configuration from environment.");

    // Create an HTTP client for making requests.
    let client = reqwest::Client::new();

    loop {
        info!("--- Starting new check cycle ---");
        let current_ip_result = ip_fetcher::get_current_ipv4().await;

        match current_ip_result {
            Ok(current_ip) => {
                let porkbun_client = PorkbunClient::new(&client, &config.api_key, &config.secret_api_key, &config.domain);

                for subdomain in &config.subdomains {
                    info!("Processing subdomain: '{}'", if subdomain.is_empty() { &config.domain } else { &subdomain });

                    if let Err(e) = process_subdomain(&porkbun_client, subdomain, &current_ip).await {
                        error!("Error processing subdomain '{}': {}", subdomain, e);
                    }
                }
            }
            Err(e) => {
                error!("Error getting current public IPv4 address: {}", e);
            }
        }

        info!("--- Check cycle finished. Sleeping for {} seconds ---", config.check_interval_seconds);
        sleep(Duration::from_secs(config.check_interval_seconds)).await;
    }
}

async fn process_subdomain(porkbun_client: &PorkbunClient<'_>, subdomain: &str, current_ip: &str) -> errors::Result<()> {
    let domain = porkbun_client.domain; // for logging
    match porkbun_client.get_a_record(subdomain).await {
        Ok(Some(record)) => {
            // Logic for EXISTING Record (Update if IP has changed)
            if record.content == current_ip {
                info!(
                    "Current IP ({}) matches existing Porkbun A record for {}.{}. No update needed.",
                    current_ip, subdomain, domain
                );
            } else {
                info!(
                    "IP change detected for {}.{}! Old IP: {}, New IP: {}",
                    subdomain, domain, record.content, current_ip
                );
                porkbun_client.update_a_record(&record.id, subdomain, current_ip).await?;
            }
        }
        Ok(None) => {
            // Logic for NON-EXISTENT Record (Create it)
            porkbun_client.create_a_record(subdomain, current_ip).await?;
        }
        Err(e) => {
            // Propagate the error up
            return Err(e);
        }
    }
    Ok(())
}