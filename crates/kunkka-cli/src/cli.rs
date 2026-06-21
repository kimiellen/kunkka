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
    Shell {
        #[arg(long = "app", value_parser = validate_non_empty)]
        app_id: String,
        #[arg(long, value_parser = validate_non_empty)]
        command: String,
    },
    Approvals {
        #[command(subcommand)]
        command: ApprovalCommand,
    },
    Dispatch {
        #[arg(long = "app", value_parser = validate_non_empty)]
        app_id: String,
        #[arg(long, value_parser = validate_non_empty)]
        method: String,
        #[arg(long, value_parser = parse_json_payload)]
        payload: serde_json::Value,
    },
    /// LLM capability management
    Llm {
        #[command(subcommand)]
        command: LlmCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ApprovalCommand {
    List,
    Approve {
        #[arg(long = "id", value_parser = validate_non_empty)]
        approval_id: String,
    },
    Reject {
        #[arg(long = "id", value_parser = validate_non_empty)]
        approval_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum LlmCommand {
    /// Preset provider management
    Presets {
        #[command(subcommand)]
        command: PresetCommand,
    },
    /// Configured provider management
    Providers {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    /// Role management
    Roles {
        #[command(subcommand)]
        command: RoleCommand,
    },
    /// Usage statistics
    Usage {
        #[command(subcommand)]
        command: UsageCommand,
    },
    /// Default role management
    DefaultRole {
        /// Role name to set as default (omit to clear)
        #[arg(value_parser = validate_non_empty)]
        name: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum UsageCommand {
    /// Show usage summary
    Summary,
    /// Show recent usage records
    Records {
        /// Number of records to show
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Clear usage records
    Clear,
}

#[derive(Debug, Subcommand)]
pub enum PresetCommand {
    /// List available preset providers
    List,
    /// Apply a preset provider with API key
    Apply {
        /// Preset name (e.g., openai, zhipu, kimi, xiaomi)
        #[arg(value_parser = validate_non_empty)]
        name: String,
        /// API key for the provider
        #[arg(long, value_parser = validate_non_empty)]
        api_key: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProviderCommand {
    /// List configured providers
    List,
    /// Add a custom provider
    Add {
        /// Provider name
        #[arg(value_parser = validate_non_empty)]
        name: String,
        /// API base URL
        #[arg(long, value_parser = validate_non_empty)]
        base_url: String,
        /// API key
        #[arg(long, value_parser = validate_non_empty)]
        api_key: String,
        /// Available models (comma-separated)
        #[arg(long, value_delimiter = ',')]
        models: Vec<String>,
    },
    /// Update a provider
    Update {
        /// Provider name
        #[arg(value_parser = validate_non_empty)]
        name: String,
        /// API key
        #[arg(long)]
        api_key: Option<String>,
        /// API base URL
        #[arg(long)]
        base_url: Option<String>,
        /// Available models (comma-separated)
        #[arg(long, value_delimiter = ',')]
        models: Option<Vec<String>>,
    },
    /// Remove a provider
    Remove {
        /// Provider name
        #[arg(value_parser = validate_non_empty)]
        name: String,
    },
    /// Test provider connection
    Test {
        /// Provider name
        #[arg(value_parser = validate_non_empty)]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum RoleCommand {
    /// List configured roles
    List,
    /// Add a new role
    Add {
        /// Role name
        #[arg(value_parser = validate_non_empty)]
        name: String,
        /// Role description
        #[arg(long, value_parser = validate_non_empty)]
        description: String,
        /// Provider name
        #[arg(long, value_parser = validate_non_empty)]
        provider: String,
        /// Model name
        #[arg(long, value_parser = validate_non_empty)]
        model: String,
        /// Temperature (0.0-2.0)
        #[arg(long)]
        temperature: Option<f32>,
        /// Max tokens
        #[arg(long)]
        max_tokens: Option<u32>,
    },
    /// Update a role
    Update {
        /// Role name
        #[arg(value_parser = validate_non_empty)]
        name: String,
        /// Role description
        #[arg(long)]
        description: Option<String>,
        /// Provider name
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long)]
        model: Option<String>,
        /// Temperature (0.0-2.0)
        #[arg(long)]
        temperature: Option<f32>,
        /// Max tokens
        #[arg(long)]
        max_tokens: Option<u32>,
    },
    /// Remove a role
    Remove {
        /// Role name
        #[arg(value_parser = validate_non_empty)]
        name: String,
    },
    /// Role preset management
    Presets {
        #[command(subcommand)]
        command: RolePresetCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum RolePresetCommand {
    /// List available role presets
    List,
    /// Apply a role preset
    Apply {
        /// Preset name (e.g., thinker, coder, collector, reviewer)
        #[arg(value_parser = validate_non_empty)]
        name: String,
        /// Provider name
        #[arg(long, value_parser = validate_non_empty)]
        provider: String,
        /// Model name
        #[arg(long, value_parser = validate_non_empty)]
        model: String,
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
