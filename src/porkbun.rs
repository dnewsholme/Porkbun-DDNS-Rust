// src/porkbun.rs

use crate::errors::{DdnsError, Result};
use log::{error, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const API_BASE_URL: &str = "https://api.porkbun.com/api/json/v3/dns";
const DEFAULT_TTL: u32 = 600;

// Helper function to deserialize a field that might be an integer or a string into an Option<String>
fn optional_string_from_int_or_string<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        String(String),
        Int(u64),
    }

    match Option::<StringOrInt>::deserialize(deserializer)? {
        Some(StringOrInt::String(s)) => Ok(Some(s)),
        Some(StringOrInt::Int(i)) => Ok(Some(i.to_string())),
        None => Ok(None),
    }
}

// Structs for Porkbun API request and response bodies.

#[derive(Serialize)]
struct AuthPayload<'a> {
    apikey: &'a str,
    secretapikey: &'a str,
}

#[derive(Serialize)]
struct UpdateRecordPayload<'a> {
    #[serde(flatten)]
    auth: AuthPayload<'a>,
    name: &'a str,
    #[serde(rename = "type")]
    record_type: &'a str,
    content: &'a str,
    ttl: u32,
}

#[derive(Serialize)]
struct CreateRecordPayload<'a> {
    #[serde(flatten)]
    auth: AuthPayload<'a>,
    name: &'a str,
    #[serde(rename = "type")]
    record_type: &'a str,
    content: &'a str,
    ttl: u32,
}

#[derive(Debug, Deserialize)]
pub struct DnsRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    pub content: String,
    #[allow(dead_code)]
    ttl: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
struct RetrieveRecordsResponse {
    status: String,
    records: Option<Vec<DnsRecord>>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    status: String,
    message: Option<String>,
    #[serde(default, deserialize_with = "optional_string_from_int_or_string")]
    id: Option<String>,
}

/// A client for interacting with the Porkbun API.
pub struct PorkbunClient<'a> {
    client: &'a Client,
    api_key: &'a str,
    secret_api_key: &'a str,
    pub domain: &'a str,
}

impl<'a> PorkbunClient<'a> {
    pub fn new(
        client: &'a Client,
        api_key: &'a str,
        secret_api_key: &'a str,
        domain: &'a str,
    ) -> Self {
        // No change here, this is just context
        Self {
            client,
            api_key,
            secret_api_key,
            domain,
        }
    }

    fn auth_payload(&self) -> AuthPayload<'_> {
        AuthPayload {
            apikey: self.api_key,
            secretapikey: self.secret_api_key,
        }
    }

    pub async fn get_a_record(&self, subdomain: &str) -> Result<Option<DnsRecord>> {
        let full_name = if subdomain.is_empty() {
            self.domain.to_string()
        } else {
            format!("{}.{}", subdomain, self.domain)
        };
        info!("Retrieving A record for {} from Porkbun...", full_name);

        let url = format!(
            "{}/retrieveByNameType/{}/A/{}",
            API_BASE_URL, self.domain, subdomain
        );
        let res = self
            .client
            .post(url)
            .json(&self.auth_payload())
            .send()
            .await?;

        let response_body: RetrieveRecordsResponse = res
            .json()
            .await
            .map_err(|e| DdnsError::PorkbunApi(format!("Failed to parse JSON response: {}", e)))?;

        if response_body.status == "SUCCESS" {
            let a_record = response_body.records.and_then(|records| {
                records
                    .into_iter()
                    .find(|r| r.record_type == "A" && r.name == full_name)
            });

            if let Some(record) = &a_record {
                info!(
                    "Found existing A record for {}: {}",
                    full_name, record.content
                );
            } else {
                warn!("No A record found for {}.", full_name);
            }
            Ok(a_record)
        } else {
            let message = response_body
                .message
                .unwrap_or_else(|| "Unknown error".to_string());
            error!("Failed to retrieve A record from Porkbun: {}", message);
            Err(DdnsError::PorkbunApi(message))
        }
    }

    pub async fn update_a_record(
        &self,
        record_id: &str,
        subdomain: &str,
        new_ip: &str,
    ) -> Result<()> {
        info!(
            "Updating A record for {}.{} to new IP: {}",
            subdomain, self.domain, new_ip
        );

        let payload = UpdateRecordPayload {
            auth: self.auth_payload(),
            name: subdomain,
            record_type: "A",
            content: new_ip,
            ttl: DEFAULT_TTL,
        };

        let url = format!("{}/edit/{}/{}", API_BASE_URL, self.domain, record_id);
        let res = self.client.post(url).json(&payload).send().await?;

        let response_body: ApiResponse = res
            .json()
            .await
            .map_err(|e| DdnsError::PorkbunApi(format!("Failed to parse JSON response: {}", e)))?;

        if response_body.status == "SUCCESS" {
            info!(
                "Successfully updated A record for {}.{} to {}",
                subdomain, self.domain, new_ip
            );
            Ok(())
        } else {
            let message = response_body
                .message
                .unwrap_or_else(|| "Unknown error".to_string());
            error!("Failed to update A record on Porkbun: {}", message);
            Err(DdnsError::PorkbunApi(message))
        }
    }

    pub async fn create_a_record(&self, subdomain: &str, new_ip: &str) -> Result<()> {
        warn!(
            "Creating new A record for {}.{} with IP: {}",
            subdomain, self.domain, new_ip
        );

        let payload = CreateRecordPayload {
            auth: self.auth_payload(),
            name: subdomain,
            record_type: "A",
            content: new_ip,
            ttl: DEFAULT_TTL,
        };

        let url = format!("{}/create/{}", API_BASE_URL, self.domain);
        let res = self.client.post(url).json(&payload).send().await?;

        let response_body: ApiResponse = res
            .json()
            .await
            .map_err(|e| DdnsError::PorkbunApi(format!("Failed to parse JSON response: {}", e)))?;

        if response_body.status == "SUCCESS" {
            info!(
                "Successfully created new A record (ID: {}) for {}.{} to {}",
                response_body.id.unwrap_or_else(|| "N/A".to_string()),
                subdomain,
                self.domain,
                new_ip
            );
            Ok(())
        } else {
            let message = response_body
                .message
                .unwrap_or_else(|| "Unknown error".to_string());
            error!("Failed to create A record on Porkbun: {}", message);
            Err(DdnsError::PorkbunApi(message))
        }
    }
}
