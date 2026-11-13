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
# The window system loads the keyboard libraries at run time; bare desktops and the launcher's
# test machines may not have them, so they travel inside the AppImage.
mkdir -p "$app/usr/lib"
for lib in libxkbcommon.so.0 libxkbcommon-x11.so.0 libxcb-xkb.so.1; do
  src="$(ldconfig -p | awk -v l="$lib" '$1 == l && /x86-64/ { print $NF; exit }')"
  if [ -n "$src" ]; then cp -L "$src" "$app/usr/lib/$lib"; else echo "warning: $lib not found to bundle" >&2; fi
done
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
export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$HERE/usr/bin/urban-sprawl" "$@"
RUN
chmod +x "$app/AppRun"
curl -fsSL -o "$work/appimagetool" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
chmod +x "$work/appimagetool"
ARCH=x86_64 "$work/appimagetool" --appimage-extract-and-run --no-appstream "$app" "$out"
test -s "$out"
