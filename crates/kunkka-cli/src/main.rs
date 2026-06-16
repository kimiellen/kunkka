use clap::Parser;
use kunkka_cli::cli::Cli;
use kunkka_cli::run_command;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

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
