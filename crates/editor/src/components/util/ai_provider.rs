//! AI provider and model definitions.
//!
//! Provides shared types for AI provider and model selection,
//! used by the `AgentPanel` overlay and `AgentInputBar` widget.

/// Available AI providers for agent chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AiProvider {
    /// Claude Code (Anthropic) - default
    #[default]
    Claude,
    /// Codex (OpenAI)
    Codex,
}

impl AiProvider {
    /// Get the display name for this provider.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
        }
    }

    /// Parse a provider from a string name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "claude" | "anthropic" => Some(Self::Claude),
            "codex" | "openai" => Some(Self::Codex),
            _ => None,
        }
    }

    /// List all available providers.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[Self::Claude, Self::Codex]
    }
}

/// Available AI models (varies by provider).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AiModel {
    // Claude models
    /// Claude Sonnet 4.5
    ClaudeSonnet45,
    /// Claude Opus 4.5
    ClaudeOpus45,
    /// Claude Haiku 4.5
    ClaudeHaiku45,
    // OpenAI models (GPT-5.2 series)
    /// GPT-5.2 base model
    Gpt52,
    /// GPT-5.2 Pro model
    Gpt52Pro,
    /// GPT-5.2 Codex model
    Gpt52Codex,
}

impl AiModel {
    /// Get the display name for this model.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeSonnet45 => "Sonnet 4.5",
            Self::ClaudeOpus45 => "Opus 4.5",
            Self::ClaudeHaiku45 => "Haiku 4.5",
            Self::Gpt52 => "GPT-5.2",
            Self::Gpt52Pro => "GPT-5.2 Pro",
            Self::Gpt52Codex => "GPT-5.2 Codex",
        }
    }

    /// Get the API model ID for this model.
    #[must_use]
    pub fn model_id(self) -> &'static str {
        match self {
            Self::ClaudeSonnet45 => "claude-sonnet-4-5-20250514",
            Self::ClaudeOpus45 => "claude-opus-4-5-20250514",
            Self::ClaudeHaiku45 => "claude-haiku-4-5-20250514",
            Self::Gpt52 => "gpt-5.2-2025-12-11",
            Self::Gpt52Pro => "gpt-5.2-pro-2025-12-11",
            Self::Gpt52Codex => "gpt-5.2-codex",
        }
    }

    /// Get models available for a provider.
    #[must_use]
    pub fn for_provider(provider: AiProvider) -> &'static [Self] {
        match provider {
            AiProvider::Claude => &[
                Self::ClaudeSonnet45,
                Self::ClaudeOpus45,
                Self::ClaudeHaiku45,
            ],
            AiProvider::Codex => &[Self::Gpt52Codex, Self::Gpt52, Self::Gpt52Pro],
        }
    }

    /// Get the default model for a provider.
    #[must_use]
    pub fn default_for(provider: AiProvider) -> Self {
        match provider {
            AiProvider::Claude => Self::ClaudeSonnet45,
            AiProvider::Codex => Self::Gpt52Codex,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_parse() {
        assert_eq!(AiProvider::parse("claude"), Some(AiProvider::Claude));
        assert_eq!(AiProvider::parse("ANTHROPIC"), Some(AiProvider::Claude));
        assert_eq!(AiProvider::parse("codex"), Some(AiProvider::Codex));
        assert_eq!(AiProvider::parse("openai"), Some(AiProvider::Codex));
        assert_eq!(AiProvider::parse("unknown"), None);
    }

    #[test]
    fn test_provider_all() {
        let all = AiProvider::all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&AiProvider::Claude));
        assert!(all.contains(&AiProvider::Codex));
    }

    #[test]
    fn test_model_for_provider() {
        let claude_models = AiModel::for_provider(AiProvider::Claude);
        assert!(claude_models.contains(&AiModel::ClaudeSonnet45));
        assert!(!claude_models.contains(&AiModel::Gpt52));

        let codex_models = AiModel::for_provider(AiProvider::Codex);
        assert!(codex_models.contains(&AiModel::Gpt52Codex));
        assert!(!codex_models.contains(&AiModel::ClaudeSonnet45));
    }

    #[test]
    fn test_model_default_for_provider() {
        assert_eq!(
            AiModel::default_for(AiProvider::Claude),
            AiModel::ClaudeSonnet45
        );
        assert_eq!(AiModel::default_for(AiProvider::Codex), AiModel::Gpt52Codex);
    }
}
