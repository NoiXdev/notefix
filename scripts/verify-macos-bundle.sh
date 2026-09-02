#!/usr/bin/env bash
# Fail unless a Notefix.app is a complete, correctly entitled widget build.
# Used by scripts/macos-release.sh after signing (CI) and handy locally:
#   scripts/verify-macos-bundle.sh /Applications/Notefix.app
#
# Guards against the failure modes that shipped silently before: a missing or
# hollow .appex (no Info.plist), an app group that isn't Team-ID-prefixed (on
# macOS 15+ that means a consent prompt for the app and a silently denied,
# empty widget), and a broken signature.
set -euo pipefail
APP="${1:?usage: verify-macos-bundle.sh <Notefix.app>}"
GROUP="5V8ZCK434F.dev.noix.notefix"
APPEX="$APP/Contents/PlugIns/NotefixWidget.appex"
fail() { echo "::error::verify-macos-bundle: $*" >&2; exit 1; }

[ -d "$APP" ] || fail "no app bundle at $APP"
[ -f "$APPEX/Contents/Info.plist" ] || fail "widget extension missing or hollow: $APPEX/Contents/Info.plist"
[ -x "$APPEX/Contents/MacOS/NotefixWidget" ] || fail "widget executable missing"
[ "$(plutil -extract NSExtension.NSExtensionPointIdentifier raw "$APPEX/Contents/Info.plist")" = "com.apple.widgetkit-extension" ] \
  || fail "widget Info.plist lacks NSExtension/com.apple.widgetkit-extension (xcodegen 'info:' block regenerated the plist?)"
for target in "$APP" "$APPEX"; do
  codesign -d --entitlements :- "$target" 2>/dev/null | grep -q "$GROUP" \
    || fail "$(basename "$target") is not entitled for app group $GROUP"
done
codesign --verify --deep --strict "$APP" || fail "signature verification failed for $APP"
echo "verify-macos-bundle: OK — widget embedded, app group $GROUP on app + widget, signature valid"
