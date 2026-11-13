#!/usr/bin/env bash
# Builds an ad-hoc signed Urban Sprawl.app and its disk image. macOS runner only.
#   make-dmg.sh <universal binary> <version> <output.dmg>
# Signing is ad-hoc (codesign -s -). No Developer ID, no secrets.
set -euo pipefail
bin="${1:?binary}"; version="${2:?version}"; out="${3:?output}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
app="$work/stage/Urban Sprawl.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m 0755 "$bin" "$app/Contents/MacOS/urban-sprawl"
numeric="${version%%-*}"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Urban Sprawl</string>
<key>CFBundleDisplayName</key><string>Urban Sprawl</string>
<key>CFBundleIdentifier</key><string>net.lakesidegames.urban-sprawl</string>
<key>CFBundleExecutable</key><string>urban-sprawl</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>${numeric}</string>
<key>CFBundleVersion</key><string>${numeric}</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign -s - --force --deep "$app"
codesign --verify --deep --strict "$app"
ln -s /Applications "$work/stage/Applications"
rm -f "$out"
hdiutil create -volname "Urban Sprawl" -srcfolder "$work/stage" -ov -format UDZO "$out"
hdiutil verify "$out"
