# TVS Routes Architecture

## Current State

The `setup_tvs_routes()` function in `server_builder.rs` currently returns the router unchanged with explanatory comments. This is intentional and reflects an architectural decision about how TVS routes should be served.

## Why TVS Routes Are Not Integrated Here

### The State Mismatch Problem

**TFS HTTP Routes** use: `TfsHttpAppShell` as state
- Contains: `TFSAppInterface`, `NodeId`, `StaticConfig`
- Managed by: `tfs_http` crate
- Purpose: Core TFS node operations

**TVS Vote Routes** use: `TVSAppState` as state
- Contains: `VoteService`, `TFSAppInterface`, configuration
- Managed by: `tvs::webserver` module
- Purpose: Vote management operations

These are **fundamentally different state types** and cannot be easily merged without significant refactoring.

### TVS Route Structure

Located in `tvs/src/webserver/`:

```rust
// Vote service management routes
vote_service_handlers::create_vote_service_router()
  - POST /start_vote
  - PUT  /cancel_vote/{uuid}
  - PUT  /preempt_vote/{uuid}
  - GET  /results/{uuid}
  - GET  /votes/active
  - GET  /votes/completed
  - GET  /votes/cancelled
  - GET  /vote/{uuid}

// Vote casting routes (with authentication)
cast_vote_handler::get_router()
  - POST /cast_vote/{vote_uuid}
```

Both routes are combined via:
```rust
tvs::webserver::create_nested_vote_router(combined_state: TVSAppState) -> Router
```

## Three Possible Solutions

### Option 1: Separate Server (Current Design) ✅ **RECOMMENDED**

**Status:** Partially implemented in `tvs::webserver`

**Approach:**
```rust
// Run TVS routes on a completely separate axum server
let tvs_server = TvsWebServer::start_vote_server(
    vote_service,
    tfs_app_interface,
    tvs_config
).await?;

// TVS routes available on http://localhost:8090/...
```

**Pros:**
- ✅ Clean separation of concerns
- ✅ Independent scaling (can scale vote service separately)
- ✅ No state type conflicts
- ✅ Already partially implemented in `tvs::webserver`

**Cons:**
- ⚠️ Additional port required
- ⚠️ More complex deployment (two servers to manage)

**Implementation:**
```rust
// In tvs_node main.rs or server_builder.rs
impl TvsNodeRunner {
    pub async fn with_vote_server(
        config: AppConfig,
    ) -> Result<(Self, TvsWebServerRunner), Box<dyn std::error::Error>> {
        // 1. Build TFS node
        let tfs_runner = Self::build_with_config(config).await?;

        // 2. Get vote service and app interface
        let node_id = tfs_runner.get_app_interface().get_this_node_id();
        let vote_service = tvs::services::vote_service::get_vote_service(&node_id)
            .ok_or("Vote service not configured")?;
        let app_interface = tfs_runner.get_app_interface().clone();

        // 3. Start separate TVS server
        let tvs_runner = TvsWebServer::start_vote_server(
            vote_service,
            app_interface,
            TvsConfig::default()
        ).await?;

        Ok((tfs_runner, tvs_runner))
    }
}
```

### Option 2: State Adapter Pattern

**Approach:** Create an adapter that bridges `TfsHttpAppShell` to `TVSAppState`

```rust
struct TVSStateAdapter {
    shell: Arc<TfsHttpAppShell>,
    vote_service: Arc<Mutex<Box<dyn VoteService>>>,
}

impl TVSStateAdapter {
    fn to_tvs_state(&self) -> TVSAppState {
        TVSAppState::new(
            self.vote_service.clone(),
            self.shell.app.clone()
        )
    }
}

// Then in setup_tvs_routes:
fn setup_tvs_routes(router: Router<Arc<TfsHttpAppShell>>) -> Router<Arc<TfsHttpAppShell>> {
    // Create adapter from shell state
    // Add TVS routes with adapter layer
    // Complexity: High
}
```

**Pros:**
- ✅ Single server
- ✅ All routes on TFS ports

