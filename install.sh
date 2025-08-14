#!/usr/bin/env bash
set -e

# Config
REPO="hexput/main"
BIN_PATH="/usr/local/bin/hexput-runtime"
SERVICE_NAME="hexput-runtime.service"
SERVICE_PATH="/etc/systemd/system/$SERVICE_NAME"
ARCH="x86_64-unknown-linux-gnu"

# Get latest release download URL from GitHub API
echo "📡 Fetching latest release info for $REPO..."
DOWNLOAD_URL=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" \
  | grep "browser_download_url" \
  | grep "$ARCH" \
  | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
  echo "❌ Could not find a release asset for architecture: $ARCH"
  exit 1
fi

echo "Latest release asset found:"
echo "$DOWNLOAD_URL"

# Download binary
echo "⬇️  Downloading binary to $BIN_PATH..."
sudo curl -L "$DOWNLOAD_URL" -o "$BIN_PATH"
sudo chmod +x "$BIN_PATH"

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

# Reload systemd to pick up changes
echo "🔄 Reloading systemd..."
sudo systemctl daemon-reload

# Check if service exists
if systemctl list-unit-files | grep -q "^$SERVICE_NAME"; then
  # Service exists — check if running
  if systemctl is-active --quiet "$SERVICE_NAME"; then
    echo "🔄 Service is running — restarting..."
    sudo systemctl restart "$SERVICE_NAME"
  else
    echo "▶️ Service is installed but not running — enabling and starting..."
    sudo systemctl enable "$SERVICE_NAME"
    sudo systemctl start "$SERVICE_NAME"
  fi
else
  # Service is new — enable and start
  echo "🚀 Enabling and starting new service..."
  sudo systemctl enable "$SERVICE_NAME"
  sudo systemctl start "$SERVICE_NAME"
fi

echo "✅ Hexput installed/updated and service is running."
