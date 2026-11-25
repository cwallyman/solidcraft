#!/usr/bin/env bash
# Builds the SolidCraft app for iPadOS into the app bundle Xcode is making (docs/ios.md): the
# SolidCraft-iPad target runs this as its build phase, after Xcode has written the bundle's
# Info.plist and before it signs the bundle. Like packaging/macos/package.sh, it puts in the
# binary and the licence files, stamps the version from Cargo.toml and checks the result.
#
# Xcode sets:
#   PLATFORM_NAME                      iphoneos or iphonesimulator
#   CONFIGURATION                      Debug builds Cargo's dev profile (optimised, with debug
#                                      info; its incremental rebuild after an edit is quicker than
#                                      the profiling profile's); Release (and Archive), the
#                                      release profile
#   TARGET_BUILD_DIR, EXECUTABLE_PATH, INFOPLIST_PATH, UNLOCALIZED_RESOURCES_FOLDER_PATH
#   IPHONEOS_DEPLOYMENT_TARGET         the oldest iPadOS the binary runs on
#
# CARGO_BUILD_JOBS, when set (in the environment or as an xcodebuild setting), caps Cargo's jobs:
# a Mac with 8 GB builds with less swapping at 4.
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"

case "${PLATFORM_NAME:?}" in
  iphoneos) TARGET=aarch64-apple-ios ;;
  iphonesimulator) TARGET=aarch64-apple-ios-sim ;;
  *) echo "error: no iPadOS Rust target for platform $PLATFORM_NAME" >&2; exit 1 ;;
esac
case "${CONFIGURATION:-Debug}" in
  Release) PROFILE=release; PROFILE_DIR=release ;;
  *) PROFILE=dev; PROFILE_DIR=debug ;;
esac
SHORT_VERSION="${VERSION%%-*}"

# Cargo from rustup, at the version rust-toolchain.toml names. Xcode's PATH has neither.
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
cargo="$(command -v cargo || true)"
if [ -z "$cargo" ] && command -v rustup >/dev/null; then
  cargo="$(cd "$ROOT" && rustup which cargo)"
fi
if [ -z "$cargo" ]; then
  echo "error: cargo not found; install Rust with rustup (https://rustup.rs)" >&2
  exit 1
fi
(cd "$ROOT" && rustup target add "$TARGET" >/dev/null 2>&1 || true)

echo "==> SolidCraft $VERSION for iPadOS ($TARGET, $PROFILE${CARGO_BUILD_JOBS:+, $CARGO_BUILD_JOBS jobs})"
# Xcode's environment (SDKROOT for iPadOS above all) would make Cargo build its host-side build
# scripts against the iPadOS SDK. Cargo gets a clean one instead.
clean=(HOME="$HOME" USER="${USER:-}" TERM=dumb PATH="$(dirname "$cargo"):/usr/bin:/bin:/usr/sbin:/sbin"
  IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}" CARGO_TARGET_DIR="$CARGO_TARGET_DIR"
  SOLIDCRAFT_BUILD_DATE="$SOLIDCRAFT_BUILD_DATE" ${CARGO_BUILD_JOBS:+CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS"})
(cd "$ROOT" && env -i "${clean[@]}" "$cargo" build --locked -p solidcraft --bin solidcraft --target "$TARGET" --profile "$PROFILE")

APP="$TARGET_BUILD_DIR/$WRAPPER_NAME"
PLIST="$TARGET_BUILD_DIR/$INFOPLIST_PATH"
RESOURCES="$TARGET_BUILD_DIR/$UNLOCALIZED_RESOURCES_FOLDER_PATH"
echo "==> assembling $APP"
mkdir -p "$(dirname "$TARGET_BUILD_DIR/$EXECUTABLE_PATH")"
cp "$CARGO_TARGET_DIR/$TARGET/$PROFILE_DIR/solidcraft" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
mkdir -p "$RESOURCES/Licenses"
copy_docs "$RESOURCES/Licenses"
# The versions, as packaging/macos/package.sh writes them into the macOS Info.plist.
test -f "$PLIST" || { echo "error: Xcode has not written $PLIST yet" >&2; exit 1; }
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $SHORT_VERSION" -c "Set :CFBundleVersion $SHORT_VERSION" \
  -c "Set :SolidCraftVersion $VERSION" "$PLIST"

# Check the bundle as the iPad gets it (Xcode signs it next).
plutil -lint "$PLIST"
for key in CFBundleName CFBundleDisplayName CFBundleIconName; do
  test "$(/usr/libexec/PlistBuddy -c "Print :$key" "$PLIST")" = SolidCraft
done
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$PLIST")" = "$EXECUTABLE_NAME"
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$PLIST")" = "$SHORT_VERSION"
test -x "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
lipo "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" -verify_arch arm64
test -s "$RESOURCES/Licenses/THIRD-PARTY-LICENSES.txt"
echo "==> done"
