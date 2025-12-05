use clap::Parser;

use crate::{config::TvsNodeConfig, server_builder::TvsNodeRunner};

mod config;
mod server_builder;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = "config.json")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();

    let mut config = TvsNodeConfig::read_config(&args.config)
        .expect(&format!("Failed to read config {}", &args.config));

    // Apply environment variable overrides (for Docker/containerized deployments)
    config.apply_env_overrides();

    // Build and run the TVS node with feature-based persistence
    let runner = TvsNodeRunner::build_with_config(config).await?;

    // Run until shutdown (consumes runner)
    runner.run_until_shutdown().await
}
