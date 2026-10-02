#!/usr/bin/env bash
# Usage: bundle.sh <gui-binary> <output-dir>
set -euo pipefail

binary=$1
out=$2
here=$(cd "$(dirname "$0")" && pwd)
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$here/../../Cargo.toml" | head -n 1)
app="$out/Simple Confidence Monitor.app"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$binary" "$app/Contents/MacOS/simple-confidence-monitor-gui"
sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist" > /dev/null
# The linker signs only the binary. Signing the bundle puts Info.plist inside the seal too.
codesign --force --sign - "$app"
echo "$app"
