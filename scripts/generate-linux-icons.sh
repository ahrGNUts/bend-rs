#!/bin/sh
# Regenerate Linux icon sizes from source icon (requires ImageMagick)
# Run this when assets/icon.png changes
SOURCE="assets/icon.png"
OUTDIR="assets/linux"
mkdir -p "$OUTDIR"
cp assets/icon_32x32.png "$OUTDIR/32x32.png"
for size in 48 128 256; do
    magick "$SOURCE" -resize ${size}x${size} "$OUTDIR/${size}x${size}.png"
done
cp "$SOURCE" "$OUTDIR/512x512.png"
