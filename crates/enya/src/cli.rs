use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use console::style;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

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

    /// Handle an enya:// deep link URL (used internally by macOS URL scheme handler)
    #[arg(long, hide = true)]
    url: Option<String>,
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

        /// Start from a built-in template (default, demo, complex)
        #[arg(short, long)]
        template: Option<String>,

        /// Write to a specific file path instead of the project directory
        #[arg(short, long)]
        output: Option<String>,

        /// Project name (required unless --output is used)
        #[arg(short, long, required_unless_present = "output")]
        project: Option<String>,
    },

    /// List available workspaces
    List,

    /// Display workspace information
    Show {
        /// Workspace name
        name: String,

        /// Project name
        #[arg(short, long)]
        project: String,
    },

    /// Validate a workspace configuration (all workspaces if no name given)
    Check {
        /// Workspace name or path (omit to check all)
        name: Option<String>,
    },

    /// Format a workspace TOML file (all workspaces if no name given)
    Fmt {
        /// Workspace name or path (omit to format all)
        name: Option<String>,
    },

    /// Delete a workspace
    Rm {
        /// Workspace name
        name: String,

        /// Project name
        #[arg(short, long)]
        project: String,
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
        /// Workspace name
        name: String,
        /// Property key (e.g. "time.preset", "metrics.endpoint")
        key: String,
        /// Project name
        #[arg(short, long)]
        project: String,
    },

    /// Set a workspace property
    Set {
        /// Workspace name
        name: String,
        /// Property key (e.g. "time.preset", "metrics.endpoint")
        key: String,
        /// New value
        value: String,
        /// Project name
        #[arg(short, long)]
        project: String,
    },

    /// Add a query pane to a workspace
    AddPane {
        /// Workspace name
        name: String,
        /// Query expression
        query: String,
        /// Display name for the pane
        #[arg(long = "name")]
        pane_name: Option<String>,
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
        /// Project name
        #[arg(short, long)]
        project: String,
    },

    /// Remove a pane from a workspace
    RemovePane {
        /// Workspace name
        name: String,
        /// Pane name to remove
        pane: String,
        /// Project name
        #[arg(short, long)]
        project: String,
    },

    /// Open a workspace in the GUI editor
    Open {
        /// Workspace name or path
        name: String,
    },

    /// Start the Enya agent (API server, Prometheus proxy, WASM editor)
    Serve {
        /// Optional workspace name or path for WASM UI
        workspace: Option<String>,

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

    /// Watch a metric and alert when it crosses a threshold
    Watch {
        /// Workspace name (for endpoint resolution)
        workspace: Option<String>,

        /// PromQL expression to watch
        expression: String,

        /// Alert when any value exceeds this threshold
        #[arg(long, conflicts_with = "below", required_unless_present = "below")]
        above: Option<f64>,

        /// Alert when any value drops below this threshold
        #[arg(long, conflicts_with = "above", required_unless_present = "above")]
        below: Option<f64>,

        /// Poll interval (e.g. "30s", "1m", "5m")
        #[arg(long, default_value = "30s")]
        every: String,

        /// Condition must sustain for this duration before alerting (e.g. "5m", "30m")
        #[arg(long, value_name = "DURATION")]
        r#for: Option<String>,

        /// Prometheus endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,
    },

    /// Capture a snapshot of workspace query results at this point in time
    Snapshot {
        /// Workspace name
        name: String,

        /// Override Prometheus endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Write snapshot to a file instead of stdout
        #[arg(short, long)]
        output: Option<String>,

        /// Project name
        #[arg(short, long)]
        project: String,
    },

    /// Discover Prometheus metrics, labels, and metadata
    Metrics {
        #[command(subcommand)]
        command: MetricsCommand,
    },

    /// Start a JSON-RPC 2.0 session over stdin/stdout (for agent integration)
    Session,
}

