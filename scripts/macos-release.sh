#!/usr/bin/env bash
set -euo pipefail

# Build the macOS universal app with the WidgetKit extension embedded, sign it
# inside-out, notarize it, and produce a signed + notarized .dmg.
#
# tauri-action can't do this: a WidgetKit .appex has to be signed with its own
# sandbox + app-group entitlements (different from the host app's) and sealed
# inside the app *before* the app is signed and notarized. So this script owns
# the whole macOS build/sign/notarize pipeline; release.yml uses tauri-action
# only for Windows and Linux.
#
# Required env:
#   VERSION   release version without the leading "v" (e.g. 0.1.0)
#
# Optional signing env (all-or-nothing). When APPLE_SIGNING_IDENTITY is empty
# (e.g. a fork without secrets) the app + widget are built and bundled UNSIGNED
# and notarization is skipped, so the workflow still produces an artifact.
#   APPLE_SIGNING_IDENTITY   full "Developer ID Application: … (TEAMID)" string
#   APPLE_API_KEY_PATH       path to the App Store Connect .p8 key
#   APPLE_API_KEY_ID         10-character key id
#   APPLE_API_ISSUER         issuer UUID

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="universal-apple-darwin"
APP_DIR="$ROOT/src-tauri/target/$TARGET/release/bundle/macos"
APP="$APP_DIR/Notefix.app"
WIDGET_DIR="$ROOT/src-tauri/widget"
APP_ENT="$ROOT/src-tauri/entitlements.plist"
WIDGET_ENT="$WIDGET_DIR/NotefixWidget/NotefixWidget.entitlements"
SIGN="${APPLE_SIGNING_IDENTITY:-}"
DMG="$APP_DIR/Notefix_${VERSION}_universal.dmg"
TMP="${RUNNER_TEMP:-/tmp}"

notarize() { # $1 = path to a .zip or .dmg to submit
  xcrun notarytool submit "$1" \
    --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" \
    --wait
}

echo "==> 1/6 Build the app bundle (unsigned — the widget is embedded and signed below)"
# Scrub Apple signing env so Tauri does not sign/notarize the widget-less app;
# we sign the complete bundle ourselves once the .appex is embedded.
( cd "$ROOT" && env -u APPLE_SIGNING_IDENTITY -u APPLE_CERTIFICATE \
    -u APPLE_API_KEY -u APPLE_API_KEY_PATH -u APPLE_API_ISSUER \
    npm run tauri -- build --target "$TARGET" --bundles app )
[ -d "$APP" ] || { echo "app bundle not found at $APP"; exit 1; }

echo "==> 2/6 Build the WidgetKit extension (universal)"
( cd "$WIDGET_DIR"
  command -v xcodegen >/dev/null || brew install xcodegen
  xcodegen generate
  xcodebuild -project NotefixWidget.xcodeproj -scheme NotefixWidget \
    -configuration Release -derivedDataPath build \
    ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO CODE_SIGNING_ALLOWED=NO build )
APPEX="$WIDGET_DIR/build/Build/Products/Release/NotefixWidget.appex"
[ -d "$APPEX" ] || { echo "widget .appex not found at $APPEX"; exit 1; }

echo "==> 3/6 Embed the extension"
mkdir -p "$APP/Contents/PlugIns"
rm -rf "$APP/Contents/PlugIns/NotefixWidget.appex"
cp -R "$APPEX" "$APP/Contents/PlugIns/"

if [ -z "$SIGN" ]; then
  echo "==> No signing identity — producing an UNSIGNED build"
  hdiutil create -volname "Notefix" -srcfolder "$APP" -ov -format UDZO "$DMG"
  echo "DMG=$DMG"
  exit 0
fi

echo "==> 4/6 Sign inside-out (widget first, then app); hardened runtime + secure timestamp"
codesign --force --options runtime --timestamp \
  --entitlements "$WIDGET_ENT" --sign "$SIGN" \
  "$APP/Contents/PlugIns/NotefixWidget.appex"
codesign --force --options runtime --timestamp \
  --entitlements "$APP_ENT" --sign "$SIGN" "$APP"
codesign --verify --strict --verbose=2 "$APP"

echo "==> 5/6 Notarize the app, then staple"
APP_ZIP="$TMP/Notefix.zip"
ditto -c -k --keepParent "$APP" "$APP_ZIP"
notarize "$APP_ZIP"
xcrun stapler staple "$APP"

echo "==> 6/6 Build, sign, notarize & staple the .dmg"
hdiutil create -volname "Notefix" -srcfolder "$APP" -ov -format UDZO "$DMG"
codesign --force --timestamp --sign "$SIGN" "$DMG"
notarize "$DMG"
xcrun stapler staple "$DMG"

echo "--- Gatekeeper assessment (informational) ---"
spctl -a -vvv "$APP" || true
echo "DMG=$DMG"
