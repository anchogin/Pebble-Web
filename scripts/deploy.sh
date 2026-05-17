#!/usr/bin/env bash
set -Eeuo pipefail

REPO_URL="${PEBBLE_REPO_URL:-https://github.com/QingJ01/Pebble-Web.git}"
BRANCH="${PEBBLE_BRANCH:-master}"
INSTALL_DIR="${PEBBLE_INSTALL_DIR:-${HOME:-}/pebble-web}"
PEBBLE_PORT="${PEBBLE_PORT:-8080}"
PEBBLE_SYNC_INTERVAL="${PEBBLE_SYNC_INTERVAL:-300}"

log() {
  printf '\033[1;34m==>\033[0m %s\n' "$*"
}

fail() {
  printf '\033[1;31merror:\033[0m %s\n' "$*" >&2
  exit 1
}

random_alnum() {
  local length="$1"
  local byte_count="$(((length + 1) / 2))"
  local value
  if command -v openssl >/dev/null 2>&1; then
    value="$(openssl rand -hex "$byte_count")"
  else
    value="$(od -An -N "$byte_count" -tx1 -v /dev/urandom | tr -d ' \n')"
  fi
  printf '%s' "${value:0:length}"
}

compose_cmd() {
  if docker compose version >/dev/null 2>&1; then
    printf 'docker compose'
  elif command -v docker-compose >/dev/null 2>&1; then
    printf 'docker-compose'
  else
    return 1
  fi
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"
}

if [ -z "$INSTALL_DIR" ]; then
  fail "PEBBLE_INSTALL_DIR is empty and HOME is not set"
fi

case "$PEBBLE_PORT" in
  ''|*[!0-9]*) fail "PEBBLE_PORT must be a number" ;;
esac

require_command git
require_command docker

COMPOSE="$(compose_cmd)" || fail "Docker Compose is required. Install the Docker Compose plugin or docker-compose."

log "Preparing Pebble Web in $INSTALL_DIR"
if [ ! -e "$INSTALL_DIR" ]; then
  git clone --branch "$BRANCH" "$REPO_URL" "$INSTALL_DIR"
elif [ -d "$INSTALL_DIR/.git" ]; then
  git -C "$INSTALL_DIR" fetch origin "$BRANCH"
  git -C "$INSTALL_DIR" checkout "$BRANCH"
  git -C "$INSTALL_DIR" pull --ff-only origin "$BRANCH"
else
  fail "$INSTALL_DIR already exists but is not a git repository"
fi

cd "$INSTALL_DIR"

generated_password=""
if [ ! -f .env ]; then
  log "Creating .env"
  umask 077
  generated_password="${PEBBLE_PASSWORD:-$(random_alnum 24)}"
  jwt_secret="${PEBBLE_JWT_SECRET:-$(random_alnum 64)}"
  cat >.env <<EOF
PEBBLE_PASSWORD=$generated_password
PEBBLE_JWT_SECRET=$jwt_secret

PEBBLE_DATA_DIR=/data
PEBBLE_PORT=$PEBBLE_PORT
PEBBLE_SYNC_INTERVAL=$PEBBLE_SYNC_INTERVAL
PEBBLE_ENCRYPTION_KEY=
EOF
else
  log "Using existing .env"
  if grep -Eq '^PEBBLE_PASSWORD=(your-password-here|changeme)?$' .env; then
    fail ".env contains an insecure PEBBLE_PASSWORD placeholder. Edit .env and rerun."
  fi
  if grep -Eq '^PEBBLE_JWT_SECRET=(your-random-secret-at-least-32-chars|change-this-to-a-random-string|generate-a-random-string-here)?$' .env; then
    fail ".env contains an insecure PEBBLE_JWT_SECRET placeholder. Edit .env and rerun."
  fi
fi

log "Building and starting containers"
# shellcheck disable=SC2086
$COMPOSE up -d --build

log "Deployment complete"
printf 'URL: http://localhost:%s\n' "$PEBBLE_PORT"
printf 'Directory: %s\n' "$INSTALL_DIR"
if [ -n "$generated_password" ]; then
  printf 'Generated login password: %s\n' "$generated_password"
  printf 'The password is also saved in %s/.env\n' "$INSTALL_DIR"
fi
