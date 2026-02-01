#[cfg(feature = "ui")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

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

    /// Run a query (PromQL by default, SQL with --sql)
    Query {
        /// Query expression (PromQL or SQL)
        expression: String,

        /// Execute as SQL via DataFusion instead of PromQL
        #[arg(long)]
        sql: bool,

        /// Backend endpoint URL (e.g. http://localhost:9090 for Prometheus)
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Read endpoint from a workspace
        #[arg(short = 'w', long = "workspace")]
        query_workspace: Option<String>,

        /// Start of time range (default: 1 hour ago; e.g. "1h", "30m", "2024-01-01T00:00:00Z")
        #[arg(long, default_value = "1h")]
        start: String,

        /// End of time range (default: now)
        #[arg(long, default_value = "now")]
        end: String,

        /// Query step/resolution for PromQL (e.g. "15s", "1m", "5m")
        #[arg(long, default_value = "60s")]
        step: String,

        /// Register a local file as a SQL table (NAME=PATH or just PATH)
        #[arg(long, value_name = "NAME=PATH")]
        file: Vec<String>,

        /// Maximum rows to return
        #[arg(long)]
        limit: Option<usize>,
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
            } => enya_headless::workspace::init(
                name,
                endpoint.as_deref(),
                template.as_deref(),
                output.as_deref(),
                json,
            ),
            Command::List => enya_headless::workspace::list(json),
            Command::Show { name } => enya_headless::workspace::show(&name, json),
            Command::Rm { name } => enya_headless::workspace::rm(&name, json),
            Command::Plugins { command: sub } => match sub {
                None => enya_headless::plugins::plugins(json),
                Some(PluginsCommand::Install { source }) => {
                    enya_headless::plugins::plugins_install(&source, json)
                }
                Some(PluginsCommand::Remove { name }) => {
                    enya_headless::plugins::plugins_remove(&name, json)
                }
                Some(PluginsCommand::Commands) => enya_headless::plugins::plugins_commands(json),
            },
            Command::Exec { command, args } => {
                enya_headless::plugins::exec(&command, &args.join(" "), json)
            }
            Command::Query {
                expression,
                sql,
                endpoint,
                query_workspace,
                start,
                end,
                step,
                file,
                limit,
            } => {
                if sql {
                    #[cfg(feature = "sql")]
                    {
                        enya_headless::query::sql::query(&expression, &file, limit, json)
                    }
                    #[cfg(not(feature = "sql"))]
                    {
                        let _ = (&expression, &file, limit);
                        Err(
                            "SQL queries require the 'sql' feature (rebuild with --features sql)"
                                .into(),
                        )
                    }
                } else {
                    enya_headless::query::promql::query(
                        &expression,
                        endpoint.as_deref(),
                        query_workspace.as_deref(),
                        &start,
                        &end,
                        &step,
                        limit,
                        json,
                    )
                }
            }
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
