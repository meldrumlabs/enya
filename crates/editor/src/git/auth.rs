//! GitHub authentication via the Authorization Code flow.
//!
//! - **Native**: Opens the browser, waits for the redirect on a local
//!   TCP server, exchanges the code for a token via the API worker.
//! - **WASM**: Redirects the page to GitHub, detects the callback
//!   `?code=…&state=…` on reload, and exchanges via the API worker.

use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::AsyncRuntime;

/// GitHub OAuth App client ID for Enya.
const GITHUB_CLIENT_ID: &str = "Ov23likv8UvuCncMfUsm";

/// API worker endpoint for exchanging an authorization code for a token.
const EXCHANGE_URL: &str = "https://api.enya.build/auth/exchange";

/// GitHub User API URL.
///
/// On WASM we proxy through the API worker to bypass CORS.
fn user_api_url() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    {
        "https://api.enya.build/auth/user"
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "https://api.github.com/user"
    }
}

/// Persisted GitHub user information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub id: u64,
}

/// Persisted auth credentials.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitHubCredentials {
    pub access_token: String,
    pub user: GitHubUser,
}

/// High-level auth state for UI rendering.
#[derive(Debug, Clone)]
pub enum AuthState {
    SignedOut,
    /// Authentication in progress (browser opened / page redirected).
    Authenticating,
    SignedIn(GitHubCredentials),
    /// An error occurred during auth.
    Error(String),
}

impl Default for AuthState {
    fn default() -> Self {
        Self::SignedOut
    }
}

// ── Internal response types ─────────────────────────────────────────────

/// GitHub User API response (subset).
#[derive(Debug, Deserialize)]
struct GitHubUserResponse {
    login: String,
    avatar_url: String,
    name: Option<String>,
    id: u64,
}

/// Completed auth result: access token + user info.
type AuthResult = Result<(String, GitHubUserResponse), String>;

/// Pending avatar download result.
type AvatarResult = Result<Vec<u8>, String>;

// ── GitHubAuthManager ───────────────────────────────────────────────────

/// Manages GitHub authentication.
pub struct GitHubAuthManager {
    state: AuthState,
    http_client: reqwest::Client,
    async_runtime: AsyncRuntime,

    /// Completed auth result from the Authorization Code flow.
    pending_auth_result: Arc<Mutex<Option<AuthResult>>>,

    /// Pending avatar image download.
    pending_avatar: Arc<Mutex<Option<AvatarResult>>>,
    /// Cached avatar image bytes (PNG/JPEG).
    avatar_bytes: Option<Vec<u8>>,
}

impl GitHubAuthManager {
    /// Restore from persisted credentials, or start signed out.
    pub fn restore(
        credentials: Option<GitHubCredentials>,
        async_runtime: AsyncRuntime,
        http_client: reqwest::Client,
    ) -> Self {
        let state = match credentials {
            Some(creds) => AuthState::SignedIn(creds),
            None => AuthState::SignedOut,
        };
        let pending_avatar = Arc::new(Mutex::new(None));

        // If already signed in, fetch the avatar image in the background
        if let AuthState::SignedIn(ref creds) = state {
            let avatar_url = creds.user.avatar_url.clone();
            if !avatar_url.is_empty() {
                let client = http_client.clone();
                let pending = Arc::clone(&pending_avatar);
                async_runtime.spawn(async move {
                    let result = fetch_avatar(&client, &avatar_url).await;
                    *pending.lock() = Some(result);
                });
            }
        }

        Self {
            state,
            http_client,
            async_runtime,
            pending_auth_result: Arc::new(Mutex::new(None)),
            pending_avatar,
            avatar_bytes: None,
        }
    }

    /// Current auth state for UI rendering.
    pub fn state(&self) -> &AuthState {
        &self.state
    }

    /// Current credentials if signed in.
    pub fn credentials(&self) -> Option<&GitHubCredentials> {
        if let AuthState::SignedIn(ref creds) = self.state {
            Some(creds)
        } else {
            None
        }
    }

