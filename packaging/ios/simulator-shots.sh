#!/usr/bin/env bash
# Installs the simulator build on one simulator, launches it and takes screenshots. macOS only.
#   simulator-shots.sh <iphone|ipad> <Urban Sprawl.app> <output dir>
# Shots: menu (fresh launch), town (new town placed by the smoke autopilot, then it keeps running).
set -euo pipefail
kind="${1:?iphone or ipad}"; app="${2:?app bundle}"; out="${3:?output dir}"
bundle=net.lakesidegames.urbansprawl
mkdir -p "$out"

# macOS has no `timeout`: a perl alarm gives every simulator call a hard limit.
within() { local secs="$1"; shift; perl -e 'alarm shift; exec @ARGV' "$secs" "$@"; }
step() { echo "[$(date +%H:%M:%S)] $*"; }

case "$kind" in
  iphone) pattern='^iPhone [0-9]+$' ;;
  ipad) pattern='^iPad (Pro|Air)' ;;
  *) echo "unknown kind $kind" >&2; exit 2 ;;
esac
pick() {
  xcrun simctl list devices available -j |
    jq -r --arg re "$1" '[.devices | to_entries[] | select(.key|test("iOS")) | .value[] | select(.name|test($re))] | sort_by(.name) | last | .udid // empty'
}
udid="$(pick "$pattern")"
[ -n "$udid" ] || udid="$(pick "$( [ "$kind" = iphone ] && echo '^iPhone' || echo '^iPad')")"
[ -n "$udid" ] || { echo "no $kind simulator available" >&2; xcrun simctl list devices available; exit 1; }
echo "using $kind simulator $udid"
xcrun simctl list devices available | grep "$udid" || true

step "boot"
within 60 xcrun simctl boot "$udid" || true
within 300 xcrun simctl bootstatus "$udid" -b || { echo "FAIL: simulator did not finish booting" >&2; exit 1; }
step "install"
within 120 xcrun simctl install "$udid" "$app"

running() { within 30 xcrun simctl spawn "$udid" launchctl list | grep -q "UIKitApplication:$bundle"; }
show_log() {
  local data; data="$(xcrun simctl get_app_container "$udid" "$bundle" data 2>/dev/null || true)"
  [ -n "$data" ] && cat "$data/tmp/urban-sprawl.log" 2>/dev/null || true
}
fail() { echo "FAIL: $1" >&2; show_log; within 60 xcrun simctl io "$udid" screenshot "$out/$kind-failed.png" || true; exit 1; }

# menu
step "launch menu"
within 60 xcrun simctl launch "$udid" "$bundle"
sleep 12
running || fail "app is not running after launch"
step "screenshot menu"
within 60 xcrun simctl io "$udid" screenshot "$out/$kind-1-menu.png"
xcrun simctl terminate "$udid" "$bundle"
sleep 2

# town: the smoke autopilot starts a new town and places the hall, then runs for 10 minutes
step "launch town"
SIMCTL_CHILD_URBAN_SPRAWL_SMOKE=600 SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_DAYS=900 within 60 xcrun simctl launch "$udid" "$bundle"
sleep 30
running || fail "app stopped while building the town"
step "screenshot town"
within 60 xcrun simctl io "$udid" screenshot "$out/$kind-2-town.png"
show_log
xcrun simctl terminate "$udid" "$bundle" || true
xcrun simctl shutdown "$udid" || true
