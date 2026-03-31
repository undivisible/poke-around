#!/bin/bash
set -euo pipefail

APP_NAME="Poke macOS Around"
SCHEME="Poke macOS Around"
PROJECT="clients/Poke macOS Around/Poke macOS Around.xcodeproj"
BUILD_DIR="build"
RELEASE_DIR="${BUILD_DIR}/release"
ARM64_DIR="${BUILD_DIR}/arm64"
X86_DIR="${BUILD_DIR}/x86_64"
UNIVERSAL_APP="${RELEASE_DIR}/${APP_NAME}.app"
DMG_NAME="Poke.macOS.Around.dmg"
DMG_PATH="${RELEASE_DIR}/${DMG_NAME}"

rm -rf "$BUILD_DIR"
mkdir -p "$RELEASE_DIR"

echo "Building arm64..."
xcodebuild -project "$PROJECT" -scheme "$SCHEME" -configuration Release \
  -arch arm64 -derivedDataPath "$ARM64_DIR" \
  CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=NO \
  DEVELOPMENT_TEAM="" \
  clean build 2>&1 | tail -5

echo "Building x86_64..."
xcodebuild -project "$PROJECT" -scheme "$SCHEME" -configuration Release \
  -arch x86_64 -derivedDataPath "$X86_DIR" \
  CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=NO \
  DEVELOPMENT_TEAM="" \
  clean build 2>&1 | tail -5

ARM64_APP=$(find "$ARM64_DIR" -name "${APP_NAME}.app" -type d | head -1)
X86_APP=$(find "$X86_DIR" -name "${APP_NAME}.app" -type d | head -1)

echo "Creating universal binary..."
cp -R "$ARM64_APP" "$UNIVERSAL_APP"

find "$ARM64_APP" -type f -perm +111 | while read arm_bin; do
  rel_path="${arm_bin#$ARM64_APP/}"
  x86_bin="${X86_APP}/${rel_path}"
  uni_bin="${UNIVERSAL_APP}/${rel_path}"

  if [ -f "$x86_bin" ] && file "$arm_bin" | grep -q "Mach-O"; then
    lipo -create "$arm_bin" "$x86_bin" -output "$uni_bin" 2>/dev/null || true
  fi
done

echo "Creating DMG..."
hdiutil create -volname "$APP_NAME" -srcfolder "$UNIVERSAL_APP" \
  -ov -format UDZO "$DMG_PATH"

echo "Done: $DMG_PATH"
