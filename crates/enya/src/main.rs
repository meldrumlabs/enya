use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod commands;

/// Enya — headless observability editor for machines and AI agents
#[derive(Parser)]
#[command(name = "enya", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new workspace
    Init {
        /// Workspace name (defaults to current directory name)
        name: Option<String>,

        /// Prometheus endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Start from a built-in template (default, demo, complex, atlas)
        #[arg(short, long)]
        template: Option<String>,

        /// Write to a specific file path instead of .enya/workspaces/
        #[arg(short, long)]
        output: Option<String>,
    },

    /// List available workspaces
    List,

    /// Display workspace information
    Show {
        /// Workspace name or path to TOML file
        name: String,
    },

    /// Delete a workspace
    Rm {
        /// Workspace name or path
        name: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Init {
            name,
            endpoint,
            template,
            output,
        } => commands::init(
            name,
            endpoint.as_deref(),
            template.as_deref(),
            output.as_deref(),
        ),
        Command::List => commands::list(),
        Command::Show { name } => commands::show(&name),
        Command::Rm { name } => commands::rm(&name),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
