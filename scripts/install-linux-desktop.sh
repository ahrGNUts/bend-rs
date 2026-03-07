#!/bin/sh
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"

# Check if bend-rs is in PATH
if ! command -v bend-rs >/dev/null 2>&1; then
    echo "Warning: 'bend-rs' is not in your \$PATH."
    echo "The .desktop file's Exec=bend-rs will not work until you either:"
    echo "  - Run: cargo install --path $PROJECT_DIR"
    echo "  - Or copy the binary to a directory in your \$PATH (e.g., ~/.local/bin/)"
    echo ""
fi

# Install .desktop file
install -Dm644 "$PROJECT_DIR/assets/com.bend.databending.desktop" \
    "$DATA_DIR/applications/com.bend.databending.desktop"

# Install icons into hicolor theme at multiple sizes
for size in 32 48 128 256 512; do
    install -Dm644 "$PROJECT_DIR/assets/linux/${size}x${size}.png" \
        "$DATA_DIR/icons/hicolor/${size}x${size}/apps/com.bend.databending.png"
done

# Update caches (best-effort — not all DEs have/need these)
gtk-update-icon-cache -f -t "$DATA_DIR/icons/hicolor/" 2>/dev/null || true
update-desktop-database "$DATA_DIR/applications/" 2>/dev/null || true

echo "Installed successfully."
echo "You may need to log out and back in for icon changes to take effect."