**Cons:**
- ❌ Complex adapter layer
- ❌ State conversion overhead on every request
- ❌ Tight coupling between TFS and TVS

### Option 3: Refactor to Shared State

**Approach:** Refactor both `TfsHttpAppShell` and `TVSAppState` to use a common base state

```rust
struct CommonAppState {
    tfs_app: TFSAppInterface,
    vote_service: Option<Arc<Mutex<Box<dyn VoteService>>>>,
    node_id: NodeId,
    static_config: StaticConfig,
}

// Both TFS and TVS routes use this
```

**Pros:**
- ✅ Clean shared state
- ✅ Single server possible
- ✅ No adapters needed

**Cons:**
- ❌ Requires refactoring `tfs_http` (violates Option B)
- ❌ Breaking change for all `tfs_http` consumers
- ❌ Couples TFS and TVS at the library level

## Recommended Approach: Option 1 (Separate Server)

### Implementation Steps

1. **Enhance `TvsNodeRunner`** to support optional vote server:

```rust
pub struct TvsNodeRunner {
    tfs_web_server_runner: TfsWebServerRunner,
    tvs_web_server_runner: Option<TvsWebServerRunner>,  // NEW
}

impl TvsNodeRunner {
    pub async fn build_with_config(config: AppConfig)
        -> Result<TvsNodeRunner, Box<dyn std::error::Error>>
    {
        // Current TFS setup...

        // NEW: Optionally start TVS server
        let tvs_runner = Self::start_tvs_server_if_configured(&node_id, &app_interface).await?;

        Ok(Self {
            tfs_web_server_runner,
            tvs_web_server_runner: tvs_runner,
        })
    }

    async fn start_tvs_server_if_configured(
        node_id: &NodeId,
        app_interface: &TFSAppInterface,
    ) -> Result<Option<TvsWebServerRunner>, Box<dyn std::error::Error>> {
        // Check if vote service is configured
        if let Some(vote_service) = tvs::services::vote_service::get_vote_service(node_id) {
            let tvs_runner = TvsWebServer::start_vote_server(
                vote_service,
                app_interface.clone(),
                TvsConfig::from_env()  // Port from env or config
            ).await?;

            println!("✓ TVS vote server started on port {}", tvs_runner.port());
            Ok(Some(tvs_runner))
        } else {
            println!("⚠ No vote service configured - TVS routes disabled");
            Ok(None)
        }
    }

    pub async fn run_until_shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tvs_runner) = self.tvs_web_server_runner.take() {
            // Run both servers concurrently
            tokio::select! {
                result = self.tfs_web_server_runner.run_until_shutdown() => result,
                result = tvs_runner.run_until_shutdown() => result,
            }
        } else {
            // Just TFS server
            self.tfs_web_server_runner.run_until_shutdown().await
        }
    }
}
```

2. **Configuration** in `config.json`:

```json
{
  "server": {
    "cluster_message_port": 8080,
    "app_port": 8081,
    "admin_port": 8082
  },
  "tvs_server": {
    "enabled": true,
    "port": 8090,
    "host": "127.0.0.1"
  }
}
```

## Summary

**Current Status:**
- ✅ `setup_tvs_routes()` exists but returns router unchanged
- ✅ Documented with clear explanation of the architectural decision
- ✅ TVS routes fully implemented in `tvs::webserver`
- ⚠️ Not yet integrated into `tvs_node` binary

**Why Not Integrated:**
- TVS routes require different state type (`TVSAppState` vs `TfsHttpAppShell`)
- Merging would require significant refactoring or complex adapter patterns
- Separate server approach is cleaner and more scalable

**Recommendation:**
- Use Option 1 (Separate Server)
- Add `tvs_web_server_runner: Option<TvsWebServerRunner>` to `TvsNodeRunner`
- Conditionally start TVS server when vote service is configured
- Run both servers concurrently via `tokio::select!`

**Estimated Work:** ~50-80 lines of code to implement Option 1

This approach maintains clean separation, avoids tight coupling, and provides a scalable architecture for vote services.
