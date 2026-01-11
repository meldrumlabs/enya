//! Database queries.
//!
//! Uses runtime-checked queries to avoid requiring DATABASE_URL at compile time.

use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::models::*;
use crate::error::ApiError;

// =============================================================================
// User queries
// =============================================================================

/// Get user by ID.
pub async fn get_user(pool: &PgPool, user_id: Uuid) -> Result<DbUser, ApiError> {
    sqlx::query_as::<_, DbUser>(
        r#"SELECT id, email, display_name, avatar_url, github_id, slack_id, created_at, updated_at
           FROM users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

/// Get user by email (for future user lookup features).
#[allow(dead_code)]
pub async fn get_user_by_email(pool: &PgPool, email: &str) -> Result<Option<DbUser>, ApiError> {
    sqlx::query_as::<_, DbUser>(
        r#"SELECT id, email, display_name, avatar_url, github_id, slack_id, created_at, updated_at
           FROM users WHERE email = $1"#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

/// Get user by GitHub ID (for OAuth deduplication).
#[allow(dead_code)]
pub async fn get_user_by_github_id(
    pool: &PgPool,
    github_id: &str,
) -> Result<Option<DbUser>, ApiError> {
    sqlx::query_as::<_, DbUser>(
        r#"SELECT id, email, display_name, avatar_url, github_id, slack_id, created_at, updated_at
           FROM users WHERE github_id = $1"#,
    )
    .bind(github_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

/// Create or update user from GitHub OAuth.
pub async fn upsert_github_user(
    pool: &PgPool,
    github_id: &str,
    email: &str,
    display_name: &str,
    avatar_url: Option<&str>,
) -> Result<DbUser, ApiError> {
    sqlx::query_as::<_, DbUser>(
        r#"INSERT INTO users (id, email, display_name, avatar_url, github_id)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (github_id) DO UPDATE SET
             email = EXCLUDED.email,
             display_name = EXCLUDED.display_name,
             avatar_url = EXCLUDED.avatar_url,
             updated_at = NOW()
           RETURNING id, email, display_name, avatar_url, github_id, slack_id, created_at, updated_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(display_name)
    .bind(avatar_url)
    .bind(github_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

// =============================================================================
// Team queries
// =============================================================================

/// Get team by ID.
pub async fn get_team(pool: &PgPool, team_id: Uuid) -> Result<DbTeam, ApiError> {
    sqlx::query_as::<_, DbTeam>(
        r#"SELECT id, name, created_at, updated_at FROM teams WHERE id = $1"#,
    )
    .bind(team_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

/// Get teams for a user.
pub async fn get_user_teams(pool: &PgPool, user_id: Uuid) -> Result<Vec<DbTeam>, ApiError> {
    sqlx::query_as::<_, DbTeam>(
        r#"SELECT t.id, t.name, t.created_at, t.updated_at
           FROM teams t
           JOIN team_members tm ON t.id = tm.team_id
           WHERE tm.user_id = $1
           ORDER BY t.name"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Get team members.
pub async fn get_team_members(pool: &PgPool, team_id: Uuid) -> Result<Vec<DbUser>, ApiError> {
    sqlx::query_as::<_, DbUser>(
        r#"SELECT u.id, u.email, u.display_name, u.avatar_url, u.github_id, u.slack_id, u.created_at, u.updated_at
           FROM users u
           JOIN team_members tm ON u.id = tm.user_id
           WHERE tm.team_id = $1
           ORDER BY u.display_name"#,
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Check if user is member of team.
pub async fn is_team_member(pool: &PgPool, team_id: Uuid, user_id: Uuid) -> Result<bool, ApiError> {
    let row = sqlx::query(
        r#"SELECT EXISTS(SELECT 1 FROM team_members WHERE team_id = $1 AND user_id = $2) as exists"#,
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(row.get::<bool, _>("exists"))
}

/// Create a team and add the creator as admin.
pub async fn create_team(pool: &PgPool, name: &str, creator_id: Uuid) -> Result<DbTeam, ApiError> {
    let team_id = Uuid::new_v4();

    // Create team
    let team = sqlx::query_as::<_, DbTeam>(
        r#"INSERT INTO teams (id, name) VALUES ($1, $2)
           RETURNING id, name, created_at, updated_at"#,
    )
    .bind(team_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    // Add creator as admin
    sqlx::query(r#"INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'admin')"#)
        .bind(team_id)
        .bind(creator_id)
        .execute(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(team)
}

// =============================================================================
// Annotation queries
// =============================================================================

/// List annotations for a query fingerprint.
pub async fn list_annotations(
    pool: &PgPool,
    team_id: Uuid,
    query_fingerprint: &str,
) -> Result<Vec<DbAnnotation>, ApiError> {
    sqlx::query_as::<_, DbAnnotation>(
        r#"SELECT id, team_id, pane_id, query_fingerprint, timestamp_ns, created_by, thread_id, created_at
           FROM annotations
           WHERE team_id = $1 AND query_fingerprint = $2
           ORDER BY timestamp_ns"#,
    )
    .bind(team_id)
    .bind(query_fingerprint)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Get annotation by ID.
pub async fn get_annotation(pool: &PgPool, annotation_id: Uuid) -> Result<DbAnnotation, ApiError> {
    sqlx::query_as::<_, DbAnnotation>(
        r#"SELECT id, team_id, pane_id, query_fingerprint, timestamp_ns, created_by, thread_id, created_at
           FROM annotations WHERE id = $1"#,
    )
    .bind(annotation_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

/// Create an annotation with its thread.
pub async fn create_annotation(
    pool: &PgPool,
    team_id: Uuid,
    pane_id: &str,
    query_fingerprint: &str,
    timestamp_ns: i64,
    created_by: Uuid,
    initial_message: &str,
) -> Result<(DbAnnotation, DbThread, DbMessage), ApiError> {
    let annotation_id = Uuid::new_v4();
    let thread_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();

    // Create thread
    let thread = sqlx::query_as::<_, DbThread>(
        r#"INSERT INTO threads (id, annotation_id, resolved)
           VALUES ($1, $2, false)
           RETURNING id, annotation_id, resolved, created_at"#,
    )
    .bind(thread_id)
    .bind(annotation_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    // Create annotation
    let annotation = sqlx::query_as::<_, DbAnnotation>(
        r#"INSERT INTO annotations (id, team_id, pane_id, query_fingerprint, timestamp_ns, created_by, thread_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, team_id, pane_id, query_fingerprint, timestamp_ns, created_by, thread_id, created_at"#,
    )
    .bind(annotation_id)
    .bind(team_id)
    .bind(pane_id)
    .bind(query_fingerprint)
    .bind(timestamp_ns)
    .bind(created_by)
    .bind(thread_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    // Extract mentions from initial message
    let mentions = extract_mentions(initial_message);

    // Create initial message
    let message = sqlx::query_as::<_, DbMessage>(
        r#"INSERT INTO messages (id, thread_id, author_id, content, mentions)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, thread_id, author_id, content, mentions, created_at, edited_at"#,
    )
    .bind(message_id)
    .bind(thread_id)
    .bind(created_by)
    .bind(initial_message)
    .bind(&mentions)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok((annotation, thread, message))
}

/// Delete an annotation.
pub async fn delete_annotation(pool: &PgPool, annotation_id: Uuid) -> Result<(), ApiError> {
    // Thread and messages are cascade deleted
    sqlx::query(r#"DELETE FROM annotations WHERE id = $1"#)
        .bind(annotation_id)
        .execute(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(())
}

// =============================================================================
// Thread/Message queries
// =============================================================================

/// Get thread by ID.
pub async fn get_thread(pool: &PgPool, thread_id: Uuid) -> Result<DbThread, ApiError> {
    sqlx::query_as::<_, DbThread>(
        r#"SELECT id, annotation_id, resolved, created_at FROM threads WHERE id = $1"#,
    )
    .bind(thread_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

/// List messages in a thread.
pub async fn list_messages(pool: &PgPool, thread_id: Uuid) -> Result<Vec<DbMessage>, ApiError> {
    sqlx::query_as::<_, DbMessage>(
        r#"SELECT id, thread_id, author_id, content, mentions, created_at, edited_at
           FROM messages
           WHERE thread_id = $1
           ORDER BY created_at ASC"#,
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Create a message.
pub async fn create_message(
    pool: &PgPool,
    thread_id: Uuid,
    author_id: Uuid,
    content: &str,
) -> Result<DbMessage, ApiError> {
    let mentions = extract_mentions(content);

    sqlx::query_as::<_, DbMessage>(
        r#"INSERT INTO messages (id, thread_id, author_id, content, mentions)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, thread_id, author_id, content, mentions, created_at, edited_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(thread_id)
    .bind(author_id)
    .bind(content)
    .bind(&mentions)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

/// Update thread resolved status.
pub async fn update_thread_resolved(
    pool: &PgPool,
    thread_id: Uuid,
    resolved: bool,
) -> Result<DbThread, ApiError> {
    sqlx::query_as::<_, DbThread>(
        r#"UPDATE threads SET resolved = $2 WHERE id = $1
           RETURNING id, annotation_id, resolved, created_at"#,
    )
    .bind(thread_id)
    .bind(resolved)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

/// Get thread participants (users who have posted messages).
pub async fn get_thread_participants(
    pool: &PgPool,
    thread_id: Uuid,
) -> Result<Vec<Uuid>, ApiError> {
    let rows = sqlx::query(r#"SELECT DISTINCT author_id FROM messages WHERE thread_id = $1"#)
        .bind(thread_id)
        .fetch_all(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(rows
        .iter()
        .map(|row| row.get::<Uuid, _>("author_id"))
        .collect())
}

// =============================================================================
// Channel queries
// =============================================================================

/// List channels for a team.
pub async fn list_channels(
    pool: &PgPool,
    team_id: Uuid,
) -> Result<Vec<enya_team_api::Channel>, ApiError> {
    let channels = sqlx::query_as::<_, DbChannel>(
        r#"SELECT id, team_id, name, description, kind, created_at
           FROM channels
           WHERE team_id = $1
           ORDER BY
             CASE kind
               WHEN 'general' THEN 0
               WHEN 'incidents' THEN 1
               WHEN 'deployments' THEN 2
               WHEN 'alerts' THEN 3
               ELSE 4
             END,
             name"#,
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(channels.into_iter().map(Into::into).collect())
}

/// Get channel by ID.
pub async fn get_channel(
    pool: &PgPool,
    channel_id: Uuid,
) -> Result<enya_team_api::Channel, ApiError> {
    let channel = sqlx::query_as::<_, DbChannel>(
        r#"SELECT id, team_id, name, description, kind, created_at
           FROM channels WHERE id = $1"#,
    )
    .bind(channel_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(channel.into())
}

/// Create a channel.
pub async fn create_channel(
    pool: &PgPool,
    team_id: Uuid,
    request: &enya_team_api::NewChannel,
) -> Result<enya_team_api::Channel, ApiError> {
    let kind = match request.kind {
        enya_team_api::ChannelKind::General => "general",
        enya_team_api::ChannelKind::Incidents => "incidents",
        enya_team_api::ChannelKind::Deployments => "deployments",
        enya_team_api::ChannelKind::Alerts => "alerts",
        enya_team_api::ChannelKind::Custom => "custom",
    };

    let channel = sqlx::query_as::<_, DbChannel>(
        r#"INSERT INTO channels (id, team_id, name, description, kind)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, team_id, name, description, kind, created_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(team_id)
    .bind(&request.name)
    .bind(&request.description)
    .bind(kind)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(channel.into())
}

/// List threads in a channel.
pub async fn list_channel_threads(
    pool: &PgPool,
    channel_id: Uuid,
) -> Result<Vec<enya_team_api::ChatThread>, ApiError> {
    let threads = sqlx::query_as::<_, DbChannelThread>(
        r#"SELECT id, channel_id, title, created_by, created_at, resolved, message_count, last_message_at
           FROM channel_threads
           WHERE channel_id = $1
           ORDER BY last_message_at DESC NULLS LAST, created_at DESC"#,
    )
    .bind(channel_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(threads.into_iter().map(Into::into).collect())
}

/// Get channel thread by ID.
pub async fn get_channel_thread(
    pool: &PgPool,
    thread_id: Uuid,
) -> Result<enya_team_api::ChatThread, ApiError> {
    let thread = sqlx::query_as::<_, DbChannelThread>(
        r#"SELECT id, channel_id, title, created_by, created_at, resolved, message_count, last_message_at
           FROM channel_threads WHERE id = $1"#,
    )
    .bind(thread_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(thread.into())
}

/// Create a thread in a channel with initial message.
pub async fn create_channel_thread(
    pool: &PgPool,
    channel_id: Uuid,
    created_by: Uuid,
    request: &enya_team_api::NewThread,
) -> Result<(enya_team_api::ChatThread, DbMessage), ApiError> {
    let thread_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();

    // Create thread
    let thread = sqlx::query_as::<_, DbChannelThread>(
        r#"INSERT INTO channel_threads (id, channel_id, title, created_by, message_count, last_message_at)
           VALUES ($1, $2, $3, $4, 1, NOW())
           RETURNING id, channel_id, title, created_by, created_at, resolved, message_count, last_message_at"#,
    )
    .bind(thread_id)
    .bind(channel_id)
    .bind(&request.title)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    // Extract mentions from initial message
    let mentions = extract_mentions(&request.initial_message);

    // Create initial message
    let message = sqlx::query_as::<_, DbMessage>(
        r#"INSERT INTO messages (id, thread_id, author_id, content, mentions)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, thread_id, author_id, content, mentions, created_at, edited_at"#,
    )
    .bind(message_id)
    .bind(thread_id)
    .bind(created_by)
    .bind(&request.initial_message)
    .bind(&mentions)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok((thread.into(), message))
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract @mentions from message content.
/// Looks for patterns like @<uuid> in the text.
fn extract_mentions(content: &str) -> Vec<Uuid> {
    let mut mentions = Vec::new();

    for word in content.split_whitespace() {
        if let Some(rest) = word.strip_prefix('@') {
            // Try to parse as UUID
            if let Ok(uuid) =
                Uuid::parse_str(rest.trim_matches(|c: char| !c.is_alphanumeric() && c != '-'))
            {
                mentions.push(uuid);
            }
        }
    }

    mentions
}

// =============================================================================
// Team member role queries
// =============================================================================

/// Get a team member with role information.
pub async fn get_team_member(
    pool: &PgPool,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<Option<super::models::DbTeamMember>, ApiError> {
    sqlx::query_as::<_, super::models::DbTeamMember>(
        r#"SELECT team_id, user_id, role, created_at
           FROM team_members
           WHERE team_id = $1 AND user_id = $2"#,
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

/// Check if user is an admin of the team.
pub async fn is_team_admin(pool: &PgPool, team_id: Uuid, user_id: Uuid) -> Result<bool, ApiError> {
    let row = sqlx::query(
        r#"SELECT EXISTS(
            SELECT 1 FROM team_members
            WHERE team_id = $1 AND user_id = $2 AND role = 'admin'
        ) as is_admin"#,
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(row.get::<bool, _>("is_admin"))
}

/// Update a team member's role.
pub async fn update_member_role(
    pool: &PgPool,
    team_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<(), ApiError> {
    sqlx::query(r#"UPDATE team_members SET role = $3 WHERE team_id = $1 AND user_id = $2"#)
        .bind(team_id)
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(())
}

/// Remove a member from a team.
pub async fn remove_team_member(
    pool: &PgPool,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<bool, ApiError> {
    let result = sqlx::query(r#"DELETE FROM team_members WHERE team_id = $1 AND user_id = $2"#)
        .bind(team_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(result.rows_affected() > 0)
}

/// Add a member to a team.
pub async fn add_team_member(
    pool: &PgPool,
    team_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<(), ApiError> {
    sqlx::query(r#"INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, $3)"#)
        .bind(team_id)
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(())
}

/// Count team admins (to prevent removing the last admin).
pub async fn count_team_admins(pool: &PgPool, team_id: Uuid) -> Result<i64, ApiError> {
    let row = sqlx::query(
        r#"SELECT COUNT(*) as count FROM team_members WHERE team_id = $1 AND role = 'admin'"#,
    )
    .bind(team_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(row.get::<i64, _>("count"))
}

// =============================================================================
// Invitation queries
// =============================================================================

/// Create a team invitation.
pub async fn create_invitation(
    pool: &PgPool,
    team_id: Uuid,
    email: Option<&str>,
    token: &str,
    role: &str,
    invited_by: Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<super::models::DbTeamInvitation, ApiError> {
    sqlx::query_as::<_, super::models::DbTeamInvitation>(
        r#"INSERT INTO team_invitations (id, team_id, email, token, role, invited_by, expires_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, team_id, email, token, role, invited_by, expires_at, accepted_at, accepted_by, created_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(team_id)
    .bind(email)
    .bind(token)
    .bind(role)
    .bind(invited_by)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

/// Get invitation by token.
pub async fn get_invitation_by_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<super::models::DbTeamInvitation>, ApiError> {
    sqlx::query_as::<_, super::models::DbTeamInvitation>(
        r#"SELECT id, team_id, email, token, role, invited_by, expires_at, accepted_at, accepted_by, created_at
           FROM team_invitations
           WHERE token = $1"#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

/// Get pending invitations for a team.
pub async fn list_team_invitations(
    pool: &PgPool,
    team_id: Uuid,
) -> Result<Vec<super::models::DbTeamInvitation>, ApiError> {
    sqlx::query_as::<_, super::models::DbTeamInvitation>(
        r#"SELECT id, team_id, email, token, role, invited_by, expires_at, accepted_at, accepted_by, created_at
           FROM team_invitations
           WHERE team_id = $1 AND accepted_at IS NULL
           ORDER BY created_at DESC"#,
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Accept an invitation.
pub async fn accept_invitation(
    pool: &PgPool,
    invitation_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"UPDATE team_invitations
           SET accepted_at = NOW(), accepted_by = $2
           WHERE id = $1"#,
    )
    .bind(invitation_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(())
}

/// Delete an invitation.
pub async fn delete_invitation(pool: &PgPool, invitation_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query(r#"DELETE FROM team_invitations WHERE id = $1"#)
        .bind(invitation_id)
        .execute(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(result.rows_affected() > 0)
}

/// Check if an invitation already exists for an email in a team.
pub async fn invitation_exists_for_email(
    pool: &PgPool,
    team_id: Uuid,
    email: &str,
) -> Result<bool, ApiError> {
    let row = sqlx::query(
        r#"SELECT EXISTS(
            SELECT 1 FROM team_invitations
            WHERE team_id = $1 AND email = $2 AND accepted_at IS NULL AND expires_at > NOW()
        ) as exists"#,
    )
    .bind(team_id)
    .bind(email)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(row.get::<bool, _>("exists"))
}

// =============================================================================
// Audit log queries
// =============================================================================

/// Create an audit log entry.
#[allow(clippy::too_many_arguments)]
pub async fn create_audit_log(
    pool: &PgPool,
    team_id: Uuid,
    actor_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
    details: Option<serde_json::Value>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<super::models::DbAuditLog, ApiError> {
    sqlx::query_as::<_, super::models::DbAuditLog>(
        r#"INSERT INTO audit_logs (id, team_id, actor_id, action, resource_type, resource_id, details, ip_address, user_agent)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id, team_id, actor_id, action, resource_type, resource_id, details, ip_address, user_agent, created_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(team_id)
    .bind(actor_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(details)
    .bind(ip_address)
    .bind(user_agent)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

/// List audit logs for a team with pagination.
pub async fn list_audit_logs(
    pool: &PgPool,
    team_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<super::models::DbAuditLog>, ApiError> {
    sqlx::query_as::<_, super::models::DbAuditLog>(
        r#"SELECT id, team_id, actor_id, action, resource_type, resource_id, details, ip_address, user_agent, created_at
           FROM audit_logs
           WHERE team_id = $1
           ORDER BY created_at DESC
           LIMIT $2 OFFSET $3"#,
    )
    .bind(team_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Count audit logs for a team.
pub async fn count_audit_logs(pool: &PgPool, team_id: Uuid) -> Result<i64, ApiError> {
    let row = sqlx::query(r#"SELECT COUNT(*) as count FROM audit_logs WHERE team_id = $1"#)
        .bind(team_id)
        .fetch_one(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(row.get::<i64, _>("count"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mentions() {
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        let content = format!("Hey @{uuid1} check this out! Also @{uuid2}");
        let mentions = extract_mentions(&content);

        assert_eq!(mentions.len(), 2);
        assert!(mentions.contains(&uuid1));
        assert!(mentions.contains(&uuid2));
    }

    #[test]
    fn test_extract_mentions_invalid() {
        let content = "Hey @invalid-uuid check this out!";
        let mentions = extract_mentions(content);
        assert!(mentions.is_empty());
    }
}
