#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
ipad_device="${1:?Usage: build-and-install.sh IPAD_UDID}"
xcodebuild -project SolidcraftForIPad.xcodeproj -scheme SolidcraftForIPad \
  -configuration Debug -destination "id=$ipad_device" \
  -derivedDataPath build -allowProvisioningUpdates build
xcrun devicectl device install app --device "$ipad_device" build/Build/Products/Debug-iphoneos/SolidcraftForIPad.app
