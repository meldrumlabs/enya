# Team Collaboration Features

This document summarizes the team collaboration features added to the Enya editor, enabling real-time collaboration for observability workflows.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         enya-editor                             │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐  │
│  │  TeamState  │  │ chat module  │  │ team_status/team_menu  │  │
│  │  (team.rs)  │  │  (chat/*.rs) │  │     (widgets)          │  │
│  └──────┬──────┘  └──────┬───────┘  └────────────────────────┘  │
└─────────┼────────────────┼──────────────────────────────────────┘
          │                │
          v                v
    ┌─────────────────────────┐
    │      enya-team-api      │
    │  ┌───────┐ ┌─────────┐  │
    │  │Client │ │WebSocket│  │
    │  └───────┘ └─────────┘  │
    └────────────┬────────────┘
                 │
                 v
          Team Cloud Server
```

## Modules Added

### 1. Chat Module (`src/chat/`)

The chat module provides Slack/Zed-style team messaging with channels and threads.

#### Files:
- **`mod.rs`** - Module exports and documentation
- **`channel.rs`** - Channel data model (General, Incidents, Deployments, Alerts)
- **`thread.rs`** - Thread data model with status (Active, Resolved, Archived) and priority
- **`message.rs`** - Message model with @mentions support (users, agents, charts)
- **`chat_view.rs`** - Main chat UI with inline visualizations
- **`channels_panel.rs`** - Split-view sidebar with threads-first layout
- **`state.rs`** - Chat state management and demo data
- **`theme_helpers.rs`** - Chat-specific color helpers

#### Key Types:
```rust
// Channels
pub struct Channel { id, name, kind, description, unread_count, ... }
pub enum ChannelKind { General, Incidents, Deployments, Alerts, Custom }

// Threads
pub struct Thread { id, channel_id, title, status, priority, reply_count, ... }
pub enum ThreadStatus { Active, Resolved, Archived }
pub enum ThreadPriority { Normal, Low, High, Critical }

// Messages
pub struct ChatMessage { id, author, content, mentions, inline_charts, ... }
pub enum ChatMessageAuthor { User { user_id, name }, Agent { model, name }, System }
pub enum MentionKind { User(UserId), Agent { model }, Chart { chart_name }, Everyone }
```

### 2. Inline Visualizations

Messages can embed rich visualizations captured from the workspace:

```rust
pub enum InlineVisualization {
    Chart(InlineChart),      // Time series line chart
    Stat(InlineStat),        // Single metric card
    Table(InlineTable),      // Tabular data
    BarChart(InlineBarChart), // Bar chart
    Gauge(InlineGauge),      // Gauge/dial visualization
}
```

**Snapshot Model**: When a user embeds a visualization, the data is captured at share time and stored in the message. This ensures:
- All team members see the exact same data
- Charts are frozen at the moment of sharing (critical for incident post-mortems)
- No dependency on individual Prometheus connections
- Works offline - data persists even if metrics are no longer available

#### @ Autocomplete for Embedding

The chat input supports `@` autocomplete to embed workspace panes:

```
┌─ Embed visualization ───────────────────┐
│ 📈 P99 Latency          (Time Series)   │
│ 🎛️  DB Connections       (Gauge)         │
│ 📊 Request Distribution (Bar Chart)     │
│ 🔢 Active Users         (Stat)          │
└─────────────────────────────────────────┘
```

The `PaneVisualization` enum captures data from all visualization types:
```rust
pub enum PaneVisualization {
    TimeSeries { series: Vec<Series> },
    Stat { value: f64, unit: String, sparkline: Vec<f64> },
    Gauge { value: f64, min: f64, max: f64, unit: String },
    BarChart { bars: Vec<(String, f64)> },
    Sparkline { data: Vec<f64> },
    Heatmap,
}
```

### 3. Team State Management (`src/team.rs`)

Decoupled team state that works both with and without a backend:

```rust
pub struct TeamState {
    manager: TeamManager,
    enabled: bool,
    members: Vec<TeamMember>,
    unread_count: usize,
    chat_state: ChatState,
    demo_mode: Option<DemoTeamInfo>,
}
```

Features:
- Optional team connectivity (editor works standalone)
- Demo mode for testing UI without backend
- Async polling compatible with egui's immediate mode
- Member presence tracking

### 4. Team Widgets (`src/components/widget/`)

#### `team_status.rs` - Status Line Widget
Shows team connection status in the bottom status line:
- Online member count
- Unread notifications badge
- WebSocket connection indicator
- Only visible when connected

#### `team_menu.rs` - Team Menu Overlay
Centered overlay for team actions:
- Team member list with presence (online/idle/offline)
- Quick actions: Add Annotation, Share View, Start War Room
- Team settings and sign out

### 5. Team API Crate (`crates/team-api/`)

Standalone crate providing the API client and types:

- **`client.rs`** - HTTP client with promise-based async
- **`websocket.rs`** - Real-time updates via WebSocket
- **`manager.rs`** - High-level team manager
- **`types.rs`** - Shared types (User, Team, Channel, etc.)
- **`promise.rs`** - egui-compatible promise handling

### 6. Commit Reference Autocomplete

The chat input supports `#` autocomplete for git commit references:

```
Type: Looking at the changes from #abc123
      ┌─ Select commit ─────────────────────┐
      │ abc1234  fix: resolve latency spike │
      │ def5678  feat: add caching layer    │
      └─────────────────────────────────────┘
```

When selected, opens the diff viewer to show the commit changes.

## UI Layout

### Channels Panel (Threads-First Layout)

```
┌─────────────────────────────────────────────────────────┐
│ Channels    │  #incidents > P99 latency spike          │
├─────────────┤──────────────────────────────────────────│
│ THREADS     │  Alice: P99 latency spike detected...    │
│ 🔥 P99 spike│  Bob: Seeing elevated error rates too    │
│             │  Claude: Based on the metrics, the...    │
├─────────────┤  You: Scaling up db replicas now         │
│ CHANNELS    │                                          │
│ # general   │  [Inline chart embed]                    │
│ # incidents │                                          │
│ # deploys   │                                          │
├─────────────┤──────────────────────────────────────────│
│ ONLINE — 3  │  ┌────────────────────────────────────┐  │
│ ● Alice     │  │ Type a message... @mention  [Send] │  │
│ ● Bob       │  └────────────────────────────────────┘  │
└─────────────┴──────────────────────────────────────────┘
```

### Inline Visualization in Messages

```
┌─ Message Bubble ────────────────────────┐
│ P99 latency spike detected! Look:       │
│                                         │
│ ┌─ Inline Chart ──────────────────────┐ │
│ │ 📈 P99 Latency                      │ │
│ │ ┌───────────────────────────────┐   │ │
│ │ │ 500 ─┼───────────────────────│   │ │
│ │ │      │     ╱╲                │   │ │
│ │ │ 250 ─┼────╱  ╲───────────────│   │ │
│ │ │      │   ╱    ╲              │   │ │
│ │ │   0 ─┼──╱──────╲─────────────│   │ │
│ │ │      14:00    14:30    15:00 │   │ │
│ │ └───────────────────────────────┘   │ │
│ └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

## Demo Mode

The chat module includes comprehensive demo data for testing:
- Sample channels (#general, #incidents, #deployments)
- Active threads with different priorities
- Messages with inline charts showing realistic incident data
- Team member presence

Enable demo mode via `TeamState::enable_demo_mode()`.

---

# Local Development

## Quick Start (Complete Setup)

Follow these steps to run the full team collaboration stack locally:

### 1. Start PostgreSQL

```bash
docker run -d --name enya-postgres \
  -e POSTGRES_USER=enya \
  -e POSTGRES_PASSWORD=enya \
  -e POSTGRES_DB=enya \
  -p 5432:5432 \
  postgres:16
```

### 2. Create `.env` file in `crates/cloud/`

```bash
cp crates/cloud/.env.example crates/cloud/.env
```

Or create manually:
```bash
cat > crates/cloud/.env << 'EOF'
DATABASE_URL=postgres://enya:enya@localhost:5432/enya
JWT_SECRET=dev-secret-change-in-production
HOST=0.0.0.0
PORT=3000
FRONTEND_URL=http://localhost:8080
DEV_AUTH=true
EOF
```

### 3. Run the cloud server

```bash
cd crates/cloud
cargo run
```

The server will automatically run migrations and start on `http://localhost:3000`.

### 4. Get a dev auth token (in another terminal)

```bash
# Create a test user and get token
curl -X POST http://localhost:3000/auth/dev \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice"}'
```

This returns JSON with `access_token` - copy that token.

### 5. Run the editor

```bash
cargo run -p enya-editor
```

### 6. Connect to the server

In the editor, press `:` to open command palette and type:

```
:team connect http://localhost:3000 <paste-your-token-here>
```

You should now see the team status indicator in the status line and have access to team chat features.

---

## Detailed Setup

### Prerequisites

**PostgreSQL** - Install via Homebrew or Docker:

```bash
# macOS with Homebrew
brew install postgresql@16
brew services start postgresql@16
createuser -s enya
createdb -O enya enya

# Or use Docker (recommended)
docker run -d --name enya-postgres \
  -e POSTGRES_USER=enya \
  -e POSTGRES_PASSWORD=enya \
  -e POSTGRES_DB=enya \
  -p 5432:5432 \
  postgres:16
```

### Environment Configuration

The `.env` file in `crates/cloud/` configures the server:

```bash
DATABASE_URL=postgres://enya:enya@localhost:5432/enya
JWT_SECRET=dev-secret-change-in-production
HOST=0.0.0.0
PORT=3000
FRONTEND_URL=http://localhost:8080
DEV_AUTH=true  # Enables /auth/dev endpoint
```

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/auth/dev` | POST | Create dev user (requires `DEV_AUTH=true`) |
| `/auth/github` | GET | GitHub OAuth login |
| `/auth/github/callback` | POST | GitHub OAuth callback |
| `/auth/me` | GET | Get current user |
| `/teams` | GET/POST | List/create teams |
| `/teams/:id` | GET/PUT/DELETE | Team CRUD |
| `/teams/:id/members` | GET/POST | Team members |
| `/channels` | GET/POST | List/create channels |
| `/channels/:id/messages` | GET/POST | Channel messages |
| `/threads` | GET/POST | List/create threads |
| `/threads/:id/messages` | GET/POST | Thread messages |
| `/ws` | WebSocket | Real-time updates |

### Programmatic Connection

To connect the editor programmatically (for custom builds):

```rust
use enya_editor::{TeamConfig, TeamState};

let config = TeamConfig {
    server_url: Some("http://localhost:3000".to_string()),
    auth_token: Some("your-jwt-token-from-dev-auth".to_string()),
};

// Native
let team_state = TeamState::new(config, async_runtime, &ctx);

// WASM
let team_state = TeamState::new(config, &ctx);
```

### Testing Without Backend (Demo Mode)

For UI development without running the server:

```rust
let mut team_state = TeamState::default();
team_state.enable_demo_mode("Demo Team", user_id);
```

This populates the chat with realistic demo data for testing the UI.

---

# Next Steps

## Short Term

1. **Real-time Message Sync**
   - Connect chat state to WebSocket events from team-api
   - Handle incoming messages, reactions, and presence updates
   - Implement optimistic UI updates for sent messages

2. **Message Persistence**
   - Store/load messages from team server
   - Implement pagination for message history
   - Cache recent messages locally

3. **Annotation Integration**
   - Connect chart annotations to team threads
   - Allow creating threads from annotations
   - Show annotation markers in embedded charts

## Medium Term

4. **War Room Mode**
   - Dedicated incident collaboration view
   - Auto-aggregate related charts and alerts
   - Shared cursor/focus between team members
   - Timeline of actions during incident

5. **AI Agent Integration**
   - @claude mentions trigger AI analysis
   - Agent can reference embedded charts
   - Suggested actions based on metrics

6. **Notification System**
   - Desktop notifications for mentions
   - Unread indicators per channel/thread
   - Notification preferences

## Long Term

7. **Offline Support**
   - Queue messages when disconnected
   - Sync on reconnection
   - Conflict resolution for concurrent edits

8. **Advanced Sharing**
   - Share dashboard links
   - Collaborative dashboard editing
   - Screen sharing for live troubleshooting

9. **Audit Trail**
   - Track who viewed what during incidents
   - Record actions taken (queries run, alerts ack'd)
   - Generate incident reports

10. **External Integrations**
    - Slack/Teams bridging
    - PagerDuty integration
    - Jira ticket creation from threads
