# Enya Cloud - Team Collaboration Backend

## Overview

Enya Cloud is the SaaS backend for Enya's enterprise team collaboration features. It provides:

- User authentication (GitHub OAuth)
- Team management with role-based access control
- Team invitations (email and magic link)
- Annotations and threaded discussions
- Real-time updates via WebSocket
- Audit logging for compliance
- War room mode for incident collaboration

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌────────────────┐
│   Enya Editor   │────▶│   Enya Cloud     │────▶│   PostgreSQL   │
│   (egui/WASM)   │◀────│   (Axum/Tokio)   │◀────│   (PlanetScale)│
└─────────────────┘     └──────────────────┘     └────────────────┘
         │                      │
         │                      │
         └──────WebSocket───────┘
              (Real-time)
```

## Authentication

### GitHub OAuth Flow

1. Client redirects to `/auth/github`
2. User authenticates with GitHub
3. Callback posts code to `/auth/github/callback`
4. Server exchanges code for access token
5. Server creates/updates user, generates JWT
6. Client stores JWT for subsequent requests

### JWT Claims

```json
{
  "sub": "user-uuid",
  "email": "user@example.com",
  "exp": 1234567890
}
```

Default expiry: 7 days (configurable via `JWT_EXPIRY_SECS`).

## Team Management

### Roles

| Role | Permissions |
|------|-------------|
| `admin` | Full access: invite members, manage roles, view audit logs, remove members |
| `member` | Standard access: view team, create annotations, participate in discussions |

### Team Creation

When a user creates a team, they automatically become an `admin`. At least one admin must exist at all times.

### Member Management

**Add Member** (via invitation):
- Admin creates invitation with email or magic link
- Invited user accepts invitation
- User is added with specified role

**Update Role**:
- Admin-only operation
- Cannot demote the last admin

**Remove Member**:
- Admin-only operation
- Cannot remove the last admin
- Cannot remove yourself (use "leave" instead)

**Leave Team**:
- Any member can leave
- Last admin cannot leave (must transfer ownership first)

## Invitations

### Types

1. **Email Invitation**: Tied to specific email address
2. **Magic Link**: Can be used by anyone (shareable link)

### Invitation Lifecycle

```
Created → Pending → Accepted
              ↓
           Expired (7 days)
              ↓
           Revoked (by admin)
```

### Security

- Tokens are 64-character hex strings (32 bytes of randomness)
- Email invitations verify the accepting user's email
- Expired/accepted invitations cannot be reused

## API Endpoints

### Authentication

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/auth/github` | Initiate GitHub OAuth |
| POST | `/auth/github/callback` | Handle OAuth callback |
| GET | `/auth/me` | Get current user info |

### Teams

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/teams` | List user's teams | User |
| POST | `/teams` | Create team | User |
| GET | `/teams/{id}` | Get team | Member |
| GET | `/teams/{id}/members` | List members | Member |
| GET | `/teams/{id}/members/roles` | List members with roles | Member |
| PATCH | `/teams/{id}/members/{uid}/role` | Update member role | Admin |
| DELETE | `/teams/{id}/members/{uid}` | Remove member | Admin |
| POST | `/teams/{id}/leave` | Leave team | Member |

### Invitations

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/teams/{id}/invitations` | List pending invitations | Admin |
| POST | `/teams/{id}/invitations` | Create invitation | Admin |
| DELETE | `/teams/{id}/invitations/{inv_id}` | Revoke invitation | Admin |
| GET | `/invitations/{token}` | Get invitation info | Public |
| POST | `/invitations/accept` | Accept invitation | User |

### Audit Logs

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/teams/{id}/audit-logs` | List audit logs | Admin |

Query parameters:
- `limit` (default: 50, max: 100)
- `offset` (default: 0)

### Annotations

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/teams/{id}/annotations` | List annotations | Member |
| POST | `/teams/{id}/annotations` | Create annotation | Member |
| DELETE | `/teams/{id}/annotations/{ann_id}` | Delete annotation | Member |

