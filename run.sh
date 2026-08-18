#!/usr/bin/env bash

set -Eeuo pipefail

# Resolve all paths relative to this script so it works from any directory.
PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_DIR="$PROJECT_DIR/backend"
FRONTEND_DIR="$PROJECT_DIR/frontend"
ENV_FILE="$PROJECT_DIR/.env"
COMPOSE_FILE="$PROJECT_DIR/docker-compose.yml"
FRONTEND_URL="http://localhost:5174"

backend_pid=""
frontend_pid=""
browser_wait_pid=""

cleanup() {
    trap - INT TERM EXIT

    # Do not display a shutdown message if nothing was started.
    if [[ -z "$backend_pid" && -z "$frontend_pid" ]]; then
        return
    fi

    printf '\nStopping ShillTrace...\n'

    # Stop the background browser-wait process if it is still running.
    if [[ -n "$browser_wait_pid" ]]; then
        kill "$browser_wait_pid" 2>/dev/null || true
    fi

    # Stop only the frontend and backend started by this script.
    # PostgreSQL remains running so its data is preserved.
    if [[ -n "$frontend_pid" ]]; then
        kill "$frontend_pid" 2>/dev/null || true
    fi

    if [[ -n "$backend_pid" ]]; then
        kill "$backend_pid" 2>/dev/null || true
    fi

    wait "$browser_wait_pid" 2>/dev/null || true
    wait "$frontend_pid" 2>/dev/null || true
    wait "$backend_pid" 2>/dev/null || true

    printf 'ShillTrace stopped.\n'
}

trap cleanup INT TERM EXIT

# Check that all required programs are installed.
for command_name in docker cargo npm; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'Error: required command "%s" is not installed.\n' \
            "$command_name" >&2
        exit 1
    fi
done

# Check that the Docker Compose plugin is available.
if ! docker compose version >/dev/null 2>&1; then
    printf 'Error: Docker Compose plugin is not installed.\n' >&2
    exit 1
fi

# The project uses one shared .env file located in the project root.
if [[ ! -f "$ENV_FILE" ]]; then
    printf 'Error: missing %s\n' "$ENV_FILE" >&2
    exit 1
fi

# Export variables from the root .env so the backend receives them even though
# Cargo is started from inside the backend directory.
set -a
source "$ENV_FILE"
set +a

printf 'Starting PostgreSQL...\n'
docker compose -f "$COMPOSE_FILE" up -d postgres

printf 'Waiting for PostgreSQL'

postgres_ready=false

for _ in {1..30}; do
    if docker compose -f "$COMPOSE_FILE" exec -T postgres \
        pg_isready -U signal_ledger >/dev/null 2>&1; then
        postgres_ready=true
        printf ' ready.\n'
        break
    fi

    printf '.'
    sleep 1
done

if [[ "$postgres_ready" != true ]]; then
    printf '\nError: PostgreSQL did not become ready in time.\n' >&2
    exit 1
fi

# Install frontend dependencies only when they are not already installed.
if [[ ! -d "$FRONTEND_DIR/node_modules" ]]; then
    printf 'Installing frontend dependencies...\n'
    npm --prefix "$FRONTEND_DIR" install
fi

printf 'Starting backend...\n'
(
    cd "$BACKEND_DIR"
    cargo run
) &
backend_pid=$!

printf 'Starting frontend...\n'
npm --prefix "$FRONTEND_DIR" run dev &
frontend_pid=$!

# Wait until Vite is reachable and then open the application in the browser.
# Browser opening is optional and does not prevent the application from running.
if command -v curl >/dev/null 2>&1; then
    (
        for _ in {1..60}; do
            if curl --silent --fail "$FRONTEND_URL" >/dev/null 2>&1; then
                if command -v xdg-open >/dev/null 2>&1; then
                    xdg-open "$FRONTEND_URL" >/dev/null 2>&1 || true
                fi

                exit 0
            fi

            sleep 1
        done
    ) &

    browser_wait_pid=$!
fi

printf '\nShillTrace is starting at %s\n' "$FRONTEND_URL"
printf 'Press Ctrl+C to stop the backend and frontend.\n\n'

# If either the backend or frontend exits, stop the remaining processes.
wait -n "$backend_pid" "$frontend_pid"