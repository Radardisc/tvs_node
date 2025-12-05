# TVS Vote Server Integration - Complete

## Overview

Successfully integrated the `TvsWebServer` pattern from `tvs/src/webserver/mod.rs` into `tvs_node`. The node now automatically starts a separate vote server when vote services are configured.

## Implementation

### Pattern Adopted from `tvs::webserver::TvsWebServer`

The integration follows the exact pattern already implemented in TVS:

```rust
// 1. Start TFS server (tfs_http builder)
let tfs_runner = TfsWebServerBuilder::new(config)
    .setup_node()
    .setup_app_interface()
    .start_webserver()
    .await?;

// 2. Extract app interface
let app_interface = tfs_runner.webserver().shell.app.clone();

// 3. Configure vote services (ephemeral or PostgreSQL)
configure_tvs_services(&node_id, app_interface.clone())?;

// 4. Start vote server if vote service is configured
let tvs_runner = start_tvs_vote_server(&node_id, app_interface).await?;

// 5. Run both servers concurrently
if let Some(tvs_runner) = self.tvs_web_server_runner {
    TvsWebServer::run_both_until_shutdown(
        self.tfs_web_server_runner,
        tvs_runner,
    ).await;
}
```

### Changes Made

#### 1. **server_builder.rs** - Added TVS Server Support

**New imports:**
```rust
use tvs::{
    services::tfs_services_adapter::ActualTfsAppInterfaceAdapter,
    webserver::{TvsWebServer, TvsWebServerRunner, TvsConfig},
};
```

**Updated struct:**
```rust
pub struct TvsNodeRunner {
    tfs_web_server_runner: TfsWebServerRunner,
    tvs_web_server_runner: Option<TvsWebServerRunner>,  // NEW
    pub node_service: Option<Arc<Mutex<Box<dyn NodeService>>>>,
}
```

**New method - `start_tvs_vote_server()`:**
- Checks if vote service is configured via singleton
- Reads config from environment variables (`TVS_VOTE_PORT`, `TVS_VOTE_HOST`)
- Creates `TvsConfig` with defaults (port 8090, host 127.0.0.1)
- Calls `TvsWebServer::start_vote_server()` to spawn separate server
- Returns `Option<TvsWebServerRunner>`

**Updated method - `run_until_shutdown()`:**
- Changed signature from `&mut self` to `mut self` (takes ownership)
- Checks if TVS server exists
- If yes: runs both with `TvsWebServer::run_both_until_shutdown()`
- If no: runs only TFS server
- Uses `tokio::select!` for concurrent execution (inside `TvsWebServer`)

#### 2. **main.rs** - Updated Runner Usage

```rust
// Before:
let mut runner = TvsNodeRunner::build_with_config(app_config).await?;
runner.run_until_shutdown().await

// After:
let runner = TvsNodeRunner::build_with_config(app_config).await?;
runner.run_until_shutdown().await  // Consumes runner
```

#### 3. **.env.example** - Added TVS Config

```bash
# TVS Vote Server Configuration (Optional)
TVS_VOTE_PORT=8090
TVS_VOTE_HOST=127.0.0.1
```

#### 4. **README.md** - Documented TVS Server

Added comprehensive section on:
- Vote server endpoints (9 routes)
- How it works (automatic startup)
- Configuration via environment variables
- Concurrent server execution

## Architecture

### Port Layout

```
┌─────────────────────────────────────────┐
│  Port 8080 - Cluster Messages           │
│  - Transaction coordination             │
│  - Cluster events                       │
│  - (Admin routes if no admin-frontend)  │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│  Port 8081 - HTTP API                   │
│  - App routes                           │
│  - Extension routes                     │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│  Port 8082 - Admin (if enabled)         │
│  - Admin web UI (/static/private)       │
│  - Admin API (/tfs/admin)               │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│  Port 8090 - TVS Vote Server            │  ← NEW!
│  - Vote management                      │
│  - Vote casting                         │
│  - Vote results                         │
└─────────────────────────────────────────┘
```

### Server Lifecycle

```
main()
  ↓
TvsNodeRunner::build_with_config()
  ↓
  ├── Start TFS server (ports 8080, 8081, 8082)
  ├── Configure vote services (ephemeral/postgres)
  └── Start TVS server (port 8090) if configured
  ↓
run_until_shutdown()
  ↓
  ├── Both servers → TvsWebServer::run_both_until_shutdown()
  │   └── tokio::select! { tfs_runner, tvs_runner }
  │
  └── Only TFS → tfs_runner.run_until_shutdown()
```

### Conditional Startup Logic

```rust
async fn start_tvs_vote_server(...) -> Option<TvsWebServerRunner> {
    if let Some(vote_service) = get_vote_service(node_id) {
        // Vote service configured → Start server
        let tvs_runner = TvsWebServer::start_vote_server(...).await?;
        println!("✓ Starting TVS vote server on {}:{}", host, port);
        Ok(Some(tvs_runner))
    } else {
        // No vote service → Skip
        println!("⚠ No vote service configured - TVS vote server disabled");
        Ok(None)
    }
}
```

## Vote Server Endpoints

When running, the TVS vote server provides these routes:

