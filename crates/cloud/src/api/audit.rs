//! Audit log API routes.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::responses::{AuditLogResponse, AuditLogsResponse};
use crate::auth::AuthUser;
use crate::auth::middleware::require_team_admin;
use crate::db;
use crate::error::ApiError;
use crate::state::AppState;

/// Query parameters for listing audit logs.
#[derive(Debug, Deserialize)]
pub struct ListAuditLogsQuery {
    /// Maximum number of logs to return (default: 50, max: 100).
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Offset for pagination (default: 0).
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// List audit logs for a team.
///
/// Admin-only. Returns paginated audit logs with actor information.
pub async fn list_audit_logs(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
    Query(query): Query<ListAuditLogsQuery>,
) -> Result<Json<AuditLogsResponse>, ApiError> {
    // Require admin access
    require_team_admin(&state, team_id, user.id).await?;

    // Clamp limit
    let limit = query.limit.clamp(1, 100);
    let offset = query.offset.max(0);

    // Get logs and total count
    let logs = db::queries::list_audit_logs(&state.db, team_id, limit, offset).await?;
    let total = db::queries::count_audit_logs(&state.db, team_id).await?;

    // Build responses with actor names
    let mut log_responses = Vec::with_capacity(logs.len());
    for log in logs {
        let actor_name = db::queries::get_user(&state.db, log.actor_id)
            .await
            .ok()
            .map(|u| u.display_name);

        log_responses.push(AuditLogResponse {
            id: log.id,
            actor_id: log.actor_id,
            actor_name,
            action: log.action,
            resource_type: log.resource_type,
            resource_id: log.resource_id,
            details: log.details,
            created_at: log.created_at.timestamp() as u64,
        });
    }

    Ok(Json(AuditLogsResponse {
        logs: log_responses,
        total,
        limit,
        offset,
    }))
}
