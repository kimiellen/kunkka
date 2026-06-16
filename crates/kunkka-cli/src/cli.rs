use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "kunkka", version, about = "Kunkka CLI frontend")]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    Ping,
    Status,
    Dispatch {
        #[arg(long = "app", value_parser = validate_non_empty)]
        app_id: String,
        #[arg(long, value_parser = validate_non_empty)]
        method: String,
        #[arg(long, value_parser = parse_json_payload)]
        payload: serde_json::Value,
    },
}

fn validate_non_empty(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err("must not be empty".to_string())
    } else {
        Ok(value.to_string())
    }
}

fn parse_json_payload(value: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(value).map_err(|err| format!("invalid JSON: {err}"))
}
