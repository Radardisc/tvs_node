# TVS Node Quick Start Guide

## Building

### For Development (In-Memory Storage)

```bash
# Default feature (ephemeral)
cargo build -p tvs_node

# Run with default config
./target/debug/tvs_node

# Run with custom config
./target/debug/tvs_node --config my_config.json
```

### For Production (PostgreSQL Storage)

```bash
# Build with postgres feature
cargo build -p tvs_node --features postgres --no-default-features --release

# Set database URL
export POSTGRES_DATABASE_URL="postgres://user:password@localhost/tvs_db"

# Run migrations (first time only)
cd ../persistence_plugins/tvs_postgres
diesel migration run
cd ../../tvs_node

# Run the node
./target/release/tvs_node --config production_config.json
```

## Configuration Files

See `config.example.json` for a template.

## Testing Both Modes

```bash
# Test ephemeral mode
cargo test -p tvs_node

# Test postgres mode (requires running PostgreSQL)
export POSTGRES_DATABASE_URL="postgres://localhost/tvs_test_db"
cargo test -p tvs_node --features postgres --no-default-features
```

## How It Works

The binary uses **Cargo features** to select the persistence backend at **compile time**:

- **ephemeral** feature → Uses `tvs::services::EphemeralVoteService` (in-memory HashMaps)
- **postgres** feature → Uses `tvs_postgres::PostgresVoteService` (PostgreSQL via Diesel)

No runtime overhead - unused code is completely removed from the binary.

## Architecture

```
tvs_node
├─ Cargo features select backend
│
├─ [ephemeral] → tvs::EphemeralVoteService
│                └── In-memory HashMap storage
│
└─ [postgres]  → tvs_postgres::PostgresVoteService
                 └── PostgreSQL via Diesel ORM
                     └── Uses tfs_postgres::DbPool
```

## Switching Backends

Simply rebuild with different features - no code changes needed!

```bash
# Switch from ephemeral to postgres
cargo clean -p tvs_node
cargo build -p tvs_node --features postgres --no-default-features
```