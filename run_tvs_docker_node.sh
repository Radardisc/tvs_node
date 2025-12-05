#!/bin/bash
# Script to run a TVS node in Docker with configurable base port
#
# Usage:
#   ./run_tvs_docker_node.sh [base_port] [config_file] [node_name]
#
# Examples:
#   ./run_tvs_docker_node.sh 10000 configs/node1.json node_1
#   ./run_tvs_docker_node.sh 20000 configs/node2.json node_2

set -e

# Default values
BASE_PORT=${1:-10000}
CONFIG_FILE=${2:-configs/node1.json}
NODE_NAME=${3:-tvs_node_${BASE_PORT}}

# Calculate ports based on base port
CLUSTER_PORT=$BASE_PORT
APP_PORT=$((BASE_PORT + 1))
ADMIN_PORT=$((BASE_PORT + 2))
VOTE_PORT=$((BASE_PORT + 3))

# Container name
CONTAINER_NAME="tvs_${BASE_PORT}"

echo "================================================"
echo "Starting TVS Node"
echo "================================================"
echo "Container:      $CONTAINER_NAME"
echo "Node Name:      $NODE_NAME"
echo "Config:         $CONFIG_FILE"
echo "Base Port:      $BASE_PORT"
echo "Cluster Port:   $CLUSTER_PORT"
echo "App Port:       $APP_PORT"
echo "Admin Port:     $ADMIN_PORT"
echo "Vote Port:      $VOTE_PORT"
echo "================================================"

# Check if container already exists
if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo "Container $CONTAINER_NAME already exists. Removing..."
    docker stop $CONTAINER_NAME 2>/dev/null || true
    docker rm $CONTAINER_NAME 2>/dev/null || true
fi

# Check if config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Config file not found: $CONFIG_FILE"
    exit 1
fi

# Run the container
docker run -d \
  --name "$CONTAINER_NAME" \
  -p "${CLUSTER_PORT}:${CLUSTER_PORT}" \
  -p "${APP_PORT}:${APP_PORT}" \
  -p "${ADMIN_PORT}:${ADMIN_PORT}" \
  -p "${VOTE_PORT}:${VOTE_PORT}" \
  -e CLUSTER_MESSAGE_PORT="$CLUSTER_PORT" \
  -e APP_PORT="$APP_PORT" \
  -e ADMIN_PORT="$ADMIN_PORT" \
  -e TVS_VOTE_PORT="$VOTE_PORT" \
  -e TVS_VOTE_HOST="0.0.0.0" \
  -e NODE_NAME="$NODE_NAME" \
  -e LOG_JSON="false" \
  -e LOG_DISABLE_FILE="true" \
  -e LOG_LEVEL="info,tower_http=debug,tvs=debug" \
  -v "$(pwd)/$CONFIG_FILE:/app/config.json:ro" \
  --health-cmd "curl -f http://localhost:${VOTE_PORT}/health || exit 1" \
  --health-interval 30s \
  --health-timeout 10s \
  --health-retries 3 \
  --health-start-period 40s \
  tvs_node:local

echo ""
echo "Container started successfully!"
echo ""
echo "Endpoints:"
echo "  Vote Service: http://localhost:${VOTE_PORT}"
echo "  HTTP API:     http://localhost:${APP_PORT}"
echo "  Health:       http://localhost:${VOTE_PORT}/health"
echo ""
echo "Commands:"
echo "  View logs:    docker logs -f $CONTAINER_NAME"
echo "  Check status: docker ps | grep $CONTAINER_NAME"
echo "  Stop:         docker stop $CONTAINER_NAME"
echo "  Remove:       docker rm $CONTAINER_NAME"
echo ""
echo "Attaching to container logs (Ctrl+C to detach)..."
echo ""

# Follow logs
docker logs -f $CONTAINER_NAME
