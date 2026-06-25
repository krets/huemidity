#!/bin/bash
set -e

# Change directory to the script location
cd "$(dirname "$0")"

echo "Building huemidity in release mode..."
cargo build --release

echo "Creating HueMIDIty.app structure..."
APP_DIR="target/release/HueMIDIty.app"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

echo "Copying binary..."
cp target/release/huemidity "$APP_DIR/Contents/MacOS/huemidity"

echo "Creating Info.plist..."
cat << 'EOF' > "$APP_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>huemidity</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
    <key>CFBundleIdentifier</key>
    <string>com.huemidity.app</string>
    <key>CFBundleName</key>
    <string>HueMIDIty</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSLocalNetworkUsageDescription</key>
    <string>HueMIDIty needs access to your local network to connect to the Philips Hue Bridge.</string>
</dict>
</plist>
EOF

echo "Generating icon.icns from resources/icon_512.png..."
ICON_DIR="target/icon.iconset"
mkdir -p "$ICON_DIR"

sips -z 16 16 resources/icon_512.png --out "$ICON_DIR/icon_16x16.png" > /dev/null 2>&1
sips -z 32 32 resources/icon_512.png --out "$ICON_DIR/icon_16x16@2x.png" > /dev/null 2>&1
sips -z 32 32 resources/icon_512.png --out "$ICON_DIR/icon_32x32.png" > /dev/null 2>&1
sips -z 64 64 resources/icon_512.png --out "$ICON_DIR/icon_32x32@2x.png" > /dev/null 2>&1
sips -z 128 128 resources/icon_512.png --out "$ICON_DIR/icon_128x128.png" > /dev/null 2>&1
sips -z 256 256 resources/icon_512.png --out "$ICON_DIR/icon_128x128@2x.png" > /dev/null 2>&1
sips -z 256 256 resources/icon_512.png --out "$ICON_DIR/icon_256x256.png" > /dev/null 2>&1
sips -z 512 512 resources/icon_512.png --out "$ICON_DIR/icon_256x256@2x.png" > /dev/null 2>&1
sips -z 512 512 resources/icon_512.png --out "$ICON_DIR/icon_512x512.png" > /dev/null 2>&1

iconutil -c icns "$ICON_DIR" -o "$APP_DIR/Contents/Resources/icon.icns"
rm -rf "$ICON_DIR"

echo "--------------------------------------------------------"
echo "Success! Application successfully bundled:"
echo "  $(pwd)/$APP_DIR"
echo "--------------------------------------------------------"
echo "You can now run the app or move/link it to your Applications folder:"
echo "  open $(pwd)/$APP_DIR"
echo "  ln -sfn $(pwd)/$APP_DIR /Applications/HueMIDIty.app"
echo "--------------------------------------------------------"
