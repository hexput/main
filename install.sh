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

REMOTE_VERSION=""
DOWNLOAD_URL=""

fetch_release_info() {
  echo "📡 Fetching latest release info for $REPO..."
  local api_json
  if ! api_json=$(curl -fsSL -H "User-Agent: hexput-installer" "https://api.github.com/repos/$REPO/releases/latest"); then
    echo "❌ Failed to fetch release metadata"
    exit 1
  fi
  if command -v jq >/dev/null 2>&1; then
    REMOTE_VERSION=$(echo "$api_json" | jq -r '.tag_name // empty')
    DOWNLOAD_URL=$(echo "$api_json" | jq -r --arg arch "$ARCH" '.assets[] | select(.browser_download_url | contains($arch)) | .browser_download_url' | head -n1)
  else
    # Fallback parsing without jq (more fragile)
    REMOTE_VERSION=$(printf '%s\n' "$api_json" | grep -E '"tag_name"' | head -n1 | sed -E 's/.*"tag_name" *: *"([^"]+)".*/\1/')
    DOWNLOAD_URL=$(printf '%s\n' "$api_json" | grep 'browser_download_url' | grep "$ARCH" | head -n1 | sed -E 's/.*"(https:[^"]+)".*/\1/')
  fi
  if [ -z "${DOWNLOAD_URL:-}" ]; then
    echo "❌ Could not find a release asset for architecture: $ARCH"
    echo "(Tip: assets in release must contain substring '$ARCH')"
    exit 1
  fi
  if [ -z "${REMOTE_VERSION:-}" ]; then
    echo "⚠️  Could not determine remote version (tag_name). Version tracking may be skipped."
  fi
}

get_local_version() {
  if [ -f "$VERSION_FILE" ]; then
    LOCAL_VERSION=$(cat "$VERSION_FILE" 2>/dev/null || true)
  else
    LOCAL_VERSION=""
  fi
}

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

download_and_deploy_binary() {
  echo "⬇️  Downloading binary (version: ${REMOTE_VERSION:-unknown})..."
  local tmpfile
  tmpfile=$(mktemp /tmp/hexput-runtime.XXXXXX)
  trap 'rm -f "$tmpfile"' EXIT
  if ! curl -L --fail -H "User-Agent: hexput-installer" "$DOWNLOAD_URL" -o "$tmpfile"; then
    echo "❌ Download failed"
    exit 1
  fi
  # Basic integrity checks
  if [ ! -s "$tmpfile" ]; then
    echo "❌ Downloaded file is empty"
    exit 1
  fi
  # Validate temp binary BEFORE replacing
  if ! validate_temp_binary "$tmpfile"; then
    echo "❌ Validation of downloaded artifact failed (not installing)."
    return 1
  fi
  chmod +x "$tmpfile"
  echo "📦 Replacing $BIN_PATH atomically..."
  sudo mv "$tmpfile" "$BIN_PATH"
  sudo chmod 0755 "$BIN_PATH" || true
  sudo chown root:root "$BIN_PATH" || true
  trap - EXIT
  # Persist version
  if [ -n "${REMOTE_VERSION:-}" ]; then
    if [ ! -d "$VERSION_STATE_DIR" ]; then
      sudo mkdir -p "$VERSION_STATE_DIR"
    fi
    echo "$REMOTE_VERSION" | sudo tee "$VERSION_FILE" >/dev/null
  fi
}

validate_binary() {
  if [ ! -x "$BIN_PATH" ]; then
    echo "❌ Binary not executable after install"
    return 1
  fi
  # If 'file' is available, ensure it's an ELF binary
  if command -v file >/dev/null 2>&1; then
    if ! file "$BIN_PATH" | grep -q "ELF 64-bit"; then
      echo "❌ Installed file does not appear to be an ELF 64-bit binary"
      return 1
    fi
  fi
  # Try running --version (should exit 0 quickly)
  if ! "$BIN_PATH" --version >/dev/null 2>&1; then
    echo "❌ Executing binary for version check failed"
    return 1
  fi
  return 0
}

validate_temp_binary() {
  # Arg: path to temp file
  local f="$1"
  # Magic bytes check for ELF (0x7f 'E' 'L' 'F')
  if head -c 4 "$f" | grep -q $'\x7fELF'; then
    return 0
  fi
  # Show first few bytes for debugging (safe printable)
  echo "---- File head (hex) ----"
  hexdump -C "$f" | head -n3 || true
  echo "-------------------------"
  return 1
}

fetch_release_info
get_local_version

if [ -n "${REMOTE_VERSION:-}" ] && [ -n "${LOCAL_VERSION:-}" ] && [ "$REMOTE_VERSION" = "$LOCAL_VERSION" ] && [ "$FORCE_UPDATE" = false ]; then
  echo "ℹ️  Already up-to-date (version $LOCAL_VERSION). Use --force to reinstall."
  UP_TO_DATE=1
else
  # Ensure directory exists (normally /usr/local/bin already exists)
  if [ ! -d "$(dirname "$BIN_PATH")" ]; then
    echo "📁 Creating directory $(dirname "$BIN_PATH")"
    sudo mkdir -p "$(dirname "$BIN_PATH")"
  fi
  stop_running_instances
  download_and_deploy_binary
  UP_TO_DATE=0
fi

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
      if [ "${UP_TO_DATE:-0}" -eq 1 ]; then
        echo "✅ Service already running with latest version (no restart needed)."
      else
        echo "🔄 Service is running — restarting..."
        sudo systemctl restart "$SERVICE_NAME"
      fi
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
