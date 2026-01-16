#!/bin/bash
# Cider Relay Server - Installation Script
# Run as root or with sudo

set -e

INSTALL_DIR="/opt/cider-relay"
SERVICE_USER="cider-relay"
BINARY_NAME="cider-relay"

echo "═══════════════════════════════════════════════════════════════"
echo "           Cider Relay Server - Installation"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root or with sudo"
    exit 1
fi

# Create service user if it doesn't exist
if ! id "$SERVICE_USER" &>/dev/null; then
    echo "[1/5] Creating service user '$SERVICE_USER'..."
    useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"
else
    echo "[1/5] Service user '$SERVICE_USER' already exists"
fi

# Create install directory
echo "[2/5] Creating install directory..."
mkdir -p "$INSTALL_DIR"

# Copy binary
echo "[3/5] Installing binary..."
BINARY_PATH=""

# Check common locations
if [ -f "./$BINARY_NAME" ]; then
    BINARY_PATH="./$BINARY_NAME"
elif [ -f "../target/release/$BINARY_NAME" ]; then
    BINARY_PATH="../target/release/$BINARY_NAME"
elif [ -f "../../target/release/$BINARY_NAME" ]; then
    # Workspace root (when relay-server is part of a workspace)
    BINARY_PATH="../../target/release/$BINARY_NAME"
elif [ -f "./target/release/$BINARY_NAME" ]; then
    BINARY_PATH="./target/release/$BINARY_NAME"
fi

if [ -z "$BINARY_PATH" ]; then
    echo "ERROR: Binary not found. Searched in:"
    echo "  ./$BINARY_NAME"
    echo "  ../target/release/$BINARY_NAME"
    echo "  ../../target/release/$BINARY_NAME"
    echo "  ./target/release/$BINARY_NAME"
    echo ""
    echo "Please build first with:"
    echo "  cd ~/relay-server && cargo build --release"
    echo ""
    echo "Or copy the binary to this directory:"
    echo "  cp /path/to/cider-relay ."
    exit 1
fi

echo "  Found binary at: $BINARY_PATH"
cp "$BINARY_PATH" "$INSTALL_DIR/"

chmod +x "$INSTALL_DIR/$BINARY_NAME"
chown -R "$SERVICE_USER:$SERVICE_USER" "$INSTALL_DIR"

# Install systemd service
echo "[4/5] Installing systemd service..."
cp cider-relay.service /etc/systemd/system/
systemctl daemon-reload

# Enable and start service
echo "[5/5] Enabling and starting service..."
systemctl enable cider-relay
systemctl start cider-relay

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "                    Installation Complete!"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "  Useful commands:"
echo "    systemctl status cider-relay    # Check status"
echo "    journalctl -u cider-relay -f    # View logs"
echo "    systemctl restart cider-relay   # Restart"
echo "    systemctl stop cider-relay      # Stop"
echo ""
echo "  Configuration:"
echo "    Edit /etc/systemd/system/cider-relay.service"
echo "    Then: systemctl daemon-reload && systemctl restart cider-relay"
echo ""

# Show initial status
sleep 2
systemctl status cider-relay --no-pager || true
