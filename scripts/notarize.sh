#!/bin/bash
set -euo pipefail

VERSION="${1:?Usage: ./scripts/notarize.sh <version> (e.g. 0.3.0)}"
TAG="v${VERSION}"
REPO="Stoffberg/record"
WORK_DIR=$(mktemp -d)

echo "Downloading DMGs for ${TAG}..."
GH_CONFIG_DIR=~/.config/gh-personal gh release download "${TAG}" \
  --repo "${REPO}" \
  --pattern "*.dmg" \
  --dir "${WORK_DIR}"

for DMG in "${WORK_DIR}"/*.dmg; do
  FILENAME=$(basename "${DMG}")
  echo ""
  echo "=== Notarizing ${FILENAME} ==="

  xcrun notarytool submit "${DMG}" \
    --keychain-profile "record-notarize" \
    --wait

  echo "Stapling ${FILENAME}..."
  xcrun stapler staple "${DMG}"

  echo "Re-uploading ${FILENAME}..."
  GH_CONFIG_DIR=~/.config/gh-personal gh release upload "${TAG}" \
    --repo "${REPO}" \
    --clobber \
    "${DMG}"
done

echo ""
echo "Updating Homebrew cask..."
ARM64_SHA=$(shasum -a 256 "${WORK_DIR}/Record_${VERSION}_aarch64.dmg" | awk '{print $1}')
X64_SHA=$(shasum -a 256 "${WORK_DIR}/Record_${VERSION}_x64.dmg" | awk '{print $1}')

GH_CONFIG_DIR=~/.config/gh-personal gh api repos/Stoffberg/homebrew-tap/dispatches \
  -f event_type=update-record-cask \
  -f "client_payload[version]=${VERSION}" \
  -f "client_payload[arm64_sha]=${ARM64_SHA}" \
  -f "client_payload[x64_sha]=${X64_SHA}"

rm -rf "${WORK_DIR}"
echo ""
echo "Done. ${TAG} is signed, notarized, and stapled."
