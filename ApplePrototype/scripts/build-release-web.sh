#!/bin/bash
# The App Store engine: wasm-opt size optimization, full LTO and no debug info. Much slower than
# build-local-web.sh, and built in its own target directory so the quick build's cache survives.
set -euo pipefail
prototype_root="$(cd "$(dirname "$0")/.." && pwd)"
solidcraft_root="$(cd "$prototype_root/../upstream-solidcraft" && pwd)"
web_root="$solidcraft_root/apps/solidcraft-web"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
cd "$web_root"
CARGO_TARGET_DIR="$solidcraft_root/target/appstore" CARGO_PROFILE_RELEASE_LTO=true CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  CARGO_PROFILE_RELEASE_DEBUG=false \
  env -u NO_COLOR trunk build index.html --release --public-url ./ --no-sri
SOLIDCRAFT_BUNDLE_PROFILE="release, LTO, wasm-opt -Oz, no debug info (App Store)" python3 "$prototype_root/scripts/bundle-local-web.py"