    /// Start the sign-in flow appropriate for the current platform.
    ///
    /// - **Native**: Opens the browser for Authorization Code flow.
    /// - **WASM**: Redirects the page to GitHub for Authorization Code flow.
    pub fn start_sign_in(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.start_auth_code_flow(ctx);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            self.start_auth_redirect();
        }
    }

    /// Sign out and clear all auth state.
    pub fn sign_out(&mut self) {
        self.state = AuthState::SignedOut;
        self.avatar_bytes = None;
    }

    /// Cached avatar image bytes, if available.
    pub fn avatar_bytes(&self) -> Option<&[u8]> {
        self.avatar_bytes.as_deref()
    }

    /// Poll for auth state changes. Call each frame from `draw_settings`.
    pub fn poll(&mut self, _ctx: &egui::Context) {
        if let Some(result) = self.pending_auth_result.lock().take() {
            match result {
                Ok((token, user_resp)) => {
                    let avatar_url = user_resp.avatar_url.clone();
                    self.state = AuthState::SignedIn(GitHubCredentials {
                        access_token: token,
                        user: GitHubUser {
                            login: user_resp.login,
                            avatar_url: user_resp.avatar_url,
                            name: user_resp.name,
                            id: user_resp.id,
                        },
                    });

                    // Fetch avatar image in the background
                    if !avatar_url.is_empty() {
                        let client = self.http_client.clone();
                        let pending = Arc::clone(&self.pending_avatar);
                        self.async_runtime.spawn(async move {
                            let result = fetch_avatar(&client, &avatar_url).await;
                            *pending.lock() = Some(result);
                        });
                    }
                }
                Err(e) => {
                    self.state = AuthState::Error(e);
                }
            }
        }

        // Check for completed avatar download
        if let Some(result) = self.pending_avatar.lock().take() {
            match result {
                Ok(bytes) => self.avatar_bytes = Some(bytes),
                Err(e) => log::warn!("Failed to fetch avatar: {e}"),
            }
        }
    }

    // ── Native: Authorization Code Flow ─────────────────────────────────

    #[cfg(not(target_arch = "wasm32"))]
    fn start_auth_code_flow(&mut self, ctx: &egui::Context) {
        self.state = AuthState::Authenticating;
        let pending = Arc::clone(&self.pending_auth_result);
        let client = self.http_client.clone();
        let ctx = ctx.clone();

        self.async_runtime.spawn(async move {
            let result = run_auth_code_flow(&client).await;
            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    // ── WASM: Authorization Code Flow via page redirect ─────────────────

    /// Redirect the page to GitHub's authorization URL.
    ///
    /// If the user is on `localhost`, we first redirect to the `127.0.0.1`
    /// equivalent so that sessionStorage writes and the GitHub callback
    /// share the same origin (sessionStorage is per-origin).
    #[cfg(target_arch = "wasm32")]
    fn start_auth_redirect(&mut self) {
        let Some(window) = web_sys::window() else {
            self.state = AuthState::Error("No window object".to_string());
            return;
        };

        // If on localhost, redirect to 127.0.0.1 first so sessionStorage
        // is readable when GitHub redirects back to 127.0.0.1.
        let hostname = window.location().hostname().unwrap_or_default();
        if hostname == "localhost" {
            let href = window.location().href().unwrap_or_default();
            let new_href = href.replace("localhost", "127.0.0.1");
            // Add a query param so the app knows to start auth on reload
            let separator = if new_href.contains('?') { "&" } else { "?" };
            let _ = window
                .location()
                .set_href(&format!("{new_href}{separator}start_auth=1"));
            return;
        }

        self.do_auth_redirect(&window);
    }

    /// Actually perform the GitHub redirect (after ensuring correct origin).
    #[cfg(target_arch = "wasm32")]
    fn do_auth_redirect(&self, window: &web_sys::Window) {
        // Generate a random state for CSRF protection
        let state = generate_state_wasm();

        // Save state to sessionStorage so we can verify on return
        if let Ok(Some(storage)) = window.session_storage() {
            let _ = storage.set_item("github_oauth_state", &state);
        }

        let redirect_uri = wasm_redirect_uri(window);

        let auth_url = format!(
            "https://github.com/login/oauth/authorize?client_id={GITHUB_CLIENT_ID}&redirect_uri={redirect_uri}&scope=user&state={state}",
        );

        // Redirect the page
        let _ = window.location().set_href(&auth_url);
    }

    /// Check if we arrived back from a GitHub OAuth redirect, or if we
    /// need to continue a redirect started from a `localhost` origin.
    ///
    /// Call once during app initialization (WASM only).
    ///
    /// Returns `true` if an auth flow is in progress (caller should
    /// navigate to the settings page).
    #[cfg(target_arch = "wasm32")]
    pub fn check_auth_callback(&mut self, ctx: &egui::Context) -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };

        let search = match window.location().search() {
            Ok(s) => s,
            Err(_) => return false,
        };

        if !search.starts_with('?') {
            return false;
        }

        // Parse query params
        let mut code = None;
        let mut state = None;
        let mut start_auth = false;

        for param in search[1..].split('&') {
            if let Some(value) = param.strip_prefix("code=") {
                if !value.is_empty() {
                    code = Some(value.to_string());
                }
            } else if let Some(value) = param.strip_prefix("state=") {
                if !value.is_empty() {
                    state = Some(value.to_string());
                }
            } else if param == "start_auth=1" {
                start_auth = true;
            }
        }

        // Continuing auth after localhost→127.0.0.1 redirect
        if start_auth {
            clean_url_params(&window);
            self.do_auth_redirect(&window);
            return true;
        }

        let (Some(code), Some(received_state)) = (code, state) else {
            return false;
        };

        // Validate state against sessionStorage
        let expected_state = window.session_storage().ok().flatten().and_then(|s| {
            let val = s.get_item("github_oauth_state").ok().flatten();
            let _ = s.remove_item("github_oauth_state");
            val
        });

        if expected_state.as_deref() != Some(&received_state) {
            log::warn!("OAuth state mismatch — ignoring callback");
            return false;
        }

        // Clean the URL: remove code and state params, keep others
        clean_url_params(&window);

        let redirect_uri = wasm_redirect_uri(&window);

        log::info!("GitHub OAuth callback detected, exchanging code...");

        // Start exchange
        self.state = AuthState::Authenticating;
        let pending = Arc::clone(&self.pending_auth_result);
        let client = self.http_client.clone();
        let ctx = ctx.clone();

        self.async_runtime.spawn(async move {
            let result = async {
                let token = exchange_code(&client, &code, &redirect_uri).await?;
                let user = fetch_github_user(&client, &token).await?;
                Ok((token, user))
            }
            .await;
            *pending.lock() = Some(result);
            ctx.request_repaint();
        });

        true
    }
}

