//! AI provider and model definitions.
//!
//! Provides shared types for AI provider and model selection,
//! used by the `AgentPanel` overlay and `AgentInputBar` widget.
//!
//! Model definitions are loaded from `providers.json` at the repo root,
//! bundled into the binary at compile time. A remote fetch from GitHub
//! can hot-update the manifest at runtime without a rebuild.

use std::sync::LazyLock;

use parking_lot::RwLock;
use serde::Deserialize;

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

    /// Get the manifest provider ID for this provider.
    fn manifest_id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

/// An AI model with an API ID and display name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiModel {
    /// API model identifier (e.g. `"claude-sonnet-4-5-20250514"`).
    pub id: String,
    /// Human-readable display name (e.g. `"Sonnet 4.5"`).
    pub name: String,
}

impl AiModel {
    /// Get the display name for this model.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.name
    }

    /// Get the API model ID for this model.
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.id
    }
}

// -- Provider manifest (loaded from bundled JSON, hot-updatable from remote) --

/// Bundled providers.json content (compile-time fallback).
const PROVIDERS_JSON: &str = include_str!("../../../../../providers.json");

/// Global manifest, initialized from bundled JSON and updatable at runtime.
static MANIFEST: LazyLock<RwLock<ProviderManifest>> = LazyLock::new(|| {
    RwLock::new(serde_json::from_str(PROVIDERS_JSON).expect("providers.json is valid"))
});

/// Top-level manifest loaded from `providers.json`.
#[derive(Debug, Deserialize)]
pub struct ProviderManifest {
    providers: Vec<ProviderEntry>,
}

/// A provider entry in the manifest.
#[derive(Debug, Deserialize)]
struct ProviderEntry {
    id: String,
    #[allow(dead_code)]
    name: String,
    models: Vec<ModelEntry>,
}

/// A model entry in the manifest.
#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    name: String,
}

