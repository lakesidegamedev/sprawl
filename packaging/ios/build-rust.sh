#!/usr/bin/env bash
# Xcode build phase: compiles the Rust game (crate us-app) into a static library for the SDK and
# configuration Xcode is building, and leaves it in $DERIVED_FILE_DIR/rust for the linker.
set -euo pipefail
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

root="$(cd "${SRCROOT}/../.." && pwd)"
case "${PLATFORM_NAME}" in
  iphoneos) triple=aarch64-apple-ios ;;
  iphonesimulator) triple=aarch64-apple-ios-sim ;;
  *) echo "unsupported platform ${PLATFORM_NAME}" >&2; exit 1 ;;
esac
if [ "${CONFIGURATION}" = "Release" ]; then profile=release; flag=--release; else profile=debug; flag=; fi

# Cargo build scripts run on the Mac itself, so Xcode's SDK variables must not leak into them.
cd "$root"
env -u SDKROOT -u IPHONEOS_DEPLOYMENT_TARGET -u MACOSX_DEPLOYMENT_TARGET \
  IPHONEOS_DEPLOYMENT_TARGET=17.0 \
  cargo build --locked -p us-app --lib $flag --target "$triple"

mkdir -p "${DERIVED_FILE_DIR}/rust"
cp "$root/target/$triple/$profile/libus_app.a" "${DERIVED_FILE_DIR}/rust/libus_app.a"