// ── Native: Authorization Code Flow implementation ──────────────────────

#[cfg(not(target_arch = "wasm32"))]
async fn run_auth_code_flow(client: &reqwest::Client) -> AuthResult {
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // 1. Bind local callback server on a random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Failed to start callback server: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local address: {e}"))?
        .port();

    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let state = generate_state();

    // 2. Open browser to GitHub authorization page
    let auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={GITHUB_CLIENT_ID}&redirect_uri={redirect_uri}&scope=user&state={state}",
    );
    open::that(&auth_url).map_err(|e| format!("Failed to open browser: {e}"))?;

    // 3. Wait for the callback (5 minute timeout)
    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(300), listener.accept())
        .await
        .map_err(|_| "Authorization timed out. Try again.".to_string())?
        .map_err(|e| format!("Failed to accept callback: {e}"))?;

    // 4. Read the HTTP request
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Failed to read callback: {e}"))?;
    let request_text = String::from_utf8_lossy(&buf[..n]);

    // 5. Parse code and state from the request
    let (code, received_state) = parse_callback_params(&request_text)?;

    if received_state != state {
        let response = b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
            <html><body><h2>Authentication Failed</h2>\
            <p>Invalid state parameter. Please try again.</p></body></html>";
        let _ = stream.write_all(response).await;
        return Err("State mismatch — possible CSRF attack. Try again.".to_string());
    }

    // 6. Send success page to the browser
    let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body style=\"font-family: system-ui, sans-serif; display: flex; \
        justify-content: center; align-items: center; height: 80vh; color: #333;\">\
        <div style=\"text-align: center;\"><h2>Signed in to Enya</h2>\
        <p style=\"color: #666;\">You can close this tab.</p></div></body></html>";
    let _ = stream.write_all(response).await;

    // 7. Exchange authorization code for access token via Worker
    let token = exchange_code(client, &code, &redirect_uri).await?;

    // 8. Fetch user info
    let user = fetch_github_user(client, &token).await?;

    Ok((token, user))
}

