#!/bin/sh
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"

rm -f "$DATA_DIR/applications/com.bend.databending.desktop"

for size in 32 48 128 256 512; do
    rm -f "$DATA_DIR/icons/hicolor/${size}x${size}/apps/com.bend.databending.png"
done

# Update caches (best-effort)
gtk-update-icon-cache -f -t "$DATA_DIR/icons/hicolor/" 2>/dev/null || true
update-desktop-database "$DATA_DIR/applications/" 2>/dev/null || true

echo "Uninstalled successfully."
