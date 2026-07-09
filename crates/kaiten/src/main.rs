mod cli;
mod commands;
mod config;
mod error;
mod mcp;
mod output;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::error::CliError;

fn init_tracing(verbosity: u8) {
    let filter = match verbosity {
        0 => tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        1 => tracing_subscriber::EnvFilter::new("kaiten=debug,kaiten_client=debug"),
        _ => tracing_subscriber::EnvFilter::new("kaiten=trace,kaiten_client=trace,reqwest=debug"),
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

async fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Completion { shell } => commands::completion::run(shell),
        Commands::Auth(cmd) => commands::auth::run(cmd, cli.json).await,
        Commands::Mcp(cmd) => mcp::run(cmd).await,
        command => {
            let resolved = config::resolve()?;
            let client = kaiten_client::KaitenClient::new(&resolved.base_url, &resolved.token)?;
            match command {
                Commands::Space(cmd) => commands::space::run(cmd, &client, cli.json).await,
                Commands::Board(cmd) => {
                    commands::board::run(cmd, &client, &resolved.defaults, cli.json).await
                }
                Commands::Card(cmd) => {
                    commands::card::run(cmd, &client, &resolved.defaults, cli.json).await
                }
                Commands::Tag(cmd) => commands::tag::run(cmd, &client, cli.json).await,
                Commands::CardType(cmd) => commands::card_type::run(cmd, &client, cli.json).await,
                Commands::Api { method, path, data } => {
                    commands::api::run(&client, &method, &path, data).await
                }
                Commands::Completion { .. } | Commands::Auth(_) | Commands::Mcp(_) => {
                    unreachable!("handled above")
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kaiten: {err}");
            if let CliError::Api(kaiten_client::KaitenError::Api { message, body, .. }) = &err
                && !body.is_empty()
                && body != message
            {
                eprintln!("{body}");
            }
            ExitCode::FAILURE
        }
    }
}
