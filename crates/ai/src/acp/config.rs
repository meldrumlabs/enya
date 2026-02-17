//! Agent configuration.

use std::path::PathBuf;

/// Known agent types with predefined configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    /// Claude Code (Anthropic)
    ClaudeCode,
    /// Gemini CLI (Google)
    GeminiCli,
    /// Codex (OpenAI)
    Codex,
    /// Goose (Square)
    Goose,
    /// Custom agent
    Custom,
}

impl AgentKind {
    /// Get a display name for the agent.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::GeminiCli => "Gemini CLI",
            Self::Codex => "Codex",
            Self::Goose => "Goose",
            Self::Custom => "Custom Agent",
        }
    }
}

/// Configuration for connecting to an ACP agent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// The kind of agent.
    pub kind: AgentKind,
    /// Command to spawn the agent.
    pub command: String,
    /// Arguments to pass to the agent.
    pub args: Vec<String>,
    /// Working directory for the agent.
    pub working_dir: Option<PathBuf>,
    /// Environment variables to set.
    pub env: Vec<(String, String)>,
    /// Environment variables to remove (for clean environment).
    pub env_remove: Vec<String>,
}

impl AgentConfig {
    /// Create a configuration for Claude Code via the ACP adapter.
    ///
    /// Uses the `@zed-industries/claude-code-acp` npm package which wraps
    /// the Claude Agent SDK with ACP protocol support.
    ///
    /// The `-y` flag auto-confirms the npx install prompt (same as avante.nvim).
    #[must_use]
    pub fn claude_code() -> Self {
        Self {
            kind: AgentKind::ClaudeCode,
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@zed-industries/claude-code-acp".to_string(),
            ],
            working_dir: None,
            env: vec![],
            env_remove: vec![],
        }
    }

    /// Create a configuration for Claude Code with a specific API key.
    #[must_use]
    pub fn claude_code_with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            env: vec![("ANTHROPIC_API_KEY".to_string(), api_key.into())],
            ..Self::claude_code()
        }
    }

    /// Create a configuration for Claude Code with a custom path.
    #[must_use]
    pub fn claude_code_with_path(path: impl Into<String>) -> Self {
        Self {
            command: path.into(),
            ..Self::claude_code()
        }
    }

    /// Create a configuration for Gemini CLI.
    #[must_use]
    pub fn gemini_cli() -> Self {
        Self {
            kind: AgentKind::GeminiCli,
            command: "gemini".to_string(),
            args: vec!["--acp".to_string()],
            working_dir: None,
            env: vec![],
            env_remove: vec![],
        }
    }

    /// Create a configuration for OpenAI Codex via the ACP adapter.
    ///
    /// Uses the `@zed-industries/codex-acp` npm package which wraps
    /// the Codex CLI with ACP protocol support.
    ///
    /// Requires `OPENAI_API_KEY` environment variable to be set.
    #[must_use]
    pub fn codex() -> Self {
        Self {
            kind: AgentKind::Codex,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@zed-industries/codex-acp".to_string()],
            working_dir: None,
            env: vec![],
            env_remove: vec![],
        }
    }

    /// Create a configuration for Codex with a specific API key.
    #[must_use]
    pub fn codex_with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            env: vec![("OPENAI_API_KEY".to_string(), api_key.into())],
            ..Self::codex()
        }
    }

    /// Create a configuration for Goose.
    #[must_use]
    pub fn goose() -> Self {
        Self {
            kind: AgentKind::Goose,
            command: "goose".to_string(),
            args: vec!["--acp".to_string()],
            working_dir: None,
            env: vec![],
            env_remove: vec![],
        }
    }

    /// Create a custom agent configuration.
    #[must_use]
    pub fn custom(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            kind: AgentKind::Custom,
            command: command.into(),
            args,
            working_dir: None,
            env: vec![],
            env_remove: vec![],
        }
    }

    /// Set the working directory.
    #[must_use]
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Add an environment variable.
    #[must_use]
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Add a command-line argument.
    #[must_use]
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}
