#[cfg(feature = "ui")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod commands;

/// Enya — observability editor for humans, machines, and AI agents
///
/// Run without a subcommand to launch the GUI editor.
/// Use subcommands for headless CLI operations (ideal for AI agents).
#[derive(Parser)]
#[command(name = "enya", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Output as JSON for machine consumption
    #[arg(long, global = true)]
    json: bool,

    /// Open a specific workspace (GUI mode only)
    #[arg(long)]
    workspace: Option<String>,
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

        /// Write to a specific file path instead of ~/.enya/workspaces/
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

    /// Manage plugins (list, install, remove)
    Plugins {
        #[command(subcommand)]
        command: Option<PluginsCommand>,
    },

    /// Execute a plugin command
    Exec {
        /// Command name
        command: String,

        /// Arguments to pass to the command
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum PluginsCommand {
    /// Install a plugin from a local file
    Install {
        /// Path to plugin file (.toml or .lua)
        source: String,
    },

    /// Remove an installed plugin
    Remove {
        /// Plugin name
        name: String,
    },

    /// List all available plugin commands
    Commands,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // If a subcommand is provided, run headless CLI
    if let Some(command) = cli.command {
        let json = cli.json;
        let result = match command {
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
                json,
            ),
            Command::List => commands::list(json),
            Command::Show { name } => commands::show(&name, json),
            Command::Rm { name } => commands::rm(&name, json),
            Command::Plugins { command: sub } => match sub {
                None => commands::plugins(json),
                Some(PluginsCommand::Install { source }) => {
                    commands::plugins_install(&source, json)
                }
                Some(PluginsCommand::Remove { name }) => commands::plugins_remove(&name, json),
                Some(PluginsCommand::Commands) => commands::plugins_commands(json),
            },
            Command::Exec { command, args } => commands::exec(&command, &args.join(" "), json),
        };

        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                if json {
                    let _ = serde_json::to_writer(
                        std::io::stdout(),
                        &serde_json::json!({"error": e.to_string()}),
                    );
                    println!();
                } else {
                    eprintln!("error: {e}");
                }
                ExitCode::FAILURE
            }
        };
    }

    // No subcommand: launch GUI editor
    #[cfg(feature = "ui")]
    {
        match enya_editor::run_native_app(cli.workspace) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    }

    #[cfg(not(feature = "ui"))]
    {
        eprintln!("error: GUI not available (built without 'ui' feature)");
        eprintln!("Use a subcommand: enya init, enya list, enya show, enya rm");
        ExitCode::FAILURE
    }
}