/// Generate a random state parameter for CSRF protection.
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::disallowed_types)] // Native-only: SystemTime is safe here.
fn generate_state() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    std::process::id().hash(&mut hasher);
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Generate a random state parameter for CSRF protection (WASM).
#[cfg(target_arch = "wasm32")]
fn generate_state_wasm() -> String {
    let r1 = (js_sys::Math::random() * 1e16) as u64;
    let r2 = (js_sys::Math::random() * 1e16) as u64;
    format!("{r1:016x}{r2:016x}")
}

/// Parse `code` and `state` from the OAuth callback HTTP request.
#[cfg(not(target_arch = "wasm32"))]
fn parse_callback_params(request: &str) -> Result<(String, String), String> {
    // Extract the request path (e.g. "GET /callback?code=abc&state=xyz HTTP/1.1")
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or("Invalid callback request")?;

    let query = path
        .split_once('?')
        .map(|(_, q)| q)
        .ok_or("No query parameters in callback")?;

    let mut code = None;
    let mut state = None;

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            match key {
                "code" => code = Some(value.to_string()),
                "state" => state = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Ok((
        code.ok_or("Missing 'code' parameter in callback")?,
        state.ok_or("Missing 'state' parameter in callback")?,
    ))
}

/// Build the redirect URI for the WASM Authorization Code flow.
///
/// Uses the current page's origin + pathname so it works both on
/// production (`enya.build/editor`) and local dev (`localhost:8080`).
#[cfg(target_arch = "wasm32")]
fn wasm_redirect_uri(window: &web_sys::Window) -> String {
    let location = window.location();
    let hostname = location.hostname().unwrap_or_default();
    let origin = location.origin().unwrap_or_default();
    let pathname = location.pathname().unwrap_or_default();

    // GitHub OAuth requires redirect_uri host to match the registered
    // callback URL. The app is registered with 127.0.0.1, so swap
    // localhost → 127.0.0.1 for local dev.
    let origin = if hostname == "localhost" {
        origin.replace("localhost", "127.0.0.1")
    } else {
        origin
    };

    if pathname == "/" {
        origin
    } else {
        format!("{origin}{pathname}")
    }
}

/// Remove OAuth-related query parameters from the current URL (WASM).
#[cfg(target_arch = "wasm32")]
fn clean_url_params(window: &web_sys::Window) {
    let location = window.location();
    let pathname = location.pathname().unwrap_or_default();
    let search = location.search().unwrap_or_default();
    let hash = location.hash().unwrap_or_default();

    // Rebuild query string without OAuth params
    let clean_params: Vec<&str> = if search.starts_with('?') {
        search[1..]
            .split('&')
            .filter(|p| {
                !p.starts_with("code=") && !p.starts_with("state=") && !p.starts_with("start_auth=")
            })
            .collect()
    } else {
        Vec::new()
    };

    let clean_url = if clean_params.is_empty() {
        format!("{pathname}{hash}")
    } else {
        format!("{pathname}?{}{hash}", clean_params.join("&"))
    };

    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&clean_url));
    }
}

// ── Shared helpers ──────────────────────────────────────────────────────

/// Exchange an authorization code for an access token via the API worker.
async fn exchange_code(
    client: &reqwest::Client,
    code: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    let resp = client
        .post(EXCHANGE_URL)
        .json(&serde_json::json!({
            "code": code,
            "redirect_uri": redirect_uri,
        }))
        .send()
        .await
        .map_err(|e| format!("Token exchange failed: {e}"))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse failed: {e}"))?;

    if let Some(token) = body["access_token"].as_str() {
        Ok(token.to_string())
    } else if let Some(error) = body["error_description"].as_str() {
        Err(format!("GitHub error: {error}"))
    } else if let Some(error) = body["error"].as_str() {
        Err(format!("GitHub error: {error}"))
    } else {
        Err("Unexpected response from token exchange".to_string())
    }
}

/// Fetch the authenticated user's profile from GitHub.
async fn fetch_github_user(
    client: &reqwest::Client,
    token: &str,
) -> Result<GitHubUserResponse, String> {
    let resp = client
        .get(user_api_url())
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "enya-editor")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error: {body}"));
    }

    resp.json::<GitHubUserResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Download a user's GitHub avatar image (small size).
