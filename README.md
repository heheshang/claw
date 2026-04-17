# claw

A Tauri + Vue 3 AI coding assistant with real-time SSE streaming, tool execution, and session management.

## Features

- **Real-time SSE Streaming** - AI responses stream live via Server-Sent Events
- **Tool Execution** - Built-in tools for file operations, grep, git, and more
- **Session Management** - Persistent conversation sessions with JSONLines storage
- **Permission System** - Configurable tool access levels
- **Cost Tracking** - Usage telemetry with token and cost monitoring

## Tech Stack

- **Frontend**: Vue 3 + TypeScript + Vite
- **Backend**: Tauri 2.0 (Rust)
- **LLM**: Anthropic Claude API compatible

## Commands

| Command | Description |
|---------|-------------|
| `/review <file>` | Review code file for issues |
| `/security-review <file>` | Review code for security vulnerabilities |
| `/model [name]` | Show or switch model (opus/sonnet/haiku) |
| `/cost` | Show usage cost statistics |
| `/stats` | Show session statistics |
| `/clear` | Clear conversation history |
| `/compact` | Compact session messages |
| `/status` | Show current session status |
| `/help` | Show all available commands |

## Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Project Structure

```
├── src/                 # Vue frontend source
│   ├── views/          # Vue components (Chat.vue, etc.)
│   ├── composables/    # Vue composables
│   └── App.vue        # Main app component
├── src-tauri/          # Tauri/Rust backend
│   └── src/
│       ├── harness/    # AI harness implementation
│       │   ├── api/    # API client, SSE parsing
│       │   ├── runtime/ # Session management
│       │   └── tools/  # Tool definitions and executor
│       └── lib.rs      # Tauri commands
└── public/            # Static assets
```

## Tool Permissions

Tools respect a permission level system:
- `DangerFullAccess` - All tools allowed
- `ReadOnly` - Read operations only
- `Restricted` - Limited tool set