#[cfg(feature = "ui")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "serve")]
mod serve;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
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

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
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

    /// Get a workspace property
    Get {
        /// Workspace name or path
        name: String,
        /// Property key (e.g. "time.preset", "metrics.endpoint")
        key: String,
    },

    /// Set a workspace property
    Set {
        /// Workspace name or path
        name: String,
        /// Property key (e.g. "time.preset", "metrics.endpoint")
        key: String,
        /// New value
        value: String,
    },

    /// Add a section to a workspace
    AddSection {
        /// Workspace name or path
        name: String,
        /// Section name
        section_name: String,
        /// Layout: horizontal, vertical, grid, tabs
        #[arg(long, default_value = "horizontal")]
        layout: String,
        /// Number of columns (for grid layout)
        #[arg(long)]
        columns: Option<usize>,
        /// Start section collapsed
        #[arg(long)]
        collapsed: bool,
    },

    /// Add a query pane to a workspace
    AddPane {
        /// Workspace name or path
        name: String,
        /// Query expression
        query: String,
        /// Display name for the pane
        #[arg(long = "name")]
        pane_name: Option<String>,
        /// Target section (defaults to last section)
        #[arg(long)]
        section: Option<String>,
        /// Tag (e.g. "Critical", "Warning")
        #[arg(long)]
        tag: Option<String>,
        /// Unit suffix (e.g. "ms", "req/s")
        #[arg(long)]
        unit: Option<String>,
        /// Granularity (e.g. "1m", "5m", "15m")
        #[arg(long)]
        granularity: Option<String>,
        /// Visualization type (e.g. "time_series", "stat")
        #[arg(long)]
        visualization: Option<String>,
        /// Description text
        #[arg(long)]
        description: Option<String>,
    },

    /// Remove a section from a workspace
    RemoveSection {
        /// Workspace name or path
        name: String,
        /// Section name to remove
        section_name: String,
    },

    /// Remove a pane from a workspace
    RemovePane {
        /// Workspace name or path
        name: String,
        /// Pane name to remove
        pane: String,
        /// Limit search to a specific section
        #[arg(long)]
        section: Option<String>,
    },

    /// Open a workspace in the GUI editor
    Open {
        /// Workspace name or path
        name: String,
    },

    /// Serve the WASM editor over HTTP with a Prometheus proxy
    Serve {
        /// Workspace name or path to TOML file
        workspace: String,

        /// Port to listen on
        #[arg(long, default_value = "3030")]
        port: u16,

        /// Address to bind to
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,

        /// Open browser after starting
        #[arg(long)]
        open: bool,
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

    // `enya open <name>` is equivalent to `enya --workspace <name>` (launches GUI)
    let (command, workspace) = match cli.command {
        Some(Command::Open { name }) => (None, Some(name)),
        cmd => (cmd, cli.workspace),
    };

    // If a subcommand is provided, run headless CLI
    if let Some(command) = command {
        let json = cli.json;
        let result = match command {
            Command::Completions { shell } => {
                clap_complete::generate(shell, &mut Cli::command(), "enya", &mut std::io::stdout());
                Ok(())
            }
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
            Command::Get { name, key } => enya_headless::workspace::get(&name, &key, json),
            Command::Set { name, key, value } => {
                enya_headless::workspace::set(&name, &key, &value, json)
            }
            Command::AddSection {
                name,
                section_name,
                layout,
                columns,
                collapsed,
            } => enya_headless::workspace::add_section(
                &name,
                &section_name,
                &layout,
                columns,
                collapsed,
                json,
            ),
            Command::AddPane {
                name,
                query,
                pane_name,
                section,
                tag,
                unit,
                granularity,
                visualization,
                description,
            } => enya_headless::workspace::add_pane(
                &name,
                &query,
                pane_name.as_deref(),
                section.as_deref(),
                tag.as_deref(),
                unit.as_deref(),
                granularity.as_deref(),
                visualization.as_deref(),
                description.as_deref(),
                json,
            ),
            Command::RemoveSection { name, section_name } => {
                enya_headless::workspace::remove_section(&name, &section_name, json)
            }
            Command::RemovePane {
                name,
                pane,
                section,
            } => enya_headless::workspace::remove_pane(&name, &pane, section.as_deref(), json),
            Command::Serve {
                workspace,
                port,
                bind,
                open,
            } => {
                #[cfg(feature = "serve")]
                {
                    serve::run(&workspace, port, &bind, open)
                }
                #[cfg(not(feature = "serve"))]
                {
                    let _ = (&workspace, port, &bind, open);
                    Err("serve requires the 'serve' feature (rebuild with --features serve)".into())
                }
            }
            Command::Open { .. } => unreachable!("handled above"),
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

    // No subcommand (or `open`): launch GUI editor
    #[cfg(feature = "ui")]
    {
        match enya_editor::run_native_app(workspace) {
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