pub(crate) async fn fetch_avatar(
    client: &reqwest::Client,
    avatar_url: &str,
) -> Result<Vec<u8>, String> {
    // Request a small avatar (80px) to keep memory usage low
    let url = if avatar_url.contains('?') {
        format!("{avatar_url}&s=80")
    } else {
        format!("{avatar_url}?s=80")
    };

    let resp = client
        .get(&url)
        .header("User-Agent", "enya-editor")
        .send()
        .await
        .map_err(|e| format!("Avatar fetch failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Avatar HTTP {}", resp.status()));
    }

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Avatar read failed: {e}"))
}

// ── Git credential helper ───────────────────────────────────────────────

/// Read a GitHub token from the system's git credential helper.
///
/// Runs `git credential fill` which queries configured credential helpers
/// (gh CLI, macOS Keychain, Git Credential Manager, etc.). This is useful
/// for accessing org repos where the OAuth App token may lack permissions.
#[cfg(not(target_arch = "wasm32"))]
pub async fn git_credential_fill() -> Result<String, String> {
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new("git")
        .args(["credential", "fill"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run git credential fill: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(b"protocol=https\nhost=github.com\n\n")
            .await
            .map_err(|e| format!("Failed to write to git credential fill: {e}"))?;
    }

    // Timeout to avoid hanging if the credential helper prompts interactively
    let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .map_err(|_| {
            "git credential fill timed out. Run `gh auth login` to set up credentials.".to_string()
        })?
        .map_err(|e| format!("git credential fill failed: {e}"))?;

    if !output.status.success() {
        return Err(
            "No GitHub credentials found. Run `gh auth login` to set up credentials.".to_string(),
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_credential_output(&stdout)
}

/// Parse the `password=...` field from `git credential fill` output.
#[cfg(not(target_arch = "wasm32"))]
fn parse_credential_output(output: &str) -> Result<String, String> {
    for line in output.lines() {
        if let Some(password) = line.strip_prefix("password=") {
            if !password.is_empty() {
                return Ok(password.to_string());
            }
        }
    }
    Err("No GitHub credentials found. Run `gh auth login` to set up credentials.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_auth_state_is_signed_out() {
        let state = AuthState::default();
        assert!(matches!(state, AuthState::SignedOut));
    }

    #[test]
    fn test_github_user_serde_roundtrip() {
        let user = GitHubUser {
            login: "testuser".to_string(),
            avatar_url: "https://example.com/avatar.png".to_string(),
            name: Some("Test User".to_string()),
            id: 12345,
        };
        let json = serde_json::to_string(&user).unwrap();
        let parsed: GitHubUser = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.login, "testuser");
        assert_eq!(parsed.id, 12345);
    }

    #[test]
    fn test_credentials_serde_roundtrip() {
        let creds = GitHubCredentials {
            access_token: "gho_test123".to_string(),
            user: GitHubUser {
                login: "testuser".to_string(),
                avatar_url: "https://example.com/avatar.png".to_string(),
                name: None,
                id: 42,
            },
        };
        let json = serde_json::to_string(&creds).unwrap();
        let parsed: GitHubCredentials = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "gho_test123");
        assert_eq!(parsed.user.login, "testuser");
    }

    #[test]
    fn test_parse_callback_params() {
        let request = "GET /callback?code=abc123&state=xyz789 HTTP/1.1\r\nHost: 127.0.0.1\r\n";
        let (code, state) = parse_callback_params(request).unwrap();
        assert_eq!(code, "abc123");
        assert_eq!(state, "xyz789");
    }

    #[test]
    fn test_parse_callback_params_missing_code() {
        let request = "GET /callback?state=xyz789 HTTP/1.1\r\n";
        assert!(parse_callback_params(request).is_err());
    }

    #[test]
    fn test_parse_credential_output() {
        let output =
            "protocol=https\nhost=github.com\nusername=x-access-token\npassword=gho_abc123\n";
        let token = parse_credential_output(output).unwrap();
        assert_eq!(token, "gho_abc123");
    }

    #[test]
    fn test_parse_credential_output_no_password() {
        let output = "protocol=https\nhost=github.com\n";
        assert!(parse_credential_output(output).is_err());
    }

    #[test]
    fn test_generate_state_is_nonempty() {
        let state = generate_state();
        assert!(!state.is_empty());
        assert_eq!(state.len(), 16); // 16 hex chars from u64
    }
}
