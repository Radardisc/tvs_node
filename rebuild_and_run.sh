#!/bin/bash
# Complete rebuild script for TVS Node
# Rebuilds binary, Docker image, and restarts container

set -e

# Configuration
BASE_PORT=${1:-10000}
CONFIG_FILE=${2:-configs/node1.json}
NODE_NAME=${3:-tvs_node_${BASE_PORT}}
FEATURES=${4:-ephemeral}
CONTAINER_NAME="tvs_${BASE_PORT}"

echo "================================================"
echo "TVS Node Complete Rebuild"
echo "================================================"
echo "Base Port:      $BASE_PORT"
echo "Config:         $CONFIG_FILE"
echo "Node Name:      $NODE_NAME"
echo "Features:       $FEATURES"
echo "Container:      $CONTAINER_NAME"
echo "================================================"
echo ""

# Step 1: Build the binary
echo "Step 1/4: Building Rust binary..."
echo "Command: cargo build --release --no-default-features --features $FEATURES"
cargo build --release --no-default-features --features "$FEATURES"
if [ $? -ne 0 ]; then
    echo "❌ Binary build failed!"
    exit 1
fi
echo "✅ Binary built successfully"
echo ""

# Step 3: Stop and remove old container
echo "Step 2/4: Cleaning up old container..."
if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo "Stopping and removing $CONTAINER_NAME..."
    docker stop $CONTAINER_NAME 2>/dev/null || true
    docker rm $CONTAINER_NAME 2>/dev/null || true
    echo "✅ Old container removed"
else
    echo "ℹ️  No existing container found"
fi
echo ""

# Step 2: Build Docker image
echo "Step 3/4: Building Docker image..."
cd ..
docker build -f tvs_node/Dockerfile.local -t tvs_node:local .
if [ $? -ne 0 ]; then
    echo "❌ Docker build failed!"
    exit 1
fi
echo "✅ Docker image built successfully"
echo ""



# Step 4: Start new container
echo "Step 4/4: Starting new container..."
cd tvs_node
./run_tvs_docker_node.sh "$BASE_PORT" "$CONFIG_FILE" "$NODE_NAME"

echo ""
echo "================================================"
echo "✅ Rebuild complete!"
echo "================================================"
echo ""
echo "Container is running. Logs are attached above."
echo "Press Ctrl+C to detach from logs (container keeps running)"
echo ""