#[derive(Subcommand)]
enum MetricsCommand {
    /// List all metric names
    List {
        /// Prometheus endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Read endpoint from a workspace
        #[arg(short = 'w', long = "workspace")]
        metrics_workspace: Option<String>,

        /// Filter by PromQL match selector (e.g. '{job="api"}')
        #[arg(long, value_name = "SELECTOR")]
        r#match: Option<String>,
    },

    /// List all label names
    Labels {
        /// Prometheus endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Read endpoint from a workspace
        #[arg(short = 'w', long = "workspace")]
        metrics_workspace: Option<String>,

        /// Filter by PromQL match selector
        #[arg(long, value_name = "SELECTOR")]
        r#match: Option<String>,
    },

    /// List values for a specific label
    LabelValues {
        /// Label name (e.g. "job", "instance", "env")
        label: String,

        /// Prometheus endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Read endpoint from a workspace
        #[arg(short = 'w', long = "workspace")]
        metrics_workspace: Option<String>,
    },

    /// Show metric type and help text (metadata)
    Info {
        /// Metric name (omit for all metrics)
        metric: Option<String>,

        /// Prometheus endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Read endpoint from a workspace
        #[arg(short = 'w', long = "workspace")]
        metrics_workspace: Option<String>,
    },

    /// Find series matching a selector
    Series {
        /// PromQL series selector (e.g. '{job="api"}' or 'http_requests_total')
        selector: String,

        /// Prometheus endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Read endpoint from a workspace
        #[arg(short = 'w', long = "workspace")]
        metrics_workspace: Option<String>,
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

/// Initialize the tracing subscriber for agent commands (serve, session).
///
/// Writes to stderr so stdout remains available for JSON-RPC.
/// Respects `RUST_LOG`; defaults to `info` for enya crates, `warn` elsewhere.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("warn,enya_agent=info,enya_headless=info,enya_config=info")
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(true)
        .init();
}

fn handle_error(e: &dyn std::fmt::Display, json: bool) -> ExitCode {
    if json {
        let _ = serde_json::to_writer(
            std::io::stdout(),
            &serde_json::json!({"error": e.to_string()}),
        );
        println!();
    } else {
        eprintln!("{} {e}", style("error:").red().bold());
    }
    ExitCode::FAILURE
}

pub fn run() -> ExitCode {
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
                project,
            } => enya_headless::workspace::init(
                name,
                endpoint.as_deref(),
                template.as_deref(),
                output.as_deref(),
                project.as_deref(),
                json,
            ),
            Command::List => enya_headless::workspace::list(json),
            Command::Show { name, project } => {
                enya_headless::workspace::show(&name, &project, json)
            }
            Command::Check { name } => {
                return match enya_headless::check::check(name.as_deref(), json) {
                    Ok(true) => ExitCode::FAILURE,
                    Ok(false) => ExitCode::SUCCESS,
                    Err(e) => handle_error(&*e, json),
                };
            }
            Command::Fmt { name } => enya_headless::fmt::fmt(name.as_deref(), json),
            Command::Rm { name, project } => enya_headless::workspace::rm(&name, &project, json),
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
            Command::Get { name, key, project } => {
                enya_headless::workspace::get(&name, &key, &project, json)
            }
            Command::Set {
                name,
                key,
                value,
                project,
            } => enya_headless::workspace::set(&name, &key, &value, &project, json),
            Command::AddPane {
                name,
                query,
                pane_name,
                tag,
                unit,
                granularity,
                visualization,
                description,
                project,
            } => enya_headless::workspace::add_pane(
                &enya_headless::workspace::AddPaneParams {
                    name: &name,
                    query: &query,
                    pane_name: pane_name.as_deref(),
                    tag: tag.as_deref(),
                    unit: unit.as_deref(),
                    granularity: granularity.as_deref(),
                    visualization: visualization.as_deref(),
                    description: description.as_deref(),
                },
                &project,
                json,
            ),
            Command::RemovePane {
                name,
                pane,
                project,
            } => enya_headless::workspace::remove_pane(&name, &pane, &project, json),
            Command::Snapshot {
                name,
                endpoint,
                output,
                project,
            } => enya_headless::workspace::snapshot_cmd(
                &name,
                endpoint.as_deref(),
                output.as_deref(),
                &project,
                json,
            ),
            Command::Serve {
                workspace,
                port,
                bind,
                open,
            } => {
                #[cfg(feature = "serve")]
                {
                    init_tracing();
                    enya_agent::run(workspace.as_deref(), port, &bind, open)
                }
                #[cfg(not(feature = "serve"))]
                {
                    let _ = (&workspace, port, &bind, open);
                    Err("serve requires the 'serve' feature (rebuild with --features serve)".into())
                }
            }
            Command::Metrics { command: sub } => match sub {
                MetricsCommand::List {
                    endpoint,
                    metrics_workspace,
                    r#match,
                } => enya_headless::query::discovery::metrics_list(
                    endpoint.as_deref(),
                    metrics_workspace.as_deref(),
                    r#match.as_deref(),
                    json,
                ),
                MetricsCommand::Labels {
                    endpoint,
                    metrics_workspace,
                    r#match,
                } => enya_headless::query::discovery::metrics_labels(
                    endpoint.as_deref(),
                    metrics_workspace.as_deref(),
                    r#match.as_deref(),
                    json,
                ),
                MetricsCommand::LabelValues {
                    label,
                    endpoint,
                    metrics_workspace,
                } => enya_headless::query::discovery::metrics_label_values(
                    endpoint.as_deref(),
                    metrics_workspace.as_deref(),
                    &label,
                    json,
                ),
                MetricsCommand::Info {
                    metric,
                    endpoint,
                    metrics_workspace,
                } => enya_headless::query::discovery::metrics_info(
                    endpoint.as_deref(),
                    metrics_workspace.as_deref(),
                    metric.as_deref(),
                    json,
                ),
                MetricsCommand::Series {
                    selector,
                    endpoint,
                    metrics_workspace,
                } => enya_headless::query::discovery::metrics_series(
                    endpoint.as_deref(),
                    metrics_workspace.as_deref(),
                    &selector,
                    json,
                ),
            },
            Command::Watch {
                workspace,
                expression,
                above,
                below,
                every,
                r#for,
                endpoint,
            } => {
                return match enya_headless::watch::run_cli(&enya_headless::watch::WatchCliParams {
                    expression: &expression,
                    endpoint: endpoint.as_deref(),
                    workspace: workspace.as_deref(),
                    above,
                    below,
                    every: &every,
                    for_duration: r#for.as_deref(),
                    json,
                }) {
                    Ok(true) => ExitCode::FAILURE,
                    Ok(false) => ExitCode::SUCCESS,
                    Err(e) => handle_error(&*e, json),
                };
            }
            Command::Session => {
                init_tracing();
                return match enya_agent::run_session() {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("session error: {e}");
                        ExitCode::FAILURE
                    }
                };
            }
            Command::Open { .. } => unreachable!("handled above"),
        };

        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => handle_error(&*e, json),
        };
    }

    // Parse enya:// deep link URL if provided (e.g. from macOS URL scheme cold launch)
    let startup_snapshot = cli.url.as_deref().and_then(|url| {
        url.strip_prefix("enya://snapshot/")
            .map(|id| id.to_string())
    });

    // No subcommand (or `open`): launch GUI editor
    #[cfg(feature = "ui")]
    {
        match enya_editor::run_native_app(workspace, startup_snapshot) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{} {e}", style("error:").red().bold());
                ExitCode::FAILURE
            }
        }
    }

    #[cfg(not(feature = "ui"))]
    {
        let _ = (workspace, startup_snapshot);
        eprintln!(
            "{} GUI not available (built without 'ui' feature)",
            style("error:").red().bold()
        );
        eprintln!("Use a subcommand: enya init, enya list, enya show, enya rm");
        ExitCode::FAILURE
    }
}
