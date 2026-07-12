#!/bin/sh
# Conditionally sign, notarize, and staple an already-built Lavtype DMG.
# Required environment: APPLE_SIGNING_IDENTITY, APPLE_ID, APPLE_TEAM_ID,
# APPLE_APP_PASSWORD. The caller must have imported the signing certificate.
set -eu

dmg=${1:?usage: sign-macos-dmg.sh path/to/Lavtype.dmg}
: "${APPLE_SIGNING_IDENTITY:?APPLE_SIGNING_IDENTITY is required}"
: "${APPLE_ID:?APPLE_ID is required}"
: "${APPLE_TEAM_ID:?APPLE_TEAM_ID is required}"
: "${APPLE_APP_PASSWORD:?APPLE_APP_PASSWORD is required}"

work=$(mktemp -d "${TMPDIR:-/tmp}/lavtype-sign.XXXXXX")
mountpoint="$work/mount"
appdir="$work/appdir"
mkdir -p "$mountpoint" "$appdir"
cleanup() {
  hdiutil detach "$mountpoint" >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT INT TERM

hdiutil attach -nobrowse -readonly -mountpoint "$mountpoint" "$dmg" >/dev/null
app=$(find "$mountpoint" -maxdepth 1 -name '*.app' -print -quit)
if [ -z "$app" ]; then
  echo "no .app found in $dmg" >&2
  exit 1
fi
cp -R "$app" "$appdir/Lavtype.app"
hdiutil detach "$mountpoint" >/dev/null

codesign --force --deep --options runtime --timestamp \
  --entitlements packaging/macos/entitlements.plist \
  --sign "$APPLE_SIGNING_IDENTITY" "$appdir/Lavtype.app"
codesign --verify --deep --strict "$appdir/Lavtype.app"
spctl --assess --type execute "$appdir/Lavtype.app"

signed="$work/signed.dmg"
hdiutil create -volname Lavtype -srcfolder "$appdir" -format UDZO -ov "$signed" >/dev/null
xcrun notarytool submit "$signed" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
xcrun stapler staple "$signed"
xcrun stapler validate "$signed"
cp "$signed" "$dmg"
