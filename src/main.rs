// main.rs

// Import necessary crates for asynchronous operations, HTTP requests, JSON serialization,
// environment variable loading, logging, and time utilities.
use tokio;
use reqwest;
use serde::{Deserialize, Serialize};
//use serde_json::json;
use dotenv::dotenv;
use std::env;
use log::{info, error, warn, LevelFilter};
use env_logger::Builder;
use tokio::time::{sleep, Duration}; // Added for continuous loop delay


// Simplified Helper function to deserialize a field that might be an integer or a string into an Option<String>
fn optional_string_from_int_or_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Define a struct that can capture both String and u64 (or i64)
    #[derive(Deserialize)]
    #[serde(untagged)] // Try to deserialize as one type, then the other
    enum StringOrInt {
        String(String),
        Int(u64),
    }

    // Attempt to deserialize the value as an Option<StringOrInt>
    match Option::<StringOrInt>::deserialize(deserializer)? {
        Some(StringOrInt::String(s)) => Ok(Some(s)),
        Some(StringOrInt::Int(i)) => Ok(Some(i.to_string())),
        None => Ok(None),
    }
}

// Define structs for Porkbun API request and response bodies.
// These structs use `serde` to automatically serialize/deserialize to/from JSON.

// Struct for the common part of Porkbun API requests (API key and secret key).
#[derive(Serialize)]
struct AuthPayload {
    apikey: String,
    secretapikey: String,
}

// Struct for the DNS record update/create request payload.
// NOTE: Porkbun uses the same payload structure for creating and editing, 
// just different endpoints.
#[derive(Serialize)]
struct UpdateRecordPayload {
    #[serde(flatten)]
    auth: AuthPayload,
    name: String, 	 // The subdomain part (e.g., "www" for www.example.com, or "" for example.com)
    #[serde(rename = "type")] // Rename 'type' field to avoid Rust keyword collision
    record_type: String, // e.g., "A" for IPv4
    content: String, // The IP address
    ttl: u32, 	 	 // Time To Live in seconds
}

// Struct for the DNS record creation request payload. (Identical to UpdateRecordPayload for simplicity)
#[derive(Serialize)]
struct CreateRecordPayload {
    #[serde(flatten)]
    auth: AuthPayload,
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    content: String,
    ttl: u32,
}

// Struct to represent a single DNS record in the Porkbun API response.
#[derive(Debug, Deserialize)]
struct DnsRecord {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    #[allow(dead_code)] // Added to silence the unused field warning
    ttl: String, // TTL is returned as a string, we'll parse it to u32 if needed
    id: String,	 // Record ID, needed for updates
}

// Struct for the response when retrieving DNS records.
#[derive(Debug, Deserialize)]
struct RetrieveRecordsResponse {
    status: String,
    records: Option<Vec<DnsRecord>>, // Option because 'records' might be null if none found
    message: Option<String>,
}

// Struct for the general Porkbun API response (e.g., for update, create, or ping).
#[derive(Debug, Deserialize)]
struct ApiResponse {
    status: String,
    message: Option<String>,
    #[serde(default, deserialize_with = "optional_string_from_int_or_string")] 
    id: Option<String>, // Added for the create record response
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


    let payload = AuthPayload {
        apikey: api_key.to_string(),
        secretapikey: secret_api_key.to_string(),
    };
    
    // format the url with the domain and subdomain.
    // NOTE: The previous code had `retrieveByNameType` which expects the full domain name in the payload.
    // The Porkbun API is typically called via POST to a URL, using the path for domain/subdomain/type.
    let url: String = format!("https://api.porkbun.com/api/json/v3/dns/retrieveByNameType/{}/A/{}", &domain, &subdomain);
    let res = client
        .post(url)
        .json(&payload)
        .send()
        .await?;

    let response_body: RetrieveRecordsResponse = res.json().await?;

    if response_body.status == "SUCCESS" {
        if let Some(records) = response_body.records {
            // Filter for the specific A record for the given name (should match full_name)
            let a_record = records.into_iter().find(|r| {
                r.record_type == "A" && r.name == full_name
            });

            if let Some(record) = &a_record {
                info!("Found existing A record for {}: {}", full_name, record.content);
            } else {
                // This is the case where the retrieve call succeeded but the record does not exist.
                warn!("No A record found for {}.", full_name);
            }
            Ok(a_record)
        } else {
            // Porkbun API returns success with an empty records array if the record doesn't exist
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

// --- NEW FUNCTION: Create A Record ---
// Asynchronous function to create a new A record on Porkbun.
async fn create_porkbun_a_record(
    client: &reqwest::Client,
    api_key: &str,
    secret_api_key: &str,
    domain: &str,
    subdomain: &str,
    new_ip: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    warn!(
        "Creating new A record for {}.{} with IP: {}",
        subdomain, domain, new_ip
    );

    let payload = CreateRecordPayload {
        auth: AuthPayload {
            apikey: api_key.to_string(),
            secretapikey: secret_api_key.to_string(),
        },
        name: subdomain.to_string(), // For create, `name` is just the subdomain part
        record_type: "A".to_string(),
        content: new_ip.to_string(),
        ttl: 600, // Default TTL to 600 seconds (10 minutes)
    };

    // The API URL for creating a record
    let url = format!("https://api.porkbun.com/api/json/v3/dns/create/{}", domain);

    let res = client
        .post(&url)
        .json(&payload)
        .send()
        .await?;

    let response_body: ApiResponse = res.json().await?;

    if response_body.status == "SUCCESS" {
        info!(
            "Successfully created new A record (ID: {}) for {}.{} to {}",
            response_body.id.unwrap_or_else(|| "N/A".to_string()),
            subdomain, domain, new_ip
        );
        Ok(())
    } else {
        let message = response_body.message.unwrap_or_else(|| "Unknown error".to_string());
        error!("Failed to create A record on Porkbun: {}", message);
        Err(format!("Porkbun API error: {}", message).into())
    }
}
// -------------------------------------


// Main asynchronous function where the program execution begins.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the logger.
    Builder::new()
        .filter_level(LevelFilter::Info) // Set default log level to INFO
        .init();

    info!("Starting Porkbun Dynamic DNS Updater...");

    // Load environment variables from a .env file if it exists.
    dotenv().ok();

    // Retrieve configuration from environment variables.
    let api_key = env::var("PORKBUN_API_KEY")
        .expect("PORKBUN_API_KEY environment variable not set.");
    let secret_api_key = env::var("PORKBUN_SECRET_API_KEY")
        .expect("PORKBUN_SECRET_API_KEY environment variable not set.");
    let domain = env::var("PORKBUN_DOMAIN")
        .expect("PORKBUN_DOMAIN environment variable not set.");

    // Parse subdomains from a comma-separated string.
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
                                    // Logic for NON-EXISTENT Record (Create it)
                                    // The main new feature implementation:
                                    if let Err(e) = create_porkbun_a_record(
                                        &client,
                                        &api_key,
                                        &secret_api_key,
                                        &domain,
                                        &subdomain,
                                        &current_ip,
                                    )
                                    .await {
                                        error!("Error creating A record for {}.{}: {}", subdomain, domain, e);
                                    }
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