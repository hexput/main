#!/usr/bin/env bash
set -e

BIN_PATH="/usr/local/bin/hexput-runtime"
SERVICE_NAME="hexput-runtime.service"
SERVICE_PATH="/etc/systemd/system/$SERVICE_NAME"
DOWNLOAD_URL="https://github.com/hexput/main/releases/download/dev-20250731-093856/hexput-runtime-x86_64-unknown-linux-gnu"

sudo curl -L "$DOWNLOAD_URL" -o "$BIN_PATH"
sudo chmod +x "$BIN_PATH"

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

sudo systemctl daemon-reload
sudo systemctl enable "$SERVICE_NAME"
sudo systemctl start "$SERVICE_NAME"
