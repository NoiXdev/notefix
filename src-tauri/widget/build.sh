#!/usr/bin/env bash
set -euo pipefail

# Widget spike pipeline: build the Tauri app + the WidgetKit extension, embed the
# extension, then sign inside-out with the Developer ID + per-target entitlements,
# and verify the signature.

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"          # repo root
WIDGET_DIR="$ROOT/src-tauri/widget"
APP="$ROOT/src-tauri/target/release/bundle/macos/Notefix.app"
APP_ENT="$ROOT/src-tauri/entitlements.plist"
WIDGET_ENT="$WIDGET_DIR/NotefixWidget/NotefixWidget.entitlements"

# Keep the widget version in lockstep with the app version (tauri.conf.json).
VERSION="$(node -p "require('$ROOT/src-tauri/tauri.conf.json').version")"

# Resolve a Developer ID signing identity (first matching cert hash).
# Override the match string by exporting SIGNING_IDENTITY, e.g.
#   SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
SIGN_MATCH="${SIGNING_IDENTITY:-Developer ID Application}"
IDENTITY="$(security find-identity -v -p codesigning | awk -v m="$SIGN_MATCH" 'index($0, m) {print $2; exit}')"
[ -n "$IDENTITY" ] || { echo "No Developer ID Application identity found"; exit 1; }
echo "Signing with $IDENTITY"

echo "==> 1/5 tauri build"
( cd "$ROOT" && npm run tauri build )

echo "==> 2/5 build widget extension"
( cd "$WIDGET_DIR"
  command -v xcodegen >/dev/null || brew install xcodegen
  xcodegen generate
  xcodebuild -project NotefixWidget.xcodeproj -scheme NotefixWidget -configuration Release \
    -derivedDataPath build CODE_SIGNING_ALLOWED=NO \
    MARKETING_VERSION="$VERSION" CURRENT_PROJECT_VERSION="$VERSION" build )
APPEX="$WIDGET_DIR/build/Build/Products/Release/NotefixWidget.appex"

echo "==> 3/5 embed .appex"
mkdir -p "$APP/Contents/PlugIns"
rm -rf "$APP/Contents/PlugIns/NotefixWidget.appex"
cp -R "$APPEX" "$APP/Contents/PlugIns/"

echo "==> 4/5 sign inside-out"
codesign --force --options runtime --timestamp=none \
  --entitlements "$WIDGET_ENT" --sign "$IDENTITY" \
  "$APP/Contents/PlugIns/NotefixWidget.appex"
codesign --force --options runtime --timestamp=none \
  --entitlements "$APP_ENT" --sign "$IDENTITY" "$APP"

echo "==> 5/5 verify"
codesign --verify --deep --strict --verbose=2 "$APP"
echo "--- app entitlements ---"
codesign -d --entitlements - "$APP" 2>/dev/null | grep -A2 application-groups || true
echo "--- widget entitlements ---"
codesign -d --entitlements - "$APP/Contents/PlugIns/NotefixWidget.appex" 2>/dev/null | grep -A2 application-groups || true
echo "--- gatekeeper (informational; unnotarized may warn) ---"
spctl -a -vv "$APP" || true
echo "DONE: $APP"
