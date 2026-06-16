use clap::Parser;
use kunkka_cli::cli::Cli;
use kunkka_cli::output::CliOutput;
use kunkka_cli::run_command;

#[tokio::main]
async fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let output = CliOutput::error("invalid_request", err.to_string());
            eprintln!("{}", serde_json::to_string(&output).unwrap());
            std::process::exit(1);
        }
    };

    match run_command(&cli).await {
        Ok(output) => {
            println!("{}", serde_json::to_string(&output).unwrap());
            if output.is_success() {
                std::process::exit(0);
            } else {
                std::process::exit(1);
            }
        }
        Err(err) => {
            let output = err.to_output();
            eprintln!("{}", serde_json::to_string(&output).unwrap());
            std::process::exit(err.exit_code());
        }
    }
}
