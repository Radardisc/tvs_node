# TVS Node

A configurable Transaction Voting Service node that can run with different persistence backends.

## Features

The node supports multiple independent feature flags that can be combined:

### Persistence Backend
- **ephemeral** (default): In-memory storage for development and testing
- **postgres**: PostgreSQL-backed persistent storage for production

### Admin Interface
- **admin-frontend**: Enable admin web UI and dedicated admin port (optional)

## Building

### Development (Ephemeral/In-Memory)

```bash
# Basic build - no admin UI
cargo build -p tvs_node

# With admin UI
cargo build -p tvs_node --features admin-frontend
```

### Production (PostgreSQL)

```bash
# PostgreSQL without admin UI
cargo build -p tvs_node --features postgres --no-default-features

# PostgreSQL with admin UI
cargo build -p tvs_node --features postgres,admin-frontend --no-default-features
```

### Feature Combinations

Features can be combined as needed:

| Build Command | Persistence | Admin UI | Use Case |
|--------------|-------------|----------|----------|
| `cargo build` | Ephemeral | No | Quick development |
| `cargo build --features admin-frontend` | Ephemeral | Yes | Development with UI |
| `cargo build --features postgres --no-default-features` | PostgreSQL | No | Headless production |
| `cargo build --features postgres,admin-frontend --no-default-features` | PostgreSQL | Yes | Full-featured production |

## Running

```bash
# With default config
./target/debug/tvs_node

# With custom config
./target/debug/tvs_node --config /path/to/config.json
```

## Configuration

See `config.example.json` for configuration options.

### PostgreSQL Setup

When using the `postgres` feature:

1. Copy `.env.example` to `.env` and configure database connection:
   ```bash
   cp .env.example .env
   # Edit .env and set:
   # POSTGRES_DATABASE_URL=postgres://user:password@localhost:5432/tfs_tvs_db
   # TVS_ROOT_URL=http://localhost:8081/vote
   ```

2. Create the PostgreSQL database:
   ```bash
   createdb tfs_tvs_db
   ```

3. Migrations are automatically run on startup - the node will:
   - Create a per-node schema (e.g., `tfs_tvs_node_1_550e8400...`)
   - Run TFS migrations (nodes, grid_transactions, cluster_events, etc.)
   - Run TVS migrations (votes, vote_results, vote_url_mappings, etc.)

4. Start the node:
   ```bash
   cargo run --features postgres --no-default-features -- --config config.json
   ```

### Environment Variables

- **POSTGRES_DATABASE_URL** (required for postgres feature): Database connection string
- **TVS_ROOT_URL** (optional): Base URL for vote URLs (default: `http://localhost:8081/vote`)
- **TVS_VOTE_PORT** (optional): Port for TVS vote server (default: `8090`)
- **TVS_VOTE_HOST** (optional): Host for TVS vote server (default: `127.0.0.1`)
- **RUST_LOG** (optional): Override logging level

### Admin Frontend

When the `admin-frontend` feature is enabled:

1. **Admin UI** is available at: `http://localhost:8082/static/private`
   - React-based web interface
   - Node status dashboard
   - Transaction monitoring
   - Requires authentication (configured via middleware)

2. **Admin API** is available at: `http://localhost:8082/tfs/admin`
   - Node management endpoints
   - Transaction inspection
   - System metrics

3. **Separate Port**: Admin routes run on dedicated port (8082 by default in config)

When `admin-frontend` is **NOT** enabled:
- Admin routes are merged with cluster port (8080)
- No separate admin web UI
- Admin API endpoints still technically available but not on dedicated interface
- Smaller attack surface for production deployments

### TVS Vote Server

The node automatically starts a **separate vote server** when vote services are configured:

**Vote Server Endpoints** (on port 8090 by default):
- `POST /start_vote` - Start a new vote
- `PUT /cancel_vote/{uuid}` - Cancel an active vote
- `PUT /preempt_vote/{uuid}` - Preempt an active vote
- `GET /results/{uuid}` - Get vote results
- `GET /votes/active` - List active votes
- `GET /votes/completed` - List completed votes
- `GET /votes/cancelled` - List cancelled votes
- `GET /vote/{uuid}` - Get specific vote info
- `POST /cast_vote/{vote_uuid}` - Submit a vote (with authentication)

