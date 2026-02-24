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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_kind_display_names() {
        assert_eq!(AgentKind::ClaudeCode.display_name(), "Claude Code");
        assert_eq!(AgentKind::GeminiCli.display_name(), "Gemini CLI");
        assert_eq!(AgentKind::Codex.display_name(), "Codex");
        assert_eq!(AgentKind::Custom.display_name(), "Custom Agent");
    }

    #[test]
    fn agent_kind_equality() {
        assert_eq!(AgentKind::ClaudeCode, AgentKind::ClaudeCode);
        assert_ne!(AgentKind::ClaudeCode, AgentKind::Codex);
    }

    #[test]
    fn claude_code_config() {
        let config = AgentConfig::claude_code();
        assert_eq!(config.kind, AgentKind::ClaudeCode);
        assert_eq!(config.command, "npx");
        assert!(config.args.contains(&"-y".to_string()));
        assert!(
            config
                .args
                .contains(&"@zed-industries/claude-code-acp".to_string())
        );
        assert!(config.working_dir.is_none());
        assert!(config.env.is_empty());
        assert!(config.env_remove.is_empty());
    }

    #[test]
    fn claude_code_with_api_key() {
        let config = AgentConfig::claude_code_with_api_key("sk-test-123");
        assert_eq!(config.kind, AgentKind::ClaudeCode);
        assert_eq!(config.command, "npx");
        assert_eq!(config.env.len(), 1);
        assert_eq!(config.env[0].0, "ANTHROPIC_API_KEY");
        assert_eq!(config.env[0].1, "sk-test-123");
    }

    #[test]
    fn claude_code_with_path() {
        let config = AgentConfig::claude_code_with_path("/usr/local/bin/claude-acp");
        assert_eq!(config.kind, AgentKind::ClaudeCode);
        assert_eq!(config.command, "/usr/local/bin/claude-acp");
        // Should still have the npx args (inherited from claude_code())
        assert!(
            config
                .args
                .contains(&"@zed-industries/claude-code-acp".to_string())
        );
    }

    #[test]
    fn gemini_cli_config() {
        let config = AgentConfig::gemini_cli();
        assert_eq!(config.kind, AgentKind::GeminiCli);
        assert_eq!(config.command, "gemini");
        assert_eq!(config.args, vec!["--acp"]);
    }

    #[test]
    fn codex_config() {
        let config = AgentConfig::codex();
        assert_eq!(config.kind, AgentKind::Codex);
        assert_eq!(config.command, "npx");
        assert!(
            config
                .args
                .contains(&"@zed-industries/codex-acp".to_string())
        );
    }

    #[test]
    fn codex_with_api_key() {
        let config = AgentConfig::codex_with_api_key("sk-openai-key");
        assert_eq!(config.kind, AgentKind::Codex);
        assert_eq!(config.env.len(), 1);
        assert_eq!(config.env[0].0, "OPENAI_API_KEY");
        assert_eq!(config.env[0].1, "sk-openai-key");
    }

    #[test]
    fn custom_config() {
        let config = AgentConfig::custom("my-agent", vec!["--mode".into(), "acp".into()]);
        assert_eq!(config.kind, AgentKind::Custom);
        assert_eq!(config.command, "my-agent");
        assert_eq!(config.args, vec!["--mode", "acp"]);
    }

    #[test]
    fn with_working_dir() {
        let config = AgentConfig::claude_code().with_working_dir("/home/user/project");
        assert_eq!(
            config.working_dir,
            Some(PathBuf::from("/home/user/project"))
        );
    }

    #[test]
    fn with_env() {
        let config = AgentConfig::claude_code()
            .with_env("FOO", "bar")
            .with_env("BAZ", "qux");
        assert_eq!(config.env.len(), 2);
        assert_eq!(config.env[0], ("FOO".to_string(), "bar".to_string()));
        assert_eq!(config.env[1], ("BAZ".to_string(), "qux".to_string()));
    }

    #[test]
    fn with_arg() {
        let config = AgentConfig::claude_code()
            .with_arg("--verbose")
            .with_arg("--timeout=30");
        assert!(config.args.contains(&"--verbose".to_string()));
        assert!(config.args.contains(&"--timeout=30".to_string()));
    }

    #[test]
    fn builder_chaining() {
        let config = AgentConfig::custom("agent", vec![])
            .with_working_dir("/tmp")
            .with_env("KEY", "value")
            .with_arg("--flag");

        assert_eq!(config.working_dir, Some(PathBuf::from("/tmp")));
        assert_eq!(config.env.len(), 1);
        // original empty args + --flag
        assert_eq!(config.args, vec!["--flag"]);
    }
}
