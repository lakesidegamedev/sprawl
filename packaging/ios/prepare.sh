#!/usr/bin/env bash
# Makes the Xcode project: the app icon, then xcodegen. macOS only.
#   prepare.sh [build number]
# The version shown to players is the workspace version.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$here/../../Cargo.toml" | head -1)"
export MARKETING_VERSION="${version%%-*}"
export BUILD_NUMBER="${1:-1}"
python3 "$here/../make-icon.py" "$here/Assets.xcassets/AppIcon.appiconset/AppIcon-1024.png" 1024
cd "$here"
xcodegen generate --spec project.yml --project .