impl ProviderManifest {
    /// Get models available for a provider.
    #[must_use]
    pub fn models_for(provider: AiProvider) -> Vec<AiModel> {
        let manifest = MANIFEST.read();
        manifest
            .providers
            .iter()
            .find(|p| p.id == provider.manifest_id())
            .map(|p| {
                p.models
                    .iter()
                    .map(|m| AiModel {
                        id: m.id.clone(),
                        name: m.name.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the default model for a provider (first model in the list).
    #[must_use]
    pub fn default_model_for(provider: AiProvider) -> Option<AiModel> {
        let models = Self::models_for(provider);
        models.into_iter().next()
    }

    /// Get the default model ID for a provider.
    #[must_use]
    pub fn default_model_id_for(provider: AiProvider) -> Option<String> {
        Self::default_model_for(provider).map(|m| m.id)
    }

    /// Find a model by its API ID across all providers.
    #[must_use]
    pub fn find_model(id: &str) -> Option<AiModel> {
        let manifest = MANIFEST.read();
        for provider in &manifest.providers {
            for model in &provider.models {
                if model.id == id {
                    return Some(AiModel {
                        id: model.id.clone(),
                        name: model.name.clone(),
                    });
                }
            }
        }
        None
    }

    /// Get the display name for a model ID, or the ID itself if not found.
    #[must_use]
    pub fn display_name_for(model_id: &str) -> String {
        let manifest = MANIFEST.read();
        for provider in &manifest.providers {
            for model in &provider.models {
                if model.id == model_id {
                    return model.name.clone();
                }
            }
        }
        model_id.to_string()
    }

    /// Replace the global manifest with a freshly fetched one.
    #[cfg(not(target_arch = "wasm32"))]
    fn update(new_manifest: ProviderManifest) {
        *MANIFEST.write() = new_manifest;
    }
}

// -- ManifestFetcher: async fetch from GitHub raw URL --

/// Raw GitHub URL for the providers manifest on the main branch.
#[cfg(not(target_arch = "wasm32"))]
const MANIFEST_URL: &str = "https://raw.githubusercontent.com/meldrumlabs/enya/main/providers.json";

/// How often to re-fetch the manifest (6 hours).
#[cfg(not(target_arch = "wasm32"))]
const FETCH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

/// Delay before the first fetch after startup (15 seconds).
#[cfg(not(target_arch = "wasm32"))]
const FETCH_STARTUP_DELAY: std::time::Duration = std::time::Duration::from_secs(15);

/// Fetches the provider manifest from GitHub at runtime.
///
/// Follows the same async-poll pattern as `UpdateChecker`:
/// spawns an async HTTP request, polls for the result each frame.
#[cfg(not(target_arch = "wasm32"))]
pub struct ManifestFetcher {
    pending: std::sync::Arc<parking_lot::Mutex<Option<Result<ProviderManifest, String>>>>,
    last_fetch: Option<crate::util::Instant>,
    started_at: crate::util::Instant,
    http_client: reqwest::Client,
    async_runtime: crate::AsyncRuntime,
}

#[cfg(not(target_arch = "wasm32"))]
impl ManifestFetcher {
    /// Create a new manifest fetcher.
    pub fn new(async_runtime: crate::AsyncRuntime) -> Self {
        Self {
            pending: std::sync::Arc::new(parking_lot::Mutex::new(None)),
            last_fetch: None,
            started_at: crate::util::Instant::now(),
            http_client: reqwest::Client::new(),
            async_runtime,
        }
    }

    /// Poll for fetch results. Call this each frame.
    pub fn poll(&mut self, ctx: &egui::Context) {
        // Check for completed fetch
        if let Some(result) = self.pending.lock().take() {
            match result {
                Ok(manifest) => {
                    log::info!("Updated provider manifest from remote");
                    ProviderManifest::update(manifest);
                }
                Err(e) => {
                    log::warn!("Failed to fetch provider manifest: {e}");
                }
            }
        }

        // Determine if we should trigger a new fetch
        let should_fetch = match self.last_fetch {
            None => self.started_at.elapsed() >= FETCH_STARTUP_DELAY,
            Some(last) => last.elapsed() >= FETCH_INTERVAL,
        };

        if should_fetch {
            self.fetch(ctx);
        }
    }

    /// Fire off an async fetch.
    fn fetch(&mut self, ctx: &egui::Context) {
        self.last_fetch = Some(crate::util::Instant::now());

        let pending = std::sync::Arc::clone(&self.pending);
        let client = self.http_client.clone();
        let ctx = ctx.clone();

        self.async_runtime.spawn(async move {
            let result = Self::fetch_manifest(&client).await;
            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    /// Fetch and parse the manifest from GitHub.
    async fn fetch_manifest(client: &reqwest::Client) -> Result<ProviderManifest, String> {
        let response = client
            .get(MANIFEST_URL)
            .header("User-Agent", "enya-editor")
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }

        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {e}"))?;

        serde_json::from_str(&text).map_err(|e| format!("Failed to parse manifest: {e}"))
    }
}

/// Map legacy enum variant names to model API IDs for backwards compatibility.
#[must_use]
pub fn migrate_legacy_model_name(name: &str) -> &str {
    name
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
    fn test_manifest_loads() {
        let manifest = MANIFEST.read();
        assert_eq!(manifest.providers.len(), 2);
    }

    #[test]
    fn test_models_for_provider() {
        let claude_models = ProviderManifest::models_for(AiProvider::Claude);
        assert!(!claude_models.is_empty());
        assert!(claude_models.iter().any(|m| m.name == "Opus 4.6"));
        assert!(claude_models.iter().all(|m| m.id.contains("claude")));

        let codex_models = ProviderManifest::models_for(AiProvider::Codex);
        assert!(!codex_models.is_empty());
        assert!(codex_models.iter().any(|m| m.name == "GPT-5.3 Codex"));
        // Codex models may use different ID prefixes (gpt, o3, etc.)
        assert!(!codex_models.is_empty());
    }

    #[test]
    fn test_default_model_for_provider() {
        let claude_default = ProviderManifest::default_model_for(AiProvider::Claude).unwrap();
        assert_eq!(claude_default.name, "Opus 4.6");

        let codex_default = ProviderManifest::default_model_for(AiProvider::Codex).unwrap();
        assert_eq!(codex_default.name, "GPT-5.3 Codex");
    }

    #[test]
    fn test_find_model() {
        let model = ProviderManifest::find_model("claude-opus-4-6").unwrap();
        assert_eq!(model.name, "Opus 4.6");

        assert!(ProviderManifest::find_model("nonexistent").is_none());
    }

    #[test]
    fn test_display_name_for() {
        assert_eq!(
            ProviderManifest::display_name_for("claude-opus-4-6"),
            "Opus 4.6"
        );
        assert_eq!(
            ProviderManifest::display_name_for("unknown-model"),
            "unknown-model"
        );
    }

    #[test]
    fn test_legacy_migration() {
        assert_eq!(
            migrate_legacy_model_name("claude-opus-4-6"),
            "claude-opus-4-6"
        );
        assert_eq!(
            migrate_legacy_model_name("some-new-model-id"),
            "some-new-model-id"
        );
    }
}
