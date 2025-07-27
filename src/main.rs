// main.rs

// Import necessary crates for asynchronous operations, HTTP requests, JSON serialization,
// environment variable loading, logging, and time utilities.
use tokio;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use dotenv::dotenv;
use std::env;
use log::{info, error, warn, LevelFilter};
use env_logger::Builder;
use tokio::time::{sleep, Duration}; // Added for continuous loop delay

// Define structs for Porkbun API request and response bodies.
// These structs use `serde` to automatically serialize/deserialize to/from JSON.

// Struct for the common part of Porkbun API requests (API key and secret key).
#[derive(Serialize)]
struct AuthPayload {
    apikey: String,
    secretapikey: String,
}

// Struct for the DNS record retrieval request payload.
#[derive(Serialize)]
struct RetrieveRecordsPayload {
    #[serde(flatten)] // This flattens the AuthPayload fields into this struct
    auth: AuthPayload,
    name: String, // The full domain or subdomain name (e.g., "example.com" or "sub.example.com")
}

// Struct for the DNS record update request payload.
#[derive(Serialize)]
struct UpdateRecordPayload {
    #[serde(flatten)]
    auth: AuthPayload,
    name: String,    // The subdomain part (e.g., "www" for www.example.com, or "" for example.com)
    #[serde(rename = "type")] // Rename 'type' field to avoid Rust keyword collision
    record_type: String, // e.g., "A" for IPv4
    content: String, // The IP address
    ttl: u32,        // Time To Live in seconds
}

// Struct to represent a single DNS record in the Porkbun API response.
#[derive(Debug, Deserialize)]
struct DnsRecord {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: String, // TTL is returned as a string, we'll parse it to u32 if needed
    id: String,  // Record ID, needed for updates
}

// Struct for the response when retrieving DNS records.
#[derive(Debug, Deserialize)]
struct RetrieveRecordsResponse {
    status: String,
    records: Option<Vec<DnsRecord>>, // Option because 'records' might be null if none found
    message: Option<String>,
}

// Struct for the general Porkbun API response (e.g., for update or ping).
#[derive(Debug, Deserialize)]
struct ApiResponse {
    status: String,
    message: Option<String>,
}

// Asynchronous function to get the current public IPv4 address from an external service.
async fn get_current_ipv4() -> Result<String, reqwest::Error> {
    info!("Attempting to retrieve current public IPv4 address...");
    let response = reqwest::get("https://api.ipify.org").await?;
    let ip = response.text().await?;
    info!("Successfully retrieved current public IPv4: {}", ip);
    Ok(ip.trim().to_string()) // Trim whitespace from the IP address
}

// Asynchronous function to retrieve the current A record content from Porkbun.
// It takes the HTTP client, API keys, domain, and subdomain as input.
async fn get_porkbun_a_record(
    client: &reqwest::Client,
    api_key: &str,
    secret_api_key: &str,
    domain: &str,
    subdomain: &str,
) -> Result<Option<DnsRecord>, Box<dyn std::error::Error>> {
    let full_name = if subdomain.is_empty() {
        domain.to_string()
    } else {
        format!("{}.{}", subdomain, domain)
    };
    info!("Retrieving A record for {} from Porkbun...", full_name);


    let payload = RetrieveRecordsPayload {
        auth: AuthPayload {
            apikey: api_key.to_string(),
            secretapikey: secret_api_key.to_string(),
        },
        name: full_name.clone(),
    };
    // format the url with the  domain and subdomain.
    let url: String = format!("https://api.porkbun.com/api/json/v3/dns/retrieveByNameType/{}/A/{}",&domain,&subdomain);
    let res = client
        .post(url)
        .json(&payload)
        .send()
        .await?;

    let response_body: RetrieveRecordsResponse = res.json().await?;

    if response_body.status == "SUCCESS" {
        if let Some(records) = response_body.records {
            // Filter for the specific A record for the given name
            let a_record = records.into_iter().find(|r| {
                r.record_type == "A" && r.name == full_name
            });

            if let Some(record) = &a_record {
                info!("Found existing A record for {}: {}", full_name, record.content);
            } else {
                warn!("No A record found for {}.", full_name);
            }
            Ok(a_record)
        } else {
            warn!("Porkbun API returned success but no records for {}.", full_name);
            Ok(None)
        }
    } else {
        let message = response_body.message.unwrap_or_else(|| "Unknown error".to_string());
        error!("Failed to retrieve A record from Porkbun: {}", message);
        Err(format!("Porkbun API error: {}", message).into())
    }
}

