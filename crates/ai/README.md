# enya-ai

AI agent integration for Enya, providing LLM provider clients and an agent framework for AI-powered features.

## Architecture

Enya uses the **Agent Client Protocol (ACP)** to communicate with AI coding agents. This is a JSON-RPC 2.0 protocol over stdio that allows connecting to any ACP-compatible agent.

### Note on ACP Implementation

We use a **custom ACP implementation** (~400 lines) rather than the [`agent-client-protocol`](https://docs.rs/agent-client-protocol) crate. This was a deliberate choice:

**Why custom:**
- Minimal footprint - we only need the basic prompt→stream flow
- Full control over parsing - we extract only the message types we use
- No extra dependency to track (the crate is pre-1.0 at v0.9.2)
- Our `AgentEvent` enum stays simple and fits our editor's needs

**When to reconsider:**
- If we need agent-initiated requests (file reads, terminal, permissions)
- If ACP spec changes frequently and manual tracking becomes burdensome
- If we want richer content types (images, audio)

```
┌─────────────────────────────────────────────────────────────┐
│                     Editor (Enya)                           │
│  ┌───────────────────────────────────────────────────────┐  │
│  │               AcpClient                               │  │
│  │  - Spawns agent process                               │  │
│  │  - JSON-RPC 2.0 messages                              │  │
│  │  - Converts ACP events → AgentEvent                   │  │
│  └───────────────────────────────────────────────────────┘  │
│                          │                                   │
│                JSON-RPC 2.0 over stdio                      │
└──────────────────────────┼───────────────────────────────────┘
                           │
              ┌─────────────┴─────────────┐
              │                           │
              ▼                           ▼
      ┌───────────────┐          ┌───────────────┐
      │  Claude Code  │          │    Codex      │
      └───────────────┘          └───────────────┘
```

## Crate Structure

```
src/
├── lib.rs              # Main entry point & re-exports
├── types.rs            # Core types (Message, AgentEvent, AgentError, etc.)
├── tool.rs             # Tool trait & execution context
├── acp/
│   ├── mod.rs          # ACP module docs & exports
│   ├── client.rs       # AcpClient - spawns agent, handles JSON-RPC
│   └── config.rs       # AgentConfig, AgentKind
└── provider/
    ├── mod.rs          # Provider enum (Anthropic, OpenAI)
    ├── anthropic.rs    # Direct Anthropic API client
    └── openai.rs       # Direct OpenAI API client
```

## Key Types

| Type | Description |
|------|-------------|
| `AcpClient` | Spawns agent subprocess, sends JSON-RPC messages, returns `Receiver<AgentEvent>` |
| `AgentConfig` | Configuration (command, args, working_dir, env) |
| `AgentKind` | Enum: ClaudeCode, Codex, Custom |
| `AgentEvent` | Streaming events: TextDelta, ThinkingDelta, ToolCallStart, ToolResult, Done, Error |
| `Provider` | Direct API client (Anthropic or OpenAI) - alternative to ACP |

## Usage

### Using ACP (Recommended)

The Agent Client Protocol allows connecting to any ACP-compatible agent like Claude Code or Codex:

```rust
use enya_ai::{AcpClient, AgentEvent};

// Create a client for Claude Code
let client = AcpClient::claude_code();

// Send a prompt and receive streaming events
let rx = client.prompt("Help me understand this code", None);

while let Ok(event) = rx.try_recv() {
    match event {
        AgentEvent::TextDelta(text) => print!("{}", text),
        AgentEvent::ToolCallStart { name, .. } => println!("[{}]", name),
        AgentEvent::Done { .. } => break,
        _ => {}
    }
}
```

### Using Direct Provider API

For direct API access without the CLI:

```rust
use enya_ai::{Provider, Message, AgentEvent};

let provider = Provider::anthropic("sk-...", "claude-sonnet-4-20250514");
let rx = provider.stream("You are helpful", &messages, &tools);

while let Ok(event) = rx.try_recv() {
    match event {
        AgentEvent::TextDelta(text) => print!("{}", text),
        AgentEvent::Done { .. } => break,
        _ => {}
    }
}
```

## Supported Agents

| Agent | Command | Status |
|-------|---------|--------|
| Claude Code | `npx @zed-industries/claude-code-acp` | Primary |
| Codex | `npx @zed-industries/codex-acp` | Supported |

## Communication Flow

1. User types prompt in agent panel/pane
2. Editor builds `EditorContext` and prepends to prompt
3. `AcpClient::prompt_with_context()` spawns agent via npx
4. JSON-RPC messages: `initialize` → `session/new` → `session/prompt`
5. Agent streams `session/update` notifications:
   - `agent_message_chunk` → `TextDelta`
   - `agent_thought_chunk` → `ThinkingDelta`
   - `tool_call` / `tool_call_update` → tool events
6. Editor polls `Receiver<AgentEvent>` each frame
7. Responses parsed for `enya-command` blocks → `AgentCommand`
8. Commands executed (create pane, show inline chart, etc.)
