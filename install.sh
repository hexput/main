#!/usr/bin/env bash
set -euo pipefail

# Config
REPO="hexput/main"
BIN_PATH="/usr/local/bin/hexput-runtime"
SERVICE_NAME="hexput-runtime.service"
SERVICE_PATH="/etc/systemd/system/$SERVICE_NAME"
ARCH="x86_64-unknown-linux-gnu"
VERSION_STATE_DIR="/var/lib/hexput-runtime"
VERSION_FILE="$VERSION_STATE_DIR/version"
FORCE_UPDATE=false

for arg in "$@"; do
  case "$arg" in
    -f|--force)
      FORCE_UPDATE=true
      shift
      ;;
    -h|--help)
      cat <<USAGE
Hexput installer/updater
Usage: $0 [options]
  -f, --force   Force update even if version matches
  -h, --help    Show this help
USAGE
      exit 0
      ;;
  esac
done

stop_running_instances() {
  # Stop systemd service if active
  if command -v systemctl >/dev/null 2>&1; then
    if systemctl is-active --quiet "$SERVICE_NAME"; then
      echo "⏹ Stopping running service ($SERVICE_NAME)..."
      sudo systemctl stop "$SERVICE_NAME" || true
    fi
  fi

  # Gracefully terminate any remaining processes using the binary
  if pgrep -f "hexput-runtime" >/dev/null 2>&1; then
    echo "⚠️  Runtime process still running — sending SIGTERM..."
    pkill -f "hexput-runtime" || true
    # Wait up to 5 seconds for graceful exit
    for i in {1..10}; do
      if ! pgrep -f "hexput-runtime" >/dev/null 2>&1; then
        break
      fi
      sleep 0.5
    done
    if pgrep -f "hexput-runtime" >/dev/null 2>&1; then
      echo "⛔ Force killing remaining process (SIGKILL)..."
      pkill -9 -f "hexput-runtime" || true
    fi
  fi
}

download_latest_binary() {
  echo "📡 Fetching latest release info for $REPO..."
  local api_json
  if ! api_json=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest"); then
    echo "❌ Failed to fetch release metadata"
    exit 1
  fi

  # Extract tag_name and asset URL
  REMOTE_VERSION=$(echo "$api_json" | grep -m1 '"tag_name"' | cut -d '"' -f4)
  local download_url
  download_url=$(echo "$api_json" | grep "browser_download_url" | grep "$ARCH" | cut -d '"' -f 4 | head -n1)

  if [ -z "${download_url:-}" ]; then
    echo "❌ Could not find a release asset for architecture: $ARCH"
    exit 1
  fi
  if [ -z "${REMOTE_VERSION:-}" ]; then
    echo "⚠️  Could not determine remote version (tag_name). Proceeding but version tracking may be skipped."
  fi

  # Compare with local version if exists
  if [ -f "$VERSION_FILE" ]; then
    LOCAL_VERSION=$(cat "$VERSION_FILE" 2>/dev/null || true)
  else
    LOCAL_VERSION=""
  fi

  if [ -n "${REMOTE_VERSION:-}" ] && [ -n "${LOCAL_VERSION:-}" ] && [ "$REMOTE_VERSION" = "$LOCAL_VERSION" ] && [ "$FORCE_UPDATE" = false ]; then
    echo "ℹ️  Local version ($LOCAL_VERSION) is already the latest ($REMOTE_VERSION). Skipping update. Use --force to override."
    SKIP_DOWNLOAD=true
    return 0
  fi

  echo "Latest release asset found (version: ${REMOTE_VERSION:-unknown}):"
  echo "$download_url"

  # Download to temp file first to avoid ETXTBUSY, then atomic move
  local tmpfile
  tmpfile=$(mktemp /tmp/hexput-runtime.XXXXXX)
  trap 'rm -f "$tmpfile"' EXIT
  echo "⬇️  Downloading binary to temp file..."
  curl -L --fail "$download_url" -o "$tmpfile"
  chmod +x "$tmpfile"
  echo "📦 Replacing $BIN_PATH atomically..."
  sudo mv "$tmpfile" "$BIN_PATH"
  # Remove trap for tmpfile since it has been moved
  trap - EXIT

  # Write version file
  if [ -n "${REMOTE_VERSION:-}" ]; then
    if [ ! -d "$VERSION_STATE_DIR" ]; then
      sudo mkdir -p "$VERSION_STATE_DIR"
    fi
    echo "$REMOTE_VERSION" | sudo tee "$VERSION_FILE" >/dev/null
  fi
}

# Ensure directory exists (normally /usr/local/bin already exists)
if [ ! -d "$(dirname "$BIN_PATH")" ]; then
  echo "📁 Creating directory $(dirname "$BIN_PATH")"
  sudo mkdir -p "$(dirname "$BIN_PATH")"
fi

# Stop any running instances before replacement to avoid 'Text file busy'.
stop_running_instances

SKIP_DOWNLOAD=false
download_latest_binary

# Create/overwrite systemd service
echo "🛠 Writing systemd service file..."
sudo tee "$SERVICE_PATH" > /dev/null <<EOF
[Unit]
Description=Hexput Runtime Service
After=network.target

[Service]
Type=simple
ExecStart=$BIN_PATH
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

if command -v systemctl >/dev/null 2>&1; then
  # Reload systemd to pick up changes
  echo "🔄 Reloading systemd..."
  sudo systemctl daemon-reload

  # Check if service exists now (we just wrote it)
  if systemctl list-unit-files | grep -q "^$SERVICE_NAME"; then
    if systemctl is-active --quiet "$SERVICE_NAME"; then
      echo "🔄 Service is running — restarting..."
      sudo systemctl restart "$SERVICE_NAME"
    else
      echo "▶️ Service installed — enabling and starting..."
      sudo systemctl enable "$SERVICE_NAME"
      sudo systemctl start "$SERVICE_NAME"
    fi
  else
    echo "🚀 Enabling and starting new service..."
    sudo systemctl enable "$SERVICE_NAME"
    sudo systemctl start "$SERVICE_NAME"
  fi
else
  echo "⚠️ systemctl not found — skipping service management. Binary installed at $BIN_PATH"
fi

echo "✅ Hexput installed/updated and service is running."