**How it works:**
1. Vote services are configured during startup (ephemeral or PostgreSQL)
2. If vote service is detected, TVS vote server starts on separate port
3. Both TFS and TVS servers run concurrently via `tokio::select!`
4. Shutdown signal terminates both servers gracefully

**Configuration:**
```bash
# Set custom vote server port
export TVS_VOTE_PORT=9000
export TVS_VOTE_HOST=0.0.0.0  # Listen on all interfaces
```

If no vote service is configured, only the TFS server runs.

## Architecture

### Component Hierarchy

```
tvs_node (binary crate with features)
├── ephemeral feature (default)
│   ├── tvs::services::EphemeralVoteService (vote storage)
│   ├── tvs::services::EphemeralVoteUrlService (URL mapping)
│   └── tfs::TfsServices::ephemeral() (TFS node services)
│
└── postgres feature
    ├── tvs_postgres::PostgresVoteService (persistent vote storage)
    ├── tvs_postgres::PostgresVoteUrlService (persistent URL mapping)
    ├── tfs_postgres::DbSession (connection pool + schema isolation)
    ├── tfs_postgres::SchemaContext (per-node schema: tfs_{name}_{uuid})
    └── tfs::TfsServices::ephemeral() (TFS services remain in-memory for now)
```

### Database Schema Isolation

Each node gets its own PostgreSQL schema for data isolation:

```
Database: tfs_tvs_db
├── Schema: tfs_tvs_node_1_550e8400e29b41d4a716446655440000
│   ├── TFS Tables (managed by tfs_postgres)
│   │   ├── nodes
│   │   ├── grid_transactions
│   │   ├── cluster_events
│   │   └── ...
│   └── TVS Tables (managed by tvs_postgres)
│       ├── votes
│       ├── vote_results
│       ├── vote_count_urls
│       └── vote_url_mappings
└── Schema: tfs_tvs_node_2_... (another node)
```

### Service Configuration Pattern

The `tvs_node` binary uses Cargo features to conditionally compile different persistence backends:

- **Ephemeral**: Uses in-memory HashMaps (fast, no setup, data lost on restart)
- **PostgreSQL**: Uses Diesel ORM with PostgreSQL (persistent, survives restarts)

The server builder in `src/server_builder.rs` handles feature-based configuration:

```rust
#[cfg(feature = "postgres")]
{
    // Shared connection pool for TFS and TVS
    let db_pool = establish_connection_pool();
    let schema_ctx = SchemaContext::from_node_id(node_id, false);
    let session = DbSession::new(db_pool, schema_ctx);

    // Initialize schemas and migrations
    session.initialize_schema()?;
    initialize_tvs_tables(&session)?;

    // Configure PostgreSQL services
    let vote_service = PostgresVoteService::new(session.clone());
    let vote_url_service = PostgresVoteUrlService::with_root_url(session, root_url);

    // Register services via singleton pattern
    configure_vote_service(node_id, Box::new(vote_service))?;
    configure_vote_url_service(node_id, Box::new(vote_url_service))?;
}

#[cfg(feature = "ephemeral")]
{
    // In-memory services
    configure_ephemeral_vote_service(node_id)?;
    configure_ephemeral_vote_url_service(node_id, root_url)?;
}
```

This allows the same binary to support different backends without runtime overhead - the unused backend code is completely removed at compile time.

## Development

### Testing PostgreSQL Integration

```bash
# 1. Set up database
createdb tfs_tvs_db
cp .env.example .env
# Edit .env with your database credentials

# 2. Build with postgres feature
cargo build --features postgres --no-default-features

# 3. Run the node
./target/debug/tvs_node --config config.json

# You should see:
# ✓ Configured PostgreSQL persistence for node: tvs_node_1
```

### Testing Ephemeral Mode

```bash
# Default build uses ephemeral
cargo build

./target/debug/tvs_node --config config.json

# You should see:
# ✓ Configured ephemeral (in-memory) persistence for node: tvs_node_1
```

### Switching Between Modes

Since persistence backend is selected at compile time via Cargo features, you need to rebuild:

```bash
# Switch to postgres
cargo clean
cargo build --features postgres --no-default-features

# Switch back to ephemeral
cargo clean
cargo build
```