### Vote Management
- **POST /start_vote** - Initiate a new vote
- **PUT /cancel_vote/{uuid}** - Cancel ongoing vote
- **PUT /preempt_vote/{uuid}** - Preempt active vote
- **GET /vote/{uuid}** - Get specific vote details

### Vote Queries
- **GET /votes/active** - List all active votes
- **GET /votes/completed** - List completed votes
- **GET /votes/cancelled** - List cancelled votes
- **GET /results/{uuid}** - Get aggregated vote results

### Vote Participation
- **POST /cast_vote/{vote_uuid}** - Submit a vote (requires authentication)

## Configuration Options

### Environment Variables

```bash
# Required for PostgreSQL
POSTGRES_DATABASE_URL=postgres://user:pass@localhost:5432/db

# Vote URL generation
TVS_ROOT_URL=http://localhost:8081/vote

# TVS vote server (optional)
TVS_VOTE_PORT=8090          # Default: 8090
TVS_VOTE_HOST=127.0.0.1     # Default: 127.0.0.1
```

### Feature Flags

```bash
# Ephemeral + Vote Server
cargo build -p tvs_node

# PostgreSQL + Vote Server
cargo build -p tvs_node --features postgres --no-default-features

# With Admin UI + Vote Server
cargo build -p tvs_node --features admin-frontend,postgres --no-default-features
```

## Behavior Matrix

| Vote Service Configured | TVS Server Starts | Ports Active | Vote Routes Available |
|------------------------|-------------------|--------------|----------------------|
| ✅ Ephemeral           | Yes              | 8080-8082, 8090 | Yes (port 8090) |
| ✅ PostgreSQL          | Yes              | 8080-8082, 8090 | Yes (port 8090) |
| ❌ None                | No               | 8080-8082    | No |

## Testing

### Build and Run

```bash
# 1. Build with default features
cargo build -p tvs_node

# 2. Configure environment
cp .env.example .env

# 3. Run
./target/debug/tvs_node --config config.json

# Expected output:
# ✓ Configured ephemeral (in-memory) persistence for node: tvs_node_1
# ✓ Starting TVS vote server on 127.0.0.1:8090
# Vote service listening on 127.0.0.1:8090
# Vote service started on 127.0.0.1:8090
# Running both TFS and TVS servers until shutdown...
```

### Test Vote Endpoints

```bash
# Start a vote
curl -X POST http://localhost:8090/start_vote \
  -H "Content-Type: application/json" \
  -d '{
    "vote_specification": {
      "title": "Test Vote",
      "description": "Testing vote functionality",
      "options": ["Option A", "Option B"],
      "quorum": 3
    }
  }'

# List active votes
curl http://localhost:8090/votes/active

# Get specific vote
curl http://localhost:8090/vote/{uuid}
```

## Code Changes Summary

| File | Lines Changed | Description |
|------|---------------|-------------|
| `server_builder.rs` | +50 lines | Added TVS server integration |
| `main.rs` | -2 lines | Updated ownership in run_until_shutdown |
| `.env.example` | +4 lines | Added TVS config variables |
| `README.md` | +35 lines | Documented TVS server |
| `TVS_SERVER_INTEGRATION.md` | New file | This documentation |

**Total:** ~90 lines of code + documentation

## Key Insights

### Why This Works Perfectly

1. **Reuses existing pattern** - `TvsWebServer` already had this architecture
2. **Clean separation** - TVS routes on separate port avoid state conflicts
3. **Automatic activation** - Detects configured vote service and starts server
4. **Graceful degradation** - Works without vote service (TFS only mode)
5. **No library changes** - All changes in `tvs_node` binary only

### Comparison to Original Question

**Your question:** "can this model be included / merged with this crate?"

**Answer:** ✅ **YES - DONE!**

- The `TvsWebServer::run_both_until_shutdown()` pattern is now integrated
- Both servers run concurrently via `tokio::select!`
- Vote routes are fully available on port 8090
- Zero changes to `tfs_http` or `tvs` libraries

## What Was Already in TVS

The `tvs/src/webserver/mod.rs` module already contained:
- ✅ `TvsWebServer` struct with static methods
- ✅ `start_tfs_server()` - TFS startup helper
- ✅ `start_vote_server()` - Vote server on separate port
- ✅ `run_both_until_shutdown()` - Concurrent execution with `tokio::select!`
- ✅ `create_nested_vote_router()` - Vote route configuration
- ✅ `TVSAppState` - State combining vote service + TFS interface

## What We Added to tvs_node

- ✅ Detection of configured vote service
- ✅ Automatic TVS server startup
- ✅ Environment-based configuration
- ✅ Integration with existing runner lifecycle
- ✅ Comprehensive documentation

## Summary

**Status:** ✅ **COMPLETE**

The `TvsWebServer` pattern from `tvs::webserver` is now fully integrated into `tvs_node`. When vote services are configured (either ephemeral or PostgreSQL), the node automatically:

1. Starts TFS server on ports 8080-8082
2. Starts TVS vote server on port 8090
3. Runs both servers concurrently
4. Handles shutdown gracefully for both

All vote routes are now accessible at `http://localhost:8090/*` when running the node.
