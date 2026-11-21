#!/usr/bin/env bash
# Installs the simulator build on one simulator, launches it and takes screenshots. macOS only.
#   simulator-shots.sh <iphone|ipad> <Urban Sprawl.app> <output dir>
# Shots: menu, map selector, town, the Budget sheet and the phone road toolbar.
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
within 240 xcrun simctl boot "$udid" || true
within 600 xcrun simctl bootstatus "$udid" -b || { echo "FAIL: simulator did not finish booting" >&2; exit 1; }
step "install"
within 400 xcrun simctl install "$udid" "$app"

# The runner's simulator answers slowly, so a check that times out counts as "not known to be
# stopped"; only a clear answer that the app is gone fails the run.
running() {
  local list
  list="$(within 240 xcrun simctl spawn "$udid" launchctl list 2>/dev/null)" || return 0
  echo "$list" | grep -q "UIKitApplication:$bundle"
}
show_log() {
  local data; data="$(xcrun simctl get_app_container "$udid" "$bundle" data 2>/dev/null || true)"
  [ -n "$data" ] && cat "$data/tmp/urban-sprawl.log" 2>/dev/null || true
}
fail() { echo "FAIL: $1" >&2; show_log; within 240 xcrun simctl io "$udid" screenshot "$out/$kind-failed.png" || true; exit 1; }

# menu
step "launch menu"
within 240 xcrun simctl launch "$udid" "$bundle"
sleep 5
running || fail "app is not running after launch"
step "screenshot menu"
within 240 xcrun simctl io "$udid" screenshot "$out/$kind-1-menu.png"
show_log
xcrun simctl terminate "$udid" "$bundle"
sleep 2

# Map selector with its engine-rendered map example.
step "launch map selector"
SIMCTL_CHILD_URBAN_SPRAWL_SMOKE=600 SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_SCREEN=newtown \
  within 240 xcrun simctl launch "$udid" "$bundle"
sleep 8
running || fail "app stopped while displaying the map selector"
within 240 xcrun simctl io "$udid" screenshot "$out/$kind-4-map-selector.png"
xcrun simctl terminate "$udid" "$bundle" || true
sleep 2

# Compare the Detailed storm at town scale on both devices and inspect phone materials up close.
storm_shot() {
  local view="$1" file="$2"
  step "launch coastal storm with Detailed atmosphere and Balanced resolution ($view)"
  SIMCTL_CHILD_URBAN_SPRAWL_SMOKE=600 SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_DAYS=900 \
    SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_PRESET=coast SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_SEED=4 \
    SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_GFX=detailed \
    SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_UI="$view,clean" within 240 xcrun simctl launch "$udid" "$bundle"
  sleep 20
  running || fail "app stopped while building the town"
  within 240 xcrun simctl io "$udid" screenshot "$out/$file"
  show_log
  xcrun simctl terminate "$udid" "$bundle" || true
  sleep 2
}
storm_shot storm "$kind-2-town.png"
if [[ "$kind" == iphone ]]; then
  storm_shot storm:70 "$kind-5-close.png"
fi

step "launch town with budget"
SIMCTL_CHILD_URBAN_SPRAWL_SMOKE=600 SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_DAYS=370 \
  SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_PRESET=gorge SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_SEED=4 \
  SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_UI=budget within 240 xcrun simctl launch "$udid" "$bundle"
sleep 20
running || fail "app stopped while displaying budget"
step "screenshot budget"
within 240 xcrun simctl io "$udid" screenshot "$out/$kind-3-budget.png"
xcrun simctl terminate "$udid" "$bundle" || true
if [[ "$kind" == iphone ]]; then
  step "launch town with road controls"
  SIMCTL_CHILD_URBAN_SPRAWL_SMOKE=600 SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_DAYS=370 \
    SIMCTL_CHILD_URBAN_SPRAWL_SMOKE_UI=roads within 240 xcrun simctl launch "$udid" "$bundle"
  sleep 20
  running || fail "app stopped while displaying road controls"
  within 240 xcrun simctl io "$udid" screenshot "$out/$kind-6-road-controls.png"
  xcrun simctl terminate "$udid" "$bundle" || true
fi
xcrun simctl shutdown "$udid" || true
