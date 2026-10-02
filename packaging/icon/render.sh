#!/usr/bin/env bash
# Rebuilds every icon file from icon.svg. Needs macOS and ImageMagick.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# ImageMagick's own SVG renderer drops gradients and strokes, so Quick Look draws it.
qlmanage -t -s 1024 -o "$tmp" "$here/icon.svg" > /dev/null
# Quick Look paints the corners white. This mask matches the rect in icon.svg.
magick "$tmp/icon.svg.png" \
  \( -size 1024x1024 xc:black -fill white -draw "roundrectangle 100,100 923,923 185,185" \) \
  -alpha off -compose CopyOpacity -composite "$tmp/icon.png"

magick "$tmp/icon.png" -resize 256x256 "$here/icon-256.png"

iconset="$tmp/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  magick "$tmp/icon.png" -resize "${size}x${size}" "$iconset/icon_${size}x${size}.png"
  magick "$tmp/icon.png" -resize "$((size * 2))x$((size * 2))" "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$here/../macos/AppIcon.icns"

magick "$tmp/icon.png" -define icon:auto-resize=256,64,48,32,24,16 "$here/../windows/app.ico"
