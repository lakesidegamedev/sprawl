#!/usr/bin/env bash
# Builds the Linux AppImage.
#   build-appimage.sh <urban-sprawl binary> <icon.png> <output.AppImage>
# CI only (needs network for appimagetool). AppImages are not signed.
set -euo pipefail
bin="${1:?binary}"; icon="${2:?icon png}"; out="${3:?output}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
app="$work/UrbanSprawl.AppDir"
mkdir -p "$app/usr/bin"
install -m 0755 "$bin" "$app/usr/bin/urban-sprawl"
cp "$icon" "$app/urban-sprawl.png"
cat > "$app/urban-sprawl.desktop" <<'DESK'
[Desktop Entry]
Type=Application
Name=Urban Sprawl
Exec=urban-sprawl
Icon=urban-sprawl
Categories=Game;Simulation;
Terminal=false
DESK
cat > "$app/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/urban-sprawl" "$@"
RUN
chmod +x "$app/AppRun"
curl -fsSL -o "$work/appimagetool" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
chmod +x "$work/appimagetool"
ARCH=x86_64 "$work/appimagetool" --appimage-extract-and-run --no-appstream "$app" "$out"
test -s "$out"
