#!/bin/bash
# Build Kevro in release mode and wrap it in a menu-bar-only .app bundle.
#
# Output: dist/Kevro.app (ad-hoc signed). Copy it to /Applications yourself.
set -euo pipefail

cd "$(dirname "$0")/.."

BUNDLE_ID="io.github.lenard-0.kevro"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
APP="dist/Kevro.app"

cargo build --release

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/kevro "$APP/Contents/MacOS/kevro"

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Kevro</string>
    <key>CFBundleDisplayName</key>
    <string>Kevro</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleExecutable</key>
    <string>kevro</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

# Ad-hoc signature. macOS ties the Accessibility grant to this signature, so
# after every rebuild you may need to re-grant permission (see README).
codesign --force --sign - --identifier "$BUNDLE_ID" "$APP"

echo "Built $APP (version $VERSION)"
