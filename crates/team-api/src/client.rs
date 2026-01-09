//! HTTP client for the team collaboration API.
//!
//! Provides a promise-based interface for making API requests that works
//! with egui's immediate mode rendering.

use std::future::Future;

use poll_promise::Promise;

use crate::error::{TeamApiError, TeamApiResult};
use crate::promise::promise_channel;
use crate::types::*;

/// HTTP client for the team API.
///
/// Uses reqwest for HTTP requests with promise-based async that works
/// on both native and WASM platforms.
pub struct TeamClient {
    /// Base URL of the team server.
    base_url: String,
    /// Authentication token.
    auth_token: Option<String>,
    /// HTTP client.
    http_client: reqwest::Client,
    /// Tokio runtime handle for spawning async tasks (native only).
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: tokio::runtime::Handle,
}

impl TeamClient {
    /// Create a new team client (native).
    ///
    /// Requires a tokio runtime handle for spawning async HTTP requests.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(base_url: impl Into<String>, runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            base_url: normalize_url(base_url.into()),
            auth_token: None,
            http_client: reqwest::Client::new(),
            runtime_handle,
        }
    }

    /// Create a new team client (WASM).
    ///
    /// On WASM, no runtime handle is needed - tasks are spawned using
    /// `wasm-bindgen-futures::spawn_local`.
    #[cfg(target_arch = "wasm32")]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: normalize_url(base_url.into()),
            auth_token: None,
            http_client: reqwest::Client::new(),
        }
    }

    /// Get the runtime handle (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn runtime_handle(&self) -> &tokio::runtime::Handle {
        &self.runtime_handle
    }

    /// Set the authentication token.
    pub fn set_auth_token(&mut self, token: impl Into<String>) {
        self.auth_token = Some(token.into());
    }

    /// Clear the authentication token.
    pub fn clear_auth_token(&mut self) {
        self.auth_token = None;
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Check if authenticated.
    pub fn is_authenticated(&self) -> bool {
        self.auth_token.is_some()
    }

    /// Get the authentication token.
    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    /// Build a URL path relative to the base URL.
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Add auth header to request if token is set.
    fn with_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.auth_token {
            Some(token) => request.header("Authorization", format!("Bearer {token}")),
            None => request,
        }
    }

    // -------------------------------------------------------------------------
    // Auth endpoints
    // -------------------------------------------------------------------------

    /// Exchange an OAuth code for an access token.
    pub fn exchange_oauth_code(
        &self,
        provider: OAuthProvider,
        code: &str,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<AuthResponse>> {
        let body = serde_json::json!({ "code": code });
        self.post_json(&format!("/auth/{provider}/callback"), body, ctx)
    }

    /// Get the current authenticated user.
    pub fn get_current_user(&self, ctx: &egui::Context) -> Promise<TeamApiResult<User>> {
        self.get_json("/auth/me", ctx)
    }

    // -------------------------------------------------------------------------
    // Team endpoints
    // -------------------------------------------------------------------------

    /// List all teams the current user belongs to.
    pub fn list_teams(&self, ctx: &egui::Context) -> Promise<TeamApiResult<Vec<Team>>> {
        self.get_json("/teams", ctx)
    }

    /// Get team by ID.
    pub fn get_team(&self, team_id: TeamId, ctx: &egui::Context) -> Promise<TeamApiResult<Team>> {
        self.get_json(&format!("/teams/{team_id}"), ctx)
    }

    /// List team members.
    pub fn list_team_members(
        &self,
        team_id: TeamId,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Vec<User>>> {
        self.get_json(&format!("/teams/{team_id}/members"), ctx)
    }

    // -------------------------------------------------------------------------
    // Annotation endpoints
    // -------------------------------------------------------------------------

    /// List annotations for a query fingerprint.
    pub fn list_annotations(
        &self,
        team_id: TeamId,
        query_fingerprint: &str,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Vec<Annotation>>> {
        self.get_json(
            &format!("/teams/{team_id}/annotations?query_fp={query_fingerprint}"),
            ctx,
        )
    }

    /// Create a new annotation.
    pub fn create_annotation(
        &self,
        team_id: TeamId,
        annotation: &NewAnnotation,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Annotation>> {
        self.post_json(&format!("/teams/{team_id}/annotations"), annotation, ctx)
    }

    /// Delete an annotation.
    pub fn delete_annotation(
        &self,
        team_id: TeamId,
        annotation_id: AnnotationId,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<()>> {
        self.delete(
            &format!("/teams/{team_id}/annotations/{annotation_id}"),
            ctx,
        )
    }

    // -------------------------------------------------------------------------
    // Thread/message endpoints
    // -------------------------------------------------------------------------

    /// Get messages in a thread.
    pub fn list_messages(
        &self,
        thread_id: ThreadId,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Vec<Message>>> {
        self.get_json(&format!("/threads/{thread_id}/messages"), ctx)
    }

    /// Send a message to a thread.
    pub fn send_message(
        &self,
        thread_id: ThreadId,
        message: &NewMessage,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Message>> {
        self.post_json(&format!("/threads/{thread_id}/messages"), message, ctx)
    }

    /// Mark a thread as resolved.
    pub fn resolve_thread(
        &self,
        thread_id: ThreadId,
        resolved: bool,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Thread>> {
        let body = serde_json::json!({ "resolved": resolved });
        self.patch_json(&format!("/threads/{thread_id}"), body, ctx)
    }

    // -------------------------------------------------------------------------
    // War room endpoints
    // -------------------------------------------------------------------------

    /// Share current view with team (war room mode).
    pub fn share_view(
        &self,
        team_id: TeamId,
        workspace_url: &str,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<()>> {
        let body = serde_json::json!({ "workspace_url": workspace_url });
        self.post_json_no_response(&format!("/teams/{team_id}/war-room/share"), body, ctx)
    }

    // -------------------------------------------------------------------------
    // Channel endpoints
    // -------------------------------------------------------------------------

    /// List channels in a team.
    pub fn list_channels(
        &self,
        team_id: TeamId,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Vec<Channel>>> {
        self.get_json(&format!("/teams/{team_id}/channels"), ctx)
    }

    /// Create a new channel.
    pub fn create_channel(
        &self,
        team_id: TeamId,
        channel: &NewChannel,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Channel>> {
        self.post_json(&format!("/teams/{team_id}/channels"), channel, ctx)
    }

    /// List threads in a channel.
    pub fn list_channel_threads(
        &self,
        team_id: TeamId,
        channel_id: ChannelId,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Vec<ChatThread>>> {
        self.get_json(
            &format!("/teams/{team_id}/channels/{channel_id}/threads"),
            ctx,
        )
    }

    /// Create a thread in a channel.
    pub fn create_thread(
        &self,
        team_id: TeamId,
        channel_id: ChannelId,
        thread: &NewThread,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<ChatThread>> {
        self.post_json(
            &format!("/teams/{team_id}/channels/{channel_id}/threads"),
            thread,
            ctx,
        )
    }

    /// List messages in a channel thread.
    pub fn list_channel_messages(
        &self,
        team_id: TeamId,
        channel_id: ChannelId,
        thread_id: ThreadId,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Vec<Message>>> {
        self.get_json(
            &format!("/teams/{team_id}/channels/{channel_id}/threads/{thread_id}/messages"),
            ctx,
        )
    }

    /// Send a message to a channel thread.
    pub fn send_channel_message(
        &self,
        team_id: TeamId,
        channel_id: ChannelId,
        thread_id: ThreadId,
        message: &NewMessage,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<Message>> {
        self.post_json(
            &format!("/teams/{team_id}/channels/{channel_id}/threads/{thread_id}/messages"),
            message,
            ctx,
        )
    }

    // -------------------------------------------------------------------------
    // HTTP helpers - Native (with Send bounds)
    // -------------------------------------------------------------------------

    #[cfg(not(target_arch = "wasm32"))]
    /// Make a GET request and parse JSON response.
    fn get_json<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<T>> {
        let request = self
            .http_client
            .get(self.url(path))
            .header("Accept", "application/json");
        let request = self.with_auth(request);

        spawn_request(&self.runtime_handle, ctx, async move {
            execute_json_request(request).await
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Make a POST request with JSON body and parse JSON response.
    fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        body: B,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<T>> {
        let request = self
            .http_client
            .post(self.url(path))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body);
        let request = self.with_auth(request);

        spawn_request(&self.runtime_handle, ctx, async move {
            execute_json_request(request).await
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Make a POST request with JSON body, no response body expected.
    fn post_json_no_response<B: serde::Serialize>(
        &self,
        path: &str,
        body: B,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<()>> {
        let request = self
            .http_client
            .post(self.url(path))
            .header("Content-Type", "application/json")
            .json(&body);
        let request = self.with_auth(request);

        spawn_request(&self.runtime_handle, ctx, async move {
            execute_no_response_request(request).await
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Make a PATCH request with JSON body and parse JSON response.
    fn patch_json<B: serde::Serialize, T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        body: B,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<T>> {
        let request = self
            .http_client
            .patch(self.url(path))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body);
        let request = self.with_auth(request);

        spawn_request(&self.runtime_handle, ctx, async move {
            execute_json_request(request).await
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Make a DELETE request.
    fn delete(&self, path: &str, ctx: &egui::Context) -> Promise<TeamApiResult<()>> {
        let request = self.http_client.delete(self.url(path));
        let request = self.with_auth(request);

        spawn_request(&self.runtime_handle, ctx, async move {
            execute_no_response_request(request).await
        })
    }

    // -------------------------------------------------------------------------
    // HTTP helpers - WASM (without Send bound on Future, but T must be Send)
    // -------------------------------------------------------------------------

    #[cfg(target_arch = "wasm32")]
    /// Make a GET request and parse JSON response.
    fn get_json<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<T>> {
        let request = self
            .http_client
            .get(self.url(path))
            .header("Accept", "application/json");
        let request = self.with_auth(request);

        spawn_request(ctx, async move { execute_json_request(request).await })
    }

    #[cfg(target_arch = "wasm32")]
    /// Make a POST request with JSON body and parse JSON response.
    fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        body: B,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<T>> {
        let request = self
            .http_client
            .post(self.url(path))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body);
        let request = self.with_auth(request);

        spawn_request(ctx, async move { execute_json_request(request).await })
    }

    #[cfg(target_arch = "wasm32")]
    /// Make a POST request with JSON body, no response body expected.
    fn post_json_no_response<B: serde::Serialize>(
        &self,
        path: &str,
        body: B,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<()>> {
        let request = self
            .http_client
            .post(self.url(path))
            .header("Content-Type", "application/json")
            .json(&body);
        let request = self.with_auth(request);

        spawn_request(
            ctx,
            async move { execute_no_response_request(request).await },
        )
    }

    #[cfg(target_arch = "wasm32")]
    /// Make a PATCH request with JSON body and parse JSON response.
    fn patch_json<B: serde::Serialize, T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        body: B,
        ctx: &egui::Context,
    ) -> Promise<TeamApiResult<T>> {
        let request = self
            .http_client
            .patch(self.url(path))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body);
        let request = self.with_auth(request);

        spawn_request(ctx, async move { execute_json_request(request).await })
    }

    #[cfg(target_arch = "wasm32")]
    /// Make a DELETE request.
    fn delete(&self, path: &str, ctx: &egui::Context) -> Promise<TeamApiResult<()>> {
        let request = self.http_client.delete(self.url(path));
        let request = self.with_auth(request);

        spawn_request(
            ctx,
            async move { execute_no_response_request(request).await },
        )
    }
}

// =============================================================================
// Async execution helpers
// =============================================================================

/// Spawn an async request with platform-appropriate runtime (native).
/// Returns a Promise that will complete when the request finishes.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_request<T, F>(
    runtime: &tokio::runtime::Handle,
    ctx: &egui::Context,
    future: F,
) -> Promise<TeamApiResult<T>>
where
    T: Send + 'static,
    F: Future<Output = TeamApiResult<T>> + Send + 'static,
{
    let (sender, promise) = promise_channel();
    let ctx = ctx.clone();

    runtime.spawn(async move {
        let result = future.await;
        sender.send(result);
        ctx.request_repaint();
    });

    promise
}

/// Spawn an async request with platform-appropriate runtime (WASM).
/// Returns a Promise that will complete when the request finishes.
/// Note: WASM doesn't require Send because spawn_local runs on single thread.
#[cfg(target_arch = "wasm32")]
fn spawn_request<T, F>(ctx: &egui::Context, future: F) -> Promise<TeamApiResult<T>>
where
    T: Send + 'static,
    F: Future<Output = TeamApiResult<T>> + 'static,
{
    let (sender, promise) = promise_channel();
    let ctx = ctx.clone();

    wasm_bindgen_futures::spawn_local(async move {
        let result = future.await;
        sender.send(result);
        ctx.request_repaint();
    });

    promise
}

/// Execute a request and parse JSON response.
async fn execute_json_request<T: serde::de::DeserializeOwned>(
    request: reqwest::RequestBuilder,
) -> TeamApiResult<T> {
    let response = request
        .send()
        .await
        .map_err(|e| TeamApiError::network(e.to_string()))?;

    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| TeamApiError::network(e.to_string()))?;

    if status.is_success() {
        serde_json::from_slice(&bytes).map_err(|e| TeamApiError::parse(e.to_string()))
    } else {
        let message = String::from_utf8_lossy(&bytes).to_string();
        Err(TeamApiError::server(status.as_u16(), message))
    }
}

/// Execute a request with no response body expected.
async fn execute_no_response_request(request: reqwest::RequestBuilder) -> TeamApiResult<()> {
    let response = request
        .send()
        .await
        .map_err(|e| TeamApiError::network(e.to_string()))?;

    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        let bytes = response.bytes().await.unwrap_or_default();
        let message = String::from_utf8_lossy(&bytes).to_string();
        Err(TeamApiError::server(status.as_u16(), message))
    }
}

/// Normalize a URL (remove trailing slash, ensure scheme).
fn normalize_url(url: String) -> String {
    let url = url.trim().trim_end_matches('/');

    if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("https://{url}")
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("api.enya.dev".into()), "https://api.enya.dev");
        assert_eq!(
            normalize_url("https://api.enya.dev/".into()),
            "https://api.enya.dev"
        );
        assert_eq!(
            normalize_url("http://localhost:3000".into()),
            "http://localhost:3000"
        );
    }

    #[test]
    fn test_client_auth() {
        // Create a runtime for the test
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut client = TeamClient::new("https://api.enya.dev", rt.handle().clone());
        assert!(!client.is_authenticated());

        client.set_auth_token("test_token");
        assert!(client.is_authenticated());

        client.clear_auth_token();
        assert!(!client.is_authenticated());
    }
}
