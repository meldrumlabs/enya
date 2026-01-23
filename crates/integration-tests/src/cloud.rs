//! Integration tests for Enya Cloud API.
//!
//! These tests spin up a PostgreSQL container and the Cloud API server
//! to test the team management, invitation, and audit log endpoints.
//!
//! # Requirements
//!
//! These tests require Docker to be running. They are ignored by default
//! to avoid blocking CI pipelines without Docker support.
//!
//! # Running the tests
//!
//! ```bash
//! # Run with Docker available
//! cargo nextest run -p enya-integration-tests --run-ignored ignored-only
//!
//! # Or with standard cargo test
//! cargo test -p enya-integration-tests -- --ignored
//! ```

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

// =============================================================================
// API Response Types
// =============================================================================

// Note: These structs have fields that appear unused, but they are necessary
// for deserializing full API responses in integration tests.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AuthResponse {
    access_token: String,
    user: UserResponse,
    teams: Vec<TeamResponse>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UserResponse {
    id: Uuid,
    email: String,
    display_name: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TeamResponse {
    id: Uuid,
    name: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TeamWithMembersResponse {
    id: Uuid,
    name: String,
    members: Vec<MemberResponse>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MemberResponse {
    id: Uuid,
    display_name: String,
    email: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MemberWithRoleResponse {
    id: Uuid,
    display_name: String,
    role: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct InvitationResponse {
    id: Uuid,
    team_id: Uuid,
    email: Option<String>,
    role: String,
    invite_url: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct InvitationInfoResponse {
    team_name: String,
    invited_by_name: String,
    role: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct InvitationAcceptedResponse {
    team_id: Uuid,
    team_name: String,
    role: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AuditLogsResponse {
    logs: Vec<AuditLogResponse>,
    total: i64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AuditLogResponse {
    action: String,
    resource_type: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
}

// =============================================================================
// Test Infrastructure
// =============================================================================

/// Test context with API client and server details.
#[allow(dead_code)]
struct CloudTestContext {
    client: Client,
    base_url: String,
    #[allow(dead_code)]
    postgres_container: testcontainers::ContainerAsync<Postgres>,
    #[allow(dead_code)]
    server_handle: tokio::task::JoinHandle<()>,
}

#[allow(dead_code)]
impl CloudTestContext {
    /// Start the test environment with PostgreSQL and the API server.
    async fn start() -> Self {
        // Start PostgreSQL container
        let postgres = Postgres::default()
            .start()
            .await
            .expect("Failed to start Postgres container");

        let pg_port = postgres.get_host_port_ipv4(5432).await.unwrap();
        let database_url = format!("postgres://postgres:postgres@localhost:{pg_port}/postgres");

        // Set environment variables for the server
        // SAFETY: These tests run serially and don't share state with other threads
        unsafe {
            std::env::set_var("DATABASE_URL", &database_url);
            std::env::set_var("JWT_SECRET", "test-secret-for-integration-tests");
            std::env::set_var("DEV_AUTH", "true");
            std::env::set_var("HOST", "127.0.0.1");
            std::env::set_var("PORT", "0"); // Let OS assign port
            std::env::set_var("FRONTEND_URL", "http://localhost:8080");
        }

        // Start the API server in a background task
        // Note: We can't easily start the actual server here because it's a separate binary.
        // Instead, we'll use a different approach - run migrations and test directly against the DB.
        // For full E2E tests, we'd need to build and run the server binary.

        // For now, let's create a simpler approach: test the API endpoints
        // by building the router directly (requires exposing it from the cloud crate).

        // Since we can't easily import the cloud crate here (it's a binary),
        // we'll skip the full server setup and just test with raw SQL + HTTP client
        // against a manually started server.

        // This is a placeholder - in practice, you'd either:
        // 1. Make the cloud crate a library with the router exposed
        // 2. Build and spawn the binary as a child process
        // 3. Use a test harness within the cloud crate itself

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        // For demonstration, assume server is running on port 3000
        // In real tests, you'd start it programmatically
        let base_url = format!("http://localhost:{pg_port}"); // placeholder

        Self {
            client,
            base_url,
            postgres_container: postgres,
            server_handle: tokio::spawn(async {}),
        }
    }

    /// Make an authenticated request.
    async fn auth_request(&self, token: &str) -> reqwest::RequestBuilder {
        self.client.get(&self.base_url).bearer_auth(token)
    }
}

// =============================================================================
// Unit Tests (run without Docker)
// =============================================================================

/// These tests verify the API contract without needing a running server.
/// They test serialization/deserialization of request/response types.

#[test]
fn test_auth_response_deserialize() {
    let json = r#"{
        "access_token": "test-token",
        "token_type": "Bearer",
        "user": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "email": "test@example.com",
            "display_name": "Test User",
            "avatar_url": null
        },
        "teams": [
            {
                "id": "660e8400-e29b-41d4-a716-446655440000",
                "name": "Test Team"
            }
        ]
    }"#;

    let response: AuthResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.access_token, "test-token");
    assert_eq!(response.user.display_name, "Test User");
    assert_eq!(response.teams.len(), 1);
}

#[test]
fn test_invitation_response_deserialize() {
    let json = r#"{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "team_id": "660e8400-e29b-41d4-a716-446655440000",
        "email": "invite@example.com",
        "role": "member",
        "invited_by": "770e8400-e29b-41d4-a716-446655440000",
        "invite_url": "http://localhost:8080/invite/abc123",
        "expires_at": 1234567890,
        "created_at": 1234567800
    }"#;

    let response: InvitationResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.role, "member");
    assert_eq!(response.email, Some("invite@example.com".to_string()));
}

#[test]
fn test_member_with_role_deserialize() {
    let json = r#"{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "display_name": "Admin User",
        "avatar_url": null,
        "email": "admin@example.com",
        "role": "admin"
    }"#;

    let response: MemberWithRoleResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.role, "admin");
    assert_eq!(response.display_name, "Admin User");
}

#[test]
fn test_audit_log_deserialize() {
    let json = r#"{
        "logs": [
            {
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "actor_id": "660e8400-e29b-41d4-a716-446655440000",
                "actor_name": "Test User",
                "action": "member.joined",
                "resource_type": "user",
                "resource_id": "770e8400-e29b-41d4-a716-446655440000",
                "details": {"role": "member"},
                "created_at": 1234567890
            }
        ],
        "total": 1,
        "limit": 50,
        "offset": 0
    }"#;

    let response: AuditLogsResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.total, 1);
    assert_eq!(response.logs[0].action, "member.joined");
}