### Channels

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/teams/{id}/channels` | List channels | Member |
| POST | `/teams/{id}/channels` | Create channel | Member |
| GET | `/teams/{id}/channels/{ch_id}` | Get channel | Member |
| GET | `/teams/{id}/channels/{ch_id}/threads` | List threads | Member |
| POST | `/teams/{id}/channels/{ch_id}/threads` | Create thread | Member |

### Threads & Messages

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/threads/{id}` | Get thread | Member |
| PATCH | `/threads/{id}` | Update thread | Member |
| GET | `/threads/{id}/messages` | List messages | Member |
| POST | `/threads/{id}/messages` | Send message | Member |

### Real-time

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/realtime?token=...&team_id=...` | WebSocket connection |

## Audit Logging

All significant actions are logged for compliance:

| Action | Resource Type | Description |
|--------|--------------|-------------|
| `invitation.created` | invitation | Admin created an invitation |
| `invitation.revoked` | invitation | Admin revoked an invitation |
| `member.joined` | user | User accepted invitation |
| `member.left` | user | User left the team |
| `member.removed` | user | Admin removed a member |
| `member.role_changed` | user | Admin changed member's role |

Log entry structure:
```json
{
  "id": "uuid",
  "team_id": "uuid",
  "actor_id": "uuid",
  "action": "member.joined",
  "resource_type": "user",
  "resource_id": "uuid",
  "details": { "role": "member" },
  "created_at": 1234567890
}
```

## Real-time Events

Events broadcast via WebSocket:

| Event | Description |
|-------|-------------|
| `annotation_created` | New annotation added |
| `annotation_deleted` | Annotation removed |
| `message_received` | New message in thread |
| `user_typing` | User typing in thread |
| `member_joined` | New team member |
| `member_left` | Member left/removed |
| `channel_created` | New channel |
| `thread_created` | New thread in channel |
| `thread_resolved` | Thread marked resolved |
| `view_shared` | War room view shared |

## Database Schema

### Core Tables

- `users` - User accounts
- `teams` - Teams/workspaces
- `team_members` - Team membership with roles
- `team_invitations` - Pending/accepted invitations
- `audit_logs` - Action audit trail

### Collaboration Tables

- `annotations` - Chart annotations
- `threads` - Discussion threads
- `messages` - Thread messages
- `channels` - Team channels
- `channel_threads` - Threads in channels

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | - | PostgreSQL connection string |
| `JWT_SECRET` | Yes | - | Secret for JWT signing |
| `HOST` | No | `0.0.0.0` | Server host |
| `PORT` | No | `3000` | Server port |
| `GITHUB_CLIENT_ID` | No | - | GitHub OAuth client ID |
| `GITHUB_CLIENT_SECRET` | No | - | GitHub OAuth client secret |
| `FRONTEND_URL` | No | `http://localhost:8080` | Frontend URL for invite links |
| `JWT_EXPIRY_SECS` | No | `604800` | JWT expiry (7 days) |

## Enterprise Features

### Self-Hosted Deployment

For enterprise customers with self-hosted requirements:

1. Provide Docker image with all dependencies
2. Customer deploys to their infrastructure
3. Configure with their PostgreSQL instance
4. Optional: Connect to their identity provider (SAML SSO - future)

### Security Considerations

- All endpoints require authentication (except health check and invite info)
- Admin operations require `admin` role
- Rate limiting recommended for production (via reverse proxy)
- CORS configured for frontend domain
- Secrets should be stored in secure vault

## Development

### Running Locally

```bash
# Start PostgreSQL
docker compose up -d postgres

# Run migrations
sqlx migrate run

# Start server
cargo run -p enya-cloud
```

### Testing

```bash
cargo nextest run -p enya-cloud
```

### Dev Authentication

For development, enable `DEV_AUTH=true` to use `/auth/dev` endpoint for creating test users without OAuth.