// Asynchronous function to update the A record on Porkbun.
// It takes the HTTP client, API keys, record ID, domain, subdomain, and new IP as input.
async fn update_porkbun_a_record(
    client: &reqwest::Client,
    api_key: &str,
    secret_api_key: &str,
    record_id: &str,
    domain: &str,
    subdomain: &str,
    new_ip: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Updating A record for {}.{} to new IP: {}",
        subdomain, domain, new_ip
    );

    let payload = UpdateRecordPayload {
        auth: AuthPayload {
            apikey: api_key.to_string(),
            secretapikey: secret_api_key.to_string(),
        },
        name: subdomain.to_string(), // For update, `name` is just the subdomain part
        record_type: "A".to_string(),
        content: new_ip.to_string(),
        ttl: 600, // Default TTL to 600 seconds (10 minutes)
    };

    let res = client
        .post(&format!(
            "https://api.porkbun.com/api/json/v3/dns/edit/{}/{}",
            domain, record_id
        ))
        .json(&payload)
        .send()
        .await?;

    let response_body: ApiResponse = res.json().await?;

    if response_body.status == "SUCCESS" {
        info!(
            "Successfully updated A record for {}.{} to {}",
            subdomain, domain, new_ip
        );
        Ok(())
    } else {
        let message = response_body.message.unwrap_or_else(|| "Unknown error".to_string());
        error!("Failed to update A record on Porkbun: {}", message);
        Err(format!("Porkbun API error: {}", message).into())
    }
}

// Main asynchronous function where the program execution begins.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the logger.
    // This allows you to see info, warn, and error messages in the console.
    Builder::new()
        .filter_level(LevelFilter::Info) // Set default log level to INFO
        .init();

    info!("Starting Porkbun Dynamic DNS Updater...");

    // Load environment variables from a .env file if it exists.
    dotenv().ok();

    // Retrieve configuration from environment variables.
    // These are critical and the program will exit if they are not found.
    let api_key = env::var("PORKBUN_API_KEY")
        .expect("PORKBUN_API_KEY environment variable not set.");
    let secret_api_key = env::var("PORKBUN_SECRET_API_KEY")
        .expect("PORKBUN_SECRET_API_KEY environment variable not set.");
    let domain = env::var("PORKBUN_DOMAIN")
        .expect("PORKBUN_DOMAIN environment variable not set.");

    // Parse subdomains from a comma-separated string.
    // If PORKBUN_SUBDOMAIN is not set or empty, it will result in a single empty string,
    // which corresponds to the root domain.
    let subdomains_str = env::var("PORKBUN_SUBDOMAIN").unwrap_or_else(|_| "".to_string());
    let subdomains: Vec<String> = subdomains_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    // Get the check interval from environment variables, default to 300 seconds (5 minutes).
    let check_interval_seconds = env::var("PORKBUN_CHECK_INTERVAL_SECONDS")
        .unwrap_or_else(|_| "300".to_string()) // Default to 300 seconds (5 minutes)
        .parse::<u64>()
        .expect("PORKBUN_CHECK_INTERVAL_SECONDS must be a valid number.");

    // Create an HTTP client for making requests.
    let client = reqwest::Client::new();

    // Continuous loop for background task.
    loop {
        info!("--- Starting new check cycle ---");
        // Get the current public IPv4 address once per cycle.
        let current_ip_result = get_current_ipv4().await;

        match current_ip_result {
            Ok(current_ip) => {
                // Loop through each subdomain and process it.
                for subdomain in &subdomains { // Iterate over references to avoid moving
                    info!("Processing subdomain: {}", if subdomain.is_empty() { "root domain" } else { &subdomain });

                    // Get the existing A record from Porkbun for the current subdomain.
                    let existing_record_result = get_porkbun_a_record(
                        &client,
                        &api_key,
                        &secret_api_key,
                        &domain,
                        &subdomain,
                    )
                    .await;

                    match existing_record_result {
                        Ok(existing_record) => {
                            // Check if an existing record was found and if its content matches the current IP.
                            match existing_record {
                                Some(record) => {
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
                                        // Update the DNS record if IPs differ.
                                        if let Err(e) = update_porkbun_a_record(
                                            &client,
                                            &api_key,
                                            &secret_api_key,
                                            &record.id, // Use the ID of the existing record to update
                                            &domain,
                                            &subdomain,
                                            &current_ip,
                                        )
                                        .await {
                                            error!("Error updating A record for {}.{}: {}", subdomain, domain, e);
                                        }
                                    }
                                }
                                None => {
                                    warn!(
                                        "No existing A record found for {}.{}. This script only updates existing records. Please create an initial A record manually on Porkbun.",
                                        subdomain, domain
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error retrieving A record for {}.{}: {}", subdomain, domain, e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error getting current public IPv4 address: {}", e);
            }
        }

        info!("--- Check cycle finished. Sleeping for {} seconds ---", check_interval_seconds);
        // Wait for the specified interval before the next check.
        sleep(Duration::from_secs(check_interval_seconds)).await;
    }
}
