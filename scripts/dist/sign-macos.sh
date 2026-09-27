#!/usr/bin/env bash
# Signs a macOS server binary with a Developer ID Application certificate
# and notarizes it (docs/release.md, "Signing"). Optional: the build
# workflow's `sign-macos` job runs this only when the MACOS_SIGNING_IDENTITY
# variable is set. The intended setup keeps these secrets on the `signing`
# environment, scoped to that job; a caller forwarding its own repository
# secrets by name (as dist.yml and release.yml do) is the fallback.
#
# A bare binary cannot carry a stapled ticket, so notarization only records
# it with Apple; Gatekeeper looks the ticket up online. notarytool takes a
# zip of the binary.
#
# Environment:
#   MACOS_CERTIFICATE_P12        base64 of the .p12 holding the certificate and key
#   MACOS_CERTIFICATE_PASSWORD   the .p12's password
#   MACOS_SIGNING_IDENTITY       e.g. "Developer ID Application: Name (TEAMID)"
#   MACOS_NOTARY_APPLE_ID        Apple ID used for notarization
#   MACOS_NOTARY_PASSWORD        an app-specific password for that Apple ID
#   MACOS_NOTARY_TEAM_ID         the team ID
#
# Usage: scripts/dist/sign-macos.sh <binary>
set -euo pipefail

binary=${1:?usage: sign-macos.sh <binary>}
work=$(mktemp -d)
keychain=$work/signing.keychain-db
keychain_password=$(uuidgen)
cleanup() {
  security delete-keychain "$keychain" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

# A throwaway keychain holding only the signing identity.
base64 --decode <<<"$MACOS_CERTIFICATE_P12" >"$work/certificate.p12"
security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$work/certificate.p12" -k "$keychain" -P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k "$keychain_password" "$keychain" >/dev/null
# `security list-keychains` indents each line with 4 spaces; word-split on
# whitespace (not `mapfile`, which keeps the indentation and needs bash 4 —
# macOS's own /usr/bin/env bash is 3.2) to get clean paths.
read -ra existing_keychains < <(security list-keychains -d user | tr -d '"' | tr '\n' ' ')
security list-keychains -d user -s "$keychain" "${existing_keychains[@]}"

# The hardened runtime and a secure timestamp are required for notarization.
codesign --force --options runtime --timestamp --keychain "$keychain" --sign "$MACOS_SIGNING_IDENTITY" "$binary"
codesign --verify --strict --verbose=2 "$binary"

ditto -c -k --keepParent "$binary" "$work/notarize.zip"
credentials=(--apple-id "$MACOS_NOTARY_APPLE_ID" --password "$MACOS_NOTARY_PASSWORD" --team-id "$MACOS_NOTARY_TEAM_ID")
result=$(xcrun notarytool submit "$work/notarize.zip" "${credentials[@]}" --wait --timeout 30m --output-format json)
echo "$result"
if [[ $(jq -r .status <<<"$result") != Accepted ]]; then
  # The log says why Apple rejected the binary.
  xcrun notarytool log "$(jq -r .id <<<"$result")" "${credentials[@]}" || true
  echo "notarization failed" >&2
  exit 1
fi
