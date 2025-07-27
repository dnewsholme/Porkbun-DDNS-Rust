# Porkbun Dynamic DNS Updater (Rust & Docker)

This project provides a lightweight and efficient Dynamic DNS (DDNS) client written in Rust, designed to update DNS A records on Porkbun.com for your domains and subdomains. It's built to run continuously as a background task, ideal for deployment in a Docker container.

## Features

* **IPv4 Support:** Automatically detects and updates your public IPv4 address.

* **Multiple Subdomain Support:** Configurable to update one or more subdomains, including the root/base domain.

* **Continuous Operation:** Runs in a loop with a configurable check interval, ensuring your DNS records are always up-to-date.

* **Environment Variable Configuration:** All sensitive information and settings are managed securely via environment variables.

* **Logging:** Provides clear log output for IP changes and operational status.

* **Docker Ready:** Includes a `Dockerfile` and `docker-compose.yml` for easy containerized deployment.

## Prerequisites

Before you begin, ensure you have the following installed:

* **Docker** and **Docker Compose**: For containerized deployment.

* **Rust (Optional)**: If you plan to build and run the application manually outside of Docker.

## Setup

1.  **Project Structure:**
    Ensure your project directory is structured as follows:

    ```
    porkbun_ddns_updater/
    ├── src/
    │   └── main.rs
    ├── Cargo.toml
    ├── Cargo.lock
    ├── Dockerfile
    └── docker-compose.yml
    
    ```

2.  **Porkbun API Keys:**
    You will need to generate an API Key and Secret Key from your Porkbun account:

    * Log in to Porkbun.

    * Go to `Account` -> `API Access`.

    * Create a new API Key. **Make sure to save your Secret Key immediately, as it will only be shown once.**

    * Ensure that API access is enabled for the specific domain(s) you intend to update under `Domain Management` -> `Details` for each domain.

3.  **Configure Environment Variables (`docker-compose.yml`)**
    Open the `docker-compose.yml` file and populate the `environment` section with your details:

    ```
    # docker-compose.yml
    version: '3.8'
    
    services:
      porkbun-ddns:
        build: .
        container_name: porkbun-ddns-updater
        environment:
          PORKBUN_API_KEY: "your_api_key_here"           # Your Porkbun API Key
          PORKBUN_SECRET_API_KEY: "your_secret_api_key_here" # Your Porkbun Secret API Key
          PORKBUN_DOMAIN: "yourdomain.com"              # Your primary domain (e.g., "example.com")
          PORKBUN_SUBDOMAIN: "www,blog"                 # Comma-separated list of subdomains.
                                                        # - Use "" for only the base domain (e.g., "yourdomain.com").
                                                        # - Use ",www,blog" to include the base domain and subdomains.
          PORKBUN_CHECK_INTERVAL_SECONDS: "300"         # Interval in seconds between IP checks (e.g., 300 for 5 minutes)
        restart: unless-stopped
        logging:
          driver: "json-file"
          options:
            max-size: "10m"
            max-file: "5"
    
    ```

    **Important:** This script *updates* existing A records. If an A record does not exist for a specified domain or subdomain, the script will log a warning and will **not** create it. You must create the initial A record(s) manually on Porkbun.

## Running the Application

### With Docker Compose (Recommended)

This is the easiest way to run the application as a background service.

1.  Navigate to the root directory of your project (where `docker-compose.yml` is located) in your terminal.

2.  Build the Docker image and start the container in detached mode:

    ```
    docker compose up -d --build
    
    ```

    * `docker compose up`: Starts the services defined in `docker-compose.yml`.

    * `-d`: Runs the container in detached mode (in the background).

    * `--build`: Builds the Docker image from the `Dockerfile` before starting. Use this for the first run or after making changes to the Rust code or `Dockerfile`.

3.  **Check Logs:**
    To view the real-time logs of your running container:

    ```
    docker logs porkbun-ddns-updater -f
    
    ```

    (Press `Ctrl+C` to exit the log stream.)

4.  **Stop the Container:**
    To stop the running container:

    ```
    docker compose down
    
    ```

### Manually (Without Docker)

If you prefer to run the application directly on your host machine (requires Rust installed):

1.  **Build the Project:**
    Navigate to your project root and build the release binary:

    ```
    cargo build --release
    
    ```

2.  **Set Environment Variables:**
    Before running, you must set the required environment variables in your shell. Replace the placeholders with your actual values:

    ```
    export PORKBUN_API_KEY="your_api_key_here"
    export PORKBUN_SECRET_API_KEY="your_secret_api_key_here"
    export PORKBUN_DOMAIN="yourdomain.com"
    export PORKBUN_SUBDOMAIN="www,blog" # Or "" for root, or ",www" for root and www
    export PORKBUN_CHECK_INTERVAL_SECONDS="300"
    
    ```

    (For persistent environment variables, refer to your operating system's documentation.)

3.  **Run the Application:**

    ```
    cargo run --release
    
    ```

    The application will start logging output to your console.

## Troubleshooting

* **"PORKBUN_API_KEY environment variable not set."**: Ensure all required environment variables are correctly set in your `docker-compose.yml` or your shell environment.

* **"Failed to retrieve A record from Porkbun: Invalid API Key"**: Double-check your `PORKBUN_API_KEY` and `PORKBUN_SECRET_API_KEY` for typos. Also, ensure API access is enabled for your domain in the Porkbun dashboard.

* **"No existing A record found for..."**: This script only updates existing A records. You need to manually create the initial A record(s) for your domain/subdomain(s) on Porkbun.

* **No IP change detected**: The script will only log an update if your public IP address has actually changed. If your IP is stable, it will simply log that no update is needed.