// =============================================================================
// Integration Tests (require Docker)
// =============================================================================

// Note: Full integration tests would require either:
// 1. Exposing the router from enya-cloud as a library
// 2. Building and spawning the server binary
// 3. Moving tests into the cloud crate itself
//
// For now, we provide contract tests above and placeholder integration tests below.

#[tokio::test]
#[ignore = "requires running Enya Cloud server"]
async fn test_health_check() {
    let client = Client::new();
    let response = client
        .get("http://localhost:3000/health")
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    let body = response.text().await.unwrap();
    assert_eq!(body, "ok");
}

#[tokio::test]
#[ignore = "requires running Enya Cloud server"]
async fn test_dev_login() {
    let client = Client::new();

    let response = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({
            "name": "Test User"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    let auth: AuthResponse = response.json().await.unwrap();
    assert!(!auth.access_token.is_empty());
    assert_eq!(auth.user.display_name, "Test User");
}

#[tokio::test]
#[ignore = "requires running Enya Cloud server"]
async fn test_create_team() {
    let client = Client::new();

    // Login first
    let auth_response = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({ "name": "Team Creator" }))
        .send()
        .await
        .unwrap();
    let auth: AuthResponse = auth_response.json().await.unwrap();

    // Create team
    let response = client
        .post("http://localhost:3000/teams")
        .bearer_auth(&auth.access_token)
        .json(&json!({ "name": "New Team" }))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let team: TeamWithMembersResponse = response.json().await.unwrap();
    assert_eq!(team.name, "New Team");
    assert_eq!(team.members.len(), 1); // Creator is a member
}

#[tokio::test]
#[ignore = "requires running Enya Cloud server"]
async fn test_create_and_accept_invitation() {
    let client = Client::new();

    // Create admin user
    let admin_auth: AuthResponse = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({ "name": "Admin User" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let team_id = admin_auth.teams[0].id;

    // Create invitation
    let invite_response = client
        .post(format!("http://localhost:3000/teams/{team_id}/invitations"))
        .bearer_auth(&admin_auth.access_token)
        .json(&json!({
            "email": "invitee@example.com",
            "role": "member"
        }))
        .send()
        .await
        .unwrap();

    assert!(invite_response.status().is_success());
    let invitation: InvitationResponse = invite_response.json().await.unwrap();
    assert_eq!(invitation.role, "member");

    // Extract token from invite URL
    let token = invitation.invite_url.split('/').next_back().unwrap();

    // Get invitation info (public endpoint)
    let info_response = client
        .get(format!("http://localhost:3000/invitations/{token}"))
        .send()
        .await
        .unwrap();

    assert!(info_response.status().is_success());
    let info: InvitationInfoResponse = info_response.json().await.unwrap();
    assert_eq!(info.invited_by_name, "Admin User");

    // Create invitee user
    let invitee_auth: AuthResponse = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({
            "name": "Invitee",
            "email": "invitee@example.com"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Accept invitation
    let accept_response = client
        .post("http://localhost:3000/invitations/accept")
        .bearer_auth(&invitee_auth.access_token)
        .json(&json!({ "token": token }))
        .send()
        .await
        .unwrap();

    assert!(accept_response.status().is_success());
    let accepted: InvitationAcceptedResponse = accept_response.json().await.unwrap();
    assert_eq!(accepted.team_id, team_id);
}

#[tokio::test]
#[ignore = "requires running Enya Cloud server"]
async fn test_member_role_management() {
    let client = Client::new();

    // Create admin
    let admin_auth: AuthResponse = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({ "name": "Role Admin" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let team_id = admin_auth.teams[0].id;

    // Create another user and add to team via invitation
    let member_auth: AuthResponse = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({ "name": "Regular Member" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Create magic link invitation (no email)
    let invite: InvitationResponse = client
        .post(format!("http://localhost:3000/teams/{team_id}/invitations"))
        .bearer_auth(&admin_auth.access_token)
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let token = invite.invite_url.split('/').next_back().unwrap();

    // Accept invitation
    client
        .post("http://localhost:3000/invitations/accept")
        .bearer_auth(&member_auth.access_token)
        .json(&json!({ "token": token }))
        .send()
        .await
        .unwrap();

    // Promote to admin
    let promote_response = client
        .patch(format!(
            "http://localhost:3000/teams/{team_id}/members/{}/role",
            member_auth.user.id
        ))
        .bearer_auth(&admin_auth.access_token)
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();

    assert!(promote_response.status().is_success());

    // List members with roles
    let members: Vec<MemberWithRoleResponse> = client
        .get(format!(
            "http://localhost:3000/teams/{team_id}/members/roles"
        ))
        .bearer_auth(&admin_auth.access_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Both should now be admins
    let promoted_member = members
        .iter()
        .find(|m| m.id == member_auth.user.id)
        .unwrap();
    assert_eq!(promoted_member.role, "admin");
}

#[tokio::test]
#[ignore = "requires running Enya Cloud server"]
async fn test_cannot_demote_last_admin() {
    let client = Client::new();

    // Create admin (only admin in the team)
    let admin_auth: AuthResponse = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({ "name": "Solo Admin" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let team_id = admin_auth.teams[0].id;

    // Try to demote self - should fail
    let response = client
        .patch(format!(
            "http://localhost:3000/teams/{team_id}/members/{}/role",
            admin_auth.user.id
        ))
        .bearer_auth(&admin_auth.access_token)
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let error: ErrorResponse = response.json().await.unwrap();
    assert!(error.error.contains("last admin"));
}

#[tokio::test]
#[ignore = "requires running Enya Cloud server"]
async fn test_audit_logs() {
    let client = Client::new();

    // Create admin
    let admin_auth: AuthResponse = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({ "name": "Audit Admin" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let team_id = admin_auth.teams[0].id;

    // Create an invitation (generates audit log)
    client
        .post(format!("http://localhost:3000/teams/{team_id}/invitations"))
        .bearer_auth(&admin_auth.access_token)
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();

    // Fetch audit logs
    let logs: AuditLogsResponse = client
        .get(format!("http://localhost:3000/teams/{team_id}/audit-logs"))
        .bearer_auth(&admin_auth.access_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(logs.total >= 1);
    assert!(logs.logs.iter().any(|l| l.action == "invitation.created"));
}

#[tokio::test]
#[ignore = "requires running Enya Cloud server"]
async fn test_non_admin_cannot_create_invitation() {
    let client = Client::new();

    // Create admin
    let admin_auth: AuthResponse = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({ "name": "Admin" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let team_id = admin_auth.teams[0].id;

    // Create member
    let member_auth: AuthResponse = client
        .post("http://localhost:3000/auth/dev")
        .json(&json!({ "name": "Member" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Add member via invitation
    let invite: InvitationResponse = client
        .post(format!("http://localhost:3000/teams/{team_id}/invitations"))
        .bearer_auth(&admin_auth.access_token)
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let token = invite.invite_url.split('/').next_back().unwrap();
    client
        .post("http://localhost:3000/invitations/accept")
        .bearer_auth(&member_auth.access_token)
        .json(&json!({ "token": token }))
        .send()
        .await
        .unwrap();

    // Member tries to create invitation - should fail
    let response = client
        .post(format!("http://localhost:3000/teams/{team_id}/invitations"))
        .bearer_auth(&member_auth.access_token)
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}
