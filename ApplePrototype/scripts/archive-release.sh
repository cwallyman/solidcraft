#!/bin/bash
# A Release archive for App Store Connect. Build the engine with build-release-web.sh first;
# validate and upload the archive from Xcode's Organizer.
set -euo pipefail
cd "$(dirname "$0")/.."
if ! grep -q '"profile": "release, LTO' SolidcraftForIPad/Web/asset-manifest.json; then
  echo "The bundled engine is not the App Store build: run scripts/build-release-web.sh first." >&2
  exit 1
fi
archive=build/SolidcraftForIPad.xcarchive
xcodebuild -project SolidcraftForIPad.xcodeproj -scheme SolidcraftForIPad -configuration Release \
  -destination 'generic/platform=iOS' -archivePath "$archive" -allowProvisioningUpdates archive
echo "Archived to $archive. Open it in Xcode's Organizer to validate and upload: open \"$archive\""
